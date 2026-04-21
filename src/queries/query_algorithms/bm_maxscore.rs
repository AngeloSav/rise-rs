use crate::{
    DocScorer,
    indexes::*,
    queries::{
        BlockPostingMetadata, QueryOperator, RankedQueryOperator, query_algorithms::query_freqs,
        topk_heap::TopKHeap,
    },
};

const INNER_WINDOW_SIZE: u64 = 4096;
const BITSET_WORDS: usize = (INNER_WINDOW_SIZE as usize) / 64;

pub struct BMMaxScore<'a, Scorer: DocScorer> {
    p_data: &'a BlockPostingMetadata<Scorer>,
    topk_heap: TopKHeap,
    bitset: [u64; BITSET_WORDS],
    score_buffer: Vec<f32>,
    cum_block_max: Vec<f32>,
}

impl<'a, Scorer: DocScorer> BMMaxScore<'a, Scorer> {
    pub fn new(p_data: &'a BlockPostingMetadata<Scorer>, k: usize) -> Self {
        Self {
            p_data,
            topk_heap: TopKHeap::new(k),
            bitset: [0u64; BITSET_WORDS],
            score_buffer: vec![0.0; INNER_WINDOW_SIZE as usize],
            cum_block_max: Vec::new(),
        }
    }
}

impl<Scorer: DocScorer> QueryOperator for BMMaxScore<'_, Scorer> {
    fn query<I>(&mut self, idx: &I, terms: &[usize]) -> usize
    where
        I: InvertedIndex,
    {
        if terms.is_empty() {
            return 0;
        }
        let n_docs = idx.n_docs() as u64;
        let query_freqs = query_freqs(terms);

        // Tuple layout: (iter, block_iter, q_weight, max_weight)
        let mut enums = Vec::with_capacity(query_freqs.len());
        self.topk_heap.clear();

        for (term, freq) in query_freqs {
            let it = idx.get_plist_iter(term);
            let q_weight = Scorer::query_term_weight(freq as u64, it.len() as u64, n_docs);
            let max_weight = q_weight * self.p_data.get_max_term_weight(term);
            let block_iter = self.p_data.get_block_posting_metadata_iterator(term);
            enums.push((it, block_iter, q_weight, max_weight));
        }

        enums.sort_by(|x, y| x.3.partial_cmp(&y.3).unwrap());

        let upper_bounds: Vec<f32> = enums
            .iter()
            .map(|x| x.3)
            .scan(0f32, |s, x| {
                *s += x;
                Some(*s)
            })
            .collect();

        let mut non_essential_lists = 0;
        let mut cur_doc = enums.iter().map(|x| x.0.current_doc()).min().unwrap();

        // Cached block-level state. Refreshed when cur_doc exits the current region.
        let mut block_region_end: u64 = 0;
        let mut block_upper: f32 = 0.0;
        let mut block_state_valid = false;

        // cum_block_max cache coordinates — invalidated when either the block region
        // advances or the essential/non-essential partition changes.
        let mut cum_cached_for_region: u64 = u64::MAX;
        let mut cum_cached_for_partition: usize = usize::MAX;

        while cur_doc < n_docs && non_essential_lists < enums.len() {
            // Refresh block_next_geq, block_region_end, block_upper when we've
            // moved beyond the previously-cached region.
            if !block_state_valid || cur_doc > block_region_end {
                for i in 0..enums.len() {
                    enums[i].1.block_next_geq(cur_doc);
                }
                block_region_end = enums
                    .iter()
                    .map(|e| e.1.block_docid())
                    .min()
                    .unwrap()
                    .max(cur_doc);
                block_upper = enums.iter().map(|e| e.1.block_max_score() * e.2).sum();
                block_state_valid = true;
            }

            // Block skip: no doc in this region can enter top-k.
            if !self.topk_heap.can_enter(block_upper) {
                let jump = block_region_end + 1;
                for i in non_essential_lists..enums.len() {
                    if enums[i].0.current_doc() < jump {
                        enums[i].0.next_geq(jump);
                    }
                }
                cur_doc = enums[non_essential_lists..]
                    .iter()
                    .map(|e| e.0.current_doc())
                    .min()
                    .unwrap_or(n_docs);
                block_state_valid = false;
                continue;
            }

            if cum_cached_for_region != block_region_end
                || cum_cached_for_partition != non_essential_lists
            {
                self.cum_block_max.clear();
                let mut cum = 0.0f32;
                for i in 0..non_essential_lists {
                    cum += enums[i].1.block_max_score() * enums[i].2;
                    self.cum_block_max.push(cum);
                }
                cum_cached_for_region = block_region_end;
                cum_cached_for_partition = non_essential_lists;
            }

            let window_max = (block_region_end + 1)
                .min(cur_doc + INNER_WINDOW_SIZE)
                .min(n_docs);

            let essentials_count = enums.len() - non_essential_lists;

            if essentials_count == 1 {
                // Single essential: skip BS1. Iterate the lead essential directly,
                // same shape as plain MaxScore with block-max non-essential pruning.
                let e = non_essential_lists;
                while enums[e].0.current_doc() < window_max {
                    let doc = enums[e].0.current_doc();
                    let norm_len = self.p_data.get_norm_len(doc as usize);
                    let mut score =
                        enums[e].2 * Scorer::doc_term_weight(enums[e].0.freq(), norm_len);

                    for i in (0..non_essential_lists).rev() {
                        if !self.topk_heap.can_enter(score + self.cum_block_max[i]) {
                            break;
                        }
                        enums[i].0.next_geq(doc);
                        if enums[i].0.current_doc() == doc {
                            score +=
                                enums[i].2 * Scorer::doc_term_weight(enums[i].0.freq(), norm_len);
                        }
                    }

                    if self.topk_heap.can_enter(score) {
                        self.topk_heap.push_with_id(doc, score);
                    }
                    enums[e].0.next_doc();
                }
            } else {
                // Multi essential: BS1 bitset accumulator. score_buffer is left
                // dirty; the bitset tracks which cells are live.
                self.bitset.fill(0);
                for i in non_essential_lists..enums.len() {
                    while enums[i].0.current_doc() < window_max {
                        let doc = enums[i].0.current_doc();
                        let local_idx = (doc - cur_doc) as usize;
                        let norm_len = self.p_data.get_norm_len(doc as usize);
                        let contrib =
                            enums[i].2 * Scorer::doc_term_weight(enums[i].0.freq(), norm_len);
                        let word_idx = local_idx / 64;
                        let bit_mask = 1u64 << (local_idx % 64);
                        let was_set = self.bitset[word_idx] & bit_mask != 0;
                        self.bitset[word_idx] |= bit_mask;
                        if was_set {
                            self.score_buffer[local_idx] += contrib;
                        } else {
                            self.score_buffer[local_idx] = contrib;
                        }
                        enums[i].0.next_doc();
                    }
                }

                for word_idx in 0..BITSET_WORDS {
                    let mut word = self.bitset[word_idx];
                    while word != 0 {
                        let bit_idx = word.trailing_zeros() as usize;
                        let local_idx = word_idx * 64 + bit_idx;
                        let doc = cur_doc + local_idx as u64;
                        let mut score = self.score_buffer[local_idx];
                        let norm_len = self.p_data.get_norm_len(doc as usize);

                        for i in (0..non_essential_lists).rev() {
                            if !self.topk_heap.can_enter(score + self.cum_block_max[i]) {
                                break;
                            }
                            enums[i].0.next_geq(doc);
                            if enums[i].0.current_doc() == doc {
                                score += enums[i].2
                                    * Scorer::doc_term_weight(enums[i].0.freq(), norm_len);
                            }
                        }

                        if self.topk_heap.can_enter(score) {
                            self.topk_heap.push_with_id(doc, score);
                        }
                        word &= word - 1;
                    }
                }
            }

            // Deferred non-essential promotion (cum_block_max stays consistent within the window).
            while non_essential_lists < enums.len()
                && !self.topk_heap.can_enter(upper_bounds[non_essential_lists])
            {
                non_essential_lists += 1;
            }

            cur_doc = if non_essential_lists < enums.len() {
                enums[non_essential_lists..]
                    .iter()
                    .map(|e| e.0.current_doc())
                    .min()
                    .unwrap()
            } else {
                n_docs
            };
        }

        self.topk_heap.len()
    }

    fn query_name() -> &'static str {
        "BMMaxScore"
    }
}

impl<Scorer: DocScorer> RankedQueryOperator for BMMaxScore<'_, Scorer> {
    fn topk(&self) -> &crate::queries::topk_heap::TopKHeap {
        &self.topk_heap
    }
}

use crate::{
    DocScorer,
    indexes::freq_index::{InvertedIndex, PostingListIter},
    queries::{
        BlockPostingMetadata, QueryOperator, RankedQueryOperator, query_algorithms::query_freqs,
        topk_heap::TopKHeap,
    },
};

pub struct BMMaxScore<'a, Scorer: DocScorer> {
    p_data: &'a BlockPostingMetadata<Scorer>,
    topk_heap: TopKHeap,
}

impl<'a, Scorer: DocScorer> BMMaxScore<'a, Scorer> {
    pub fn new(p_data: &'a BlockPostingMetadata<Scorer>, k: usize) -> Self {
        let topk_heap: TopKHeap = TopKHeap::new(k);
        Self { p_data, topk_heap }
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

        let mut enums = Vec::with_capacity(query_freqs.len());

        self.topk_heap.clear();

        for (term, freq) in query_freqs {
            let it = idx.get_plist_iter(term);
            let q_weight =
                Scorer::query_term_weight(freq as u64, it.len() as u64, idx.n_docs() as u64);

            let max_weight = q_weight * self.p_data.get_max_term_weight(term);
            let wand_iter = self.p_data.get_block_posting_metadata_iterator(term);

            enums.push((it, wand_iter, q_weight, max_weight));
        }

        // Sort in-place: avoids allocating a second Vec<&mut _> and the double-deref on every
        // hot-loop access.
        enums.sort_by(|x, y| x.3.partial_cmp(&y.3).unwrap());

        let upper_bounds = enums
            .iter()
            .map(|x| x.3)
            .scan(0f32, |s, x| {
                *s += x;
                Some(*s)
            })
            .collect::<Vec<_>>();

        let mut non_essential_lists = 0;
        let mut cur_doc = enums.iter().map(|x| x.0.current_doc()).min().unwrap();
        while non_essential_lists < enums.len() && cur_doc < n_docs {
            let mut score = 0.0;
            let mut next_doc = n_docs;
            // Hoist norm_len once per document instead of calling get_norm_len on every
            // essential-list hit and again on every non-essential-list hit.
            let norm_len = self.p_data.get_norm_len(cur_doc as usize);

            for i in non_essential_lists..enums.len() {
                if enums[i].0.current_doc() == cur_doc {
                    score += enums[i].2 * Scorer::doc_term_weight(enums[i].0.freq(), norm_len);
                    enums[i].0.next_doc();
                }
                if enums[i].0.current_doc() < next_doc {
                    next_doc = enums[i].0.current_doc();
                }
            }

            let mut block_upper_bound = if non_essential_lists > 0 {
                upper_bounds[non_essential_lists - 1]
            } else {
                0.0
            };

            for i in (0..non_essential_lists).rev() {
                if enums[i].1.block_docid() < cur_doc {
                    enums[i].1.block_next_geq(cur_doc);
                }

                block_upper_bound -= enums[i].3 - enums[i].1.block_max_score() * enums[i].2;

                if !self.topk_heap.can_enter(score + block_upper_bound) {
                    break;
                }
            }

            if self.topk_heap.can_enter(score + block_upper_bound) {
                for i in (0..non_essential_lists).rev() {
                    enums[i].0.next_geq(cur_doc);
                    if enums[i].0.current_doc() == cur_doc {
                        block_upper_bound +=
                            enums[i].2 * Scorer::doc_term_weight(enums[i].0.freq(), norm_len);
                    }
                    block_upper_bound -= enums[i].1.block_max_score() * enums[i].2; // query weight???

                    if !self.topk_heap.can_enter(score + block_upper_bound) {
                        break;
                    }
                }
                score += block_upper_bound;
            }

            if self.topk_heap.can_enter(score) {
                self.topk_heap.push(score);

                while non_essential_lists < enums.len()
                    && !self.topk_heap.can_enter(upper_bounds[non_essential_lists])
                {
                    non_essential_lists += 1;
                }
            }
            cur_doc = next_doc;
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

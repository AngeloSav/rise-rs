use crate::{
    indexes::freq_index::{DocList, FreqIndex, FreqList},
    queries::{
        query_algorithms::query_freqs, topk_heap::TopKHeap, BlockPostingMetadata, QueryOperator,
        RankedQueryOperator,
    },
    DocScorer,
};

pub struct BMWand<'a, Scorer: DocScorer> {
    p_data: &'a BlockPostingMetadata<Scorer>,
    topk_heap: TopKHeap,
}

impl<'a, Scorer: DocScorer> BMWand<'a, Scorer> {
    pub fn new(p_data: &'a BlockPostingMetadata<Scorer>, k: usize) -> Self {
        let topk_heap: TopKHeap = TopKHeap::new(k);
        Self { p_data, topk_heap }
    }
}
impl<Scorer: DocScorer> QueryOperator for BMWand<'_, Scorer> {
    fn query<T, S>(&mut self, idx: &FreqIndex<T, S>, terms: &[usize]) -> usize
    where
        T: DocList,
        S: FreqList,
    {
        if terms.is_empty() {
            return 0;
        }
        let n_docs = idx.n_docs as u64;
        let query_freqs = query_freqs(terms);

        let mut enums = Vec::with_capacity(query_freqs.len());

        self.topk_heap.clear();

        for (term, freq) in query_freqs {
            let it = idx.get_plist_iter(term);
            let q_weight =
                Scorer::query_term_weight(freq as u64, it.len() as u64, idx.n_docs as u64);

            let max_weight = q_weight * self.p_data.get_max_term_weight(term);
            let wand_iter = self.p_data.get_block_posting_metadata_iterator(term);

            enums.push((it, wand_iter, q_weight, max_weight));
        }

        let mut ordered_enums = enums.iter_mut().collect::<Vec<_>>();

        ordered_enums.sort_by_key(|x| x.0.current_doc());

        loop {
            let mut upper_bound = 0.0;
            let mut found_pivot = false;
            let mut pivot = 0;
            let mut pivot_id = idx.n_docs as u64;

            while pivot < ordered_enums.len() {
                if ordered_enums[pivot].0.current_doc() >= idx.n_docs as u64 {
                    break;
                }

                upper_bound += ordered_enums[pivot].3;

                if self.topk_heap.can_enter(upper_bound) {
                    found_pivot = true;
                    pivot_id = ordered_enums[pivot].0.current_doc();

                    while pivot + 1 < ordered_enums.len()
                        && ordered_enums[pivot + 1].0.current_doc() == pivot_id
                    {
                        pivot += 1;
                    }
                    break;
                }

                pivot += 1;
            }

            if !found_pivot {
                break;
            }

            let mut block_upper_bound = 0.0;

            for i in 0..=pivot {
                if ordered_enums[i].1.docid() < pivot_id {
                    ordered_enums[i].1.next_geq(pivot_id);
                }

                block_upper_bound += ordered_enums[i].1.score() * ordered_enums[i].2;
            }

            if self.topk_heap.can_enter(block_upper_bound) {
                if pivot_id == ordered_enums[0].0.current_doc() {
                    //match, score pivot
                    let mut score = 0.0;
                    let norm_len = self.p_data.get_norm_len(pivot_id as usize);

                    for scored_enum in ordered_enums.iter_mut() {
                        if scored_enum.0.current_doc() != pivot_id {
                            break;
                        }

                        let partial_score =
                            scored_enum.2 * Scorer::doc_term_weight(scored_enum.0.freq(), norm_len);

                        score += partial_score;
                        block_upper_bound -= scored_enum.1.score() * scored_enum.2 - partial_score;

                        if !self.topk_heap.can_enter(block_upper_bound) {
                            break;
                        }
                    }

                    for scored_enum in ordered_enums.iter_mut() {
                        if scored_enum.0.current_doc() != pivot_id {
                            break;
                        }
                        scored_enum.0.next_doc();
                    }

                    self.topk_heap.push(score);

                    ordered_enums.sort_by_key(|x| x.0.current_doc());
                } else {
                    //no match
                    let mut next_list = pivot;
                    while ordered_enums[next_list].0.current_doc() == pivot_id {
                        next_list -= 1;
                    }

                    ordered_enums[next_list].0.next_geq(pivot_id);

                    for i in (next_list + 1)..ordered_enums.len() {
                        if ordered_enums[i].0.current_doc() < ordered_enums[i - 1].0.current_doc() {
                            ordered_enums.swap(i, i - 1);
                        } else {
                            break;
                        }
                    }
                }
            } else {
                let mut next;
                let mut next_list = pivot;

                let mut q_weight = ordered_enums[next_list].2;

                for i in 0..pivot {
                    if ordered_enums[i].2 > q_weight {
                        next_list = i;
                        q_weight = ordered_enums[i].2;
                    }
                }

                let mut next_jump = idx.n_docs as u64;

                if pivot + 1 < ordered_enums.len() {
                    next_jump = ordered_enums[pivot + 1].0.current_doc();
                }

                for i in 0..=pivot {
                    if ordered_enums[i].1.docid() < next_jump {
                        next_jump = std::cmp::min(next_jump, ordered_enums[i].1.docid());
                    }
                }

                next = next_jump + 1;

                if pivot + 1 < ordered_enums.len()
                    && next > ordered_enums[pivot + 1].0.current_doc()
                {
                    next = ordered_enums[pivot + 1].0.current_doc();
                }

                if next <= ordered_enums[pivot].0.current_doc() {
                    next = ordered_enums[pivot].0.current_doc() + 1;
                }

                ordered_enums[next_list].0.next_geq(next);

                for i in (next_list + 1)..ordered_enums.len() {
                    if ordered_enums[i].0.current_doc() < ordered_enums[i - 1].0.current_doc() {
                        ordered_enums.swap(i, i - 1);
                    } else {
                        break;
                    }
                }
            }
        }

        self.topk_heap.len()
    }

    fn query_name() -> &'static str {
        "BMWand"
    }
}

impl<Scorer: DocScorer> RankedQueryOperator for BMWand<'_, Scorer> {
    fn topk(&self) -> &crate::queries::topk_heap::TopKHeap {
        &self.topk_heap
    }
}

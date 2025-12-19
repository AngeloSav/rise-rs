use crate::{
    indexes::freq_index::{InvertedIndex, PostingListIter},
    queries::{
        query_algorithms::query_freqs, topk_heap::TopKHeap, BlockPostingMetadata, QueryOperator,
        RankedQueryOperator,
    },
    DocScorer,
};

pub struct Wand<'a, Scorer: DocScorer> {
    p_data: &'a BlockPostingMetadata<Scorer>,
    topk_heap: TopKHeap,
}

impl<'a, Scorer: DocScorer> Wand<'a, Scorer> {
    pub fn new(p_data: &'a BlockPostingMetadata<Scorer>, k: usize) -> Self {
        let topk_heap: TopKHeap = TopKHeap::new(k);
        Self { p_data, topk_heap }
    }
}

impl<Scorer: DocScorer> QueryOperator for Wand<'_, Scorer> {
    fn query<I>(&mut self, idx: &I, terms: &[usize]) -> usize
    where
        I: InvertedIndex,
    {
        if terms.is_empty() {
            return 0;
        }

        // let mut ngeq_ctr = 0;
        // let mut next_ctr = 0;

        let query_freqs = query_freqs(terms);

        // contains pair (enum, query_weight, max_score)
        let mut enums = Vec::with_capacity(query_freqs.len());

        self.topk_heap.clear();

        for (term, freq) in query_freqs {
            let it = idx.get_plist_iter(term);
            let q_weight =
                Scorer::query_term_weight(freq as u64, it.len() as u64, idx.n_docs() as u64);

            let max_t_weight = self.p_data.get_max_term_weight(term);
            let max_weight = q_weight * self.p_data.get_max_term_weight(term);

            // println!(
            //     "term {}, q_weight {}, max_t_weight {}, max_weight {}, norm_len {}",
            //     term,
            //     q_weight,
            //     max_t_weight,
            //     max_weight,
            //     self.p_data.get_norm_len(term)
            // );
            enums.push((it, q_weight, max_weight));
        }
        // println!("---------------");

        let mut ordered_enums = enums.iter_mut().collect::<Vec<_>>();
        // println!("ordered_enums length: {:?}", ordered_enums.len());

        ordered_enums.sort_by_key(|x| x.0.current_doc());
        loop {
            let mut upper_bound = 0.0;
            let mut found_pivot = false;
            let mut pivot = 0;

            while pivot < ordered_enums.len() {
                if ordered_enums[pivot].0.current_doc() >= idx.n_docs() as u64 {
                    break;
                }

                upper_bound += ordered_enums[pivot].2;

                if self.topk_heap.can_enter(upper_bound) {
                    found_pivot = true;
                    break;
                }

                pivot += 1;
            }

            // no pivot found, stop
            if !found_pivot {
                break;
            }

            let pivot_id = ordered_enums[pivot].0.current_doc();

            if pivot_id == ordered_enums[0].0.current_doc() {
                //match, score pivot
                let mut score = 0.0;
                let norm_len = self.p_data.get_norm_len(pivot_id as usize);

                for scored_enum in ordered_enums.iter_mut() {
                    if scored_enum.0.current_doc() != pivot_id {
                        break;
                    }

                    score +=
                        scored_enum.1 * Scorer::doc_term_weight(scored_enum.0.freq(), norm_len);
                    scored_enum.0.next_doc();
                    // next_ctr += 1;
                }

                // insert in topk heap if possible
                // println!("pivot_id {}, score {}", pivot_id, score);
                // self.topk_heap.push_with_id(pivot_id, score);
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
        }

        // println!(
        //     "n_pushes = {}, ngeq_ctr = {}, next_ctr = {}",
        //     n_pushes, ngeq_ctr, next_ctr
        // );
        // println!("ngeq_ctr = {}, next_ctr = {}", ngeq_ctr, next_ctr);
        self.topk_heap.len()
    }

    fn query_name() -> &'static str {
        "Wand"
    }
}

impl<Scorer: DocScorer> RankedQueryOperator for Wand<'_, Scorer> {
    fn topk(&self) -> &crate::queries::topk_heap::TopKHeap {
        &self.topk_heap
    }
}

use crate::{
    DocScorer,
    indexes::freq_index::{InvertedIndex, PostingListIter},
    queries::{
        BlockPostingMetadata, QueryOperator, RankedQueryOperator, query_algorithms::query_freqs,
        topk_heap::TopKHeap,
    },
};
pub struct MaxScore<'a, Scorer: DocScorer> {
    p_data: &'a BlockPostingMetadata<Scorer>,
    topk_heap: TopKHeap,
}

impl<'a, Scorer: DocScorer> MaxScore<'a, Scorer> {
    pub fn new(p_data: &'a BlockPostingMetadata<Scorer>, k: usize) -> Self {
        let topk_heap: TopKHeap = TopKHeap::new(k);
        Self { p_data, topk_heap }
    }
}

impl<Scorer: DocScorer> QueryOperator for MaxScore<'_, Scorer> {
    fn query<I>(&mut self, idx: &I, terms: &[usize]) -> usize
    where
        I: InvertedIndex,
    {
        if terms.is_empty() {
            return 0;
        }
        let n_docs = idx.n_docs() as u64;
        let query_freqs = query_freqs(terms);

        // contains pair (enum, weight)
        let mut enums = Vec::with_capacity(query_freqs.len());

        self.topk_heap.clear();

        for (term, freq) in query_freqs {
            let it = idx.get_plist_iter(term);
            let q_weight =
                Scorer::query_term_weight(freq as u64, it.len() as u64, idx.n_docs() as u64);

            let max_weight = q_weight * self.p_data.get_max_term_weight(term);

            enums.push((it, q_weight, max_weight));
        }

        let mut ordered_enums = enums.iter_mut().collect::<Vec<_>>();

        ordered_enums.sort_by(|x, y| x.2.partial_cmp(&y.2).unwrap());

        let upper_bounds = ordered_enums
            .iter()
            .map(|x| x.2)
            .scan(0f32, |s, x| {
                *s += x;
                Some(*s)
            })
            .collect::<Vec<_>>();

        let mut non_essential_lists = 0;
        let mut cur_doc = ordered_enums
            .iter()
            .map(|x| x.0.current_doc())
            .min()
            .unwrap();

        while non_essential_lists < ordered_enums.len() && cur_doc < n_docs {
            let mut score = 0.0;
            let mut next_doc = n_docs;
            let norm_len = self.p_data.get_norm_len(cur_doc as usize);

            for i in non_essential_lists..ordered_enums.len() {
                if ordered_enums[i].0.current_doc() == cur_doc {
                    score += ordered_enums[i].1
                        * Scorer::doc_term_weight(ordered_enums[i].0.freq(), norm_len);
                    ordered_enums[i].0.next_doc();
                }
                if ordered_enums[i].0.current_doc() < next_doc {
                    next_doc = ordered_enums[i].0.current_doc();
                }
            }

            for i in (0..non_essential_lists).rev() {
                if !self.topk_heap.can_enter(score + upper_bounds[i]) {
                    break;
                }
                ordered_enums[i].0.next_geq(cur_doc);
                if ordered_enums[i].0.current_doc() == cur_doc {
                    score += ordered_enums[i].1
                        * Scorer::doc_term_weight(ordered_enums[i].0.freq(), norm_len);
                }
            }

            if self.topk_heap.can_enter(score) {
                // self.topk_heap.push_with_id(cur_doc, score);
                self.topk_heap.push(score);

                while non_essential_lists < ordered_enums.len()
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
        "MaxScore"
    }
}

impl<Scorer: DocScorer> RankedQueryOperator for MaxScore<'_, Scorer> {
    fn topk(&self) -> &crate::queries::topk_heap::TopKHeap {
        &self.topk_heap
    }
}

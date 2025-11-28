use crate::{
    indexes::freq_index::{DocList, FreqIndex, FreqList},
    queries::{
        query_algorithms::query_freqs, topk_heap::TopKHeap, BlockPostingMetadata, QueryOperator,
        RankedQueryOperator,
    },
    DocScorer,
};

pub struct RankedOr<'a, Scorer: DocScorer> {
    p_data: &'a BlockPostingMetadata<Scorer>,
    topk_heap: TopKHeap,
}

impl<'a, Scorer: DocScorer> RankedOr<'a, Scorer> {
    pub fn new(p_data: &'a BlockPostingMetadata<Scorer>, k: usize) -> Self {
        let topk_heap = TopKHeap::new(k);

        Self { p_data, topk_heap }
    }
}

impl<Scorer: DocScorer> QueryOperator for RankedOr<'_, Scorer> {
    fn query_name() -> &'static str {
        "RankedOr"
    }

    fn query<'a, T, S>(&mut self, idx: &'a FreqIndex<T, S>, terms: &[usize]) -> usize
    where
        T: DocList<'a>,
        S: FreqList<'a>,
    {
        if terms.is_empty() {
            return 0;
        }

        let max = idx.n_docs as u64;

        let query_freqs = query_freqs(terms);

        // contains pair (enum, weight)
        let mut enums = Vec::with_capacity(query_freqs.len());

        self.topk_heap.clear();

        for (term, freq) in query_freqs {
            let it = idx.get_plist_iter(term);
            let q_weight =
                Scorer::query_term_weight(freq as u64, it.len() as u64, idx.n_docs as u64);
            enums.push((it, q_weight));
        }

        let mut cur_doc = enums.iter().map(|x| x.0.current_doc()).min().unwrap();

        while cur_doc < max {
            let mut score = 0.0;
            let norm_len = self.p_data.get_norm_len(cur_doc as usize);
            let mut next_doc = max;

            for (it, q_weight) in enums.iter_mut() {
                if it.current_doc() == cur_doc {
                    score += *q_weight * Scorer::doc_term_weight(it.freq(), norm_len);
                    it.next_doc();
                }

                if it.current_doc() < next_doc {
                    next_doc = it.current_doc();
                }
            }

            // self.topk_heap.push_with_id(cur_doc, score);
            self.topk_heap.push(score);

            cur_doc = next_doc;
        }

        self.topk_heap.len()
    }
}

impl<'a, Scorer: DocScorer> RankedQueryOperator for RankedOr<'_, Scorer> {
    fn topk(&self) -> &crate::queries::topk_heap::TopKHeap {
        &self.topk_heap
    }
}

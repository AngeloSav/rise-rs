use crate::{
    DocScorer,
    indexes::freq_index::{InvertedIndex, PostingListIter},
    queries::{
        BlockPostingMetadata, QueryOperator, RankedQueryOperator, query_algorithms::query_freqs,
        topk_heap::TopKHeap,
    },
};

pub struct RankedAnd<'a, Scorer: DocScorer> {
    p_data: &'a BlockPostingMetadata<Scorer>,
    topk_heap: TopKHeap,
}

impl<'a, Scorer: DocScorer> RankedAnd<'a, Scorer> {
    pub fn new(p_data: &'a BlockPostingMetadata<Scorer>, k: usize) -> Self {
        let topk_heap = TopKHeap::new(k);

        Self { p_data, topk_heap }
    }
}

impl<Scorer: DocScorer> QueryOperator for RankedAnd<'_, Scorer> {
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

        // contains pair (enum, weight)
        let mut enums = Vec::with_capacity(query_freqs.len());

        self.topk_heap.clear();

        for (term, freq) in query_freqs {
            let it = idx.get_plist_iter(term);
            let q_weight =
                Scorer::query_term_weight(freq as u64, it.len() as u64, idx.n_docs() as u64);
            enums.push((it, q_weight));
        }

        // sort by increasing frequency
        enums.sort_by_key(|(it, _)| it.len());

        let max = idx.n_docs() as u64;

        let mut candidate = enums[0].0.current_doc();

        let mut i = 1;

        while candidate < max {
            for (it, q_weight) in enums.iter_mut().skip(i) {
                it.next_geq(candidate);
                // ngeq_ctr += 1;
                let current = it.current_doc();
                debug_assert!(
                    current >= candidate,
                    "Current {} , candidate {}",
                    current,
                    candidate
                );
                if core::intrinsics::likely(current != candidate) {
                    candidate = current;
                    i = 0;
                    break;
                }
                i += 1;
            }

            if i == enums.len() {
                let norm_len = self.p_data.get_norm_len(candidate as usize);
                let mut score = 0.0;

                for (it, q_weight) in enums.iter_mut() {
                    score += *q_weight * Scorer::doc_term_weight(it.freq(), norm_len);
                }

                self.topk_heap.push(score);
                self.topk_heap.push_with_id(candidate, score);

                enums[0].0.next_doc();
                // next_ctr += 1;
                candidate = enums[0].0.current_doc();
                i = 1;
            }
        }

        // println!("ngeq_ctr = {}, next_ctr = {}", ngeq_ctr, next_ctr);
        self.topk_heap.len()
    }

    fn query_name() -> &'static str {
        "RankedAnd"
    }
}

impl<Scorer: DocScorer> RankedQueryOperator for RankedAnd<'_, Scorer> {
    fn topk(&self) -> &crate::queries::topk_heap::TopKHeap {
        &self.topk_heap
    }
}

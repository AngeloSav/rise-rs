use crate::{
    indexes::freq_index::{DocList, FreqIndex, FreqList},
    queries::QueryOperator,
};

pub struct And;

impl QueryOperator for And {
    fn query<T, S>(&mut self, idx: &FreqIndex<T, S>, terms: &[usize]) -> usize
    where
        T: DocList,
        S: FreqList,
    {
        if terms.is_empty() {
            return 0;
        }

        let mut enums = Vec::with_capacity(terms.len());

        for &term in terms {
            //lets try boxing
            enums.push(Box::new(idx.get_plist_iter(term)));
        }

        // sort by non-decreasing size
        enums.sort_by_key(|it| it.len());

        let max = idx.n_docs as u64;

        let mut candidate = enums[0].current_doc();

        let mut i = 1;
        let mut size = 0;

        while candidate < max {
            for it in enums.iter_mut().skip(i) {
                it.next_geq(candidate);
                let current = it.current_doc();
                if core::intrinsics::likely(current != candidate) {
                    candidate = current;
                    i = 0;
                    break;
                }
                i += 1;
            }

            if i == enums.len() {
                // unsafe { *v.get_unchecked_mut(size) = candidate };
                size += 1;
                enums[0].next_doc();
                candidate = enums[0].current_doc();
                i = 1;
            }
        }
        size
    }

    fn query_name() -> &'static str {
        "And"
    }
}

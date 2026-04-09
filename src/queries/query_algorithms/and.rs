use crate::{indexes::*, queries::QueryOperator};

pub struct And {
    res: Vec<u64>,
}

impl And {
    pub fn new(n_docs: usize) -> Self {
        Self {
            res: Vec::with_capacity(n_docs),
        }
    }
}

impl QueryOperator for And {
    fn query<I>(&mut self, idx: &I, terms: &[usize]) -> usize
    where
        I: InvertedIndex,
    {
        self.res.clear();
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

        let max = idx.n_docs() as u64;

        let mut candidate = enums[0].current_doc();

        let mut i = 1;
        // let mut size = 0;

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
                // size += 1;
                self.res.push(candidate);
                enums[0].next_doc();
                candidate = enums[0].current_doc();
                i = 1;
            }
        }
        // size
        self.res.len()
    }

    fn query_name() -> &'static str {
        "And"
    }
}

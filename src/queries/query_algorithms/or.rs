use crate::{indexes::*, queries::QueryOperator};

pub struct Or {
    res: Vec<u64>,
}

impl Or {
    pub fn new(n_docs: usize) -> Self {
        Self {
            res: Vec::with_capacity(n_docs),
        }
    }
}

impl QueryOperator for Or {
    fn query<I>(&mut self, idx: &I, terms: &[usize]) -> usize
    where
        I: InvertedIndex,
    {
        // let mut next_ctr = 0;

        self.res.clear();
        if terms.is_empty() {
            return 0;
        }

        let mut enums = Vec::with_capacity(terms.len());
        for &term in terms {
            let it = idx.get_plist_iter(term);
            enums.push(it);
        }

        let mut cur_doc = enums.iter().map(|x| x.current_doc()).min().unwrap();
        // let mut size = 0;

        while cur_doc < idx.n_docs() as u64 {
            // println!("new round ---------------------");
            // println!("pushing {:?}", cur_doc);
            // unsafe { *v.get_unchecked_mut(size) = cur_doc };
            // size += 1;
            self.res.push(cur_doc);

            let mut next_doc = idx.n_docs() as u64;

            for it in enums.iter_mut() {
                let mut cur_term_docid = it.current_doc();
                // println!("new term ---");
                // println!("cur_docid = {:?}", cur_term_docid);
                if core::intrinsics::likely(cur_term_docid == cur_doc) {
                    // println!("update cur!");
                    // next_ctr += 1;
                    it.next_doc();
                    cur_term_docid = it.current_doc();
                }

                // println!("check less ---");
                // println!("cur_doc = {:?}", cur_doc);
                // println!("cur_term_docid = {:?}", cur_term_docid);
                if core::intrinsics::likely(cur_term_docid < next_doc) {
                    next_doc = cur_term_docid
                }
            }
            cur_doc = next_doc;
            // println!("nextdoc is {:?}", cur_doc);
        }
        // println!("next_ctr = {}, size = {}", next_ctr, size);
        // size
        self.res.len()
    }

    fn query_name() -> &'static str {
        "Or"
    }
}

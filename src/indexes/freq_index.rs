use std::marker::PhantomData;

use crate::{
    bitvector::bitvector_collection::BitVectorCollection, BitVecCollection, EnumeratorFromBitSlice,
    IncreasingSequenceEnumerator, ToBitvector,
};

pub struct FreqIndex<DocumentSequence> {
    _n_docs: usize,
    pub docs_sequences: BitVecCollection,
    _freqs_sequences: BitVecCollection,
    _phantom: PhantomData<DocumentSequence>,
}

pub trait PostingList: ToBitvector + EnumeratorFromBitSlice + From<Vec<u64>> {}
impl<T> PostingList for T where T: ToBitvector + EnumeratorFromBitSlice + From<Vec<u64>> {}

impl<DocumentSequence> FreqIndex<DocumentSequence>
where
    DocumentSequence: PostingList,
{
    pub fn new(n_docs: usize) -> Self {
        Self {
            _n_docs: n_docs,
            docs_sequences: BitVectorCollection::with_capacity(0, 0),
            _freqs_sequences: BitVectorCollection::with_capacity(0, 0),
            _phantom: PhantomData::<DocumentSequence>,
        }
    }

    /// Push the document sequence `s` in the document collection
    pub fn push_posting_list(&mut self, s: DocumentSequence) {
        self.docs_sequences.push(s.to_bv());
    }

    pub fn get_plist_iter(
        &self,
        i: usize,
    ) -> impl IncreasingSequenceEnumerator + use<'_, DocumentSequence> {
        let a = self.docs_sequences.get(i);
        DocumentSequence::iter_from_slice(a)
    }

    pub fn from_files(input_path: &str) -> Self {
        let docs_file = format!("{}.docs", input_path);
        let sizes_file = format!("{}.sizes", input_path);

        let binding = std::fs::read(docs_file).expect("cant read .docs file ");
        let mut docs_iter = binding
            .array_chunks::<4>()
            .map(|chunk| u32::from_le_bytes(*chunk));

        docs_iter.next();

        let mut idx = Self::new(docs_iter.next().unwrap() as usize);

        let mut processed = 0;

        while let Some(sz) = docs_iter.next() {
            let v: Vec<u64> = (&mut docs_iter)
                .take(sz as usize)
                .map(|x| x as u64)
                .collect();
            idx.push_posting_list(DocumentSequence::from(v.clone()));

            processed += 1;
            if processed % 10_000 == 0 {
                println!("processed {} plists", processed);
            }

            //correctness check
            let mut it = idx.get_plist_iter(processed - 1);
            let mut itv = v.iter();
            while let Some((x, _)) = it.next_val() {
                assert!(x == *itv.next().unwrap());
            }
        }

        idx
    }
}

use crate::{
    BitVecCollectionBuilder,
    indexes::{
        InvertedIndexBuilder,
        freq_index::{DocList, FreqIndex, FreqList},
    },
};
use std::{fmt::Debug, marker::PhantomData};

use crate::BitVec;

#[derive(Clone, Debug)]
pub struct FreqIndexBuilder<DocumentSequence, FreqSequence> {
    pub n_docs: usize,
    pub n_terms: usize,
    docs_sequences: BitVecCollectionBuilder,
    freqs_sequences: BitVecCollectionBuilder,
    pub _phantom: PhantomData<(DocumentSequence, FreqSequence)>,
}

impl<DocumentSequence, FreqSequence> InvertedIndexBuilder
    for FreqIndexBuilder<DocumentSequence, FreqSequence>
where
    DocumentSequence: DocList,
    FreqSequence: FreqList,
{
    type IndexType = FreqIndex<DocumentSequence, FreqSequence>;

    fn new(n_docs: usize) -> Self {
        Self {
            n_docs,
            n_terms: 0,
            docs_sequences: BitVecCollectionBuilder::default(),
            freqs_sequences: BitVecCollectionBuilder::default(),
            _phantom: PhantomData,
        }
    }

    fn push_plist_freqs(&mut self, docs: &[u64], freqs: &[u64]) {
        let sz = docs.len();
        rayon::join(
            || {
                let mut bv = BitVec::new();
                bv.append_gamma_nonzero(sz as u64);
                // println!("sz is: {}", sz);
                bv.concat(DocumentSequence::write_bitvector(
                    docs.into_iter().cloned(),
                    sz as usize,
                    self.n_docs as u64,
                ));

                self.docs_sequences.push(bv);
            },
            || {
                let freq_bv =
                    FreqSequence::write_bitvector(freqs.into_iter().cloned(), sz as usize, 0);
                self.freqs_sequences.push(freq_bv);
            },
        );

        self.n_terms += 1;
    }

    fn build(self) -> Self::IndexType {
        FreqIndex {
            n_docs: self.n_docs,
            n_terms: self.n_terms,
            docs_sequences: self.docs_sequences.build(),
            freqs_sequences: self.freqs_sequences.build(),
            _phantom: PhantomData,
        }
    }
}

//! Implements a immutable indexed collection of bitvectors. The bitvectors are stored in a
//! immutable bitvector and the endpoint (bitwise!) of each bit vector is stored.
//! It it possible to get the [`BitSlice`] of the i-th indexed bitvector.

use crate::bitvector::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct BitVecCollection {
    bv: BitVec,
    endpoints: Vec<usize>,
    n_vecs: usize,
}

impl Default for BitVecCollection {
    fn default() -> Self {
        Self::with_capacity(0, 0)
    }
}

impl BitVecCollection {
    pub fn with_capacity(n_bits: usize, n_vecs: usize) -> Self {
        let mut endpoints = Vec::<usize>::with_capacity(n_vecs + 1);
        endpoints.push(0); // First zero is always there

        Self {
            bv: BitVec::with_capacity(n_bits),
            endpoints,
            n_vecs: 0,
        }
    }

    pub fn push<W: AsRef<[u64]>>(&mut self, bv: impl AsRef<BitVector<W>>) {
        self.bv.concat(bv);
        self.endpoints.push(self.bv.len());
        self.n_vecs += 1;
    }

    pub fn get(&self, i: usize) -> BitSliceWithOffset {
        assert!(i < self.n_vecs, "Index out of bounds");

        let start_bit = self.endpoints[i];
        let end_bit = self.endpoints[i + 1];
        let n_bits = end_bit - start_bit;

        let start_word = start_bit / 64;
        let end_word = (end_bit + 63) / 64;
        let offset = start_bit % 64;

        dbg!(start_word, end_word, offset, n_bits, start_bit, end_bit);

        unsafe {
            BitSliceWithOffset::from_raw_parts(&self.bv.data[start_word..end_word], n_bits, offset)
        }
    }

    pub fn len(&self) -> usize {
        self.n_vecs
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    //use crate::gen_sequence::gen_strictly_increasing_sequence;

    #[test]
    fn test_bitvec_collection() {
        let mut bvc = BitVecCollection::default();
        assert!(bvc.is_empty());

        let vv1: Vec<usize> = vec![0, 63, 128, 129, 254, 1026];
        let bv: BitVec = vv1.iter().copied().collect();
        bvc.push(&bv);

        assert_eq!(bvc.len(), 1);
        assert!(!bvc.is_empty());

        let bv = BitVec::default();
        bvc.push(&bv);
        assert_eq!(bvc.len(), 2);

        let vv2: Vec<usize> = vec![0, 61, 127, 130, 242, 365];
        let bv: BitVec = vv2.iter().copied().collect();
        bvc.push(&bv);
        assert_eq!(bvc.len(), 3);

        let bswo = bvc.get(0);
        assert_eq!(bswo.len(), 1027);
        assert_eq!(bswo.get(0), Some(true));
        assert_eq!(bswo.get(63), Some(true));
        assert_eq!(bswo.get(64), Some(false));
        assert_eq!(bswo.get(1026), Some(true));

        assert_eq!(bswo.ones().collect::<Vec<usize>>(), vv1);

        let bswo = bvc.get(1);
        assert_eq!(bswo.len(), 0);
        assert_eq!(bswo.get(0), None);
        assert_eq!(bswo.ones().collect::<Vec<usize>>(), vec![]);

        let bswo = bvc.get(2);
        assert_eq!(bswo.len(), 366);

        assert_eq!(bswo.ones().collect::<Vec<usize>>(), vv2);
    }
}

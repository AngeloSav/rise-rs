//! Implements a immutable indexed collection of bitvectors. The bitvectors are stored in a
//! immutable bitvector and the endpoint (bitwise!) of each bit vector is stored.
//! It it possible to get the [`BitSlice`] of the i-th indexed bitvector.

use crate::bitvector::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct BitVecCollection {
    bv: BitVec,
    endpoints: Vec<usize>,
}

impl Default for BitVecCollection {
    fn default() -> Self {
        Self::with_capacity(0, 0)
    }
}

impl BitVecCollection {
    pub fn with_capacity(n_bits: usize, n_vecs: usize) -> Self {
        let mut endpoints = Vec::<usize>::with_capacity(n_vecs + 1);
        endpoints.push(0); // first zero is always there

        Self {
            bv: BitVec::with_capacity(n_bits),
            endpoints,
        }
    }

    pub fn push<W: AsRef<[u64]>>(&mut self, bv: impl AsRef<BitVector<W>>) {
        self.bv.concat(bv);
        self.endpoints.push(self.bv.len());
    }
}

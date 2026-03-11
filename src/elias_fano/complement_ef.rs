use crate::{
    BitVec, EliasFano, EliasFanoIter, EnumeratorFromBitSlice, EstimateSpace, NextGEQ,
    SequenceEnumerator, WriteBitvector,
};
use epserde::prelude::*;

#[derive(Debug, Epserde)]
pub struct ComplementEliasFano {
    u: u64,
    n: usize,
    bv: BitVec,
}

impl ComplementEliasFano {
    /// Returns the number of elements in the sequence
    pub fn len(&self) -> usize {
        self.n
    }

    pub fn iter(&self) -> ComplementEliasFanoIter<'_> {
        Self::iter_from_slice(self.bv.as_bitslice(), self.n, self.u)
    }
}

impl<'a> From<&'a [u64]> for ComplementEliasFano {
    fn from(v: &'a [u64]) -> Self {
        // todo!();
        let u = *v.iter().max().expect("sequence is empty!") + 1;
        let n = v.len();

        let bv = Self::write_bitvector(v.as_ref(), n, u);

        Self { u, n, bv }
    }
}

impl WriteBitvector for ComplementEliasFano {
    fn write_bitvector(seq: &[u64], n: usize, u: u64) -> BitVec {
        assert!(u >= n as u64);

        let mut missing = Vec::with_capacity(u as usize - n);
        for cur in 0..u {
            if !seq.contains(&cur) {
                missing.push(cur);
            }
        }

        assert!(missing.len() == (u as usize - n));

        EliasFano::write_bitvector(missing.as_ref(), missing.len(), u)
    }
}

impl<'a> EnumeratorFromBitSlice<'a> for ComplementEliasFano {
    type IterType = ComplementEliasFanoIter<'a>;

    fn iter_from_slice(bv: crate::BitSliceWithOffset<'a>, n: usize, u: u64) -> Self::IterType {
        let mut it = EliasFano::iter_from_slice(bv, u as usize - n, u);
        let next_missing = it.next_val().0;

        // println!("next_missing: {}", next_missing);

        ComplementEliasFanoIter {
            it,
            next_missing,
            u,
            cur_value: 0,
            cur_pos: 0,
        }
    }
}

#[derive(Debug)]
pub struct ComplementEliasFanoIter<'a> {
    it: EliasFanoIter<'a>,
    next_missing: u64,
    cur_value: u64,
    cur_pos: usize,
    u: u64,
}

impl SequenceEnumerator for ComplementEliasFanoIter<'_> {
    fn next_val(&mut self) -> (u64, usize) {
        // self.cur_value += 1;

        // while self.cur_value - 1 == self.next_missing {
        //     self.next_missing = self.it.next_val().0;
        //     self.cur_value += 1;
        // }
        // self.cur_pos += 1;

        loop {
            if core::intrinsics::unlikely(self.cur_value > self.u) {
                return (self.u, self.cur_pos);
            }

            self.cur_value += 1;
            if self.cur_value - 1 == self.next_missing {
                self.next_missing = self.it.next_val().0;
            } else {
                break;
            }
        }

        self.cur_pos += 1;

        (self.cur_value - 1, self.cur_pos - 1)
    }

    fn move_to_position(&mut self, pos: usize) -> (u64, usize) {
        let candidate = self.it.next_geq(pos as u64);

        // TODO: optimize
        self.next_missing = candidate.0;
        self.cur_value = pos as u64;
        self.cur_pos = pos - candidate.1;

        self.next_val()
    }

    fn len(&self) -> usize {
        self.it.len()
    }
}

impl NextGEQ for ComplementEliasFanoIter<'_> {
    fn next_geq(&mut self, target: u64) -> (u64, usize) {
        let candidate = self.it.next_geq(target);

        self.next_missing = candidate.0;
        self.cur_value = target;
        self.cur_pos = target as usize - candidate.1;

        self.next_val()
    }
}

impl Iterator for ComplementEliasFanoIter<'_> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        let (val, pos) = self.next_val();
        if pos >= self.len() {
            return None;
        }
        Some(val)
    }
}

impl EstimateSpace for ComplementEliasFano {
    fn bitsize(u: u64, n: usize) -> usize {
        assert!(u >= n as u64);

        // if the list is really dense, we can represent the elements that are not in the list
        let n_missing = u - n as u64;
        if n_missing == 0 {
            return usize::MAX;
        }

        EliasFano::bitsize(u as u64, n_missing as usize)
    }
}

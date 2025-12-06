use epserde::Epserde;

use crate::{
    BitSliceWithOffset, BitVec, EnumeratorFromBitSlice, EstimateSpace, NextGEQ, SequenceEnumerator,
    WriteBitvector,
};

#[derive(Debug, Epserde)]
pub struct AllOnes {
    n: usize,
}

impl AllOnes {
    pub fn iter(&self) -> AllOnesIter {
        AllOnesIter {
            len: self.n,
            pos: 0,
        }
    }
}

impl<'a> From<&'a [u64]> for AllOnes {
    fn from(v: &'a [u64]) -> Self {
        assert!(*v.last().unwrap() + 1 == v.len() as u64);
        assert!(
            v.array_windows::<2>().all(|[x, y]| x < y),
            "Sequence must be strictly increasing!"
        );
        Self { n: v.len() }
    }
}

impl EstimateSpace for AllOnes {
    fn bitsize(u: u64, n: usize) -> usize {
        if u == n as u64 {
            0
        } else {
            usize::MAX
        }
    }
}

impl<'a> EnumeratorFromBitSlice<'a> for AllOnes {
    type IterType = AllOnesIter;

    fn iter_from_slice(_bv: BitSliceWithOffset<'a>, n: usize, u: u64) -> Self::IterType {
        debug_assert!(n as u64 == u);
        AllOnesIter { len: n, pos: 0 }
    }
}

impl WriteBitvector for AllOnes {
    fn write_bitvector(seq: &[u64], n: usize, u: u64) -> BitVec {
        assert!(n == seq.len());
        assert!(u == n as u64);
        assert!(*seq.last().unwrap() + 1 == u);
        BitVec::new()
    }
}

#[derive(Debug)]
pub struct AllOnesIter {
    len: usize,
    pos: usize,
}

impl SequenceEnumerator for AllOnesIter {
    fn next_val(&mut self) -> (u64, usize) {
        if self.pos < self.len {
            let tmp = self.pos;
            self.pos += 1;
            (tmp as u64, tmp)
        } else {
            (self.len as u64, self.len)
        }
    }

    fn move_to_position(&mut self, pos: usize) -> (u64, usize) {
        self.pos = pos;
        self.next_val()
    }

    fn len(&self) -> usize {
        self.len
    }
}

impl NextGEQ for AllOnesIter {
    fn next_geq(&mut self, lower_bound: u64) -> (u64, usize) {
        if lower_bound >= self.len as u64 {
            (self.len as u64, self.len)
        } else {
            // Some((lower_bound, lower_bound as usize))
            self.pos = lower_bound as usize;
            self.next_val()
        }
    }
}

impl Iterator for AllOnesIter {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        let val = self.next_val().0;
        if val == self.len as u64 {
            return None;
        }
        Some(val)
    }
}

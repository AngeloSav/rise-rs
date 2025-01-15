use crate::{
    BitSliceWithOffset, BitVec, EnumeratorFromBitSlice, EstimateSpace,
    IncreasingSequenceEnumerator, ToBitvector, WriteBitvector,
};

#[derive(Debug)]
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

impl ToBitvector for AllOnes {
    fn to_bv(&self) -> BitVec {
        let mut bv = BitVec::new();
        bv.append_gamma(self.n as u64);
        bv
    }
}

impl<'a> EnumeratorFromBitSlice<'a, AllOnesIter> for AllOnes {
    fn iter_from_slice(bv: BitSliceWithOffset<'a>) -> AllOnesIter {
        let n = unsafe { bv.get_gamma_unchecked(0).0 as usize };
        AllOnesIter { len: n, pos: 0 }
    }

    fn iter_from_slice_with_data(_bv: BitSliceWithOffset<'a>, n: usize, u: u64) -> AllOnesIter {
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

impl IncreasingSequenceEnumerator for AllOnesIter {
    #[inline]
    fn next_val(&mut self) -> Option<(u64, usize)> {
        if self.pos < self.len {
            let tmp = self.pos;
            self.pos += 1;
            Some((tmp as u64, tmp))
        } else {
            None
        }
    }

    fn next_geq(&mut self, lower_bound: u64) -> Option<(u64, usize)> {
        if lower_bound >= self.len as u64 {
            None
        } else {
            Some((lower_bound, lower_bound as usize))
        }
    }

    fn move_to_position(&mut self, pos: usize) -> Option<(u64, usize)> {
        self.pos = pos;
        self.next_val()
    }

    fn current_position(&self) -> usize {
        todo!()
    }
}

impl Iterator for AllOnesIter {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.next_val()?.0)
    }
}

use crate::{
    BitSliceWithOffset, BitVec, EnumeratorFromBitSlice, EstimateSpace,
    IncreasingSequenceEnumerator, ToBitvector,
};

use super::{
    all_ones_seq::{AllOnes, AllOnesIter},
    ranked_bv::{RankedBv, RankedBvIter},
    EliasFano, EliasFanoIter,
};
#[derive(Debug)]

pub enum IndexType {
    EliasFanoT(EliasFano),
    RankedBvT(RankedBv),
    AllOnesT(AllOnes),
}

#[derive(Debug)]
pub enum IterType<'a> {
    EliasFanoItT(EliasFanoIter<'a>),
    RankedBvItT(RankedBvIter<'a>),
    AllOnesItT(AllOnesIter),
}

#[derive(Debug)]
pub struct IndexedSequence {
    sequence: IndexType,
}

impl EstimateSpace for IndexedSequence {
    fn bitsize(u: u64, n: usize) -> usize {
        let mut best_type = AllOnes::bitsize(u, n);
        best_type = best_type.min(RankedBv::bitsize(u, n));
        best_type = best_type.min(AllOnes::bitsize(u, n));
        best_type
    }
}

impl From<Vec<u64>> for IndexedSequence {
    fn from(v: Vec<u64>) -> Self {
        let n = v.len();
        let u = *v.last().unwrap();
        let sequence = if AllOnes::bitsize(u, n) == 0 {
            IndexType::AllOnesT(AllOnes::from(v))
        } else if RankedBv::bitsize(u, n) <= EliasFano::bitsize(u, n) {
            IndexType::RankedBvT(RankedBv::from(v))
        } else {
            IndexType::EliasFanoT(EliasFano::from(v))
        };

        Self { sequence }
    }
}

impl ToBitvector for IndexedSequence {
    fn to_bv(&self) -> BitVec {
        let mut bv = BitVec::new();
        let (t, bvs) = match &self.sequence {
            IndexType::AllOnesT(x) => (0, x.to_bv()),
            IndexType::RankedBvT(x) => (1, x.to_bv()),
            IndexType::EliasFanoT(x) => (2, x.to_bv()),
        };
        bv.append_bits(t, 2);
        bv.concat(bvs);
        bv
    }
}

impl<'a> EnumeratorFromBitSlice<'a, IndexedSequenceIter<'a>> for IndexedSequence {
    fn iter_from_slice(bv: BitSliceWithOffset<'a>) -> IndexedSequenceIter<'a> {
        let slice = bv.split_at(2).1;
        let it = match bv.get_bits(0, 2) {
            Some(0) => IterType::AllOnesItT(AllOnes::iter_from_slice(slice)),
            Some(1) => IterType::RankedBvItT(RankedBv::iter_from_slice(slice)),
            Some(2) => IterType::EliasFanoItT(EliasFano::iter_from_slice(slice)),
            _ => unreachable!(),
        };
        IndexedSequenceIter { it }
    }
}

#[derive(Debug)]
pub struct IndexedSequenceIter<'a> {
    it: IterType<'a>,
}

impl IncreasingSequenceEnumerator for IndexedSequenceIter<'_> {
    fn next_val(&mut self) -> Option<(u64, usize)> {
        match &mut self.it {
            IterType::EliasFanoItT(it) => it.next_val(),
            IterType::RankedBvItT(it) => it.next_val(),
            IterType::AllOnesItT(it) => it.next_val(),
        }
    }

    fn next_geq(&mut self, i: u64) -> Option<(u64, usize)> {
        match &mut self.it {
            IterType::EliasFanoItT(it) => it.next_geq(i),
            IterType::RankedBvItT(it) => it.next_geq(i),
            IterType::AllOnesItT(it) => it.next_geq(i),
        }
    }

    fn move_to_position(&mut self, pos: usize) {
        todo!()
    }

    fn position(&self) -> usize {
        todo!()
    }
}

impl Iterator for IndexedSequenceIter<'_> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.next_val()?.0)
    }
}

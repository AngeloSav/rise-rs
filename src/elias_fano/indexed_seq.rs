use std::slice::Iter;

use crate::{
    AccessBin, BitSliceWithOffset, BitVec, CostWindow, EnumeratorFromBitSlice, EstimateSpace,
    IncreasingSequenceEnumerator, PartitionableSequence, ToBitvector, WriteBitvector,
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
pub enum IndexTypeNew {
    EliasFanoT,
    RankedBvT,
    AllOnesT,
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

impl<'a> From<&'a [u64]> for IndexedSequence {
    fn from(v: &'a [u64]) -> Self {
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

impl WriteBitvector for IndexedSequence {
    fn write_bitvector(seq: &[u64], n: usize, u: u64) -> BitVec {
        let mut bv = BitVec::new();
        let (t, bv_data) = if AllOnes::bitsize(u, n) == 0 {
            (IndexTypeNew::AllOnesT, AllOnes::write_bitvector(seq, n, u))
        } else if RankedBv::bitsize(u, n) <= EliasFano::bitsize(u, n) {
            (
                IndexTypeNew::RankedBvT,
                RankedBv::write_bitvector(seq, n, u),
            )
        } else {
            (
                IndexTypeNew::EliasFanoT,
                EliasFano::write_bitvector(seq, n, u),
            )
        };

        //all ones is implicit
        // println!("writing itertype: {:?}", t);
        match t {
            IndexTypeNew::EliasFanoT => {
                bv.push(false);
            }
            IndexTypeNew::RankedBvT => {
                bv.push(true);
            }
            IndexTypeNew::AllOnesT => (), //implicit ,
        }

        //all ones is implicit
        bv.concat(bv_data);
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

    fn iter_from_slice_with_data(
        bv: BitSliceWithOffset<'a>,
        n: usize,
        u: u64,
    ) -> IndexedSequenceIter<'a> {
        let t = if AllOnes::bitsize(u, n) == 0 {
            IndexTypeNew::AllOnesT
        } else {
            match bv.get(0).unwrap() {
                true => IndexTypeNew::RankedBvT,
                false => IndexTypeNew::EliasFanoT,
            }
        };

        // println!("now using itertype: {:?}", t);

        let it = match t {
            IndexTypeNew::EliasFanoT => {
                let slice = bv.split_at(1).1;
                IterType::EliasFanoItT(EliasFano::iter_from_slice_with_data(slice, n, u))
            }
            IndexTypeNew::RankedBvT => {
                let slice = bv.split_at(1).1;
                IterType::RankedBvItT(RankedBv::iter_from_slice_with_data(slice, n, u))
            }
            IndexTypeNew::AllOnesT => {
                IterType::AllOnesItT(AllOnes::iter_from_slice_with_data(bv, n, u))
            }
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

    fn next_geq(&mut self, lower_bound: u64) -> Option<(u64, usize)> {
        match &mut self.it {
            IterType::EliasFanoItT(it) => it.next_geq(lower_bound),
            IterType::RankedBvItT(it) => it.next_geq(lower_bound),
            IterType::AllOnesItT(it) => it.next_geq(lower_bound),
        }
    }

    fn move_to_position(&mut self, pos: usize) -> Option<(u64, usize)> {
        match &mut self.it {
            IterType::EliasFanoItT(it) => it.move_to_position(pos),
            IterType::RankedBvItT(it) => it.move_to_position(pos),
            IterType::AllOnesItT(it) => it.move_to_position(pos),
        }
    }

    fn current_position(&self) -> usize {
        todo!()
    }
}

impl Iterator for IndexedSequenceIter<'_> {
    type Item = u64;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        Some(self.next_val()?.0)
    }
}

#[derive(Debug)]
pub struct IndexSeqCostWindow<'a> {
    start_it: std::iter::Peekable<Iter<'a, u64>>,
    end_it: std::iter::Peekable<Iter<'a, u64>>,
    start: usize,
    end: usize,
    min_p: u64,
    max_p: u64,
    cost_upper_bound: usize,
}

impl<'a> IndexSeqCostWindow<'a> {
    const FIX_COST: usize = 128;
}

impl<'a> CostWindow<'a> for IndexSeqCostWindow<'a> {
    fn new(sequence: &'a [u64], cost_upper_bound: usize) -> Self {
        let mut start_it = sequence.iter().peekable();
        let end_it = sequence.iter().peekable();
        let min_p = **start_it.peek().unwrap();
        let max_p = 0;

        IndexSeqCostWindow {
            start_it,
            end_it,
            start: 0,
            end: 0,
            min_p,
            max_p,
            cost_upper_bound,
        }
    }

    #[inline(always)]
    fn universe(&self) -> u64 {
        self.max_p - self.min_p + 1
    }

    #[inline(always)]
    fn size(&self) -> usize {
        self.end - self.start
    }

    #[inline(always)]
    fn window_cost(&self) -> usize {
        IndexedSequence::bitsize(self.universe(), self.size()) + Self::FIX_COST
    }

    #[inline(always)]
    fn single_block_cost(sequence: &[u64]) -> usize {
        IndexedSequence::bitsize(*sequence.last().unwrap() + 1, sequence.len()) + Self::FIX_COST
    }

    #[inline(always)]
    fn advance_start(&mut self) {
        if let Some(&&x) = self.start_it.peek() {
            self.min_p = x + 1;
            self.start += 1;
            self.start_it.next();
        } else {
            panic!("window advanced too far!")
        }
    }

    #[inline(always)]
    fn advance_end(&mut self) {
        if let Some(&&x) = self.end_it.peek() {
            self.max_p = x;
            self.end += 1;
            self.end_it.next();
        } else {
            panic!("window advanced too far!")
        }
    }

    #[inline(always)]
    fn start(&self) -> usize {
        self.start
    }

    #[inline(always)]
    fn end(&self) -> usize {
        self.end
    }

    #[inline(always)]
    fn cost_upper_bound(&self) -> usize {
        self.cost_upper_bound
    }

    #[inline(always)]
    fn minimum_cost(_sequence: &[u64]) -> usize {
        IndexedSequence::bitsize(1, 1) + Self::FIX_COST
    }
}

impl<'a> PartitionableSequence<'a> for IndexedSequence {
    type CW = IndexSeqCostWindow<'a>;
}

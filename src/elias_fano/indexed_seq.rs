use std::{marker::PhantomData, slice::Iter};

use crate::{
    indexes::freq_index::FreqList, AccessBin, BitSliceWithOffset, BitVec, CostWindow,
    EnumeratorFromBitSlice, EstimateSpace, NextGEQ, PartitionableSequence, SequenceEnumerator,
    WriteBitvector,
};

use super::{
    all_ones_seq::{AllOnes, AllOnesIter},
    ranked_bv::{RankedBv, RankedBvIter},
    strict_ef::StrictEliasFano,
    EliasFano,
};

pub trait EFVariant: for<'a> FreqList<'a> + EstimateSpace {}
impl EFVariant for EliasFano {}
impl EFVariant for StrictEliasFano {}

#[derive(Debug)]
enum IndexType<EF: EFVariant> {
    EliasFanoT(EF),
    RankedBvT(RankedBv),
    AllOnesT(AllOnes),
}

#[derive(Debug)]
enum IndexTypeNew {
    EliasFanoT,
    RankedBvT,
    AllOnesT,
}

#[derive(Debug)]
enum IterType<'a, EF: EFVariant> {
    EliasFanoItT(<EF as EnumeratorFromBitSlice<'a>>::IterType),
    RankedBvItT(RankedBvIter<'a>),
    AllOnesItT(AllOnesIter),
}

pub type IndexSequence = IndexedSequence<EliasFano>;
pub type StrictSequence = IndexedSequence<StrictEliasFano>;

// now you can chose which ef to use but not define others
#[derive(Debug)]
pub struct IndexedSequence<EF: EFVariant = EliasFano> {
    sequence: IndexType<EF>,
}

impl IndexedSequence<EliasFano> {
    pub fn iter(&self) -> IndexedSequenceIter<EliasFano> {
        IndexedSequenceIter {
            it: match &self.sequence {
                IndexType::EliasFanoT(ef) => IterType::EliasFanoItT(ef.iter()),
                IndexType::RankedBvT(rbv) => IterType::RankedBvItT(rbv.iter()),
                IndexType::AllOnesT(aos) => IterType::AllOnesItT(aos.iter()),
            },
        }
    }
}

impl IndexedSequence<StrictEliasFano> {
    pub fn iter(&self) -> IndexedSequenceIter<StrictEliasFano> {
        IndexedSequenceIter {
            it: match &self.sequence {
                IndexType::EliasFanoT(ef) => IterType::EliasFanoItT(ef.iter()),
                IndexType::RankedBvT(rbv) => IterType::RankedBvItT(rbv.iter()),
                IndexType::AllOnesT(aos) => IterType::AllOnesItT(aos.iter()),
            },
        }
    }
}

impl<EF: EFVariant> EstimateSpace for IndexedSequence<EF> {
    fn bitsize(u: u64, n: usize) -> usize {
        let mut best_type = AllOnes::bitsize(u, n);
        best_type = best_type.min(RankedBv::bitsize(u, n)) + 1;
        best_type = best_type.min(EF::bitsize(u, n)) + 1;
        best_type
    }
}

impl<'a, EF: EFVariant> From<&'a [u64]> for IndexedSequence<EF> {
    fn from(v: &'a [u64]) -> Self {
        let n = v.len();
        let u = *v.last().unwrap() + 1;
        let sequence = if AllOnes::bitsize(u, n) == 0 {
            IndexType::AllOnesT(AllOnes::from(v))
        } else if RankedBv::bitsize(u, n) <= EF::bitsize(u, n) {
            IndexType::RankedBvT(RankedBv::from(v))
        } else {
            IndexType::EliasFanoT(EF::from(v))
        };

        Self { sequence }
    }
}

impl<EF: EFVariant> WriteBitvector for IndexedSequence<EF> {
    fn write_bitvector(seq: &[u64], n: usize, u: u64) -> BitVec {
        let mut bv = BitVec::new();
        let (t, bv_data) = if AllOnes::bitsize(u, n) == 0 {
            (IndexTypeNew::AllOnesT, AllOnes::write_bitvector(seq, n, u))
        } else if RankedBv::bitsize(u, n) < EF::bitsize(u, n) {
            (
                IndexTypeNew::RankedBvT,
                RankedBv::write_bitvector(seq, n, u),
            )
        } else {
            (IndexTypeNew::EliasFanoT, EF::write_bitvector(seq, n, u))
        };

        //all ones is implicit
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

impl<'a, EF: EFVariant> EnumeratorFromBitSlice<'a> for IndexedSequence<EF> {
    type IterType = IndexedSequenceIter<'a, EF>;

    fn iter_from_slice(bv: BitSliceWithOffset<'a>, n: usize, u: u64) -> Self::IterType {
        let t = if AllOnes::bitsize(u, n) == 0 {
            IndexTypeNew::AllOnesT
        } else {
            match bv.get(0).unwrap() {
                true => IndexTypeNew::RankedBvT,
                false => IndexTypeNew::EliasFanoT,
            }
        };

        let it = match t {
            IndexTypeNew::EliasFanoT => {
                let slice = bv.split_at(1).1;
                IterType::EliasFanoItT(EF::iter_from_slice(slice, n, u))
            }
            IndexTypeNew::RankedBvT => {
                let slice = bv.split_at(1).1;
                IterType::RankedBvItT(RankedBv::iter_from_slice(slice, n, u))
            }
            IndexTypeNew::AllOnesT => IterType::AllOnesItT(AllOnes::iter_from_slice(bv, n, u)),
        };
        IndexedSequenceIter { it }
    }
}

#[derive(Debug)]
pub struct IndexedSequenceIter<'a, EF: EFVariant> {
    it: IterType<'a, EF>,
}

impl<EF: EFVariant> SequenceEnumerator for IndexedSequenceIter<'_, EF> {
    fn next_val(&mut self) -> (u64, usize) {
        match &mut self.it {
            IterType::EliasFanoItT(it) => it.next_val(),
            IterType::RankedBvItT(it) => it.next_val(),
            IterType::AllOnesItT(it) => it.next_val(),
        }
    }

    fn move_to_position(&mut self, pos: usize) -> (u64, usize) {
        match &mut self.it {
            IterType::EliasFanoItT(it) => it.move_to_position(pos),
            IterType::RankedBvItT(it) => it.move_to_position(pos),
            IterType::AllOnesItT(it) => it.move_to_position(pos),
        }
    }

    fn len(&self) -> usize {
        match &self.it {
            IterType::EliasFanoItT(it) => it.len(),
            IterType::RankedBvItT(it) => it.len(),
            IterType::AllOnesItT(it) => it.len(),
        }
    }
}

impl<EF: EFVariant<IterType: NextGEQ>> NextGEQ for IndexedSequenceIter<'_, EF> {
    fn next_geq(&mut self, lower_bound: u64) -> (u64, usize) {
        match &mut self.it {
            IterType::EliasFanoItT(it) => it.next_geq(lower_bound),
            IterType::RankedBvItT(it) => it.next_geq(lower_bound),
            IterType::AllOnesItT(it) => it.next_geq(lower_bound),
        }
    }
}

impl<EF: EFVariant> Iterator for IndexedSequenceIter<'_, EF> {
    type Item = u64;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.it {
            IterType::EliasFanoItT(it) => it.next(),
            IterType::RankedBvItT(it) => it.next(),
            IterType::AllOnesItT(it) => it.next(),
        }
    }
}

#[derive(Debug)]
pub struct IndexSeqCostWindow<'a, EF: EFVariant> {
    start_it: std::iter::Peekable<Iter<'a, u64>>,
    end_it: std::iter::Peekable<Iter<'a, u64>>,
    start: usize,
    end: usize,
    min_p: u64,
    max_p: u64,
    cost_upper_bound: usize,
    _phantom: PhantomData<EF>,
}

impl<'a, EF: EFVariant> IndexSeqCostWindow<'a, EF> {
    const FIX_COST: usize = 128;
}

impl<'a, EF: EFVariant> CostWindow<'a> for IndexSeqCostWindow<'a, EF> {
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
            _phantom: PhantomData,
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
        IndexedSequence::<EF>::bitsize(self.universe(), self.size()) + Self::FIX_COST
    }

    #[inline(always)]
    fn single_block_cost(sequence: &[u64]) -> usize {
        IndexedSequence::<EF>::bitsize(*sequence.last().unwrap() + 1, sequence.len())
            + Self::FIX_COST
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
        IndexedSequence::<EF>::bitsize(1, 1) + Self::FIX_COST
    }
}

impl<'a, EF: EFVariant> PartitionableSequence<'a> for IndexedSequence<EF> {
    type CW = IndexSeqCostWindow<'a, EF>;
}

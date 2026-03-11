use std::{marker::PhantomData, slice::Iter};

use epserde::Epserde;

use crate::{
    AccessBin, BitSliceWithOffset, BitVec, CostWindow, EnumeratorFromBitSlice, EstimateSpace,
    NextGEQ, PartitionableSequence, SequenceEnumerator, WriteBitvector,
    elias_fano::complement_ef::{ComplementEliasFano, ComplementEliasFanoIter},
    indexes::freq_index::FreqList,
};

use super::{
    EliasFano,
    all_ones_seq::{AllOnes, AllOnesIter},
    ranked_bv::{RankedBv, RankedBvIter},
    strict_ef::StrictEliasFano,
};

pub trait EFVariant: for<'a> FreqList + EstimateSpace {}
impl EFVariant for EliasFano {}
impl EFVariant for StrictEliasFano {}

#[derive(Debug, Epserde)]
enum IndexType<EF: EFVariant> {
    EliasFanoT(EF),
    RankedBvT(RankedBv),
    AllOnesT(AllOnes),
    ComplementEliasFanoT(ComplementEliasFano),
}

#[derive(Debug)]
enum IndexTypeNew {
    EliasFanoT,
    RankedBvT,
    AllOnesT,
    ComplementEliasFanoT,
}

#[derive(Debug)]
enum IterType<'a, EF: EFVariant> {
    EliasFanoItT(<EF as EnumeratorFromBitSlice<'a>>::IterType),
    RankedBvItT(RankedBvIter<'a>),
    AllOnesItT(AllOnesIter),
    CompEliasFanoItT(ComplementEliasFanoIter<'a>),
}

pub type IndexCompSequence = IndexedCompSequence<EliasFano>;
pub type StrictCompSequence = IndexedCompSequence<StrictEliasFano>;

// now you can chose which ef to use but not define others
#[derive(Debug, Epserde)]
pub struct IndexedCompSequence<EF: EFVariant = EliasFano> {
    sequence: IndexType<EF>,
}

impl IndexedCompSequence<EliasFano> {
    pub fn iter(&self) -> IndexedCompSequenceIter<'_, EliasFano> {
        IndexedCompSequenceIter {
            it: match &self.sequence {
                IndexType::EliasFanoT(ef) => IterType::EliasFanoItT(ef.iter()),
                IndexType::RankedBvT(rbv) => IterType::RankedBvItT(rbv.iter()),
                IndexType::AllOnesT(aos) => IterType::AllOnesItT(aos.iter()),
                IndexType::ComplementEliasFanoT(ce) => IterType::CompEliasFanoItT(ce.iter()),
            },
        }
    }
}

impl IndexedCompSequence<StrictEliasFano> {
    pub fn iter(&self) -> IndexedCompSequenceIter<'_, StrictEliasFano> {
        IndexedCompSequenceIter {
            it: match &self.sequence {
                IndexType::EliasFanoT(ef) => IterType::EliasFanoItT(ef.iter()),
                IndexType::RankedBvT(rbv) => IterType::RankedBvItT(rbv.iter()),
                IndexType::AllOnesT(aos) => IterType::AllOnesItT(aos.iter()),
                IndexType::ComplementEliasFanoT(ce) => IterType::CompEliasFanoItT(ce.iter()),
            },
        }
    }
}

impl<EF: EFVariant> IndexedCompSequence<EF> {
    fn best_type(u: u64, n: usize) -> (usize, IndexTypeNew) {
        let mut best_size = AllOnes::bitsize(u, n);
        let mut best_type = IndexTypeNew::AllOnesT;

        if best_size == 0 {
            return (best_size, best_type);
        }

        let fix_bits = 1;
        if RankedBv::bitsize(u, n) + fix_bits < best_size {
            best_size = RankedBv::bitsize(u, n) + fix_bits;
            best_type = IndexTypeNew::RankedBvT;
        }

        if ComplementEliasFano::bitsize(u, n) + fix_bits < best_size {
            best_size = ComplementEliasFano::bitsize(u, n) + fix_bits;
            best_type = IndexTypeNew::ComplementEliasFanoT;
        }

        if EF::bitsize(u, n) + fix_bits < best_size {
            best_size = EF::bitsize(u, n) + fix_bits;
            best_type = IndexTypeNew::EliasFanoT;
        }

        (best_size, best_type)
    }
}

impl<EF: EFVariant> EstimateSpace for IndexedCompSequence<EF> {
    fn bitsize(u: u64, n: usize) -> usize {
        let (best_size, _) = Self::best_type(u, n);
        best_size
    }
}

impl<'a, EF: EFVariant> From<&'a [u64]> for IndexedCompSequence<EF> {
    fn from(v: &'a [u64]) -> Self {
        let n = v.len();
        let u = *v.last().unwrap() + 1;

        match Self::best_type(u, n).1 {
            IndexTypeNew::AllOnesT => Self {
                sequence: IndexType::AllOnesT(AllOnes::from(v)),
            },
            IndexTypeNew::RankedBvT => Self {
                sequence: IndexType::RankedBvT(RankedBv::from(v)),
            },
            IndexTypeNew::ComplementEliasFanoT => Self {
                sequence: IndexType::ComplementEliasFanoT(ComplementEliasFano::from(v)),
            },
            IndexTypeNew::EliasFanoT => Self {
                sequence: IndexType::EliasFanoT(EF::from(v)),
            },
        }
        // let sequence = if AllOnes::bitsize(u, n) == 0 {
        //     IndexType::AllOnesT(AllOnes::from(v))
        // } else if RankedBv::bitsize(u, n) <= EF::bitsize(u, n) {
        //     if ComplementEliasFano::bitsize(u, n) < RankedBv::bitsize(u, n) {
        //         IndexType::ComplementEliasFanoT(ComplementEliasFano::from(v))
        //     } else {
        //         IndexType::RankedBvT(RankedBv::from(v))
        //     }
        // } else {
        //     IndexType::EliasFanoT(EF::from(v))
        // };
    }
}

impl<EF: EFVariant> WriteBitvector for IndexedCompSequence<EF> {
    fn write_bitvector(seq: &[u64], n: usize, u: u64) -> BitVec {
        let mut bv = BitVec::new();
        // let (t, bv_data) = if AllOnes::bitsize(u, n) == 0 {
        //     (IndexTypeNew::AllOnesT, AllOnes::write_bitvector(seq, n, u))
        // } else if RankedBv::bitsize(u, n) < EF::bitsize(u, n) {
        //     (
        //         IndexTypeNew::RankedBvT,
        //         RankedBv::write_bitvector(seq, n, u),
        //     )
        // } else if ComplementEliasFano::bitsize(u, n) < RankedBv::bitsize(u, n) {
        //     (
        //         IndexTypeNew::ComplementEliasFanoT,
        //         ComplementEliasFano::write_bitvector(seq, n, u),
        //     )
        // } else {
        //     (IndexTypeNew::EliasFanoT, EF::write_bitvector(seq, n, u))
        // };

        // //all ones is implicit
        // match t {
        //     IndexTypeNew::EliasFanoT => {
        //         bv.push(false);
        //         bv.push(false);
        //     }
        //     IndexTypeNew::RankedBvT => {
        //         bv.push(false);
        //         bv.push(true);
        //     }
        //     IndexTypeNew::ComplementEliasFanoT => {
        //         bv.push(true);
        //         bv.push(false);
        //     }
        //     IndexTypeNew::AllOnesT => (), //implicit ,
        // }

        // //all ones is implicit
        // bv.concat(bv_data);
        // bv
        match Self::best_type(u, n).1 {
            IndexTypeNew::AllOnesT => {
                //implicit
            }
            IndexTypeNew::RankedBvT => {
                bv.push(true);
                bv.concat(RankedBv::write_bitvector(seq, n, u))
            }

            IndexTypeNew::EliasFanoT => {
                bv.push(false);
                bv.concat(EF::write_bitvector(seq, n, u))
            }
            IndexTypeNew::ComplementEliasFanoT => {
                bv.push(false);
                bv.concat(ComplementEliasFano::write_bitvector(seq, n, u))
            }
        };

        bv
    }
}

impl<'a, EF: EFVariant> EnumeratorFromBitSlice<'a> for IndexedCompSequence<EF> {
    type IterType = IndexedCompSequenceIter<'a, EF>;

    fn iter_from_slice(bv: BitSliceWithOffset<'a>, n: usize, u: u64) -> Self::IterType {
        let t = if AllOnes::bitsize(u, n) == 0 {
            IndexTypeNew::AllOnesT
        } else {
            match bv.get(0).unwrap() {
                true => IndexTypeNew::RankedBvT,
                false => {
                    if EF::bitsize(u, n) < ComplementEliasFano::bitsize(u, n) {
                        IndexTypeNew::EliasFanoT
                    } else {
                        IndexTypeNew::ComplementEliasFanoT
                    }
                }
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
            IndexTypeNew::ComplementEliasFanoT => {
                let slice = bv.split_at(1).1;
                IterType::CompEliasFanoItT(ComplementEliasFano::iter_from_slice(slice, n, u))
            }
            IndexTypeNew::AllOnesT => IterType::AllOnesItT(AllOnes::iter_from_slice(bv, n, u)),
        };
        IndexedCompSequenceIter { it }
    }
}

#[derive(Debug)]
pub struct IndexedCompSequenceIter<'a, EF: EFVariant> {
    it: IterType<'a, EF>,
}

impl<EF: EFVariant> SequenceEnumerator for IndexedCompSequenceIter<'_, EF> {
    fn next_val(&mut self) -> (u64, usize) {
        match &mut self.it {
            IterType::EliasFanoItT(it) => it.next_val(),
            IterType::RankedBvItT(it) => it.next_val(),
            IterType::CompEliasFanoItT(it) => it.next_val(),
            IterType::AllOnesItT(it) => it.next_val(),
        }
    }

    fn move_to_position(&mut self, pos: usize) -> (u64, usize) {
        match &mut self.it {
            IterType::EliasFanoItT(it) => it.move_to_position(pos),
            IterType::RankedBvItT(it) => it.move_to_position(pos),
            IterType::CompEliasFanoItT(it) => it.move_to_position(pos),
            IterType::AllOnesItT(it) => it.move_to_position(pos),
        }
    }

    fn len(&self) -> usize {
        match &self.it {
            IterType::EliasFanoItT(it) => it.len(),
            IterType::RankedBvItT(it) => it.len(),
            IterType::CompEliasFanoItT(it) => it.len(),
            IterType::AllOnesItT(it) => it.len(),
        }
    }
}

impl<EF: EFVariant<IterType: NextGEQ>> NextGEQ for IndexedCompSequenceIter<'_, EF> {
    fn next_geq(&mut self, lower_bound: u64) -> (u64, usize) {
        match &mut self.it {
            IterType::EliasFanoItT(it) => it.next_geq(lower_bound),
            IterType::RankedBvItT(it) => it.next_geq(lower_bound),
            IterType::CompEliasFanoItT(it) => it.next_geq(lower_bound),
            IterType::AllOnesItT(it) => it.next_geq(lower_bound),
        }
    }
}

impl<EF: EFVariant> Iterator for IndexedCompSequenceIter<'_, EF> {
    type Item = u64;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.it {
            IterType::EliasFanoItT(it) => it.next(),
            IterType::RankedBvItT(it) => it.next(),
            IterType::CompEliasFanoItT(it) => it.next(),
            IterType::AllOnesItT(it) => it.next(),
        }
    }
}

#[derive(Debug)]
pub struct IndexCompSeqCostWindow<'a, EF: EFVariant> {
    start_it: std::iter::Peekable<Iter<'a, u64>>,
    end_it: std::iter::Peekable<Iter<'a, u64>>,
    start: usize,
    end: usize,
    min_p: u64,
    max_p: u64,
    cost_upper_bound: usize,
    _phantom: PhantomData<EF>,
}

impl<'a, EF: EFVariant> IndexCompSeqCostWindow<'a, EF> {
    const FIX_COST: usize = 128;
}

impl<'a, EF: EFVariant> CostWindow<'a> for IndexCompSeqCostWindow<'a, EF> {
    fn new(sequence: &'a [u64], cost_upper_bound: usize) -> Self {
        let mut start_it = sequence.iter().peekable();
        let end_it = sequence.iter().peekable();
        let min_p = **start_it.peek().unwrap();
        let max_p = 0;

        IndexCompSeqCostWindow {
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
        IndexedCompSequence::<EF>::bitsize(self.universe(), self.size()) + Self::FIX_COST
    }

    #[inline(always)]
    fn single_block_cost(sequence: &[u64]) -> usize {
        IndexedCompSequence::<EF>::bitsize(*sequence.last().unwrap() + 1, sequence.len())
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
        IndexedCompSequence::<EF>::bitsize(1, 1) + Self::FIX_COST
    }
}

impl<'a, EF: EFVariant> PartitionableSequence<'a> for IndexedCompSequence<EF> {
    type CW = IndexCompSeqCostWindow<'a, EF>;
}

#![feature(unchecked_shifts)]
#![feature(array_chunks)]
#![feature(iter_array_chunks)]
#![feature(core_intrinsics)]

pub mod bitvector;

pub use bitvector::bitvector_collection::{BitBoxedCollection, BitVecCollection};
pub use bitvector::BitVector;
pub use bitvector::{BitBoxed, BitSlice, BitSliceWithOffset, BitVec};

pub mod darray;
use clap::ValueEnum;
pub use darray::DArray;

pub mod elias_fano;
pub use elias_fano::{EliasFano, EliasFanoIter};

// pub mod increasing_seq;
pub mod indexes;
// pub mod positive_sequences;

pub mod space_usage;

pub mod gen_sequences;
pub mod utils;

/// A trait for the support of `get` query over the binary alphabet.
pub trait AccessBin {
    /// Returns the bit at the given position `i`,
    /// or [`None`] if ```i``` is out of bounds.
    fn get(&self, i: usize) -> Option<bool>;

    /// Returns the bit at the given position `i`.
    ///
    /// # Safety
    /// Calling this method with an out-of-bounds index is undefined behavior.
    unsafe fn get_unchecked(&self, i: usize) -> bool;
}

/// A trait for the support of `select` query over the binary alphabet.
pub trait SelectBin {
    /// Returns the position of the `i+1`-th occurrence of a bit set to `1`.
    /// Returns `None` if there is no such position.
    fn select1(&self, i: usize) -> Option<usize>;

    /// Returns the position of the `i+1`-th occurrence of a bit set to `1`.
    ///
    /// # Safety
    /// This method doesn't check that such element exists
    /// Calling this method with an i >= maximum rank1 is undefined behaviour.
    unsafe fn select1_unchecked(&self, i: usize) -> usize;

    /// Returns the position of the `i+1`-th occurrence of a bit set to `0`.
    /// Returns `None` if there is no such position.
    fn select0(&self, i: usize) -> Option<usize>;

    /// Returns the position of the `i+1`-th occurrence of a bit set to  `0`.
    ///
    /// # Safety
    /// This method doesnt check that such element exists
    /// Calling this method with an `i >= maximum rank0` is undefined behaviour.
    unsafe fn select0_unchecked(&self, i: usize) -> usize;
}

#[derive(ValueEnum, Clone, Debug)]
pub enum IdxKind {
    EFSingle,
    UPEf,
    UPIs,
    Opt,
}

pub trait IncreasingSequenceEnumerator: Iterator<Item = u64> {
    fn next_val(&mut self) -> Option<(u64, usize)>;
    fn next_geq(&mut self, lower_bound: u64) -> Option<(u64, usize)>;
    fn move_to_position(&mut self, pos: usize) -> Option<(u64, usize)>;
    fn current_position(&self) -> usize;
    fn len(&self) -> usize;
    fn prev_value(&mut self) -> (usize, u64) {
        unimplemented!();
    }
}

/// Serializer class, this result can be appended to a bitvectorCollection
pub trait ToBitvector {
    fn to_bv(&self) -> BitVec;
}

pub trait EstimateSpace {
    fn bitsize(u: u64, n: usize) -> usize;
}

pub trait CostWindow<'a> {
    fn new(sequence: &'a [u64], cost_upper_bound: usize) -> Self;
    fn universe(&self) -> u64;
    fn size(&self) -> usize;

    fn window_cost(&self) -> usize;
    fn single_block_cost(sequence: &[u64]) -> usize;
    fn minimum_cost(sequence: &[u64]) -> usize;

    fn advance_start(&mut self);
    fn advance_end(&mut self);
    fn start(&self) -> usize;
    fn end(&self) -> usize;
    fn cost_upper_bound(&self) -> usize;
}

pub trait EnumeratorFromBitSlice<'a> {
    type IterType: IncreasingSequenceEnumerator;
    fn iter_from_slice(bv: BitSliceWithOffset<'a>) -> Self::IterType;
    fn iter_from_slice_with_data(bv: BitSliceWithOffset<'a>, n: usize, u: u64) -> Self::IterType;
}

pub trait WriteBitvector {
    fn write_bitvector(seq: &[u64], n: usize, u: u64) -> BitVec;
}

/// This trait contains the associated type for a cost window, if the given sequence has a partitioning method
pub trait PartitionableSequence<'a> {
    type CW: CostWindow<'a>;
}

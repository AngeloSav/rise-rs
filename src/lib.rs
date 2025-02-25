#![allow(internal_features)]
#![feature(array_chunks)]
#![feature(array_windows)]
#![feature(iter_array_chunks)]
#![feature(core_intrinsics)]

pub mod bitvector;

use std::fmt::Debug;

pub use bitvector::bitvector_collection::{
    BitBoxedCollection, BitVecCollection, BitVecCollectionBuilder,
};
pub use bitvector::BitVector;
pub use bitvector::{BitBoxed, BitSlice, BitSliceWithOffset, BitVec};

use clap::ValueEnum;

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

#[derive(ValueEnum, Clone, Debug)]
pub enum IdxKind {
    EFSingle,
    UPEf,
    UPIs,
    Opt,
}

// pub trait IncreasingSequenceEnumerator: Iterator<Item = u64> {
//     fn next_val(&mut self) -> Option<(u64, usize)>;
//     fn next_geq(&mut self, lower_bound: u64) -> Option<(u64, usize)>;
//     fn move_to_position(&mut self, pos: usize) -> Option<(u64, usize)>;
//     fn len(&self) -> usize;
// }

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
    type IterType: SequenceEnumerator + Debug;
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

// ---------------------------------------------

pub trait SequenceEnumerator: Iterator<Item = u64> {
    fn next_val(&mut self) -> Option<(u64, usize)>;
    fn move_to_position(&mut self, pos: usize) -> Option<(u64, usize)>;
    fn len(&self) -> usize;
}

pub trait NextGEQ: SequenceEnumerator {
    fn next_geq(&mut self, lower_bound: u64) -> Option<(u64, usize)>;
}

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
use epserde::traits::TypeHash;
pub use indexes::{EFIdx, OptEFIdx, UPEFIdx, UPISIdx};
pub mod positive_sequences;

pub mod queries;

pub mod space_usage;

pub mod gen_sequences;
pub mod utils;

const LENGTH_THRESHOLD: usize = 0;
const MDATA_LENGTH_THRESHOLD: usize = LENGTH_THRESHOLD;

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
    #[value(name = "ef")]
    EFSingle,
    #[value(name = "upef")]
    UPEf,
    #[value(name = "upis")]
    UPIs,
    #[value(name = "opt")]
    Opt,
}

#[derive(ValueEnum, Copy, Clone, Debug)]
pub enum QueryKind {
    BooleanAnd,
    BooleanOr,
    RankedAnd,
    RankedOr,
    Wand,
    Maxscore,
    BMWand,
    BMMaxscore,
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
    /// `u` is the universe size, which is used to determine the number of bits in the bitvector. It is strictly greater than the maximum value in the sequence.
    fn iter_from_slice(bv: BitSliceWithOffset<'a>, n: usize, u: u64) -> Self::IterType;
}

pub trait WriteBitvector {
    /// `n` is the number of elements in the sequence.
    /// `u` is the universe size, which is used to determine the number of bits in the bitvector. It is strictly greater than the maximum value in the sequence.
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

// ---------------------------------------------

pub trait DocScorer: TypeHash {
    fn doc_term_weight(freq: u64, norm_len: f32) -> f32;
    fn query_term_weight(freq: u64, df: u64, num_docs: u64) -> f32;
}

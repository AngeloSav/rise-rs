//! Sequence encodings based on Elias-Fano and its partitioned variants.
//!
//! The central encoding is [`EliasFano`], which stores a non-decreasing integer
//! sequence in near-optimal space while supporting fast iteration and
//! [`NextGEQ`] (next-greater-or-equal) queries.
//!
//! Partitioned variants ([`UniformPartitionedSequence`], [`OptPartitionedSequence`])
//! split a long sequence into independent blocks to improve locality and reduce
//! decoding overhead.
//!
//! ## Traits
//!
//! The module exposes a small set of traits that all sequence types implement:
//!
//! | Trait | Role |
//! |---|---|
//! | [`WriteBitvector`] | Encode a sequence into a [`BitVec`] |
//! | [`EnumeratorFromBitSlice`] | Create an iterator from a raw [`BitSliceWithOffset`] |
//! | [`SequenceEnumerator`] | Iterate in order, move to position |
//! | [`NextGEQ`] | Skip forward to the first value ≥ a lower bound |
//! | [`EstimateSpace`] | Estimate the bit-size before encoding |

use std::fmt::Debug;

use crate::{BitSliceWithOffset, BitVec};

pub mod all_ones_seq;
pub mod complement_ef;
pub mod elias_fano;
pub mod indexed_seq;
pub mod indexed_seq_complement;
pub mod opt_partition;
pub mod ranked_bv;
pub mod strict_ef;
pub mod uniform_partitioned_seq;

pub use elias_fano::{EliasFano, EliasFanoIter};
pub use opt_partition::OptPartitionedSequence;
pub use opt_partition::{CostWindow, PartitionableSequence};
pub use uniform_partitioned_seq::UniformPartitionedSequence;

// ── Sequence encoding traits ─────────────────────────────────────────────────

/// Serialize a non-decreasing integer sequence into a [`BitVec`].
///
/// Every sequence encoding (Elias-Fano, partitioned variants, …) implements
/// this trait to provide a uniform construction interface.
pub trait WriteBitvector {
    /// Encode `seq` into a new bit vector and return it.
    ///
    /// * `n` — exact number of elements in `seq`.
    /// * `u` — universe size; must be strictly greater than every value in
    ///   `seq`.  Used to determine the number of bits per element.
    fn write_bitvector(seq: impl IntoIterator<Item = u64>, n: usize, u: u64) -> BitVec;
}

/// Create an iterator that decodes a sequence stored inside a [`BitSliceWithOffset`].
///
/// The returned iterator type is bound to the lifetime of the underlying bit
/// slice, so no data is copied on construction.
pub trait EnumeratorFromBitSlice<'a> {
    type IterType: SequenceEnumerator + Debug;

    /// Construct an iterator over the encoded sequence in `bv`.
    ///
    /// * `n` — number of elements.
    /// * `u` — universe size (strictly greater than the maximum value).
    fn iter_from_slice(bv: BitSliceWithOffset<'a>, n: usize, u: u64) -> Self::IterType;
}

// ── Sequence iterator traits ──────────────────────────────────────────────────

/// An ordered iterator over an encoded integer sequence.
///
/// Extends [`Iterator<Item = u64>`] with positional access: callers can ask
/// for the next value together with its rank, or jump directly to a given
/// rank.  When the iterator is exhausted the returned value equals the
/// universe size `u` that was passed at construction time.
pub trait SequenceEnumerator: Iterator<Item = u64> {
    /// Advance to the next element and return `(value, rank)`.
    fn next_val(&mut self) -> (u64, usize);

    /// Move the iterator to rank `pos` and return `(value, pos)`.
    ///
    /// If `pos ≥ len` the iterator moves to the sentinel end position.
    fn move_to_position(&mut self, pos: usize) -> (u64, usize);

    /// Return the total number of elements in the sequence.
    fn len(&self) -> usize;
}

/// Extends [`SequenceEnumerator`] with a skip-forward operation.
pub trait NextGEQ: SequenceEnumerator {
    /// Advance the iterator to the first value `≥ lower_bound` and return
    /// `(value, rank)`.
    ///
    /// If no such value exists the iterator moves past the end and returns
    /// `(u, len)` where `u` is the universe size.
    fn next_geq(&mut self, lower_bound: u64) -> (u64, usize);
}

// ── Space estimation ──────────────────────────────────────────────────────────

/// Estimate the number of bits required to encode a sequence.
///
/// Used during optimal partitioning to pick the best partition boundaries
/// without actually encoding every candidate partition.
pub trait EstimateSpace {
    /// Return the number of bits needed to encode a sequence of `n` elements
    /// with universe `u` using this encoding scheme.
    fn bitsize(u: u64, n: usize) -> usize;
}

#[cfg(test)]
mod tests;

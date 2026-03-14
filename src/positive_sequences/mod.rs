//! Encoding for strictly positive integer sequences (term frequencies).
//!
//! Term frequencies are always ≥ 1.  [`PositiveSequence`] exploits this by
//! storing the prefix-sum (cumulative sum) of the frequency list rather than
//! the raw frequencies.  The prefix-sum sequence is non-decreasing and can
//! therefore be encoded with any [`WriteBitvector`] / [`EnumeratorFromBitSlice`]
//! type (typically a [`StrictEliasFano`] variant).
//!
//! Decoding transparently inverts the prefix-sum: callers see the original
//! frequency values via [`SequenceEnumerator`].
//!
//! [`WriteBitvector`]: crate::WriteBitvector
//! [`EnumeratorFromBitSlice`]: crate::EnumeratorFromBitSlice
//! [`SequenceEnumerator`]: crate::SequenceEnumerator
//! [`StrictEliasFano`]: crate::elias_fano::strict_ef::StrictEliasFano

pub use positive_sequence::PositiveSequence;
pub mod positive_sequence;

#[cfg(test)]
mod tests;

//! Inverted index types and the generic [`FreqIndex`] container.
//!
//! An inverted index maps each term to a *posting list*: a sorted sequence of
//! document IDs and their corresponding term frequencies.  This module defines
//! [`FreqIndex`], a generic struct parameterised over the document-list
//! encoding (`DocumentSequence`) and the frequency-list encoding
//! (`FreqSequence`), together with a set of concrete type aliases that plug in
//! different compression schemes:
//!
//! | Alias | Document encoding | Frequency encoding |
//! |---|---|---|
//! | [`EFIdx`] | [`EliasFano`] | `PositiveSequence<StrictEliasFano>` |
//! | [`UPEFIdx`] | `UniformPartitionedSequence<EliasFano>` | uniform partitioned |
//! | [`UPISIdx`] | `UniformPartitionedSequence<IndexSequence>` | uniform partitioned |
//! | [`OptEFIdx`] | `OptPartitionedSequence<IndexSequence>` | optimal partitioned |
//! | [`BlockVByteIdx`] | StreamVByte blocks | StreamVByte blocks |
//! | [`BlockInterpolativeIdx`] | Interpolative blocks | Interpolative blocks |
//!
//! [`EliasFano`]: crate::EliasFano

use block_freq_index::BlockFreqIndex;
use clap::ValueEnum;
use freq_index::FreqIndex;

mod block_freq_index;

/// Selects the document-list compression scheme when building or loading an index.
///
/// Passed on the command line via `--index-kind` and mapped to concrete index
/// type aliases ([`EFIdx`], [`UPEFIdx`], …) in the binary entry points.
#[derive(ValueEnum, Clone, Debug)]
pub enum IdxKind {
    /// Standard Elias-Fano (one list per term).
    #[value(name = "ef")]
    EFSingle,
    /// Uniformly partitioned Elias-Fano.
    #[value(name = "upef")]
    UPEf,
    /// Uniformly partitioned indexed sequence.
    #[value(name = "upis")]
    UPIs,
    /// Optimally partitioned indexed sequence.
    #[value(name = "opt")]
    Opt,
    /// Optimally partitioned complement-EF indexed sequence.
    #[value(name = "optcomp")]
    OptComp,
    /// Block-based StreamVByte codec.
    #[value(name = "block_vbyte")]
    BlockVByte,
    /// Block-based interpolative coding.
    #[value(name = "block_interpolative")]
    BlockInterpolative,
}

use crate::{
    EliasFano,
    elias_fano::{
        indexed_seq::{IndexSequence, StrictSequence},
        indexed_seq_complement::{IndexCompSequence, StrictCompSequence},
        opt_partition::OptPartitionedSequence,
        strict_ef::StrictEliasFano,
        uniform_partitioned_seq::UniformPartitionedSequence,
    },
    positive_sequences::positive_sequence::PositiveSequence,
};

pub mod freq_index;

// define index types
pub type EFIdx = FreqIndex<EliasFano, PositiveSequence<StrictEliasFano>>;

pub type UPEFIdx = FreqIndex<
    UniformPartitionedSequence<EliasFano>,
    PositiveSequence<UniformPartitionedSequence<StrictEliasFano>>,
>;
pub type UPISIdx = FreqIndex<
    UniformPartitionedSequence<IndexSequence>,
    PositiveSequence<UniformPartitionedSequence<StrictSequence>>,
>;
pub type OptEFIdx = FreqIndex<
    OptPartitionedSequence<IndexSequence>,
    PositiveSequence<OptPartitionedSequence<StrictSequence>>,
>;
pub type OptCompIdx = FreqIndex<
    OptPartitionedSequence<IndexCompSequence>,
    PositiveSequence<OptPartitionedSequence<StrictCompSequence>>,
>;

pub type BlockVByteIdx =
    BlockFreqIndex<block_freq_index::block_codices::streamvbyte_codec::StreamVByteCodec>;
pub type BlockInterpolativeIdx =
    BlockFreqIndex<block_freq_index::block_codices::interpolative_coding::InterpolativeCodec>;

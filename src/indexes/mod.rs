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
use epserde::traits::{AlignHash, TypeHash};
use freq_index::FreqIndex;

mod freq_index_builder;
pub use freq_index_builder::FreqIndexBuilder;
use mem_dbg::{MemDbg, MemSize};

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

pub trait InvertedIndexBuilder {
    type IndexType: InvertedIndex;

    fn new(n_docs: usize) -> Self;
    fn push_plist_freqs(&mut self, docs: &[u64], freqs: &[u64]);
    fn build(self) -> Self::IndexType;
}

pub trait PostingListIter {
    fn current_doc(&self) -> u64;
    fn current_pos(&self) -> usize;
    fn next_geq(&mut self, lower_bound: u64);
    fn next_doc(&mut self);
    fn freq(&mut self) -> u64;
    fn len(&self) -> usize;
}

pub trait InvertedIndex: MemSize + MemDbg {
    type IterType<'a>: PostingListIter
    where
        Self: 'a;

    fn n_docs(&self) -> usize;
    fn n_terms(&self) -> usize;
    fn get_plist_iter(&self, i: usize) -> Self::IterType<'_>;
}

// define index types
pub type EFIdx = FreqIndex<EliasFano, PositiveSequence<StrictEliasFano>>;

pub type UPEFIdx = FreqIndex<
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

fn type_hash_of<T: TypeHash + AlignHash>() -> u64 {
    use std::hash::Hasher;
    use xxhash_rust::xxh3::Xxh3;
    let mut h = Xxh3::new();
    T::type_hash(&mut h);
    h.finish()
}

/// Reads the epserde type hash from the first 21 bytes of `path` and maps it
/// to the corresponding [`IdxKind`].
///
/// The epserde header layout is:
///   MAGIC (8B) | VERSION_MAJOR (2B) | VERSION_MINOR (2B) | USIZE_SIZE (1B) | TYPE_HASH (8B)
/// so the type hash lives at byte offset 13.
pub fn peek_idx_kind(path: &str) -> IdxKind {
    let mut file = std::fs::File::open(path).expect("cannot open index file");
    let mut header = [0u8; 21];
    use std::io::Read;
    file.read_exact(&mut header).expect("cannot read index header");

    let type_hash = u64::from_ne_bytes(header[13..21].try_into().unwrap());

    let kinds: &[(u64, IdxKind)] = &[
        (type_hash_of::<EFIdx>(),                  IdxKind::EFSingle),
        (type_hash_of::<UPEFIdx>(),                IdxKind::UPEf),
        (type_hash_of::<OptEFIdx>(),               IdxKind::Opt),
        (type_hash_of::<OptCompIdx>(),             IdxKind::OptComp),
        (type_hash_of::<BlockVByteIdx>(),          IdxKind::BlockVByte),
        (type_hash_of::<BlockInterpolativeIdx>(),  IdxKind::BlockInterpolative),
    ];

    kinds
        .iter()
        .find(|(h, _)| *h == type_hash)
        .map(|(_, k)| k.clone())
        .unwrap_or_else(|| panic!("unrecognised index type hash {type_hash:#x} in {path}"))
}

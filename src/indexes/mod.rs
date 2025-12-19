use block_freq_index::BlockFreqIndex;
use freq_index::FreqIndex;

mod block_freq_index;

use crate::{
    elias_fano::{
        indexed_seq::{IndexSequence, StrictSequence},
        opt_partition::OptPartitionedSequence,
        strict_ef::StrictEliasFano,
        uniform_partitioned_seq::UniformPartitionedSequence,
    },
    positive_sequences::positive_sequence::PositiveSequence,
    EliasFano,
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

pub type BlockVByteIdx = BlockFreqIndex<block_freq_index::block_codices::vbyte_codec::VbyteCodec>;

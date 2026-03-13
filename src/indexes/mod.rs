use block_freq_index::BlockFreqIndex;
use freq_index::FreqIndex;

mod block_freq_index;

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

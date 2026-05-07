#![allow(internal_features)]
#![feature(impl_trait_in_assoc_type)]
#![feature(array_windows)]
#![feature(iter_array_chunks)]
#![feature(core_intrinsics)]
#![feature(float_algebraic)]
#![feature(binary_heap_into_iter_sorted)]

pub mod bitvector;

pub use bitvector::AccessBin;
pub use bitvector::BitVector;
pub use bitvector::bitvector_collection::{
    BitBoxedCollection, BitVecCollection, BitVecCollectionBuilder,
};
pub use bitvector::{BitBoxed, BitSlice, BitSliceWithOffset, BitVec};

pub mod elias_fano;
pub use elias_fano::{
    CostWindow, EliasFano, EliasFanoIter, EnumeratorFromBitSlice, EstimateSpace, NextGEQ,
    PartitionableSequence, SequenceEnumerator, WriteBitvector,
};

// pub mod increasing_seq;
pub mod indexes;
pub use indexes::{EFIdx, IdxKind, OptEFIdx, UPEFIdx, peek_idx_kind};
pub mod positive_sequences;

pub mod queries;
pub use queries::{DocScorer, QueryKind, ScorerKind};

pub mod gen_sequences;
pub mod utils;

pub mod readers;

pub mod config;

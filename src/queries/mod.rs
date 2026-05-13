#![allow(unused_variables)]
use crate::indexes::InvertedIndex;
use clap::ValueEnum;

mod block_partitioning;
pub mod block_posting_metadata;

pub mod scorers;
pub use scorers::DocScorer;

pub use scorers::BM25;
pub use scorers::DotScorer;

/// Selects the scoring model when building metadata or running ranked queries.
///
/// Passed on the command line via `--scorer`. Defaults to `bm25`.
#[derive(clap::ValueEnum, Copy, Clone, Debug)]
pub enum ScorerKind {
    Bm25,
    Dot,
}

pub mod topk_heap;

/// Selects the query algorithm when running experiments.
///
/// Passed on the command line via `--query-kind`.
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

pub mod score_part;

pub use block_posting_metadata::BlockPostingMetadata;

pub mod query_algorithms;
pub use query_algorithms::*;

/// Reads the epserde type hash from the first 21 bytes of `path` and maps it
/// to the corresponding [`ScorerKind`].
pub fn peek_scorer_kind(path: &str) -> ScorerKind {
    use epserde::traits::TypeHash;
    use std::hash::Hasher;
    use std::io::Read;
    use xxhash_rust::xxh3::Xxh3;

    fn scorer_type_hash<S: DocScorer + TypeHash>() -> u64 {
        let mut h = Xxh3::new();
        BlockPostingMetadata::<S>::type_hash(&mut h);
        h.finish()
    }

    let mut file = std::fs::File::open(path).expect("cannot open metadata file");
    let mut header = [0u8; 21];
    file.read_exact(&mut header)
        .expect("cannot read metadata header");
    let type_hash = u64::from_ne_bytes(header[13..21].try_into().unwrap());

    if type_hash == scorer_type_hash::<BM25>() {
        ScorerKind::Bm25
    } else if type_hash == scorer_type_hash::<DotScorer>() {
        ScorerKind::Dot
    } else {
        panic!("unrecognised scorer type hash {type_hash:#x} in {path}")
    }
}

pub trait QueryOperator {
    fn query_name() -> &'static str;

    // this function takes an index `idx`, a number of terms `terms`,
    fn query<I>(&mut self, idx: &I, terms: &[usize]) -> usize
    where
        I: InvertedIndex;

    fn retrieved_docs(&self) -> Vec<usize> {
        todo!()
    }
}

pub trait RankedQueryOperator: QueryOperator {
    fn topk(&self) -> &topk_heap::TopKHeap;
}

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

pub trait RankedQueryOperator {
    fn topk(&self) -> &topk_heap::TopKHeap;
}

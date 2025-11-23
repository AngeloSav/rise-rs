#![allow(unused_variables)]
use crate::indexes::freq_index::{DocList, FreqIndex, FreqList};

mod block_partitioning;
pub mod block_posting_metadata;
pub mod bm25;
pub mod topk_heap;

pub mod score_part;

pub use block_posting_metadata::BlockPostingMetadata;

pub mod query_algorithms;
pub use query_algorithms::*;

pub trait QueryOperator {
    fn query_name() -> &'static str;

    // this function takes an index `idx`, a number of terms `terms`,
    fn query<'a, T, S>(&mut self, idx: &'a FreqIndex<T, S>, terms: &[usize]) -> usize
    where
        T: DocList<'a>,
        S: FreqList<'a>;
}

pub trait RankedQueryOperator {
    fn topk(&self) -> &topk_heap::TopKHeap;
}

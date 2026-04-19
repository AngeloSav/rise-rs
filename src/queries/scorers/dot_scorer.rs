use epserde::Epserde;

use crate::DocScorer;

#[derive(Epserde)]
pub struct DotScorer;

impl DocScorer for DotScorer {
    // 1. Just return the raw value from the index.
    // No logs, no IDF, no normalization.
    fn doc_term_weight(freq: u64, _norm_len: f32) -> f32 {
        freq as f32
    }

    // 2. Just return the number of times the term appeared in the query.
    fn query_term_weight(q_freq: u64, _df: u64, _n_docs: u64) -> f32 {
        q_freq as f32
    }
}

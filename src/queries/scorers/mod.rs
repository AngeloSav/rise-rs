use epserde::prelude::*;

mod bm25;
mod dot_scorer;

pub use bm25::BM25;
pub use dot_scorer::DotScorer;

/// Scoring contract for a term–document weighting model.
///
/// Implementations must also implement [`TypeHash`] (via `epserde`) so that
/// serialised indexes carry a type fingerprint of the scorer that was used to
/// build them.
pub trait DocScorer: TypeHash {
    /// Term-frequency component of the document-side score.
    ///
    /// * `freq` — in-document term frequency.
    /// * `norm_len` — document length normalised by the average document length.
    fn doc_term_weight(freq: u64, norm_len: f32) -> f32;

    /// IDF-like query-side weight for a term.
    ///
    /// * `freq` — query-term frequency.
    /// * `df` — number of documents containing the term.
    /// * `num_docs` — total number of documents in the collection.
    fn query_term_weight(freq: u64, df: u64, num_docs: u64) -> f32;
}

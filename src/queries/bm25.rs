use epserde::prelude::*;
use epserde::traits::TypeHash;

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

#[derive(Epserde)]
pub struct BM25;

impl BM25 {
    const B: f32 = 0.5; // 0.75?
    const K: f32 = 1.2;
}

impl DocScorer for BM25 {
    fn doc_term_weight(freq: u64, norm_len: f32) -> f32 {
        let freq = freq as f32;
        freq / (freq + Self::K * (1.0 - Self::B + Self::B * norm_len))
        // freq.algebraic_div(
        //     freq.algebraic_add(
        //         Self::K.algebraic_mul(
        //             (1.0 as f32)
        //                 .algebraic_sub(Self::B)
        //                 .algebraic_add(Self::B.algebraic_mul(norm_len)),
        //         ),
        //     ),
        // )
    }

    fn query_term_weight(freq: u64, df: u64, num_docs: u64) -> f32 {
        let freq = freq as f32;
        let df = df as f32;
        let idf = f32::ln((num_docs as f32 - df + 0.5) / (df + 0.5));

        let epsilon_score: f32 = 1.0e-6;
        freq * epsilon_score.max(idf) * (1.0 + Self::K)
    }
}

use std::hash::Hash;

use epserde::traits::TypeHash;

use crate::DocScorer;

pub struct BM25;

impl BM25 {
    const B: f32 = 0.5;
    const K: f32 = 1.2;
}

impl TypeHash for BM25 {
    fn type_hash(hasher: &mut impl core::hash::Hasher) {
        "BM25".hash(hasher);
    }
}

impl DocScorer for BM25 {
    fn doc_term_weight(freq: u64, norm_len: f32) -> f32 {
        let freq = freq as f32;
        freq / (freq + Self::K * (1.0 - Self::B + Self::B * norm_len))
    }

    fn query_term_weight(freq: u64, df: u64, num_docs: u64) -> f32 {
        let freq = freq as f32;
        let df = df as f32;
        let idf = f32::ln((num_docs as f32 - df + 0.5) / (df + 0.5));

        let epsilon_score: f32 = 1.0e-6;
        freq * epsilon_score.max(idf) * (1.0 + Self::K)
    }
}

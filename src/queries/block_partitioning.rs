use num::Float;

use crate::{queries::score_part, DocScorer};

pub fn partition_static<Scorer: DocScorer>(
    seq: impl Iterator<Item = (u64, u64)>, // pairs of (docid, freq)
    norm_lens: &Vec<f32>,
) -> (Vec<u32>, Vec<u32>, Vec<f32>) {
    const BLOCK_SIZE: usize = 128;

    let mut sizes = Vec::new();
    let mut block_docid = Vec::new();
    let mut block_max_term_weights = Vec::new();

    let mut current_block = 0;

    let mut max_score = 0.0;
    let mut block_max_score = 0.0;

    let mut i = 0;

    let mut last = 0;

    for (docid, freq) in seq {
        let norm_len = norm_lens[docid as usize];

        let score = Scorer::doc_term_weight(freq, norm_len);
        max_score = max_score.max(score);

        if i == 0 || (i / BLOCK_SIZE) == current_block {
            block_max_score = block_max_score.max(score);
        } else {
            block_docid.push(docid as u32 - 1);
            block_max_term_weights.push(block_max_score);
            current_block += 1;
            block_max_score = score.max(0.0);
            sizes.push(BLOCK_SIZE as u32);
        }
        i += 1;
        last = docid as u32;
    }

    block_docid.push(last);
    block_max_term_weights.push(block_max_score);
    sizes.push(if i % BLOCK_SIZE == 0 {
        BLOCK_SIZE
    } else {
        i % BLOCK_SIZE
    } as u32);

    (sizes, block_docid, block_max_term_weights)
}

#[allow(dead_code)]
pub fn partition_variable<Scorer: DocScorer>(
    seq: impl Iterator<Item = (u64, u64)>, // pairs of (docid, freq)
    norm_lens: &Vec<f32>,
) -> (Vec<u32>, Vec<u32>, Vec<f32>) {
    let mut doc_score_top = Vec::new();
    let mut max_score = 0.0;

    let mut seq_len = 0;
    for (docid, freq) in seq {
        let score = Scorer::doc_term_weight(freq, norm_lens[docid as usize]);
        doc_score_top.push((docid, score));
        max_score = max_score.max(score);
        seq_len += 1;
    }

    let estimated_idf = Scorer::query_term_weight(1, seq_len, norm_lens.len() as u64);

    const EPS1: f32 = 0.01;
    const EPS2: f32 = 0.4;
    const FIXED_COST_WAND: f32 = 12.0;

    score_part::score_opt_partition(&doc_score_top, estimated_idf, FIXED_COST_WAND, EPS1, EPS2)
}

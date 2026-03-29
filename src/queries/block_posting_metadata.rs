use std::{
    fs::{self, File},
    marker::PhantomData,
};

use crate::{DocScorer, config, readers::BinaryCollectionIterator, utils::pb_with_message};

use epserde::prelude::*;

use super::block_partitioning::{partition_static, partition_variable};

// This version uses Rust vectors to store the block metadata

#[allow(dead_code)]
#[derive(Epserde, Debug)]
pub struct BlockPostingMetadata<Scorer: DocScorer> {
    processed_postings: usize,
    norms_len: Box<[f32]>,
    max_term_weight: Box<[f32]>,
    blocks_start: Box<[usize]>,
    blocks_docid: Box<[u32]>,
    blocks_max_term_weight: Box<[f32]>,
    _phantom: PhantomData<Scorer>,
}

impl<Scorer: DocScorer> BlockPostingMetadata<Scorer> {
    pub fn load_file(path: &str) -> Self {
        // load the .mdata file
        log::info!("loading metadata from {}", path);
        let reader = std::fs::read(path).expect("could not read p_data file");

        unsafe { Self::deserialize_eps(&reader).expect("could not deserialize p_data") }
    }

    pub fn create_file(
        path: &str,
        variable_block: bool,
        block_size: Option<usize>,
        lambda: Option<f32>,
        out_file: &str,
    ) {
        let mut blocks_start: Vec<usize> = Vec::new();
        blocks_start.push(0);
        let mut blocks_docid = Vec::new();
        let mut blocks_max_term_weight: Vec<f32> = Vec::new();

        if variable_block {
            log::info!("using variable-size blocks | lambda: {:?}", lambda);
        } else {
            log::info!("using fixed-size blocks | block size: {:?}", block_size);
        }

        let sizes_iter = BinaryCollectionIterator::new(&format!("{}.sizes", path))
            .next()
            .unwrap();

        let n_docs = sizes_iter.len() as u64;

        let mut norms_len: Vec<f32> = Vec::with_capacity(n_docs as usize);
        let mut max_term_weight: Vec<f32> = Vec::with_capacity(n_docs as usize);
        let mut avg_len = 0.0f64;

        for doc_len in sizes_iter {
            norms_len.push(doc_len as f32);
            avg_len += doc_len as f64;
        }

        avg_len /= n_docs as f64;

        norms_len
            .iter_mut()
            .for_each(|x| *x = ((*x as f64) / avg_len as f64) as f32);

        let mmap_len = fs::metadata(&format!("{}.docs", path)).unwrap().len() / 4;

        let pb = pb_with_message(mmap_len, "Building Metadata".to_string());

        let mut docs_iter = BinaryCollectionIterator::new(&format!("{}.docs", path));
        let freqs_iter = BinaryCollectionIterator::new(&format!("{}.freqs", path));

        let mut singleton = docs_iter.next().unwrap();
        let n_docs = singleton.next().unwrap();

        let mut it = docs_iter.zip(freqs_iter);

        let mut processed_postings = 0;

        while let Some((doc_list, freq_list)) = it.next() {
            assert!(doc_list.len() == freq_list.len());
            assert!(doc_list.len() > 0);

            let sz = doc_list.len() as u64;
            if sz > config::MDATA_LENGTH_THRESHOLD as u64 {
                let v = doc_list.zip(freq_list);

                // add sequence ---------------
                let (v_sizes, v_block_docid, v_block_max_term_weights) = if !variable_block {
                    partition_static::<Scorer>(
                        v,
                        &norms_len,
                        block_size.expect("unspecified block size"),
                    )
                } else {
                    partition_variable::<Scorer>(v, &norms_len, lambda.expect("unspecified lambda"))
                };

                blocks_start.push(blocks_start.last().unwrap() + v_block_docid.len());
                blocks_docid.extend(v_block_docid);
                max_term_weight.push(
                    v_block_max_term_weights
                        .iter()
                        .cloned()
                        .reduce(f32::max)
                        .unwrap(),
                );
                blocks_max_term_weight.extend(v_block_max_term_weights);
            }

            pb.inc(sz);
            // if processed % 10_000_000 == 0 {
            //     println!("processed {} lists", processed);
            // }
            processed_postings += sz as usize;
        }

        log::info!("norms_len len: {}", norms_len.len());
        log::info!("max_term_weight len: {}", max_term_weight.len());

        log::info!("blocks_start len: {}", blocks_start.len());
        log::info!("blocks_docid len: {}", blocks_docid.len());
        log::info!(
            "blocks_max_term_weight len: {}",
            blocks_max_term_weight.len()
        );

        let avg_block_size = processed_postings as f64 / blocks_docid.len() as f64;

        log::info!("average block size: {}", avg_block_size);

        let p_data = Self {
            processed_postings,
            norms_len: norms_len.into_boxed_slice(),
            max_term_weight: max_term_weight.into_boxed_slice(),
            blocks_start: blocks_start.into_boxed_slice(),
            blocks_docid: blocks_docid.into_boxed_slice(),
            blocks_max_term_weight: blocks_max_term_weight.into_boxed_slice(),
            _phantom: PhantomData,
        };

        //save to .mdata file
        let mut mdata_file = File::create(out_file).expect("could not create mdata file");

        unsafe {
            p_data
                .serialize(&mut mdata_file)
                .expect("could not serialize p_data")
        };
    }

    pub fn get_norm_len(&self, i: usize) -> f32 {
        unsafe { *self.norms_len.get_unchecked(i) }
    }

    pub fn get_max_term_weight(&self, i: usize) -> f32 {
        unsafe { *self.max_term_weight.get_unchecked(i) }
    }

    pub fn get_block_posting_metadata_iterator(
        &'_ self,
        i: usize,
    ) -> BlockPostingMDataEnumerator<'_, Scorer> {
        let block_start = self.blocks_start[i];
        let block_number = self.blocks_start[i + 1] - self.blocks_start[i];

        BlockPostingMDataEnumerator {
            current_pos: 0,
            block_number,
            block_max_term_weight: &self.blocks_max_term_weight
                [self.blocks_start[i]..self.blocks_start[i + 1]],
            block_docid: &self.blocks_docid[self.blocks_start[i]..self.blocks_start[i + 1]],
            phantom: PhantomData,
        }
    }
}

pub struct BlockPostingMDataEnumerator<'a, Scorer: DocScorer> {
    current_pos: usize,
    block_number: usize,
    block_max_term_weight: &'a [f32],
    block_docid: &'a [u32],
    phantom: PhantomData<Scorer>,
}

impl<'a, Scorer: DocScorer> BlockPostingMDataEnumerator<'a, Scorer> {
    pub fn block_next_geq(&mut self, lower_bound: u64) {
        while self.current_pos + 1 < self.block_number
            && (self.block_docid[self.current_pos] as usize) < lower_bound as usize
        {
            self.current_pos += 1;
        }
    }

    pub fn block_max_score(&self) -> f32 {
        self.block_max_term_weight[self.current_pos]
    }

    pub fn block_docid(&self) -> u64 {
        self.block_docid[self.current_pos] as u64
    }
}

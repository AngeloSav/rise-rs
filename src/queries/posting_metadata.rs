use std::{fs::File, marker::PhantomData};

use epserde::prelude::*;
use memmap2::MmapOptions;

use crate::{
    indexes::freq_index::{DocList, FreqIndex, FreqList},
    DocScorer, LENGTH_THRESHOLD,
};

// example of cloning and taking iterators in rust (useful for parser)
// https://play.rust-lang.org/?version=stable&mode=debug&edition=2024&gist=9f4c8e1e9f57623cc5bfb243f5cacf48

#[allow(dead_code)]
#[derive(Epserde, Debug)]
pub struct PostingMetadata<Scorer: DocScorer> {
    norms_len: Vec<f32>,
    max_term_weight: Vec<f32>,
    _phantom: PhantomData<Scorer>,
}

impl<Scorer: DocScorer> PostingMetadata<Scorer> {
    pub fn load_file<'a, T, S>(idx: &'a FreqIndex<T, S>, path: &str) -> Self
    where
        T: DocList<'a>,
        S: FreqList<'a>,
    {
        // check if the file .mdata file exists
        if std::path::Path::new(&format!("{}.mdata", path)).exists() {
            // load the .mdata file
            println!("loading metadata from {}.mdata", path);
            let reader =
                std::fs::read(format!("{}.mdata", path)).expect("could not open .mdata file");

            return unsafe {
                Self::deserialize_eps(&reader).expect("could not deserialize p_data")
            };
        }

        let sizes_file = File::open(format!("{}.sizes", path)).expect("could not open docs file");
        println!("creating metadata from .sizes file");

        let mmap_sizes = unsafe {
            MmapOptions::new()
                .map(&sizes_file)
                .expect("could not memory map docs file")
        };

        let mut sizes_iter = mmap_sizes
            .array_chunks::<4>()
            .map(|chunk| u32::from_le_bytes(*chunk) as u64);

        let n_docs = sizes_iter.next().expect("malformed .sizes file");

        assert!(n_docs as usize == idx.n_docs);

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

        let docs_file = File::open(format!("{}.docs", path)).expect("could not open docs file");
        let freq_file = File::open(format!("{}.freqs", path)).expect("could not open freqs file");

        let mmap_docs = unsafe {
            MmapOptions::new()
                .map(&docs_file)
                .expect("could not memory map docs file")
        };

        let mmap_freqs = unsafe {
            MmapOptions::new()
                .map(&freq_file)
                .expect("could not memory map freqs file")
        };

        let mut docs_iter = mmap_docs
            .array_chunks::<4>()
            .map(|chunk| u32::from_le_bytes(*chunk) as u64);

        let freqs_iter = mmap_freqs
            .array_chunks::<4>()
            .map(|chunk| u32::from_le_bytes(*chunk) as u64);

        docs_iter.next();
        let n_docs = docs_iter.next().unwrap();

        let mut it = docs_iter.zip(freqs_iter);

        while let Some((sz, sz_freq)) = it.next() {
            assert!(sz == sz_freq);

            let v = (&mut it).take(sz as usize).collect::<Vec<_>>();
            assert!(v.len() == sz as usize);
            assert!(sz > 0);

            if sz <= LENGTH_THRESHOLD as u64 {
                // ignore small lists
                continue;
            }

            let mut max_score = 0.0f32;
            for (docid, freq) in v {
                assert!(docid < n_docs);
                let score = Scorer::doc_term_weight(freq, norms_len[docid as usize]);
                max_score = max_score.max(score);
            }
            max_term_weight.push(max_score);
        }

        let p_data = Self {
            norms_len,
            max_term_weight,
            _phantom: PhantomData,
        };

        //save to .mdata file
        let mut mdata_file =
            File::create(format!("{}.mdata", path)).expect("could not create mdata file");

        unsafe {
            p_data
                .serialize(&mut mdata_file)
                .expect("could not serialize p_data")
        };

        p_data
    }

    pub fn get_norm_len(&self, i: usize) -> f32 {
        self.norms_len[i]
    }

    pub fn get_max_term_weight(&self, i: usize) -> f32 {
        self.max_term_weight[i]
    }
}

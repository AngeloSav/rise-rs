use std::{
    fs::{self, File},
    path::Path,
};

use epserde::prelude::*;
use mem_dbg::{MemDbg, MemSize};

use crate::{
    BitVec, EliasFano, EnumeratorFromBitSlice, SequenceEnumerator, WriteBitvector, config,
    indexes::{
        InvertedIndex, InvertedIndexBuilder, PostingListIter,
        block_freq_index::{
            block_codices::BlockCodec,
            block_posting_list::{BlockPostingList, BlockPostingListIter},
        },
    },
    readers::BinaryCollectionIterator,
    utils::{TimingQueries, pb_with_message},
};

pub mod block_codices;
pub mod block_posting_list;

#[derive(Clone, Debug, Epserde, MemSize, MemDbg)]
pub struct BlockFreqIndex<BC> {
    pub n_docs: usize,
    pub n_terms: usize,
    data: Vec<u8>,
    endpoints: BitVec,
    _phantom: std::marker::PhantomData<BC>,
}

pub struct BlockFreqIndexBuilder<BC> {
    n_docs: usize,
    lists: Vec<u8>,
    endpoints: Vec<u64>,
    _phantom: std::marker::PhantomData<BC>,
}

impl<BC> InvertedIndexBuilder for BlockFreqIndexBuilder<BC>
where
    BC: BlockCodec,
{
    type IndexType = BlockFreqIndex<BC>;

    fn new(n_docs: usize) -> Self {
        BlockFreqIndexBuilder {
            n_docs,
            lists: Vec::new(),
            endpoints: vec![0],
            _phantom: std::marker::PhantomData,
        }
    }

    fn push_plist_freqs(&mut self, docs: &[u64], freqs: &[u64]) {
        BlockPostingList::<BC>::write(docs, freqs, &mut self.lists);
        self.endpoints.push(self.lists.len() as u64);
    }

    fn build(self) -> BlockFreqIndex<BC> {
        let list_len = self.lists.len();
        let n_terms = self.endpoints.len() - 1;

        BlockFreqIndex {
            data: self.lists,
            _phantom: std::marker::PhantomData,
            n_docs: self.n_docs,
            n_terms,
            endpoints: EliasFano::write_bitvector(
                self.endpoints[..self.endpoints.len() - 1].iter().copied(),
                n_terms,
                list_len as u64 + 1,
            ),
        }
    }
}

impl<BC> BlockFreqIndex<BC>
where
    BC: BlockCodec,
{
    pub fn from_files(input_path: &str) -> Self {
        let mmap_len = fs::metadata(&format!("{}.docs", input_path)).unwrap().len() / 4;

        let mut docs_iter = BinaryCollectionIterator::new(&format!("{}.docs", input_path));
        let freqs_iter = BinaryCollectionIterator::new(&format!("{}.freqs", input_path));

        let mut singleton = docs_iter.next().unwrap();
        let n_docs = singleton.next().unwrap();

        log::info!("number of documents: {}", n_docs);
        let pb = pb_with_message(mmap_len, "creating index".to_string());

        let mut it = docs_iter.zip(freqs_iter);

        let mut index_builder = BlockFreqIndexBuilder::<BC>::new(n_docs as usize);

        let mut n_postings = 0;

        while let Some((doc_list, freq_list)) = it.next() {
            assert_eq!(doc_list.len(), freq_list.len());
            // println!("------------- list n {} -------------", processed);
            // println!("list n {}, size is {}", idx.n_terms, sz);
            let sz = doc_list.len() as u64;

            if sz > config::LENGTH_THRESHOLD as u64 {
                let v_docs: Vec<u64> = doc_list.collect();
                let v_freqs: Vec<u64> = freq_list.collect();

                assert!(v_docs.len() == sz as usize);
                // assert!(sz > 0);

                index_builder.push_plist_freqs(&v_docs, &v_freqs);

                n_postings += sz;
                pb.inc(sz);
            }
        }

        pb.finish();
        log::info!("processed {} postings", n_postings);

        index_builder.build()
    }

    pub fn load_index(path: &str) -> Self {
        let reader = std::fs::read(path).expect("could not read index file");
        log::info!("Serialized size: {:?} bytes", reader.len());

        unsafe { Self::deserialize_eps(&reader).expect("could not deserialize index") }
    }

    pub fn load_or_build_and_save(
        input_filename: &str,
        output_filename: &str,
        force_rebuild: bool,
    ) -> Self {
        let ds: Self;
        let path = Path::new(output_filename);
        if path.exists() && !force_rebuild {
            log::info!(
                "The data structure already exists. Filename: {}. I'm going to load it ...",
                output_filename
            );

            ds = Self::load_index(output_filename);
        } else {
            let mut t = TimingQueries::new(1, 1); // measure building time
            t.start();
            ds = Self::from_files(input_filename);
            t.stop();
            let (t_min, _, _) = t.get();
            log::info!("Construction time {:?} millisecs", t_min / 1000000);

            // let serialized = bincode::serialize(&ds).unwrap();
            // println!("Serialized size: {:?} bytes", serialized.len());
            // fs::write(path, serialized).unwrap();

            //save to .mdata file
            let mut output_file = File::create(path).expect("could not create index file");

            unsafe {
                ds.serialize(&mut output_file)
                    .expect("could not serialize index")
            };
        }
        ds
    }

    pub fn check_correctness(&self, input_path: &str) {
        let mmap_len = fs::metadata(&format!("{}.docs", input_path)).unwrap().len() / 4;

        let mut docs_iter = BinaryCollectionIterator::new(&format!("{}.docs", input_path));
        let freqs_iter = BinaryCollectionIterator::new(&format!("{}.freqs", input_path));

        let mut singleton = docs_iter.next().unwrap();
        let n_docs = singleton.next().unwrap();

        log::info!("number of documents: {}", n_docs);
        let pb = pb_with_message(mmap_len, "Checking correctness".to_string());

        let mut it = docs_iter.zip(freqs_iter);

        let mut processed = 0;
        while let Some((doc_list, freq_list)) = it.next() {
            assert!(doc_list.len() == freq_list.len());
            // if sz != sz_freq {
            //     panic!("size mismatch in .docs and .freqs files");
            // }

            let sz = doc_list.len() as u64;
            if sz > config::LENGTH_THRESHOLD as u64 {
                let v_docs: Vec<u64> = doc_list.collect();
                let v_freqs: Vec<u64> = freq_list.collect();

                // println!("Checking list {} with size {}", processed, sz);
                let mut it_plist = self.get_plist_iter(processed);
                let itv = v_docs.iter().zip(v_freqs.iter());
                for (_i, s) in itv.clone().enumerate() {
                    // println!("check n {}", i);
                    // assert!(dbg!(s) == dbg!(it.next().unwrap()));
                    let docid = it_plist.current_doc();
                    let freq = it_plist.freq();
                    assert_eq!(
                        s,
                        (&docid, &freq),
                        "PLIST idx {} | Mismatch, current position is {:?}",
                        processed,
                        it_plist.current_pos(),
                    );
                    it_plist.next_doc();
                }

                pb.inc(sz);
                processed += 1;
            }
        }

        pb.finish();
    }

    pub fn get_plist_iter(&self, i: usize) -> BlockPostingListIter<'_, BC> {
        assert!(i < self.n_terms);

        let mut ef = EliasFano::iter_from_slice(
            self.endpoints.as_bitslice(),
            self.n_terms,
            self.data.len() as u64,
        );

        let start = ef.move_to_position(i).0 as usize;
        let end = ef.next_val().0 as usize;
        BlockPostingList::<BC>::iter_from_slice(&self.data[start..end], self.n_docs as u64)
    }
}

impl<BC> InvertedIndex for BlockFreqIndex<BC>
where
    BC: BlockCodec,
{
    type IterType<'a>
        = BlockPostingListIter<'a, BC>
    where
        Self: 'a;

    fn get_plist_iter(&self, i: usize) -> Self::IterType<'_> {
        self.get_plist_iter(i)
    }

    fn n_docs(&self) -> usize {
        self.n_docs
    }

    fn n_terms(&self) -> usize {
        self.n_terms
    }
}

#[cfg(test)]
mod tests;

use mem_dbg::{MemDbg, MemSize};

use std::fs::{self, File};

use crate::{
    config,
    indexes::*,
    readers::ds2i_reader::BinaryCollectionIterator,
    utils::{pb_with_message, prefetch_bitslice_word},
};
use epserde::prelude::*;
use std::{fmt::Debug, marker::PhantomData, path::Path};

use crate::{
    BitSliceWithOffset, BitVecCollection, EliasFano, EnumeratorFromBitSlice, NextGEQ,
    PartitionableSequence, SequenceEnumerator, WriteBitvector,
    elias_fano::{
        indexed_seq::{IndexSequence, StrictSequence},
        opt_partition::OptPartitionedSeqIter,
        strict_ef::StrictEliasFano,
        uniform_partitioned_seq::UniformPartitionedSeqIter,
    },
    utils::TimingQueries,
};

#[derive(Clone, Debug, Epserde, MemSize, MemDbg)]
pub struct FreqIndex<DocumentSequence, FreqSequence> {
    pub(crate) n_docs: usize,
    pub(crate) n_terms: usize,
    pub(crate) docs_sequences: BitVecCollection,
    pub(crate) freqs_sequences: BitVecCollection,
    pub(crate) _phantom: PhantomData<(DocumentSequence, FreqSequence)>,
}

#[derive(Debug)]
pub struct FreqIndexPostingListIter<'a, DocumentSequence, FreqSequence>
where
    DocumentSequence: DocList,
    FreqSequence: FreqList,
{
    current: (u64, usize),
    doc_it: <DocumentSequence as EnumeratorFromBitSlice<'a>>::IterType,
    freq_it: <FreqSequence as EnumeratorFromBitSlice<'a>>::IterType,
}

// once we build them, they are immutable
unsafe impl Send for EliasFano {}
unsafe impl Send for IndexSequence {}
unsafe impl Send for StrictSequence {}
unsafe impl<'a, T> Send for UniformPartitionedSeqIter<'a, T> where T: DocList {}
unsafe impl<'a, T> Send for OptPartitionedSeqIter<'a, T> where
    T: DocList + for<'b> PartitionableSequence<'b>
{
}

unsafe impl Send for StrictEliasFano {}
pub trait DocList:
    for<'a> EnumeratorFromBitSlice<'a, IterType: NextGEQ>
    + for<'b> From<&'b [u64]>
    + WriteBitvector
    + Send
    + Debug
    + TypeHash
{
}

pub trait FreqList:
    for<'a> EnumeratorFromBitSlice<'a>
    + for<'b> From<&'b [u64]>
    + WriteBitvector
    + Send
    + Debug
    + TypeHash
{
}

impl<T> DocList for T where
    T: for<'a> EnumeratorFromBitSlice<'a, IterType: NextGEQ>
        + for<'b> From<&'b [u64]>
        + WriteBitvector
        + Send
        + Debug
        + TypeHash
{
}

impl<T> FreqList for T where
    T: for<'a> EnumeratorFromBitSlice<'a>
        + for<'b> From<&'b [u64]>
        + WriteBitvector
        + Send
        + Debug
        + TypeHash
{
}

impl<DocumentSequence, FreqSequence> FreqIndex<DocumentSequence, FreqSequence>
where
    DocumentSequence: DocList,
    FreqSequence: FreqList,
{
    pub fn get_builder(n_docs: usize) -> FreqIndexBuilder<DocumentSequence, FreqSequence> {
        FreqIndexBuilder::new(n_docs)
    }

    pub fn from_files(input_path: &str) -> Self {
        let mmap_len = fs::metadata(&format!("{}.docs", input_path)).unwrap().len() / 4;

        let mut docs_iter = BinaryCollectionIterator::new(&format!("{}.docs", input_path));
        let freqs_iter = BinaryCollectionIterator::new(&format!("{}.freqs", input_path));

        let mut singleton = docs_iter.next().unwrap();
        let n_docs = singleton.next().unwrap();

        log::info!("number of documents: {}", n_docs);
        let pb = pb_with_message(mmap_len, "creating index".to_string());

        let mut it = docs_iter.zip(freqs_iter);

        let mut builder = Self::get_builder(n_docs as usize);

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
                assert!(sz > 0);

                builder.push_plist_freqs(&v_docs, &v_freqs);

                n_postings += sz;
                pb.inc(sz);
            }
        }

        pb.finish();
        log::info!("processed {} postings", n_postings);

        builder.build()
    }

    pub fn load_index(path: &str) -> Self {
        let reader = std::fs::read(path).expect("could not read index file");
        log::info!("Serialized size: {:?} bytes", reader.len());

        unsafe { Self::deserialize_eps(&reader).expect("could not deserialize index") }
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
                        "PLIST idx {} | Mismatch at freq iter is: {:?}, current position is {:?}",
                        processed,
                        it_plist.freq_it,
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
}

impl<DL, FL> InvertedIndex for FreqIndex<DL, FL>
where
    DL: DocList,
    FL: FreqList,
{
    type IterType<'a>
        = FreqIndexPostingListIter<'a, DL, FL>
    where
        Self: 'a;

    fn n_docs(&self) -> usize {
        self.n_docs
    }

    fn n_terms(&self) -> usize {
        self.n_terms
    }

    fn get_plist_iter(&self, i: usize) -> FreqIndexPostingListIter<'_, DL, FL> {
        let a: BitSliceWithOffset<'_> = self.docs_sequences.get(i);
        prefetch_bitslice_word(&a, 0);
        let (sz, pos) = unsafe { a.get_gamma_nonzero_unchecked(0) };
        let mut doc_it = DL::iter_from_slice(a.slice_from(pos), sz as usize, self.n_docs as u64);

        let a: BitSliceWithOffset<'_> = self.freqs_sequences.get(i);
        prefetch_bitslice_word(&a, 0);
        let freq_it = FL::iter_from_slice(a, sz as usize, self.n_docs as u64);
        let current = doc_it.move_to_position(0);

        FreqIndexPostingListIter {
            current,
            doc_it,
            freq_it,
        }
    }
}

impl<DS, FS> PostingListIter for FreqIndexPostingListIter<'_, DS, FS>
where
    DS: DocList,
    FS: FreqList,
{
    fn current_doc(&self) -> u64 {
        self.current.0
    }

    fn current_pos(&self) -> usize {
        self.current.1
    }

    fn next_geq(&mut self, lower_bound: u64) {
        self.current = self.doc_it.next_geq(lower_bound);
    }

    fn next_doc(&mut self) {
        self.current = self.doc_it.next_val();
    }

    fn freq(&mut self) -> u64 {
        let pos = self.current_pos();
        self.freq_it.move_to_position(pos).0
    }

    fn len(&self) -> usize {
        self.doc_it.len()
    }
}

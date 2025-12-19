use mem_dbg::{MemDbg, MemSize};

use std::fs::{self, File};

use crate::{config, readers::ds2i_reader::BinaryCollectionIterator, utils::pb_with_message};
use epserde::prelude::*;
use std::{fmt::Debug, marker::PhantomData, path::Path};

use crate::{
    bitvector::bitvector_collection::BitVectorCollectionBuilder,
    elias_fano::{
        indexed_seq::{IndexSequence, StrictSequence},
        opt_partition::OptPartitionedSeqIter,
        strict_ef::StrictEliasFano,
        uniform_partitioned_seq::UniformPartitionedSeqIter,
    },
    utils::TimingQueries,
    BitSliceWithOffset, BitVec, BitVecCollection, EliasFano, EnumeratorFromBitSlice, NextGEQ,
    PartitionableSequence, SequenceEnumerator, WriteBitvector,
};

#[derive(Clone, Debug, Epserde, MemSize, MemDbg)]
pub struct FreqIndex<DocumentSequence, FreqSequence> {
    pub n_docs: usize,
    pub n_terms: usize,
    docs_sequences: BitVecCollection,
    freqs_sequences: BitVecCollection,
    pub _phantom: PhantomData<(DocumentSequence, FreqSequence)>,
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

pub trait PostingListIter {
    fn current_doc(&self) -> u64;
    fn current_pos(&self) -> usize;
    fn next_geq(&mut self, lower_bound: u64);
    fn next_doc(&mut self);
    fn freq(&mut self) -> u64;
    fn len(&self) -> usize;
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

impl<'a, DocumentSequence, FreqSequence> FreqIndex<DocumentSequence, FreqSequence>
where
    DocumentSequence: DocList,
    FreqSequence: FreqList,
{
    pub fn get_plist_iter(
        &'a self,
        i: usize,
    ) -> FreqIndexPostingListIter<'a, DocumentSequence, FreqSequence> {
        let a: BitSliceWithOffset<'a> = self.docs_sequences.get(i);
        let (sz, pos) = unsafe { a.get_gamma_nonzero_unchecked(0) };
        let mut doc_it =
            DocumentSequence::iter_from_slice(a.split_at(pos).1, sz as usize, self.n_docs as u64);

        let a: BitSliceWithOffset<'a> = self.freqs_sequences.get(i);
        let freq_it = FreqSequence::iter_from_slice(a, sz as usize, self.n_docs as u64);
        let current = doc_it.next_val();

        FreqIndexPostingListIter {
            current,
            doc_it,
            freq_it,
        }
    }

    fn push_plist_freqs(
        docs_bv: &mut BitVectorCollectionBuilder<Vec<u64>>,
        freqs_bv: &mut BitVectorCollectionBuilder<Vec<u64>>,
        sz: usize,
        bv_docs: BitVec,
        bv_freqs: BitVec,
    ) {
        let mut bv = BitVec::new();
        bv.append_gamma_nonzero(sz as u64);
        // println!("sz is: {}", sz);
        bv.concat(bv_docs);

        docs_bv.push(bv);
        freqs_bv.push(bv_freqs);
    }

    pub fn from_files(input_path: &str) -> Self {
        let mmap_len = fs::metadata(&format!("{}.docs", input_path)).unwrap().len() / 4;

        let pb = pb_with_message(mmap_len, "creating index".to_string());

        let mut docs_iter = BinaryCollectionIterator::new(&format!("{}.docs", input_path));
        let freqs_iter = BinaryCollectionIterator::new(&format!("{}.freqs", input_path));

        let mut singleton = docs_iter.next().unwrap();
        let n_docs = singleton.next().unwrap();

        log::info!("number of documents: {}", n_docs);

        let mut it = docs_iter.zip(freqs_iter);

        let mut n_postings = 0;
        let mut n_terms = 0;
        let mut bvb_docs = BitVectorCollectionBuilder::default();
        let mut bvb_freqs = BitVectorCollectionBuilder::default();

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

                Self::push_plist_freqs(
                    &mut bvb_docs,
                    &mut bvb_freqs,
                    sz as usize,
                    DocumentSequence::write_bitvector(&v_docs, sz as usize, n_docs),
                    FreqSequence::write_bitvector(&v_freqs, sz as usize, 0),
                );

                n_terms += 1;
                n_postings += sz;
                pb.inc(sz);
            }
        }

        pb.finish();
        log::info!("processed {} postings", n_postings);

        FreqIndex {
            n_docs: n_docs.try_into().unwrap(),
            n_terms,
            docs_sequences: bvb_docs.build(),
            freqs_sequences: bvb_freqs.build(),
            _phantom: PhantomData,
        }
    }

    // pub fn load_index(index_path: &str) -> Self {
    //     let serialized = fs::read(index_path).unwrap();

    //     let ds = bincode::deserialize::<Self>(&serialized).unwrap();

    //     ds
    // }

    pub fn load_index(path: &str) -> Self {
        let reader = std::fs::read(path).expect("could not read index file");
        log::info!("Serialized size: {:?} bytes", reader.len());

        unsafe { Self::deserialize_eps(&reader).expect("could not deserialize index") }
    }

    pub fn check_correctness(&'a self, input_path: &str) {
        let mmap_len = fs::metadata(&format!("{}.docs", input_path)).unwrap().len() / 4;

        let pb = pb_with_message(mmap_len, "Checking correctness".to_string());

        let mut docs_iter = BinaryCollectionIterator::new(&format!("{}.docs", input_path));
        let freqs_iter = BinaryCollectionIterator::new(&format!("{}.freqs", input_path));

        let mut singleton = docs_iter.next().unwrap();
        let n_docs = singleton.next().unwrap();

        log::info!("number of documents: {}", n_docs);

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

impl<'a, DS, FS> PostingListIter for FreqIndexPostingListIter<'a, DS, FS>
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

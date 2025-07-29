use indicatif::{ProgressBar, ProgressIterator, ProgressStyle};
use mem_dbg::{MemDbg, MemSize};
use memmap2::MmapOptions;

use std::fs::File;

use serde::{Deserialize, Serialize};
use std::{fmt::Debug, fs, marker::PhantomData, mem, path::Path};

use crate::{
    bitvector::bitvector_collection::BitVectorCollectionBuilder,
    elias_fano::{
        indexed_seq::{IndexSequence, StrictSequence},
        opt_partition::OptPartitionedSeqIter,
        strict_ef::StrictEliasFano,
        uniform_partitioned_seq::UniformPartitionedSeqIter,
    },
    space_usage::SpaceUsage,
    utils::TimingQueries,
    BitSliceWithOffset, BitVec, BitVecCollection, EliasFano, EnumeratorFromBitSlice, NextGEQ,
    PartitionableSequence, SequenceEnumerator, WriteBitvector,
};

#[derive(Clone, Debug, Serialize, Deserialize, MemSize, MemDbg)]
pub struct FreqIndex<DocumentSequence, FreqSequence> {
    pub n_docs: usize,
    pub n_terms: usize,
    docs_sequences: BitVecCollection,
    freqs_sequences: BitVecCollection,
    pub _phantom: PhantomData<(DocumentSequence, FreqSequence)>,
}

#[derive(Debug)]
pub struct PostingListIter<'a, DocumentSequence, FreqSequence>
where
    DocumentSequence: DocList<'a>,
    FreqSequence: FreqList<'a>,
{
    current: Option<(u64, usize)>,
    doc_it: DocumentSequence::IterType,
    freq_it: FreqSequence::IterType,
}

// once we build them, they are immutable
unsafe impl Send for EliasFano {}
unsafe impl Send for IndexSequence {}
unsafe impl Send for StrictSequence {}
unsafe impl<'a, T> Send for UniformPartitionedSeqIter<'a, T> where T: DocList<'a> {}
unsafe impl<'a, T> Send for OptPartitionedSeqIter<'a, T> where
    T: DocList<'a> + for<'b> PartitionableSequence<'b>
{
}

unsafe impl Send for StrictEliasFano {}
pub trait DocList<'a>:
    EnumeratorFromBitSlice<'a, IterType: NextGEQ>
    + for<'b> From<&'b [u64]>
    + WriteBitvector
    + Send
    + Debug
{
}

pub trait FreqList<'a>:
    EnumeratorFromBitSlice<'a> + for<'b> From<&'b [u64]> + WriteBitvector + Send + Debug
{
}

impl<'a, T> DocList<'a> for T where
    T: EnumeratorFromBitSlice<'a, IterType: NextGEQ>
        + for<'b> From<&'b [u64]>
        + WriteBitvector
        + Send
        + Debug
{
}

impl<'a, T> FreqList<'a> for T where
    T: EnumeratorFromBitSlice<'a> + for<'b> From<&'b [u64]> + WriteBitvector + Send + Debug
{
}

impl<'a, DocumentSequence, FreqSequence> FreqIndex<DocumentSequence, FreqSequence>
where
    DocumentSequence: DocList<'a>,
    FreqSequence: FreqList<'a>,
{
    pub fn get_plist_iter(
        &'a self,
        i: usize,
    ) -> PostingListIter<'a, DocumentSequence, FreqSequence> {
        let a: BitSliceWithOffset<'a> = self.docs_sequences.get(i);
        let (sz, pos) = unsafe { a.get_gamma_nonzero_unchecked(0) };
        let mut doc_it =
            DocumentSequence::iter_from_slice(a.split_at(pos).1, sz as usize, self.n_docs as u64);

        let a: BitSliceWithOffset<'a> = self.freqs_sequences.get(i);
        let freq_it = FreqSequence::iter_from_slice(a, sz as usize, self.n_docs as u64);
        let current = doc_it.next_val();

        PostingListIter {
            current,
            doc_it,
            freq_it,
        }
    }

    // old way to push plists (with no frequency information)
    // fn push_plist(docs_bv: &mut BitVectorCollectionBuilder<Vec<u64>>, sz: usize, bv_data: BitVec) {
    //     let mut bv = BitVec::new();
    //     bv.append_gamma_nonzero(sz as u64);
    //     // println!("sz is: {}", sz);
    //     bv.concat(bv_data);

    //     docs_bv.push(bv);
    // }

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

    const LENGTH_THRESHOLD: u64 = 1 << 12;

    pub fn from_files(input_path: &str) -> Self {
        let docs_file =
            File::open(format!("{}.docs", input_path)).expect("could not open docs file");
        let freq_file =
            File::open(format!("{}.freqs", input_path)).expect("could not open freqs file");

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

        println!("file mapped!");

        let mut docs_iter = mmap_docs
            .array_chunks::<4>()
            .map(|chunk| u32::from_le_bytes(*chunk) as u64)
            // progress bar
            .progress_with(pb_with_message(
                (docs_file.metadata().unwrap().len() / 4) as u64,
                String::from("Building Index"),
            ));

        let freqs_iter = mmap_freqs
            .array_chunks::<4>()
            .map(|chunk| u32::from_le_bytes(*chunk) as u64);

        docs_iter.next();
        let n_docs = docs_iter.next().unwrap();

        let mut it = docs_iter.zip(freqs_iter);

        let mut n_postings = 0;
        let mut n_terms = 0;
        let mut bvb_docs = BitVectorCollectionBuilder::default();
        let mut bvb_freqs = BitVectorCollectionBuilder::default();

        while let Some((sz, sz_freq)) = it.next() {
            assert!(sz == sz_freq);
            // println!("------------- list n {} -------------", processed);
            // println!("list n {}, size is {}", idx.n_terms, sz);

            if sz > Self::LENGTH_THRESHOLD {
                let (v_docs, v_freqs): (Vec<_>, Vec<_>) = (&mut it).take(sz as usize).unzip();
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
            } else {
                let _x = (&mut it).nth(sz as usize - 1);
            }
            // if idx.n_terms % 10_000 == 0 {
            //     println!("processed {} plists", idx.n_terms);
            // }
        }

        println!("processed {} postings", n_postings);

        FreqIndex {
            n_docs: n_docs.try_into().unwrap(),
            n_terms,
            docs_sequences: bvb_docs.build(),
            freqs_sequences: bvb_freqs.build(),
            _phantom: PhantomData,
        }
    }

    pub fn load_index(index_path: &str) -> Self {
        let serialized = fs::read(index_path).unwrap();
        println!("Serialized size: {:?} bytes", serialized.len());

        let ds = bincode::deserialize::<Self>(&serialized).unwrap();

        ds
    }

    // #[allow(unreachable_code)]
    // #[allow(unused_variables)]
    // pub fn from_files_parallel(input_path: &str) -> Self {
    //     unreachable!();
    //     let docs_file =
    //         File::open(format!("{}.docs", input_path)).expect("could not open docs file");
    //     // let sizes_file = format!("{}.sizes", input_path);

    //     let mmap_docs = unsafe {
    //         MmapOptions::new()
    //             .map(&docs_file)
    //             .expect("could not memory map docs file")
    //     };

    //     println!("file mapped!");

    //     let binding = mmap_docs
    //         .array_chunks::<4>()
    //         .map(|chunk| u32::from_le_bytes(*chunk) as u64)
    //         .collect::<Vec<_>>();

    //     let mut docs_iter = binding.iter().enumerate();

    //     docs_iter.next();

    //     let (_, &n_docs) = docs_iter.next().unwrap();
    //     let mut n_terms = 0;
    //     let mut bvb = BitVectorCollectionBuilder::default();

    //     let mut n_postings = 0;
    //     scope(|scope| {
    //         std::iter::repeat(())
    //             .scan(docs_iter, |it, ()| {
    //                 let (i, sz) = it.next()?;
    //                 it.nth(*sz as usize - 1);
    //                 Some(&binding[(i + 1)..(i + 1 + *sz as usize)])
    //             })
    //             .filter(|&x| x.len() > Self::LENGTH_THRESHOLD as usize)
    //             .parallel_map_scoped(scope, |x| {
    //                 (
    //                     x.len(),
    //                     DocumentSequence::write_bitvector(x, x.len(), n_docs),
    //                 )
    //             })
    //             .enumerate()
    //             .for_each(|(i, (sz, data))| {
    //                 if i % 5000 == 0 {
    //                     println!("processed {} plists!", i);
    //                 }
    //                 n_postings += sz;
    //                 n_terms += 1;
    //                 Self::push_plist(&mut bvb, sz, data);
    //             });
    //     })
    //     .expect("error in parallel processing of the index");

    //     println!("processed {} postings", n_postings);

    //     FreqIndex {
    //         n_docs: n_docs.try_into().unwrap(),
    //         n_terms,
    //         docs_sequences: bvb.build(),
    //         freqs_sequences: BitVecCollection::default(),
    //         _phantom: PhantomData,
    //     }
    // }

    // pub fn check_correctness(&'a self, input_path: &str) {
    //     let docs_file = format!("{}.docs", input_path);
    //     let binding = std::fs::read(docs_file).expect("can't read .docs file ");

    //     let mut docs_iter = binding
    //         .array_chunks::<4>()
    //         .map(|chunk| u32::from_le_bytes(*chunk) as u64)
    //         // progress bar
    //         .progress_with(pb_with_message(
    //             (binding.len() / 4) as u64,
    //             String::from("Checking"),
    //         ));

    //     docs_iter.next();
    //     docs_iter.next();

    //     let mut processed = 0;
    //     while let Some(sz) = docs_iter.next() {
    //         if sz > Self::LENGTH_THRESHOLD {
    //             let v: Vec<u64> = (&mut docs_iter).take(sz as usize).collect();
    //             processed += 1;
    //             let mut it = self.get_plist_iter(processed - 1).doc_it;
    //             let itv = v.iter();
    //             for (_i, &s) in itv.enumerate() {
    //                 // println!("check n {}", i);
    //                 // assert!(dbg!(s) == dbg!(it.next().unwrap()));
    //                 assert!(s == it.next().unwrap());
    //             }
    //         } else {
    //             let _x = (&mut docs_iter).nth(sz as usize - 1);
    //         }
    //     }
    // }

    pub fn check_correctness(&'a self, input_path: &str) {
        // let docs_file = format!("{}.docs", input_path);
        // let binding = std::fs::read(docs_file).expect("can't read .docs file ");

        // let mut docs_iter = binding
        // .array_chunks::<4>()
        // .map(|chunk| u32::from_le_bytes(*chunk) as u64)
        // // progress bar
        // .progress_with(pb_with_message(
        //     (binding.len() / 4) as u64,
        //     String::from("Checking"),
        // ));

        let docs_file =
            File::open(format!("{}.docs", input_path)).expect("could not open docs file");
        let freq_file =
            File::open(format!("{}.freqs", input_path)).expect("could not open freqs file");

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
            .map(|chunk| u32::from_le_bytes(*chunk) as u64)
            // progress bar
            .progress_with(pb_with_message(
                (docs_file.metadata().unwrap().len() / 4) as u64,
                String::from("Checking Index"),
            ));

        let freqs_iter = mmap_freqs
            .array_chunks::<4>()
            .map(|chunk| u32::from_le_bytes(*chunk) as u64);

        docs_iter.next();
        docs_iter.next();

        let mut it = docs_iter.zip(freqs_iter);

        let mut processed = 0;
        while let Some((sz, sz_freq)) = it.next() {
            assert!(sz == sz_freq);
            // if sz != sz_freq {
            //     panic!("size mismatch in .docs and .freqs files");
            // }

            if sz > Self::LENGTH_THRESHOLD {
                let v: Vec<(u64, u64)> = (&mut it).take(sz as usize).collect();

                // println!("Checking list {} with size {}", processed, sz);
                let mut it_plist = self.get_plist_iter(processed);
                let itv = v.iter();
                for (_i, &s) in itv.clone().enumerate() {
                    // println!("check n {}", i);
                    // assert!(dbg!(s) == dbg!(it.next().unwrap()));
                    let docid = it_plist.current_doc().unwrap();
                    let freq = it_plist.freq().unwrap();
                    assert_eq!(
                        s,
                        (docid, freq),
                        "PLIST idx {} | Mismatch at freq iter is: {:?}, current position is {:?}",
                        processed,
                        it_plist.freq_it,
                        it_plist.current_pos(),
                    );
                    it_plist.next_doc();
                }
                processed += 1;
            } else {
                let _x = (&mut it).nth(sz as usize - 1);
            }
        }
    }

    pub fn load_or_build_and_save(
        input_filename: &str,
        output_filename: &str,
        force_rebuild: bool,
    ) -> Self {
        let ds: Self;
        let path = Path::new(output_filename);
        if path.exists() && !force_rebuild {
            println!(
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
            println!("Construction time {:?} millisecs", t_min / 1000000);

            let serialized = bincode::serialize(&ds).unwrap();
            println!("Serialized size: {:?} bytes", serialized.len());
            fs::write(path, serialized).unwrap();
        }
        ds
    }
}

fn pb_with_message(len: u64, msg: String) -> ProgressBar {
    let pb = ProgressBar::new(len as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg} [{bar:40.cyan/blue}] {percent}% {elapsed}")
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_message(msg.clone());
    pb.with_finish(indicatif::ProgressFinish::WithMessage(
        format!("{} Done!", msg).into(),
    ))
}

impl<T, S> SpaceUsage for FreqIndex<T, S> {
    fn space_usage_byte(&self) -> usize {
        self.docs_sequences.space_usage_byte()
            + self.freqs_sequences.space_usage_byte()
            + mem::size_of::<usize>() * 2
    }
}

impl<'a, DS, FS> PostingListIter<'a, DS, FS>
where
    DS: DocList<'a>,
    FS: FreqList<'a>,
{
    pub fn current_doc(&self) -> Option<u64> {
        Some(self.current?.0)
    }

    pub fn current_pos(&self) -> Option<usize> {
        Some(self.current?.1)
    }

    pub fn next_geq(&mut self, lower_bound: u64) {
        self.current = self.doc_it.next_geq(lower_bound);
    }

    pub fn next_doc(&mut self) {
        self.current = self.doc_it.next_val();
    }

    pub fn freq(&mut self) -> Option<u64> {
        let pos = self.current_pos()?;
        Some(self.freq_it.move_to_position(pos)?.0)
    }

    pub fn len(&self) -> usize {
        self.doc_it.len()
    }
}

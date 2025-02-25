use indicatif::{ProgressBar, ProgressIterator, ProgressStyle};
use memmap2::MmapOptions;
use pariter::{scope, IteratorExt};

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
    BitSliceWithOffset, BitVec, BitVecCollection, EliasFano, EnumeratorFromBitSlice,
    PartitionableSequence, ToBitvector, WriteBitvector,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FreqIndex<DocumentSequence> {
    pub n_docs: usize,
    pub n_terms: usize,
    docs_sequences: BitVecCollection,
    _freqs_sequences: BitVecCollection,
    pub _phantom: PhantomData<DocumentSequence>,
}

// once we build them, they are immutable
unsafe impl Send for EliasFano {}
unsafe impl Send for IndexSequence {}
unsafe impl Send for StrictSequence {}
unsafe impl<'a, T> Send for UniformPartitionedSeqIter<'a, T> where T: PostingList<'a> {}
unsafe impl<'a, T> Send for OptPartitionedSeqIter<'a, T> where
    T: PostingList<'a> + for<'b> PartitionableSequence<'b>
{
}
unsafe impl Send for StrictEliasFano {}
pub trait PostingList<'a>:
    ToBitvector + EnumeratorFromBitSlice<'a> + for<'b> From<&'b [u64]> + WriteBitvector + Send + Debug
{
}

impl<'a, T> PostingList<'a> for T where
    T: ToBitvector
        + EnumeratorFromBitSlice<'a>
        + for<'b> From<&'b [u64]>
        + WriteBitvector
        + Send
        + Debug
{
}

impl<'a, DocumentSequence> FreqIndex<DocumentSequence>
where
    DocumentSequence: PostingList<'a>,
{
    pub fn get_plist_iter(&'a self, i: usize) -> DocumentSequence::IterType {
        let a: BitSliceWithOffset<'a> = self.docs_sequences.get(i);
        let (sz, pos) = unsafe { a.get_gamma_nonzero_unchecked(0) };
        DocumentSequence::iter_from_slice_with_data(
            a.split_at(pos).1,
            sz as usize,
            self.n_docs as u64,
        )
    }

    fn push_plist(docs_bv: &mut BitVectorCollectionBuilder<Vec<u64>>, data_doc: (usize, BitVec)) {
        let mut bv = BitVec::new();
        let (sz, bv_data) = data_doc;
        bv.append_gamma_nonzero(sz as u64);
        // println!("sz is: {}", sz);
        bv.concat(bv_data);

        docs_bv.push(bv);
    }

    const LENGTH_THRESHOLD: u64 = 1 << 12;

    pub fn from_files(input_path: &str) -> Self {
        let docs_file = format!("{}.docs", input_path);
        // let sizes_file = format!("{}.sizes", input_path);

        let binding = std::fs::read(docs_file).expect("can't read .docs file ");
        let mut docs_iter = binding
            .array_chunks::<4>()
            .map(|chunk| u32::from_le_bytes(*chunk) as u64)
            // progress bar
            .progress_with(pb_with_message(
                (binding.len() / 4) as u64,
                String::from("Building Index"),
            ));

        docs_iter.next();

        let n_docs = docs_iter.next().unwrap();

        let mut n_postings = 0;
        let mut n_terms = 0;
        let mut bvb = BitVectorCollectionBuilder::default();

        while let Some(sz) = docs_iter.next() {
            // println!("------------- list n {} -------------", processed);
            // println!("list n {}, size is {}", idx.n_terms, sz);

            if sz > Self::LENGTH_THRESHOLD {
                let v: Vec<u64> = (&mut docs_iter).take(sz as usize).collect();
                assert!(v.len() == sz as usize);
                assert!(sz > 0);

                Self::push_plist(
                    &mut bvb,
                    (
                        sz as usize,
                        DocumentSequence::write_bitvector(&v, sz as usize, n_docs),
                    ),
                );
                n_terms += 1;
                n_postings += sz;
            } else {
                let _x = (&mut docs_iter).nth(sz as usize - 1);
            }
            // if idx.n_terms % 10_000 == 0 {
            //     println!("processed {} plists", idx.n_terms);
            // }
        }

        println!("processed {} postings", n_postings);

        FreqIndex {
            n_docs: n_docs.try_into().unwrap(),
            n_terms,
            docs_sequences: bvb.build(),
            _freqs_sequences: BitVecCollection::default(),
            _phantom: PhantomData,
        }
    }

    pub fn from_files_parallel(input_path: &str) -> Self {
        let docs_file =
            File::open(format!("{}.docs", input_path)).expect("could not open docs file");
        // let sizes_file = format!("{}.sizes", input_path);

        let mmap_docs = unsafe {
            MmapOptions::new()
                .map(&docs_file)
                .expect("could not memory map docs file")
        };

        println!("file mapped!");

        let binding = mmap_docs
            .array_chunks::<4>()
            .map(|chunk| u32::from_le_bytes(*chunk) as u64)
            .collect::<Vec<_>>();

        let mut docs_iter = binding.iter().enumerate();

        docs_iter.next();

        let (_, &n_docs) = docs_iter.next().unwrap();
        let mut n_terms = 0;
        let mut bvb = BitVectorCollectionBuilder::default();

        let mut n_postings = 0;
        scope(|scope| {
            std::iter::repeat(())
                .scan(docs_iter, |it, ()| {
                    let (i, sz) = it.next()?;
                    it.nth(*sz as usize - 1);
                    Some(&binding[(i + 1)..(i + 1 + *sz as usize)])
                })
                .filter(|&x| x.len() > Self::LENGTH_THRESHOLD as usize)
                .parallel_map_scoped(scope, |x| {
                    (
                        x.len(),
                        DocumentSequence::write_bitvector(x, x.len(), n_docs),
                    )
                })
                .enumerate()
                .for_each(|(i, data)| {
                    if i % 5000 == 0 {
                        println!("processed {} plists!", i);
                    }
                    n_postings += data.0;
                    n_terms += 1;
                    Self::push_plist(&mut bvb, data);
                });
        })
        .expect("error in parallel processing of the index");

        println!("processed {} postings", n_postings);

        FreqIndex {
            n_docs: n_docs.try_into().unwrap(),
            n_terms,
            docs_sequences: bvb.build(),
            _freqs_sequences: BitVecCollection::default(),
            _phantom: PhantomData,
        }
    }

    pub fn check_correctness(&'a self, input_path: &str) {
        let docs_file = format!("{}.docs", input_path);
        let binding = std::fs::read(docs_file).expect("can't read .docs file ");

        let mut docs_iter = binding
            .array_chunks::<4>()
            .map(|chunk| u32::from_le_bytes(*chunk) as u64)
            // progress bar
            .progress_with(pb_with_message(
                (binding.len() / 4) as u64,
                String::from("Checking"),
            ));

        docs_iter.next();
        docs_iter.next();

        let mut processed = 0;
        while let Some(sz) = docs_iter.next() {
            if sz > Self::LENGTH_THRESHOLD {
                let v: Vec<u64> = (&mut docs_iter).take(sz as usize).collect();
                processed += 1;
                let mut it = self.get_plist_iter(processed - 1);
                let itv = v.iter();
                for (_i, &s) in itv.enumerate() {
                    // println!("check n {}", i);
                    // assert!(dbg!(s) == dbg!(it.next().unwrap()));
                    assert!(s == it.next().unwrap());
                }
            } else {
                let _x = (&mut docs_iter).nth(sz as usize - 1);
            }
        }
    }

    pub fn load_or_build_and_save(
        input_filename: &str,
        output_filename: &str,
        force_rebuild: bool,
    ) -> Self {
        let ds: Self;
        let path = Path::new(&output_filename);
        if path.exists() && !force_rebuild {
            println!(
                "The data structure already exists. Filename: {}. I'm going to load it ...",
                output_filename
            );
            let serialized = fs::read(path).unwrap();
            println!("Serialized size: {:?} bytes", serialized.len());
            ds = bincode::deserialize::<Self>(&serialized).unwrap();
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

impl<T> SpaceUsage for FreqIndex<T> {
    fn space_usage_byte(&self) -> usize {
        self.docs_sequences.space_usage_byte() + mem::size_of::<usize>() * 2
    }
}

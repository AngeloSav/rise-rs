use indicatif::{ProgressBar, ProgressIterator, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::{fs, marker::PhantomData, mem, path::Path};

use crate::{
    bitvector::bitvector_collection::BitVectorCollection, space_usage::SpaceUsage,
    utils::TimingQueries, BitSliceWithOffset, BitVecCollection, EnumeratorFromBitSlice,
    IncreasingSequenceEnumerator, ToBitvector,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FreqIndex<DocumentSequence, DSIter> {
    pub _n_docs: usize,
    pub n_terms: usize,
    docs_sequences: BitVecCollection,
    _freqs_sequences: BitVecCollection,
    _phantom: PhantomData<(DocumentSequence, DSIter)>,
}

pub trait PostingList<'a, T>: ToBitvector + EnumeratorFromBitSlice<'a, T> + From<Vec<u64>>
where
    T: IncreasingSequenceEnumerator,
{
}

impl<'a, T, S> PostingList<'a, S> for T
where
    T: ToBitvector + EnumeratorFromBitSlice<'a, S> + From<Vec<u64>>,
    S: IncreasingSequenceEnumerator,
{
}

impl<'a, DocumentSequence, S> FreqIndex<DocumentSequence, S>
where
    DocumentSequence: PostingList<'a, S> + 'a,
    S: IncreasingSequenceEnumerator + 'a,
{
    pub fn new(n_docs: usize) -> Self {
        Self {
            _n_docs: n_docs,
            n_terms: 0,
            docs_sequences: BitVectorCollection::with_capacity(0, 0),
            _freqs_sequences: BitVectorCollection::with_capacity(0, 0),
            _phantom: PhantomData::<(DocumentSequence, S)>,
        }
    }

    /// Push the document sequence `s` in the document collection, can only be done at build time
    fn push_posting_list(&mut self, s: DocumentSequence) {
        let a = s.to_bv();
        self.docs_sequences.push(a);
        self.n_terms += 1;
    }

    pub fn get_plist_iter(&'a self, i: usize) -> S {
        let a: BitSliceWithOffset<'a> = self.docs_sequences.get(i);
        DocumentSequence::iter_from_slice(a)
    }

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
        let mut idx = Self::new(n_docs as usize);

        let mut n_postings = 0;

        while let Some(sz) = docs_iter.next() {
            // println!("------------- list n {} -------------", processed);
            // println!("list n {}, size is {}", idx.n_terms, sz);

            if sz > 1 << 12 {
                let v: Vec<u64> = (&mut docs_iter).take(sz as usize).collect();
                assert!(v.len() == sz as usize);
                assert!(sz > 0);
                idx.push_posting_list(DocumentSequence::from(v));

                n_postings += sz;
            } else {
                let _x = (&mut docs_iter).nth(sz as usize - 1);
            }
            // if idx.n_terms % 10_000 == 0 {
            //     println!("processed {} plists", idx.n_terms);
            // }
        }

        println!("processed {} postings", n_postings);

        idx
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
            if sz > 1 << 12 {
                let v: Vec<u64> = (&mut docs_iter).take(sz as usize).collect();
                processed += 1;
                let mut it = self.get_plist_iter(processed - 1);
                let itv = v.iter();
                for (_i, &s) in itv.enumerate() {
                    // println!("check n {}", i);
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

impl<T, S> SpaceUsage for FreqIndex<T, S> {
    fn space_usage_byte(&self) -> usize {
        self.docs_sequences.n_bits() / 8
            + self._freqs_sequences.n_bits() / 8
            + mem::size_of::<usize>() * 2
    }
}

use std::{fs::File, rc::Rc};

use memmap2::{Mmap, MmapOptions};

pub struct BinaryCollectionIterator {
    mmap: Rc<Mmap>,
    pos: usize, // number of 4-byte words already consumed
    len: usize, // total number of 4-byte words
}

// an owned iterator over a single list: holds an Rc to the mmap so it can be returned without borrowing
pub struct List {
    mmap: Rc<Mmap>,
    cur: usize, // current word index (absolute, in words)
    end: usize, // exclusive word index
}

impl Iterator for List {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur >= self.end {
            return None;
        }
        let start = self.cur * 4;
        let end = start + 4;
        let bytes: [u8; 4] = self.mmap[start..end].try_into().unwrap();
        let v = u32::from_le_bytes(bytes) as u64;
        self.cur += 1;
        Some(v)
    }
}

impl ExactSizeIterator for List {
    fn len(&self) -> usize {
        self.end - self.cur
    }
}

impl BinaryCollectionIterator {
    pub fn new(file_path: &str) -> BinaryCollectionIterator {
        let input_file = File::open(file_path).expect("could not open file");

        let mmap = unsafe {
            MmapOptions::new()
                .map(&input_file)
                .expect("could not memory map file")
        };

        let len = mmap.len() / 4;

        BinaryCollectionIterator {
            mmap: Rc::new(mmap),
            pos: 0,
            len,
        }
    }

    fn next_internal(&mut self) -> Option<u64> {
        if self.pos >= self.len {
            return None;
        }
        let start = self.pos * 4;
        let end = start + 4;
        let bytes: [u8; 4] = self.mmap[start..end].try_into().unwrap();
        let v = u32::from_le_bytes(bytes) as u64;
        self.pos += 1;
        Some(v)
    }
}

impl Iterator for BinaryCollectionIterator {
    // return an owning iterator (List) for each list, no Vec allocation
    type Item = List;

    fn next(&mut self) -> Option<Self::Item> {
        // read list size (advances pos by 1)
        let sz = self.next_internal()? as usize;
        let start = self.pos;
        let end = start + sz;
        // advance the underlying iterator past the list elements
        self.pos = end;
        Some(List {
            mmap: Rc::clone(&self.mmap),
            cur: start,
            end,
        })
    }
}

/// Returns an iterator over (docs_list, freqs_list) pairs, and the total number of documents (u64)
pub fn from_files(input_path: &str) -> (impl Iterator<Item = (List, List)>, u64) {
    let mut docs_iter = BinaryCollectionIterator::new(&format!("{}.docs", input_path));
    let freqs_iter = BinaryCollectionIterator::new(&format!("{}.freqs", input_path));

    log::info!("files mapped!");

    // read header (raw u64) using next_internal
    let n_docs = docs_iter.next_internal().expect("missing docs header");

    // iterator yields (List, List) pairs; each List is an iterator over that list's u64 elements
    let it = docs_iter.zip(freqs_iter);

    (it, n_docs)
}

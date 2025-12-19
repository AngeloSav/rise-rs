use dsi_bitstream::{
    codes::{byte_len_vbyte, VByteLeRead, VByteLeWrite},
    impls::{BufBitReader, BufBitWriter, MemWordReader, MemWordWriterVec},
    traits::LE,
};

use crate::indexes::{block_freq_index::block_codices::BlockCodec, freq_index::PostingListIter};

pub struct BlockPostingList<T>
where
    T: BlockCodec,
{
    _codec: std::marker::PhantomData<T>,
}

fn split_into_blocks(list: &[u64], block_size: usize) -> Vec<Vec<u64>> {
    list.chunks(block_size)
        .map(|chunk| chunk.to_vec())
        .collect()
}

impl<T> BlockPostingList<T>
where
    T: BlockCodec,
{
    const BLOCK_SIZE: usize = 128;

    // List layout:
    // n | [max docid of each block] | [block endpoints] | [(docids in block, freqs in block) ...]

    /// writes to out the encoded posting list
    pub fn write(doc_list: &[u64], freq_list: &[u64], out: &mut Vec<u8>) {
        // write n of docs at the beginning
        {
            let mut writer = BufBitWriter::<LE, _>::new(MemWordWriterVec::new(&mut *out));
            writer
                .write_vbyte_le(doc_list.len() as u64)
                .expect("error in writing n to block posting list");
        }

        let n_blocks = (doc_list.len() + Self::BLOCK_SIZE - 1) / Self::BLOCK_SIZE; // ceiling division
        let begin_block_maxs = out.len();
        let begin_block_endpoints = begin_block_maxs + 4 * n_blocks;
        let begin_blocks = begin_block_endpoints + 4 * (n_blocks - 1);

        out.resize(begin_blocks, 0);

        let blocked_doc_list = split_into_blocks(doc_list, Self::BLOCK_SIZE);
        let blocked_freq_list = split_into_blocks(freq_list, Self::BLOCK_SIZE);

        let mut block_base = 0;
        for (b, (doc_block, freq_block)) in blocked_doc_list
            .into_iter()
            .zip(blocked_freq_list.into_iter())
            .enumerate()
        {
            // we convert docs to a increasing sequence that subtracts block_base
            // we convert freqs to a sequence of integers >= 0

            let max_docid = *doc_block.last().unwrap();

            out[begin_block_maxs + b * 4..begin_block_maxs + (b + 1) * 4]
                .copy_from_slice(&(max_docid as u32).to_le_bytes());

            let encoded_docs = T::encode_monotone(doc_block.iter().map(|&d| d - block_base));

            debug_assert_eq!(
                T::decode_monotone(&encoded_docs, doc_block.len()).0,
                doc_block
                    .iter()
                    .map(|&d| d - block_base)
                    .collect::<Vec<u64>>()
            );

            let encoded_freqs = T::encode(freq_block.iter().map(|x| x - 1));

            debug_assert_eq!(
                T::decode(&encoded_freqs, freq_block.len()).0,
                freq_block.iter().map(|x| x - 1).collect::<Vec<u64>>()
            );

            out.extend(encoded_docs);
            out.extend(encoded_freqs);

            if b != n_blocks - 1 {
                let new_endpoint = out.len() - begin_blocks;
                out[begin_block_endpoints + b * 4..begin_block_endpoints + (b + 1) * 4]
                    .copy_from_slice(&((new_endpoint) as u32).to_le_bytes());
            }

            block_base = max_docid + 1;
        }
    }

    pub fn iter_from_slice(data: &[u8], universe: u64) -> BlockPostingListIter<'_, T> {
        let n_docs = {
            let mut reader = BufBitReader::<LE, _>::new(MemWordReader::new(data));
            reader
                .read_vbyte_le()
                .expect("error in reading n from block posting list")
        };

        let n_blocks = (n_docs as usize + Self::BLOCK_SIZE - 1) / Self::BLOCK_SIZE;

        let begin_block_maxs = byte_len_vbyte(n_docs); // after n_docs
        let begin_block_endpoints = begin_block_maxs + 4 * n_blocks;
        let begin_blocks = begin_block_endpoints + 4 * (n_blocks - 1);

        let mut it = BlockPostingListIter {
            len: n_docs as usize,
            block_maxs: &data[begin_block_maxs..begin_block_endpoints],
            block_endpoints: &data[begin_block_endpoints..begin_blocks],
            blocks_data: &data[begin_blocks..],
            docs_buf: Vec::with_capacity(Self::BLOCK_SIZE),
            freqs_buf: Vec::with_capacity(Self::BLOCK_SIZE),
            n_blocks,
            universe,
            _codec: std::marker::PhantomData,

            // all these will be filled by decode_docs_block
            cur_freqs_data: &[],
            cur_block: 0,
            decoded_freqs: false,
            cur_block_size: 0,
            cur_block_max: 0,
            cur_base: 0,
            pos_in_block: 0,
            cur_docid: 0,
        };

        it.decode_docs_block(0);
        it
    }
}

pub struct BlockPostingListIter<'a, T>
where
    T: BlockCodec,
{
    // buffers and slices
    block_maxs: &'a [u8],
    block_endpoints: &'a [u8],
    blocks_data: &'a [u8],
    docs_buf: Vec<u64>,
    freqs_buf: Vec<u64>,
    cur_freqs_data: &'a [u8],

    // bookkeping for blocks
    n_blocks: usize,
    cur_block: usize,
    decoded_freqs: bool,
    cur_block_size: usize,
    cur_block_max: u64,
    cur_base: u64,
    pos_in_block: usize,

    //iterator stuff
    len: usize,
    cur_docid: u64,
    universe: u64,
    _codec: std::marker::PhantomData<&'a T>,
}

impl<'a, T> BlockPostingListIter<'a, T>
where
    T: BlockCodec,
{
    fn decode_docs_block(&mut self, block: usize) {
        let endpoint = if block != 0 {
            let mut buf = [0u8; 4];
            buf.copy_from_slice(&self.block_endpoints[(block - 1) * 4..(block - 1) * 4 + 4]);
            u32::from_le_bytes(buf) as usize
        } else {
            0
        };

        self.cur_block_max = self.block_max(block);
        self.cur_block_size = if (block + 1) * BlockPostingList::<T>::BLOCK_SIZE <= self.len {
            BlockPostingList::<T>::BLOCK_SIZE
        } else {
            self.len % BlockPostingList::<T>::BLOCK_SIZE
        };

        self.cur_base = if block == 0 {
            0
        } else {
            self.block_max(block - 1) + 1
        };

        let block_data = &self.blocks_data[endpoint..];
        let read_bytes;
        (self.docs_buf, read_bytes) = T::decode_monotone(block_data, self.cur_block_size);
        // prefetch freqs base maybe ??

        self.cur_freqs_data = &block_data[read_bytes..];
        self.docs_buf[0] += self.cur_base;
        self.pos_in_block = 0;
        self.cur_block = block;
        self.cur_docid = self.docs_buf[0];
        self.decoded_freqs = false;
    }

    fn decode_freqs_block(&mut self) {
        let (freqs, _read_bytes) = T::decode(self.cur_freqs_data, self.cur_block_size);
        // prefetch next block in some way

        self.freqs_buf = freqs.iter().map(|&f| f + 1).collect();
        self.decoded_freqs = true;
    }

    fn block_max(&self, block: usize) -> u64 {
        let mut buf = [0u8; 4];
        buf.copy_from_slice(&self.block_maxs[block * 4..block * 4 + 4]);
        u32::from_le_bytes(buf) as u64
    }
}

impl<T> PostingListIter for BlockPostingListIter<'_, T>
where
    T: BlockCodec,
{
    fn current_doc(&self) -> u64 {
        self.cur_docid
    }

    fn current_pos(&self) -> usize {
        self.cur_block * BlockPostingList::<T>::BLOCK_SIZE + self.pos_in_block
    }

    fn next_geq(&mut self, lower_bound: u64) {
        if lower_bound > self.cur_block_max {
            if lower_bound > self.block_max(self.n_blocks - 1) {
                self.cur_docid = self.universe;
                return;
            }

            let mut block = self.cur_block + 1;

            while self.block_max(block) < lower_bound {
                block += 1;
            }

            self.decode_docs_block(block);
        }

        while self.current_doc() < lower_bound {
            self.pos_in_block += 1;
            self.cur_docid = self.docs_buf[self.pos_in_block] + self.cur_base;
        }
    }

    fn next_doc(&mut self) {
        self.pos_in_block += 1;
        if self.pos_in_block == self.cur_block_size {
            if self.cur_block + 1 == self.n_blocks {
                self.cur_docid = self.universe;
                return;
            } else {
                self.decode_docs_block(self.cur_block + 1);
            }
        } else {
            self.cur_docid = self.docs_buf[self.pos_in_block] + self.cur_base;
        }
    }

    fn freq(&mut self) -> u64 {
        if !self.decoded_freqs {
            self.decode_freqs_block();
        }
        self.freqs_buf[self.pos_in_block]
    }

    fn len(&self) -> usize {
        self.len
    }
}

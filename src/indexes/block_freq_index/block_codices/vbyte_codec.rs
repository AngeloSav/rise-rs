use dsi_bitstream::{
    codes::{VByteLeRead, VByteLeWrite},
    impls::{BufBitReader, BufBitWriter, MemWordReader, MemWordWriterVec},
    traits::{BitSeek, LE},
};
use epserde::prelude::*;
use mem_dbg::{MemDbg, MemSize};

use crate::indexes::block_freq_index::block_codices::BlockCodec;

#[derive(Clone, Debug, MemSize, MemDbg, Epserde)]
pub struct VbyteCodec;

impl BlockCodec for VbyteCodec {
    fn encode_monotone(data: impl IntoIterator<Item = u64>) -> Vec<u32> {
        // convert to dgaps
        let dgaps = data.into_iter().scan(0, |s, x| {
            let res = x - *s;
            *s = x;
            Some(res)
        });

        Self::encode(dgaps)
    }

    fn encode(data: impl IntoIterator<Item = u64>) -> Vec<u32> {
        let mut encoded = Vec::new();
        let mut writer = BufBitWriter::<LE, _>::new(MemWordWriterVec::<u32, _>::new(&mut encoded));
        for x in data {
            writer.write_vbyte_le(x).expect("error in vbyte encoding");
        }

        drop(writer);
        // cast to u8 vec
        encoded
    }

    fn decode_monotone(data: &[u32], n: usize, out: &mut [u64]) -> usize {
        let mut reader = BufBitReader::<LE, _>::new(MemWordReader::new(data));
        let mut prec = 0;

        for i in 0..n {
            let dgap = reader.read_vbyte_le().expect("error in vbyte decoding");
            out[i] = prec + dgap;
            prec = out[i];
        }

        (reader.bit_pos().unwrap() as usize).div_ceil(32)
    }

    fn decode(data: &[u32], n: usize, out: &mut [u64]) -> usize {
        let mut reader = BufBitReader::<LE, _>::new(MemWordReader::new(data));

        for i in 0..n {
            let x = reader.read_vbyte_le().expect("error in vbyte decoding");
            out[i] = x;
        }

        (reader.bit_pos().unwrap() as usize).div_ceil(32)
    }
}

use epserde::Epserde;
use mem_dbg::{MemDbg, MemSize};

use crate::indexes::block_freq_index::block_codices::{
    BlockCodec,
    streamvbyte::{self, StreamVByte},
};

#[derive(Clone, Debug, MemSize, MemDbg, Epserde)]
pub struct StreamVByteCodec;

impl BlockCodec for StreamVByteCodec {
    fn encode_monotone(data: impl IntoIterator<Item = u32>) -> Vec<u8> {
        // convert to dgaps
        let dgaps = data.into_iter().scan(0, |s, x| {
            let res = x - *s;
            *s = x;
            Some(res)
        });

        Self::encode(dgaps)
    }

    fn encode(data: impl IntoIterator<Item = u32>) -> Vec<u8> {
        let collected = data.into_iter().map(|x| x as u32).collect::<Vec<u32>>();
        let res = streamvbyte::StreamVByte::<u32>::encode_into_vec(&collected);

        res
    }

    fn decode_monotone(data: &[u8], n: usize, out: &mut [u32]) -> usize {
        let read_bytes = Self::decode(data, n, out);

        for i in 1..n {
            out[i] = out[i] + out[i - 1];
        }

        read_bytes
    }

    fn decode(data: &[u8], n: usize, out: &mut [u32]) -> usize {
        let read = StreamVByte::<u32>::decode_from_slice(data, n, out);

        read
    }
}

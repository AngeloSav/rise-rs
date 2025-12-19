use dsi_bitstream::{
    codes::{vbyte::byte_len_vbyte, VByteLeRead, VByteLeWrite},
    impls::{BufBitReader, BufBitWriter, MemWordReader, MemWordWriterVec},
    traits::LE,
};

use crate::indexes::block_freq_index::block_codices::BlockCodec;

pub struct VbyteCodec;

impl BlockCodec for VbyteCodec {
    fn encode_monotone(data: impl IntoIterator<Item = u64>) -> Vec<u8> {
        // convert to dgaps
        let psums = data.into_iter().scan(0, |s, x| {
            let res = x - *s;
            *s = x;
            Some(res)
        });

        Self::encode(psums)
    }

    fn encode(data: impl IntoIterator<Item = u64>) -> Vec<u8> {
        let mut encoded = Vec::new();
        let mut writer = BufBitWriter::<LE, _>::new(MemWordWriterVec::<u8, _>::new(&mut encoded));
        for x in data {
            writer.write_vbyte_le(x).expect("error in vbyte encoding");
        }

        drop(writer);
        // cast to u8 vec
        encoded
    }

    fn decode_monotone(data: &[u8], n: usize) -> (Vec<u64>, usize) {
        let (dgaps, read_bytes) = Self::decode(data, n);
        let psums = dgaps
            .into_iter()
            .scan(0, |s, x| {
                let res = *s + x;
                *s = res;
                Some(res)
            })
            .collect();
        (psums, read_bytes)
    }

    fn decode(data: &[u8], n: usize) -> (Vec<u64>, usize) {
        let mut result = Vec::with_capacity(n);
        let mut read_bytes = 0;
        let mut reader = BufBitReader::<LE, _>::new(MemWordReader::new(data));

        for _ in 0..n {
            let x = reader.read_vbyte_le().expect("error in vbyte decoding");
            result.push(x);
            read_bytes += byte_len_vbyte(x);
        }

        (result, read_bytes)
    }
}

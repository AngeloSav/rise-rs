use crate::{indexes::block_freq_index::block_codices::BlockCodec, utils::msb};
use dsi_bitstream::{
    codes::{VByteLeRead, VByteLeWrite},
    impls::{BufBitReader, BufBitWriter, MemWordWriterVec},
    traits::{BitRead, BitSeek, BitWrite, LE, WordRead, WordSeek},
};
use std::convert::Infallible;
use epserde::Epserde;
use mem_dbg::{MemDbg, MemSize};

/// Reads u32 words from a byte slice without requiring 4-byte alignment.
/// Returns zero past the end (matching MemWordReader<W,B,INF=true> behaviour)
/// so the BufBitReader never panics on partial final words.
struct ByteSliceU32Reader<'a> {
    data: &'a [u8],
    word_index: usize,
}

impl<'a> ByteSliceU32Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, word_index: 0 }
    }
}

impl WordRead for ByteSliceU32Reader<'_> {
    type Error = Infallible;
    type Word = u32;

    #[inline]
    fn read_word(&mut self) -> Result<u32, Infallible> {
        let byte_pos = self.word_index * 4;
        self.word_index += 1;
        let remaining = self.data.len().saturating_sub(byte_pos);
        if remaining == 0 {
            return Ok(0);
        }
        if remaining >= 4 {
            // SAFETY: pointer stays within the slice; read_unaligned handles any alignment
            Ok(unsafe { (self.data.as_ptr().add(byte_pos) as *const u32).read_unaligned() })
        } else {
            let mut buf = [0u8; 4];
            buf[..remaining].copy_from_slice(&self.data[byte_pos..]);
            Ok(u32::from_le_bytes(buf))
        }
    }
}

impl WordSeek for ByteSliceU32Reader<'_> {
    type Error = Infallible;

    #[inline]
    fn word_pos(&mut self) -> Result<u64, Infallible> {
        Ok(self.word_index as u64)
    }

    #[inline]
    fn set_word_pos(&mut self, word_index: u64) -> Result<(), Infallible> {
        self.word_index = word_index as usize;
        Ok(())
    }
}

#[derive(Clone, Debug, MemSize, MemDbg, Epserde)]
pub struct InterpolativeCodec;

impl InterpolativeCodec {
    fn write_int(
        output_writer: &mut BufBitWriter<LE, MemWordWriterVec<u32, &mut Vec<u32>>>,
        value: u64,
        u: u64,
    ) {
        let b = msb(u);
        let m = (1u64 << (b + 1)) - u;

        if value < m {
            output_writer
                .write_bits(value, b as usize)
                .expect("error in interpolative coding");
        } else {
            let value = value + m;

            output_writer
                .write_bits(value >> 1, b as usize)
                .expect("error in interpolative coding");

            output_writer
                .write_bits(value & 1, 1)
                .expect("error in interpolative coding");
        }
    }

    fn read_int(reader: &mut BufBitReader<LE, ByteSliceU32Reader<'_>>, u: u64) -> u64 {
        let b = msb(u);
        let m = (1u64 << (b + 1)) - u;

        let mut value = reader
            .read_bits(b as usize)
            .expect("error in interpolative decoding");

        if value >= m {
            let low_bit = reader
                .read_bits(1)
                .expect("error in interpolative decoding");
            value = (value << 1) + low_bit - m;
        }

        value
    }

    fn encode_monotone_helper(
        data: &[u32],
        output_writer: &mut BufBitWriter<LE, MemWordWriterVec<u32, &mut Vec<u32>>>,
        low: u32,
        high: u32,
        l: u32,
        r: u32,
    ) {
        let m = l + (r - l) / 2;
        let s_m = data[m as usize];

        let min_value = low + (m - l);
        let max_value = high - (r - m);
        let offset = s_m - min_value;

        // println!();
        // dbg!(l, r, low, high, m, s_m, max_value, min_value, offset);

        let range = max_value - min_value + 1;

        Self::write_int(output_writer, offset as u64, range as u64);

        if l < m {
            Self::encode_monotone_helper(data, output_writer, low, s_m - 1, l, m - 1);
        }
        if m < r {
            Self::encode_monotone_helper(data, output_writer, s_m + 1, high, m + 1, r);
        }
    }

    fn decode_monotone_helper(
        reader: &mut BufBitReader<LE, ByteSliceU32Reader<'_>>,
        output: &mut [u32],
        low: u32,
        high: u32,
        l: u32,
        r: u32,
    ) {
        let m = l + (r - l) / 2;
        let min_value = low + (m - l);
        let max_value = high - (r - m);

        let range = max_value - min_value + 1;

        let offset = Self::read_int(reader, range as u64) as u32;

        let s_m = min_value + offset;

        output[m as usize] = s_m;

        if l < m {
            Self::decode_monotone_helper(reader, output, low, s_m - 1, l, m - 1);
        }

        if m < r {
            Self::decode_monotone_helper(reader, output, s_m + 1, high, m + 1, r);
        }
    }
}

impl BlockCodec for InterpolativeCodec {
    fn encode_monotone(data: impl IntoIterator<Item = u32>) -> Vec<u8> {
        let data = data.into_iter().map(|x| x as u32).collect::<Vec<_>>();

        let last = *data.last().unwrap();
        let mut output = Vec::new();
        let mut writer = BufBitWriter::<LE, _>::new(MemWordWriterVec::<u32, _>::new(&mut output));
        writer
            .write_vbyte_le(last as u64)
            .expect("error in vbyte encoding");

        Self::encode_monotone_helper(&data, &mut writer, 0, last, 0, (data.len() - 1) as u32);

        let bits_in_last_word = writer.flush().expect("error flushing interpolative writer");
        drop(writer);

        let total_bits = output.len() * 32 - (32usize.wrapping_sub(bits_in_last_word)) % 32;
        let mut bytes = cast_vecu32_to_vecu8(output);
        bytes.truncate(total_bits.div_ceil(8));
        bytes
    }

    fn encode(data: impl IntoIterator<Item = u32>) -> Vec<u8> {
        let psums = data.into_iter().enumerate().scan(0, |s, (i, x)| {
            // add i because we are encoding strictly increasing sequences
            let res = x + *s as u32 + i as u32;
            *s = x + *s;
            Some(res)
        });

        Self::encode_monotone(psums)
    }

    fn decode_monotone(data: &[u8], n: usize, out: &mut [u32]) -> usize {
        let mut reader = BufBitReader::<LE, _>::new(ByteSliceU32Reader::new(data));

        let last = reader.read_vbyte_le().expect("error in vbyte decoding") as u32;

        Self::decode_monotone_helper(&mut reader, out, 0, last, 0, (n - 1) as u32);

        (reader.bit_pos().unwrap() as usize).div_ceil(8)
    }

    fn decode(data: &[u8], n: usize, out: &mut [u32]) -> usize {
        let read_bytes = Self::decode_monotone(data, n, out);

        // println!("DECODED OUT: {:?}", &out[..n]);

        for i in (1..n).rev() {
            out[i] -= out[i - 1] as u32 + 1;
            // out[i] -= i as u64 - out[i - 1];
        }

        read_bytes
    }
}

// pub fn cast_vecu8_to_vecu32(mut v: Vec<u8>) -> Vec<u32> {
//     v.resize(v.len().div_ceil(4) * 4, 0);

//     let len = v.len() / 4;
//     let capacity = v.capacity() / 4;
//     let ptr = v.as_ptr() as *mut u32;
//     std::mem::forget(v);
//     unsafe { Vec::from_raw_parts(ptr, len, capacity) }
// }

pub fn cast_vecu32_to_vecu8(v: Vec<u32>) -> Vec<u8> {
    let len = v.len() * 4;
    let capacity = v.capacity() * 4;
    let ptr = v.as_ptr() as *mut u8;
    std::mem::forget(v);
    unsafe { Vec::from_raw_parts(ptr, len, capacity) }
}


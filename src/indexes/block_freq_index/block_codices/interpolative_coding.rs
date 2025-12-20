use crate::{indexes::block_freq_index::block_codices::BlockCodec, utils::msb};
use dsi_bitstream::{
    codes::{VByteLeRead, VByteLeWrite},
    impls::{BufBitReader, BufBitWriter, MemWordReader, MemWordWriterVec},
    traits::{BitRead, BitSeek, BitWrite, LE},
};
use epserde::Epserde;
use mem_dbg::{MemDbg, MemSize};

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

    fn read_int(reader: &mut BufBitReader<LE, MemWordReader<u32, &[u32]>>, u: u64) -> u64 {
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
        data: &[u64],
        output_writer: &mut BufBitWriter<LE, MemWordWriterVec<u32, &mut Vec<u32>>>,
        low: u64,
        high: u64,
        l: u64,
        r: u64,
    ) {
        let m = l + (r - l) / 2;
        let s_m = data[m as usize];

        let min_value = low + (m - l);
        let max_value = high - (r - m);
        let offset = s_m - min_value;

        // println!();
        // dbg!(l, r, low, high, m, s_m, max_value, min_value, offset);

        let range = max_value - min_value + 1;

        Self::write_int(output_writer, offset, range);

        if l < m {
            Self::encode_monotone_helper(data, output_writer, low, s_m - 1, l, m - 1);
        }
        if m < r {
            Self::encode_monotone_helper(data, output_writer, s_m + 1, high, m + 1, r);
        }
    }

    fn decode_monotone_helper(
        reader: &mut BufBitReader<LE, MemWordReader<u32, &[u32]>>,
        output: &mut [u64],
        low: u64,
        high: u64,
        l: u64,
        r: u64,
    ) {
        let m = l + (r - l) / 2;
        let min_value = low + (m - l);
        let max_value = high - (r - m);

        let range = max_value - min_value + 1;

        let offset = Self::read_int(reader, range);

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
    fn encode_monotone(data: impl IntoIterator<Item = u64>) -> Vec<u32> {
        let data = data.into_iter().collect::<Vec<u64>>();

        // println!("ENCODING DATA: {:?}", &data);

        let last = *data.last().unwrap();
        let mut output = Vec::new();
        let mut writer = BufBitWriter::<LE, _>::new(MemWordWriterVec::<u32, _>::new(&mut output));
        writer
            .write_vbyte_le(last)
            .expect("error in vbyte encoding");

        Self::encode_monotone_helper(&data, &mut writer, 0, last, 0, (data.len() - 1) as u64);

        drop(writer);
        output
    }
    fn encode(data: impl IntoIterator<Item = u64>) -> Vec<u32> {
        let psums = data.into_iter().enumerate().scan(0, |s, (i, x)| {
            // add i because we are encoding strictly increasing sequences
            let res = x + *s as u64 + i as u64;
            *s = x + *s;
            Some(res)
        });

        Self::encode_monotone(psums)
    }
    fn decode_monotone(data: &[u32], n: usize, out: &mut [u64]) -> usize {
        let mut reader = BufBitReader::<LE, _>::new(MemWordReader::new(data));

        let last = reader.read_vbyte_le().expect("error in vbyte decoding");

        Self::decode_monotone_helper(&mut reader, out, 0, last, 0, (n - 1) as u64);

        let read_bytes = (reader.bit_pos().unwrap() as usize).div_ceil(32);
        read_bytes
    }
    fn decode(data: &[u32], n: usize, out: &mut [u64]) -> usize {
        let read_bytes = Self::decode_monotone(data, n, out);

        // println!("DECODED OUT: {:?}", &out[..n]);

        for i in (1..n).rev() {
            out[i] -= out[i - 1] as u64 + 1;
            // out[i] -= i as u64 - out[i - 1];
        }

        read_bytes
    }
}

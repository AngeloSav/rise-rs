use crate::{indexes::block_freq_index::block_codices::BlockCodec, utils::ceil_log2};
use dsi_bitstream::{
    codes::{VByteLeRead, VByteLeWrite},
    impls::{BufBitReader, BufBitWriter, MemWordReader, MemWordWriterVec},
    traits::{BitRead, BitSeek, BitWrite, LE},
};
use epserde::Epserde;
use mem_dbg::{MemDbg, MemSize};

#[derive(Clone, Debug, MemSize, MemDbg, Epserde)]
pub struct InterpolativeCodec;

/// TODO: we can waste less bits when the range is small ???
impl InterpolativeCodec {
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

        let range = max_value - min_value + 1;
        let bits_needed = ceil_log2(range);

        output_writer
            .write_bits(offset, bits_needed as usize)
            .expect("error in interpolative coding");

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
        let bits_needed = ceil_log2(range);

        let offset = reader
            .read_bits(bits_needed as usize)
            .expect("error in interpolative decoding");
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
        let psums = data.into_iter().scan(0, |s, x| {
            let res = x + *s;
            *s = res;
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
            out[i] -= out[i - 1];
        }

        read_bytes
    }
}

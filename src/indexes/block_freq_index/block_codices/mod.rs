pub mod interpolative_coding;
pub mod streamvbyte_codec;
// pub mod vbyte_codec;

mod streamvbyte;

pub trait BlockCodec {
    /// Encodes a block of monotonically increasing (a_i > a_{i+1}) u64 integers into a byte vector.
    fn encode_monotone(data: impl IntoIterator<Item = u32>) -> Vec<u8>;

    /// Encodes a block of u64 integers into a byte vector.
    fn encode(data: impl IntoIterator<Item = u32>) -> Vec<u8>;

    /// Decodes `n` elements from a byte vector back into a vector of monotonically increasing (a_i > a_{i+1}) u64 integers.
    /// Returns the number of words read
    fn decode_monotone(data: &[u8], n: usize, out: &mut [u32]) -> usize;

    /// Decodes `n` elements from a byte vector back into a vector of u64 integers.
    /// Returns the number of words read
    fn decode(data: &[u8], n: usize, out: &mut [u32]) -> usize;
}

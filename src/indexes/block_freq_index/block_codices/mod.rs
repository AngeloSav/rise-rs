pub mod interpolative_coding;
pub mod vbyte_codec;

pub trait BlockCodec {
    /// Encodes a block of monotonically increasing u64 integers into a byte vector.
    fn encode_monotone(data: impl IntoIterator<Item = u64>) -> Vec<u32>;

    /// Encodes a block of u64 integers into a byte vector.
    fn encode(data: impl IntoIterator<Item = u64>) -> Vec<u32>;

    /// Decodes `n` elements from a byte vector back into a vector of monotonically increasing u64 integers.
    /// Returns the number of words read
    fn decode_monotone(data: &[u32], n: usize, out: &mut [u64]) -> usize;

    /// Decodes `n` elements from a byte vector back into a vector of u64 integers.
    /// Returns the number of words read
    fn decode(data: &[u32], n: usize, out: &mut [u64]) -> usize;
}

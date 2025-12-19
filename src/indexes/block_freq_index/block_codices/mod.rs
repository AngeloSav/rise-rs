pub mod vbyte_codec;

pub trait BlockCodec {
    /// Encodes a block of monotonically increasing u64 integers into a byte vector.
    fn encode_monotone(data: impl IntoIterator<Item = u64>) -> Vec<u8>;

    /// Encodes a block of u64 integers into a byte vector.
    fn encode(data: impl IntoIterator<Item = u64>) -> Vec<u8>;

    /// Decodes `n` elements from a byte vector back into a vector of monotonically increasing u64 integers.
    /// Returns a vector of u64 integers and the number of bytes read
    fn decode_monotone(data: &[u8], n: usize) -> (Vec<u64>, usize);

    /// Decodes `n` elements from a byte vector back into a vector of u64 integers.
    /// Returns a vector of u64 integers and the number of bytes read
    fn decode(data: &[u8], n: usize) -> (Vec<u64>, usize);
}

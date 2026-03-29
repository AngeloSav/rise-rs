#![allow(dead_code)]
//! Stream VByte encoding and decoding implementation.
//!
//! This module provides an efficient implementation of Stream VByte (SVB) compression,
//! a variable-byte encoding scheme that compresses sequences of unsigned integers.
//!
//! # Stream VByte Overview
//!
//! Stream VByte is a compression technique that encodes integers using a variable number
//! of bytes depending on their magnitude. It separates control information from data:
//! - **Control bytes**: Store metadata about how many bytes each value uses
//! - **Data bytes**: Store the actual compressed values
//!
//! This separation allows for efficient SIMD processing and random access capabilities.
//!
//! # Supported Types
//!
//! Currently supports:
//! - `u32` - 4 values encoded per control byte
//! - `u16` - 8 values encoded per control byte
//!
//! # Basic Usage
//!
//! ```ignore
//! use toolkit::stream_vbyte::StreamVByte;
//!
//! // Encode a sequence of integers
//! let data: Vec<u32> = vec![1, 2, 100, 1000, 10000];
//! let encoded = StreamVByte::encode(&data);
//!
//! // Decode the sequence
//! let decoded = encoded.decode();
//! assert_eq!(decoded, data);
//!
//! // Iterate over values without full decompression
//! for value in encoded.iter() {
//!     println!("Value: {}", value);
//! }
//! ```
//!
//! # Random Access
//!
//! For random access to compressed data, use `StreamVByteRandomAccess`:
//!
//! ```ignore
//! use toolkit::stream_vbyte::StreamVByteRandomAccess;
//!
//! let data: Vec<u32> = (0..1000).collect();
//! let ra = StreamVByteRandomAccess::new(&data, 64); // 64 = block size
//!
//! // Access a range without decompressing the entire sequence
//! let mut buffer = vec![0u32; 10];
//! ra.get_range(&mut buffer, 100..110);
//! ```
//!
//! # Performance Notes
//!
//! - Encoding is currently scalar (TODO: SIMD implementation would provide 4-8x speedup)
//! - Decoding uses SIMD where possible for better performance
//! - Best compression for sequences with many small values
//! - Works on sequences of any length (not required to be multiples of 4/8)

use mem_dbg::*;
// use serde::{Deserialize, Serialize};

// TODO: currently encoding is not using explicitly SIMD instructions. It can be made from 4x to 8x faster.

use utils::SVBEncodable;

pub mod utils;

/// Random access wrapper for Stream VByte encoded data.
///
/// This structure enables efficient random access to compressed data by maintaining
/// a block-based index. It divides the encoded sequence into blocks and stores offsets
/// to quickly locate any range without full decompression.
///
/// # Type Parameters
///
/// * `T` - The unsigned integer type being encoded (must implement `SVBEncodable`).
/// Currently supports `u32` and `u16`.
///
/// # Fields
///
/// * `svb` - The underlying Stream VByte encoded data
/// * `offsets` - Byte offsets for each block in the data section
/// * `block_size` - Number of values per block (must be multiple of T::N_CONTROL)
///
/// # Examples
///
/// ```ignore
/// use toolkit::stream_vbyte::StreamVByteRandomAccess;
///
/// let data: Vec<u32> = (0..1000).collect();
/// let ra = StreamVByteRandomAccess::new(&data, 64);
///
/// let mut buffer = vec![0u32; 10];
/// ra.get_range(&mut buffer, 500..510);
/// assert_eq!(buffer, (500..510).collect::<Vec<u32>>());
/// ```
#[derive(Default, Debug, Clone, PartialEq, MemSize, MemDbg)]
pub struct StreamVByteRandomAccess<T: SVBEncodable> {
    svb: StreamVByte<T>,
    offsets: Box<[usize]>,
    block_size: usize, // must be multiple of 4
}

impl<T: SVBEncodable + Default> StreamVByteRandomAccess<T> {
    /// Creates a new random access structure from the input data.
    ///
    /// The data is first encoded using Stream VByte, then divided into blocks
    /// of the specified size. An index of byte offsets is built to enable
    /// efficient random access to any block.
    ///
    /// # Arguments
    ///
    /// * `input` - The slice of values to encode
    /// * `block_size` - Number of values per block (must be multiple of T::N_CONTROL)
    ///
    /// # Panics
    ///
    /// Panics if `block_size` is not a multiple of `T::N_CONTROL` (4 for u32, 8 for u16).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use toolkit::stream_vbyte::StreamVByteRandomAccess;
    ///
    /// let data: Vec<u32> = vec![1, 2, 3, 4, 5, 6, 7, 8];
    /// let ra = StreamVByteRandomAccess::new(&data, 4); // 4 values per block
    /// ```
    pub fn new(input: &[T], block_size: usize) -> Self {
        assert!(
            block_size % T::N_CONTROL == 0,
            "block_size must be multiple of T::N_CONTROL ({})",
            T::N_CONTROL
        );

        let svb = StreamVByte::encode(input);
        let mut offsets = Vec::with_capacity((input.len() + block_size - 1) / block_size);
        offsets.push(0);

        for chunk in svb.control_bytes.chunks(block_size / T::N_CONTROL) {
            let mut offset = *offsets.last().unwrap();
            offset += chunk
                .iter()
                .map(|&control_byte| T::LENGTHS[control_byte as usize] as usize)
                .sum::<usize>();

            offsets.push(offset);
        }

        Self {
            svb,
            offsets: offsets.into_boxed_slice(),
            block_size,
        }
    }

    /// Skips the given range and returns the byte offset in the data section.
    ///
    /// This method efficiently calculates how many bytes to skip in the compressed
    /// data to reach a specific position, using only the control bytes for the
    /// calculation. This is a key optimization for random access.
    ///
    /// # Arguments
    ///
    /// * `range` - The range of values to skip
    ///
    /// # Returns
    ///
    /// The byte offset in the data section after skipping the range.
    ///
    /// # Implementation Details
    ///
    /// - Uses block offsets to quickly jump to the relevant block
    /// - Processes complete control bytes efficiently
    /// - Handles remaining values (< N_CONTROL) bit by bit
    pub fn skip(&self, range: std::ops::Range<usize>) -> usize {
        let mut to_skip = range.end - range.start;

        let block_id = range.start / self.block_size;
        let mut control_bytes_index = range.start / T::N_CONTROL;

        // skip till a multiple of 4 using only control bytes
        let mut offset_in_data = self.offsets[block_id];
        offset_in_data += self.svb.control_bytes
            [control_bytes_index..control_bytes_index + (to_skip / T::N_CONTROL)]
            .iter()
            .map(|&control_byte| T::LENGTHS[control_byte as usize] as usize)
            .sum::<usize>();

        // skip the remaining values (less than T::N_CONTROL)
        control_bytes_index += to_skip / T::N_CONTROL;
        to_skip %= T::N_CONTROL;
        if to_skip > 0 {
            let mod_control_byte = self.svb.control_bytes[control_bytes_index]
                >> ((T::N_CONTROL - to_skip) * T::CONTROL_BITS); // not their correct position but ok for sum
            offset_in_data +=
                T::LENGTHS[mod_control_byte as usize] as usize - (T::N_CONTROL - to_skip);
        }

        offset_in_data
    }

    /// Decodes a specific range of values into the provided buffer.
    ///
    /// This is the main random access operation. It efficiently decodes only
    /// the requested range without decompressing the entire sequence.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Output buffer to write decoded values (must be large enough)
    /// * `range` - The range of indices to decode
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - `range.start >= range.end`
    /// - `range.end > self.svb.size`
    /// - `buffer.len() < range.len()`
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use toolkit::stream_vbyte::StreamVByteRandomAccess;
    ///
    /// let data: Vec<u32> = (0..100).collect();
    /// let ra = StreamVByteRandomAccess::new(&data, 16);
    ///
    /// let mut buffer = vec![0u32; 20];
    /// ra.get_range(&mut buffer, 40..60);
    /// assert_eq!(buffer, (40..60).collect::<Vec<u32>>());
    /// ```
    pub fn get_range(&self, buffer: &mut [T], range: std::ops::Range<usize>) {
        assert!(
            range.start < range.end && range.end <= self.svb.size,
            "Invalid range",
        );

        assert!(
            buffer.len() >= range.len(),
            "Output buffer is not large enough"
        );

        let block_id = range.start / self.block_size;

        // Skip the beginning of the block to reach the start of the range
        let mut offset_in_data = self.skip(block_id * self.block_size..range.start);

        let length = range.len().min(self.svb.size - range.start);

        // Handle the case where the range does not start at a multiple of 4
        let to_skip = range.start % T::N_CONTROL;
        let mut cur_position = 0;
        if to_skip > 0 {
            // Decode the first (partial) control byte
            let control_byte =
                self.svb.control_bytes[range.start / T::N_CONTROL] << (to_skip * T::CONTROL_BITS); // shift to ignore the already skipped values
            offset_in_data += T::decode_control_byte(
                (T::N_CONTROL - to_skip).min(length),
                control_byte,
                &self.svb.data[offset_in_data..],
                buffer,
            ); // .min() is to deal with the special case where the range is within the same control byte. In that case we decode only the required values from the first control byte.

            cur_position = T::N_CONTROL - to_skip;
            if cur_position >= length {
                return;
            }
        }
        let control_bytes_range = range.start.div_ceil(T::N_CONTROL)..range.end / T::N_CONTROL; // end without potentially partial last control byte

        if !control_bytes_range.is_empty() {
            offset_in_data += utils::decode_slice_aligned(
                &mut buffer[cur_position..],
                &self.svb.control_bytes[control_bytes_range.clone()],
                &self.svb.data[offset_in_data..],
            );
            cur_position += control_bytes_range.len() * T::N_CONTROL;
        }

        let left_in_last_control_byte = range.end % T::N_CONTROL;
        if left_in_last_control_byte > 0 {
            let control_byte = self.svb.control_bytes[control_bytes_range.end];
            let _ = T::decode_control_byte(
                left_in_last_control_byte,
                control_byte,
                &self.svb.data[offset_in_data..],
                &mut buffer[cur_position..],
            );
        }
    }
}

/// Core Stream VByte encoding/decoding structure.
///
/// This structure stores compressed integer sequences using the Stream VByte format,
/// which separates control information from data for efficient processing.
///
/// # Type Parameters
///
/// * `T` - The unsigned integer type being encoded (must implement `SVBEncodable`)
///
/// # Fields
///
/// * `control_bytes` - Control information indicating byte length for each value
/// * `data` - The compressed data bytes
/// * `size` - Number of original values in the sequence
/// * `_marker` - Zero-sized type marker for the generic parameter
///
/// # Format Details
///
/// For `u32` (N_CONTROL = 4):
/// - Each control byte encodes 4 values using 2 bits per value
/// - 2 bits indicate byte length: 00=1 byte, 01=2 bytes, 10=3 bytes, 11=4 bytes
///
/// For `u16` (N_CONTROL = 8):
/// - Each control byte encodes 8 values using 1 bit per value
/// - 1 bit indicates byte length: 0=1 byte, 1=2 bytes
///
/// # Examples
///
/// ```ignore
/// use toolkit::stream_vbyte::StreamVByte;
///
/// let data = vec![1u32, 100, 10000, 1000000];
/// let encoded = StreamVByte::encode(&data);
///
/// assert_eq!(encoded.len(), 4);
/// assert!(!encoded.is_empty());
///
/// let decoded = encoded.decode();
/// assert_eq!(decoded, data);
/// ```
#[derive(Default, Debug, Clone, PartialEq, MemSize, MemDbg)]
pub struct StreamVByte<T: SVBEncodable> {
    pub control_bytes: Box<[u8]>,
    pub data: Box<[u8]>,
    pub size: usize,
    pub _marker: std::marker::PhantomData<T>,
}

impl<T: SVBEncodable + Default> StreamVByte<T> {
    /// Encodes a slice of values using Stream VByte compression.
    ///
    /// This method compresses the input by encoding each value with the minimum
    /// number of bytes needed. Values are processed in chunks of T::N_CONTROL.
    ///
    /// # Arguments
    ///
    /// * `data` - The slice of values to encode
    ///
    /// # Returns
    ///
    /// A new `StreamVByte` instance containing the compressed data.
    ///
    /// # Padding
    ///
    /// The encoded data includes padding (3 control bytes + 15 data bytes) to
    /// support SIMD decoding operations that process 16 values at a time.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use toolkit::stream_vbyte::StreamVByte;
    ///
    /// let data = vec![1u32, 2, 3, 4, 5];
    /// let encoded = StreamVByte::encode(&data);
    /// assert_eq!(encoded.len(), 5);
    /// ```
    pub fn encode(data: &[T]) -> Self {
        if data.is_empty() {
            return Self::default();
        }
        let mut control_bytes = Vec::with_capacity(data.len().div_ceil(T::N_CONTROL));
        let mut encoded_data = Vec::with_capacity(data.len() * std::mem::size_of::<T>());

        let mut buffer = vec![0u8; std::mem::size_of::<T>() * T::N_CONTROL];
        for chunk in data.chunks(T::N_CONTROL) {
            let mut control_byte: u8 = 0;
            let encoded_length = T::encode_control_byte(chunk, &mut control_byte, &mut buffer);
            encoded_data.extend_from_slice(&buffer[..encoded_length]);
            control_bytes.push(control_byte);
        }

        // Iterator implementation decode 16 values at a time, for this reason we need to pad
        // with 3 control bytes set to 0 and 15 data bytes set to 0.
        // TODO: Pad with the minimum number of bytes, maybe there are better ways to estimate
        // the required padding
        control_bytes.extend_from_slice(&[0u8; 3]);
        encoded_data.extend_from_slice(&[0u8; 15]);

        Self {
            control_bytes: control_bytes.into_boxed_slice(),
            data: encoded_data.into_boxed_slice(),
            size: data.len(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Decodes the compressed data back to the original sequence.
    ///
    /// This method fully decompresses the Stream VByte encoded data and returns
    /// a vector containing all original values.
    ///
    /// # Returns
    ///
    /// A vector containing the decoded values.
    ///
    /// # Performance
    ///
    /// Uses SIMD-optimized decoding where available for better performance.
    /// For partial access, consider using `iter()` or `StreamVByteRandomAccess`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use toolkit::stream_vbyte::StreamVByte;
    ///
    /// let original = vec![1u32, 100, 10000];
    /// let encoded = StreamVByte::encode(&original);
    /// let decoded = encoded.decode();
    /// assert_eq!(decoded, original);
    /// ```
    pub fn decode(&self) -> Vec<T> {
        if self.size == 0 {
            return Vec::new();
        }
        let mut output = vec![T::default(); self.size];
        let control_end = self.size / T::N_CONTROL; // self.control_bytes.len() cannot be used because of padding 

        let encoded_data_index = utils::decode_slice_aligned(
            &mut output,
            &self.control_bytes[..control_end],
            &self.data,
        );

        let last_control_byte = if self.size % T::N_CONTROL == 0 {
            0
        } else {
            self.control_bytes[control_end]
        };

        // !!!
        let _encoded_data_index = T::decode_control_byte(
            self.size % T::N_CONTROL,
            last_control_byte,
            &self.data[encoded_data_index..],
            &mut output[control_end * T::N_CONTROL..],
        );

        output
    }

    pub fn encode_into_vec(data: &[T]) -> Vec<u8> {
        if data.is_empty() {
            return Vec::new();
        }

        let mut encoded = Vec::new();

        let mut control_bytes = Vec::with_capacity(data.len().div_ceil(T::N_CONTROL));
        let mut encoded_data = Vec::with_capacity(data.len() * std::mem::size_of::<T>());

        let mut buffer = vec![0u8; std::mem::size_of::<T>() * T::N_CONTROL];
        let mut prev_encoded_len = 0;
        let mut last_encoded_len = 0;
        for chunk in data.chunks(T::N_CONTROL) {
            let mut control_byte: u8 = 0;
            let encoded_length = T::encode_control_byte(chunk, &mut control_byte, &mut buffer);
            encoded_data.extend_from_slice(&buffer[..encoded_length]);
            control_bytes.push(control_byte);
            prev_encoded_len = last_encoded_len;
            last_encoded_len = encoded_length;
        }

        let n_complete = data.len() / T::N_CONTROL;
        let padding = if n_complete == 0 {
            0 // no complete groups -> no padding needed
        } else {
            let (last_complete_len, partial_len) = if data.len() % T::N_CONTROL == 0 {
                (last_encoded_len, 0)
            } else {
                (prev_encoded_len, last_encoded_len)
            };
            16usize.saturating_sub(last_complete_len + partial_len)
        };

        encoded.extend(control_bytes);
        encoded.extend(encoded_data);
        encoded.extend(std::iter::repeat(0u8).take(padding));

        encoded
    }

    pub fn decode_from_slice(encoded: &[u8], n: usize, output: &mut [T]) -> usize {
        if n == 0 {
            return 0;
        }

        let control_end = n / T::N_CONTROL; // self.control_bytes.len() cannot be used because of padding 
        let data_start = n.div_ceil(T::N_CONTROL);

        let control_bits_slice = &encoded[..data_start];
        let data_slice = &encoded[data_start..];

        // println!("control_end: {}", control_end);

        let mut read_bytes = 0;

        let complete_bytes =
            utils::decode_slice_aligned(output, &control_bits_slice[..control_end], &data_slice);

        read_bytes += complete_bytes;

        let last_control_byte = if n % T::N_CONTROL == 0 {
            0
        } else {
            control_bits_slice[control_end]
        };

        let partial_bytes = T::decode_control_byte(
            n % T::N_CONTROL,
            last_control_byte,
            &data_slice[complete_bytes..],
            &mut output[control_end * T::N_CONTROL..],
        );

        read_bytes += partial_bytes;

        let padding = if control_end == 0 {
            0
        } else {
            let last_complete_len = Self::control_byte_length(control_bits_slice[control_end - 1]);
            let partial_len = if n % T::N_CONTROL == 0 {
                0
            } else {
                partial_bytes
            };
            16usize.saturating_sub(last_complete_len + partial_len)
        };

        read_bytes + data_start + padding
    }

    /// Returns the total byte length for values encoded in a control byte.
    ///
    /// Each control byte encodes information for multiple values. This method
    /// calculates the total number of data bytes used by those values.
    ///
    /// # Arguments
    ///
    /// * `control_byte` - The control byte to analyze
    ///
    /// # Returns
    ///
    /// The total number of bytes in the data section for this control byte.
    ///
    /// # TODO
    ///
    /// Make it faster with table lookup.
    pub fn control_byte_length(control_byte: u8) -> usize {
        T::LENGTHS[control_byte as usize] as usize
    }

    /// Returns the number of values in the compressed sequence.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use toolkit::stream_vbyte::StreamVByte;
    ///
    /// let data = vec![1u32, 2, 3, 4, 5];
    /// let encoded = StreamVByte::encode(&data);
    /// assert_eq!(encoded.len(), 5);
    /// ```
    pub fn len(&self) -> usize {
        self.size
    }

    /// Returns `true` if the sequence contains no values.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use toolkit::stream_vbyte::StreamVByte;
    ///
    /// let empty: Vec<u32> = vec![];
    /// let encoded = StreamVByte::encode(&empty);
    /// assert!(encoded.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Returns an iterator over the compressed values.
    ///
    /// The iterator decodes values lazily in batches of 16 for efficiency.
    /// This is more memory-efficient than `decode()` for large sequences
    /// when you don't need all values at once.
    ///
    /// # Returns
    ///
    /// A `StreamVByteIter` that yields decoded values one at a time.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use toolkit::stream_vbyte::StreamVByte;
    ///
    /// let data = vec![1u32, 2, 3, 4, 5];
    /// let encoded = StreamVByte::encode(&data);
    ///
    /// let sum: u32 = encoded.iter().sum();
    /// assert_eq!(sum, 15);
    /// ```
    pub fn iter(&self) -> StreamVByteIter<'_, T> {
        StreamVByteIter::new(&self.control_bytes, &self.data, self.size)
    }
}

/// Buffer size for batch decoding in the iterator (must match SIMD width).
const BUFFER_SIZE: usize = 16;

/// Iterator for Stream VByte encoded sequences.
///
/// This iterator decodes values lazily in batches of 16 (BUFFER_SIZE) for efficiency.
/// It maintains an internal buffer and decodes multiple values at once using optimized
/// SIMD operations when available.
///
/// # Type Parameters
///
/// * `T` - The unsigned integer type being decoded (must implement `SVBEncodable`)
///
/// # Performance
///
/// The iterator processes values in batches of 16, which aligns with SIMD operations
/// for better performance. Individual values are then yielded from the buffer one at a time.
///
/// # Examples
///
/// ```ignore
/// use toolkit::stream_vbyte::StreamVByte;
///
/// let data = vec![1u32, 2, 3, 4, 5];
/// let encoded = StreamVByte::encode(&data);
///
/// for (i, value) in encoded.iter().enumerate() {
///     assert_eq!(value, data[i]);
/// }
/// ```
pub struct StreamVByteIter<'a, T: SVBEncodable> {
    control_bytes: &'a [u8],
    data: &'a [u8],
    size: usize,
    control_index: usize,
    data_index: usize,
    buffer: [T; BUFFER_SIZE],
    buffer_index: usize,
    position: usize,
}

impl<'a, T: SVBEncodable + Default> StreamVByteIter<'a, T> {
    /// Creates a new iterator for Stream VByte encoded data.
    ///
    /// # Arguments
    ///
    /// * `control_bytes` - Slice of control bytes from the encoded data
    /// * `data` - Slice of compressed data bytes
    /// * `size` - Total number of values in the sequence
    ///
    /// # Returns
    ///
    /// A new iterator positioned at the start of the sequence.
    fn new(control_bytes: &'a [u8], data: &'a [u8], size: usize) -> Self {
        Self {
            control_bytes,
            data,
            size,
            control_index: 0,
            data_index: 0,
            buffer: [T::default(); BUFFER_SIZE],
            buffer_index: BUFFER_SIZE, // Start with empty buffer (index points past last element)
            position: 0,
        }
    }

    /// Fills the internal buffer with up to BUFFER_SIZE decoded values.
    ///
    /// This method decodes a batch of 16 values at once using optimized decoding
    /// routines. It's called automatically by the iterator when the buffer is empty.
    ///
    /// # Implementation Details
    ///
    /// - Uses unsafe code for performance (bounds checks are elided)
    /// - Decodes exactly 16 values per call (padding ensures this is safe)
    /// - Updates control_index and data_index to track position in encoded data
    fn fill_buffer(&mut self) {
        if self.position >= self.size {
            return;
        }

        unsafe {
            self.data_index += utils::decode_16_aligned(
                &mut self.buffer,
                &self.control_bytes.get_unchecked(self.control_index..),
                &self.data.get_unchecked(self.data_index..),
            );
        }

        self.buffer_index = 0;
        self.control_index += 16 / T::LANES;
    }
}

impl<'a, T: SVBEncodable + Default> Iterator for StreamVByteIter<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        // If buffer is empty or we've consumed all values in current buffer, fill it
        if self.buffer_index >= BUFFER_SIZE {
            self.fill_buffer();
        }

        // Return the next value from the buffer if available
        if self.position < self.size {
            let value = unsafe { *self.buffer.get_unchecked(self.buffer_index) };
            self.buffer_index += 1;
            self.position += 1;
            return Some(value);
        }

        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.size - self.position;
        (remaining, Some(remaining))
    }
}

impl<'a, T: SVBEncodable + Default> ExactSizeIterator for StreamVByteIter<'a, T> {
    fn len(&self) -> usize {
        self.size - self.position
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============ BASIC FUNCTIONALITY TESTS ============

    #[test]
    fn test_empty_sequence() {
        let data: Vec<u32> = vec![];
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
        assert_eq!(encoded.len(), 0);
        assert!(encoded.is_empty());
    }

    #[test]
    fn test_empty_sequence_u16() {
        let data: Vec<u16> = vec![];
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
        assert_eq!(encoded.len(), 0);
        assert!(encoded.is_empty());
    }

    #[test]
    fn test_single_element() {
        let data: Vec<u32> = vec![42];
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
        assert_eq!(encoded.len(), 1);
        assert!(!encoded.is_empty());
    }

    #[test]
    fn test_single_element_u16() {
        let data: Vec<u16> = vec![42];
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
        assert_eq!(encoded.len(), 1);
        assert!(!encoded.is_empty());
    }

    #[test]
    fn test_basic_sequence() {
        let data: Vec<u32> = vec![1, 2, 3, 4, 5, 100, 1000, 10000];
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(encoded.control_bytes.len(), (data.len() + 3) / 4 + 3); // +3 for padding
        assert_eq!(decoded, data);
        assert_eq!(encoded.len(), data.len());
        assert!(!encoded.is_empty());
    }

    #[test]
    fn test_basic_sequence_u16() {
        let data: Vec<u16> = vec![1, 2, 3, 4, 5, 100, 1000, 10000];
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(encoded.control_bytes.len(), (data.len() + 7) / 8 + 3); // +3 for padding, 8 values per control byte for u16
        assert_eq!(decoded, data);
        assert_eq!(encoded.len(), data.len());
        assert!(!encoded.is_empty());
    }

    // ============ SIZE VARIATION TESTS ============

    #[test]
    fn test_exact_multiples_of_4() {
        // Test sequences that are exact multiples of 4 elements
        for &size in &[4, 8, 12, 16, 20, 100, 1000] {
            let data: Vec<u32> = (0..size).collect();
            let encoded = StreamVByte::encode(&data);
            let decoded = encoded.decode();

            assert_eq!(decoded, data, "Failed for size {}", size);
            assert_eq!(encoded.len(), data.len());
        }
    }

    #[test]
    fn test_exact_multiples_of_8_u16() {
        // Test sequences that are exact multiples of 8 elements (for u16)
        for &size in &[8, 16, 24, 32, 40, 100, 1000] {
            let data: Vec<u16> = (0..size).collect();
            let encoded = StreamVByte::encode(&data);
            let decoded = encoded.decode();

            assert_eq!(decoded, data, "Failed for size {}", size);
            assert_eq!(encoded.len(), data.len());
        }
    }

    #[test]
    fn test_non_multiples_of_4() {
        // Test sequences that are NOT multiples of 4 elements
        for &size in &[1, 2, 3, 5, 6, 7, 9, 10, 11, 13, 14, 15, 17, 101, 1001] {
            let data: Vec<u32> = (0..size).collect();
            let encoded = StreamVByte::encode(&data);
            let decoded = encoded.decode();

            assert_eq!(decoded, data, "Failed for size {}", size);
            assert_eq!(encoded.len(), data.len());
        }
    }

    #[test]
    fn test_non_multiples_of_8_u16() {
        // Test sequences that are NOT multiples of 8 elements (for u16)
        for &size in &[
            1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14, 15, 17, 101, 1001,
        ] {
            let data: Vec<u16> = (0..size).collect();
            let encoded = StreamVByte::encode(&data);
            let decoded = encoded.decode();

            assert_eq!(decoded, data, "Failed for size {}", size);
            assert_eq!(encoded.len(), data.len());
        }
    }

    // ============ VALUE RANGE TESTS ============

    #[test]
    fn test_small_values_1_byte() {
        // Test with values that fit in 1 byte (0-255)
        let data: Vec<u32> = (0..256).collect();
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
    }

    #[test]
    fn test_small_values_1_byte_u16() {
        // Test with values that fit in 1 byte (0-255)
        let data: Vec<u16> = (0..256).map(|x| x as u16).collect();
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
    }

    #[test]
    fn test_medium_values_2_bytes() {
        // Test with values that need 2 bytes (256-65535)
        let data: Vec<u32> = (256..1024).collect(); // 768 values needing 2 bytes each
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
        // Each value should take exactly 2 bytes in data (plus padding)
        assert_eq!(encoded.data.len(), data.len() * 2 + 15);
    }

    #[test]
    fn test_medium_values_2_bytes_u16() {
        // Test with values that need 2 bytes (256-65535)
        let data: Vec<u16> = (256..1024).map(|x| x as u16).collect(); // 768 values needing 2 bytes each
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
        // Each value should take exactly 2 bytes in data (plus padding)
        assert_eq!(encoded.data.len(), data.len() * 2 + 15);
    }

    #[test]
    fn test_large_values_3_bytes() {
        // Test with values that need 3 bytes (65536-16777215)
        let data: Vec<u32> = vec![65536, 100000, 1000000, 16777215];
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
        // Each value should take exactly 3 bytes in data (plus padding)
        assert_eq!(encoded.data.len(), data.len() * 3 + 15);
    }

    #[test]
    fn test_max_values_4_bytes() {
        // Test with values that need 4 bytes (16777216-u32::MAX)
        let data: Vec<u32> = vec![16777216, u32::MAX - 1, u32::MAX];
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
        // Each value should take exactly 4 bytes in data (plus padding)
        assert_eq!(encoded.data.len(), data.len() * 4 + 15);
    }

    #[test]
    fn test_max_values_u16() {
        // Test with u16 max values (need 2 bytes)
        let data: Vec<u16> = vec![32768, 65534, 65535, u16::MAX];
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
        // Each value should take exactly 2 bytes in data (plus padding)
        assert_eq!(encoded.data.len(), data.len() * 2 + 15);
    }

    #[test]
    fn test_mixed_value_sizes() {
        // Test with a mix of 1, 2, 3, and 4 byte values
        let data: Vec<u32> = vec![
            // 1 byte values
            0,
            1,
            127,
            255,
            // 2 byte values
            256,
            1000,
            32767,
            65535,
            // 3 byte values
            65536,
            100000,
            1000000,
            16777215,
            // 4 byte values
            16777216,
            2000000000,
            u32::MAX - 1,
            u32::MAX,
        ];
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
        // Total data size should be: 4*1 + 4*2 + 4*3 + 4*4 = 40 bytes
        assert_eq!(encoded.data.len(), 40 + 15); // +15 for padding
    }

    #[test]
    fn test_mixed_value_sizes_u16() {
        // Test with a mix of 1 and 2 byte values for u16
        let data: Vec<u16> = vec![
            // 1 byte values
            0,
            1,
            127,
            255,
            // 2 byte values
            256,
            1000,
            32767,
            65535,
            // More 1 byte values
            50,
            100,
            200,
            254,
            // More 2 byte values
            512,
            10000,
            60000,
            u16::MAX,
        ];
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
        // Total data size should be: 8*1 + 8*2 = 24 bytes
        assert_eq!(encoded.data.len(), 24 + 15); // +15 for padding
    }

    // ============ BOUNDARY VALUE TESTS ============

    #[test]
    fn test_byte_boundaries() {
        // Test specific boundary values for each byte length
        let data: Vec<u32> = vec![
            // 1-byte boundaries
            0,
            1,
            127,
            128,
            254,
            255,
            // 2-byte boundaries
            256,
            257,
            32767,
            32768,
            65534,
            65535,
            // 3-byte boundaries
            65536,
            65537,
            8388607,
            8388608,
            16777214,
            16777215,
            // 4-byte boundaries
            16777216,
            16777217,
            u32::MAX - 1,
            u32::MAX,
        ];
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
    }

    #[test]
    fn test_powers_of_two() {
        // Test all powers of 2 that fit in u32
        let data: Vec<u32> = (0..32).map(|i| 1u32 << i).collect();
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
    }

    #[test]
    fn test_powers_of_two_minus_one() {
        // Test values like 2^n - 1 (all bits set in lower positions)
        let data: Vec<u32> = (1..32).map(|i| (1u32 << i) - 1).collect();
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
    }

    // ============ PATTERN TESTS ============

    #[test]
    fn test_repeated_values() {
        // Test sequences with many repeated values
        let mut data: Vec<u32> = Vec::new();

        // Many small values
        for _ in 0..100 {
            data.push(42);
        }

        // Many medium values
        for _ in 0..100 {
            data.push(12345);
        }

        // Many large values
        for _ in 0..100 {
            data.push(0x12345678);
        }

        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
        assert_eq!(encoded.len(), data.len());
    }

    #[test]
    fn test_repeated_values_u16() {
        // Test sequences with many repeated values (u16)
        let mut data: Vec<u16> = Vec::new();

        // Many small values (1 byte)
        for _ in 0..100 {
            data.push(42);
        }

        // Many medium values (2 bytes)
        for _ in 0..100 {
            data.push(12345);
        }

        // Many max values (2 bytes)
        for _ in 0..100 {
            data.push(65535);
        }

        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
        assert_eq!(encoded.len(), data.len());
    }

    #[test]
    fn test_ascending_sequence() {
        let data: Vec<u32> = (0..1000).collect();
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
        assert_eq!(encoded.len(), data.len());
    }

    #[test]
    fn test_ascending_sequence_u16() {
        let data: Vec<u16> = (0..1000).map(|x| x as u16).collect();
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
        assert_eq!(encoded.len(), data.len());
    }

    #[test]
    fn test_descending_sequence() {
        let data: Vec<u32> = (0..1000).rev().collect();
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
        assert_eq!(encoded.len(), data.len());
    }

    #[test]
    fn test_alternating_small_large() {
        // Alternating between small and large values
        let mut data: Vec<u32> = Vec::new();
        for i in 0..1000 {
            if i % 2 == 0 {
                data.push(i % 256); // Small value (1 byte)
            } else {
                data.push(0xFF000000 + (i % 256)); // Large value (4 bytes)
            }
        }

        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
        assert_eq!(encoded.len(), data.len());
    }

    #[test]
    fn test_fibonacci_sequence() {
        // Test Fibonacci numbers (common in real data)
        let mut data: Vec<u32> = vec![0, 1];
        while data.len() < 50 {
            let next = data[data.len() - 1].saturating_add(data[data.len() - 2]);
            if next == data[data.len() - 1] {
                // Overflow detection
                break;
            }
            data.push(next);
        }

        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
        assert_eq!(encoded.len(), data.len());
    }

    // ============ LARGE SEQUENCE TESTS ============

    #[test]
    fn test_large_sequences() {
        // Test with large sequences to ensure scalability
        for &size in &[10000, 50000, 100000] {
            let data: Vec<u32> = (0..size).map(|i| (i * 17 + 13) % 65536).collect();
            let encoded = StreamVByte::encode(&data);
            let decoded = encoded.decode();

            assert_eq!(decoded, data, "Failed for large size {}", size);
            assert_eq!(encoded.len(), data.len());
        }
    }

    #[test]
    fn test_compression_effectiveness() {
        // Test that compression actually reduces size for appropriate data
        let small_values: Vec<u32> = (0..10000).map(|i| i % 256).collect(); // All 1-byte values
        let encoded = StreamVByte::encode(&small_values);
        let original_size = small_values.len() * 4; // 4 bytes per u32
        let compressed_size = encoded.control_bytes.len() + encoded.data.len();

        // Should achieve significant compression for small values
        assert!(compressed_size < original_size);
        assert!(compressed_size < original_size / 2); // At least 2x compression

        let decoded = encoded.decode();
        assert_eq!(decoded, small_values);
    }

    // ============ STRESS TESTS ============

    #[test]
    fn test_all_zeros() {
        let data: Vec<u32> = vec![0; 10000];
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
    }

    #[test]
    fn test_all_zeros_u16() {
        let data: Vec<u16> = vec![0; 10000];
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
    }

    #[test]
    fn test_all_max_values() {
        let data: Vec<u32> = vec![u32::MAX; 1000];
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
        // All max values should take maximum space (4 bytes each) + 15 for padding
        assert_eq!(encoded.data.len(), data.len() * 4 + 15);
    }

    #[test]
    fn test_all_max_values_u16() {
        let data: Vec<u16> = vec![u16::MAX; 1000];
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
        // All max values should take maximum space (2 bytes each) + 15 for padding
        assert_eq!(encoded.data.len(), data.len() * 2 + 15);
    }

    #[test]
    fn test_random_like_sequence() {
        // Generate a pseudo-random sequence using a simple PRNG
        let mut data: Vec<u32> = Vec::new();
        let mut state = 12345u64; // Simple seed

        for _ in 0..10000 {
            // Simple linear congruential generator
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            data.push((state >> 16) as u32 & 0xFFFF); // Use upper bits, limit to 16-bit values
        }

        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
        assert_eq!(encoded.len(), data.len());
    }

    // ============ CONTROL BYTE TESTS ============

    #[test]
    fn test_control_byte_patterns() {
        // Test all possible combinations of 1-4 byte lengths within a single control byte
        let test_cases = vec![
            // All 1-byte values (control = 0b00000000)
            vec![1, 2, 3, 4],
            // All 2-byte values (control = 0b01010101)
            vec![256, 512, 1024, 2048],
            // All 3-byte values (control = 0b10101010)
            vec![65536, 131072, 262144, 524288],
            // All 4-byte values (control = 0b11111111)
            vec![16777216, 33554432, 67108864, 134217728],
            // Mixed: 1,2,3,4 bytes (control = 0b11100100)
            vec![255, 65535, 16777215, u32::MAX],
        ];

        for data in test_cases {
            let encoded = StreamVByte::encode(&data);
            let decoded = encoded.decode();
            assert_eq!(decoded, data, "Failed for pattern: {:?}", data);
        }
    }

    #[test]
    fn test_partial_control_bytes() {
        // Test sequences that don't fill complete control bytes (not multiples of 4)
        let test_cases = vec![
            vec![1_u32],               // 1 element
            vec![1, 2],                // 2 elements
            vec![1, 2, 3],             // 3 elements
            vec![1, 2, 3, 4, 5],       // 5 elements (1 full + 1 partial control byte)
            vec![1, 2, 3, 4, 5, 6],    // 6 elements
            vec![1, 2, 3, 4, 5, 6, 7], // 7 elements
        ];

        for data in test_cases {
            let encoded = StreamVByte::encode(&data);
            let decoded = encoded.decode();
            assert_eq!(decoded, data, "Failed for partial sequence: {:?}", data);
            assert_eq!(encoded.len(), data.len());
        }
    }

    // ============ EDGE CASE TESTS ============

    #[test]
    fn test_single_large_value() {
        let data: Vec<u32> = vec![u32::MAX];
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
        assert_eq!(encoded.len(), 1);
    }

    #[test]
    fn test_alternating_min_max() {
        let data: Vec<u32> = vec![0, u32::MAX, 0, u32::MAX, 0, u32::MAX];
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
    }

    #[test]
    fn test_bit_patterns() {
        // Test various bit patterns that might reveal encoding issues
        let data: Vec<u32> = vec![
            0x00000000, // All zeros
            0x00000001, // Single bit set
            0x80000000, // MSB set
            0x55555555, // Alternating bits
            0xAAAAAAAA, // Alternating bits (inverse)
            0x0F0F0F0F, // Nibble pattern
            0xF0F0F0F0, // Nibble pattern (inverse)
            0x00FF00FF, // Byte pattern
            0xFF00FF00, // Byte pattern (inverse)
            0xFFFFFFFF, // All ones
        ];

        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
    }

    // ============ PERFORMANCE REGRESSION TESTS ============

    #[test]
    fn test_worst_case_compression() {
        // Create data that should compress poorly (all 4-byte values)
        let data: Vec<u32> = (0..1000).map(|i| 0xFF000000 + i).collect();
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
        // Should still be correct even if compression is poor
        assert_eq!(encoded.len(), data.len());
    }

    #[test]
    fn test_best_case_compression() {
        // Create data that should compress very well (all small values)
        let data: Vec<u32> = vec![1; 10000]; // All identical small values
        let encoded = StreamVByte::encode(&data);
        let decoded = encoded.decode();

        assert_eq!(decoded, data);
        assert_eq!(encoded.len(), data.len());
    }

    // ============ ITERATOR TESTS ============

    #[test]
    fn test_iterator_empty() {
        let data: Vec<u32> = vec![];
        let encoded = StreamVByte::encode(&data);
        let iter_result: Vec<u32> = encoded.iter().collect();

        assert_eq!(iter_result, data);
        assert_eq!(encoded.iter().len(), 0);
        assert_eq!(encoded.iter().size_hint(), (0, Some(0)));
    }

    #[test]
    fn test_iterator_empty_u16() {
        let data: Vec<u16> = vec![];
        let encoded = StreamVByte::encode(&data);
        let iter_result: Vec<u16> = encoded.iter().collect();

        assert_eq!(iter_result, data);
        assert_eq!(encoded.iter().len(), 0);
        assert_eq!(encoded.iter().size_hint(), (0, Some(0)));
    }

    #[test]
    fn test_iterator_single_element() {
        let data: Vec<u32> = vec![42];
        let encoded = StreamVByte::encode(&data);
        let iter_result: Vec<u32> = encoded.iter().collect();

        assert_eq!(iter_result, data);
        assert_eq!(encoded.iter().len(), 1);
        assert_eq!(encoded.iter().size_hint(), (1, Some(1)));
    }

    #[test]
    fn test_iterator_single_element_u16() {
        let data: Vec<u16> = vec![42];
        let encoded = StreamVByte::encode(&data);
        let iter_result: Vec<u16> = encoded.iter().collect();

        assert_eq!(iter_result, data);
        assert_eq!(encoded.iter().len(), 1);
        assert_eq!(encoded.iter().size_hint(), (1, Some(1)));
    }

    #[test]
    fn test_iterator_basic_sequence() {
        let data: Vec<u32> = vec![1, 2, 3, 4, 5, 100, 1000, 10000];
        let encoded = StreamVByte::encode(&data);
        let iter_result: Vec<u32> = encoded.iter().collect();

        assert_eq!(iter_result, data);
        assert_eq!(encoded.iter().len(), data.len());
        assert_eq!(encoded.iter().size_hint(), (data.len(), Some(data.len())));
    }

    #[test]
    fn test_iterator_basic_sequence_u16() {
        let data: Vec<u16> = vec![1, 2, 3, 4, 5, 100, 1000, 10000];
        let encoded = StreamVByte::encode(&data);
        let iter_result: Vec<u16> = encoded.iter().collect();

        assert_eq!(iter_result, data);
        assert_eq!(encoded.iter().len(), data.len());
        assert_eq!(encoded.iter().size_hint(), (data.len(), Some(data.len())));
    }

    #[test]
    fn test_iterator_vs_decode() {
        // Test that iterator produces same results as decode()
        let data: Vec<u32> = (0..1000).map(|i| (i * 17 + 13) % 65536).collect();
        let encoded = StreamVByte::encode(&data);

        let decoded = encoded.decode();
        let iter_result: Vec<u32> = encoded.iter().collect();

        assert_eq!(decoded, data);
        assert_eq!(iter_result, data);
        assert_eq!(decoded, iter_result);
    }

    #[test]
    fn test_iterator_vs_decode_u16() {
        // Test that iterator produces same results as decode() for u16
        let data: Vec<u16> = (0..1000).map(|i| ((i * 17 + 13) % 65536) as u16).collect();
        let encoded = StreamVByte::encode(&data);

        let decoded = encoded.decode();
        let iter_result: Vec<u16> = encoded.iter().collect();

        assert_eq!(decoded, data);
        assert_eq!(iter_result, data);
        assert_eq!(decoded, iter_result);
    }

    #[test]
    fn test_iterator_exact_multiples_of_16() {
        // Test sequences that are exact multiples of buffer size (16)
        for &size in &[16, 32, 48, 64, 160, 1600] {
            let data: Vec<u32> = (0..size).collect();
            let encoded = StreamVByte::encode(&data);
            let iter_result: Vec<u32> = encoded.iter().collect();

            assert_eq!(iter_result, data, "Failed for size {}", size);
        }
    }

    #[test]
    fn test_iterator_exact_multiples_of_16_u16() {
        // Test sequences that are exact multiples of buffer size (16) for u16
        for &size in &[16, 32, 48, 64, 160, 1600] {
            let data: Vec<u16> = (0..size).map(|x| x as u16).collect();
            let encoded = StreamVByte::encode(&data);
            let iter_result: Vec<u16> = encoded.iter().collect();

            assert_eq!(iter_result, data, "Failed for size {}", size);
        }
    }

    #[test]
    fn test_iterator_non_multiples_of_16() {
        // Test sequences that are NOT multiples of buffer size
        for &size in &[1, 7, 15, 17, 31, 33, 47, 49, 63, 65, 159, 161] {
            let data: Vec<u32> = (0..size).collect();
            let encoded = StreamVByte::encode(&data);
            let iter_result: Vec<u32> = encoded.iter().collect();

            assert_eq!(iter_result, data, "Failed for size {}", size);
        }
    }

    #[test]
    fn test_iterator_non_multiples_of_16_u16() {
        // Test sequences that are NOT multiples of buffer size for u16
        for &size in &[1, 7, 15, 17, 31, 33, 47, 49, 63, 65, 159, 161] {
            let data: Vec<u16> = (0..size).map(|x| x as u16).collect();
            let encoded = StreamVByte::encode(&data);
            let iter_result: Vec<u16> = encoded.iter().collect();

            assert_eq!(iter_result, data, "Failed for size {}", size);
        }
    }

    #[test]
    fn test_iterator_mixed_value_sizes() {
        let data: Vec<u32> = vec![
            // 1 byte values
            0,
            1,
            127,
            255,
            // 2 byte values
            256,
            1000,
            32767,
            65535,
            // 3 byte values
            65536,
            100000,
            1000000,
            16777215,
            // 4 byte values
            16777216,
            2000000000,
            u32::MAX - 1,
            u32::MAX,
        ];
        let encoded = StreamVByte::encode(&data);
        let iter_result: Vec<u32> = encoded.iter().collect();

        assert_eq!(iter_result, data);
    }

    #[test]
    fn test_iterator_mixed_value_sizes_u16() {
        let data: Vec<u16> = vec![
            // 1 byte values
            0,
            1,
            127,
            255,
            // 2 byte values
            256,
            1000,
            32767,
            65535,
            // More mixed values
            42,
            500,
            1024,
            60000,
            100,
            50000,
            200,
            u16::MAX,
        ];
        let encoded = StreamVByte::encode(&data);
        let iter_result: Vec<u16> = encoded.iter().collect();

        assert_eq!(iter_result, data);
    }

    #[test]
    fn test_iterator_large_sequence() {
        let data: Vec<u32> = (0..10000).map(|i| (i * 17 + 13) % 65536).collect();
        let encoded = StreamVByte::encode(&data);
        let iter_result: Vec<u32> = encoded.iter().collect();

        assert_eq!(iter_result, data);
    }

    #[test]
    fn test_iterator_large_sequence_u16() {
        let data: Vec<u16> = (0..10000).map(|i| ((i * 17 + 13) % 65536) as u16).collect();
        let encoded = StreamVByte::encode(&data);
        let iter_result: Vec<u16> = encoded.iter().collect();

        assert_eq!(iter_result, data);
    }

    #[test]
    fn test_iterator_size_hint_during_iteration() {
        let data: Vec<u32> = (0..100).collect();
        let encoded = StreamVByte::encode(&data);
        let mut iter = encoded.iter();

        // Check size_hint at different points during iteration
        assert_eq!(iter.size_hint(), (100, Some(100)));

        // Consume some elements
        for _ in 0..10 {
            iter.next();
        }
        assert_eq!(iter.size_hint(), (90, Some(90)));

        // Consume more elements
        for _ in 0..50 {
            iter.next();
        }
        assert_eq!(iter.size_hint(), (40, Some(40)));

        // Consume remaining elements
        let remaining: Vec<u32> = iter.collect();
        assert_eq!(remaining.len(), 40);
        assert_eq!(remaining, (60..100).collect::<Vec<u32>>());
    }

    #[test]
    fn test_iterator_exact_size() {
        let data: Vec<u32> = (0..173).collect(); // Non-multiple of 16
        let encoded = StreamVByte::encode(&data);
        let mut iter = encoded.iter();

        assert_eq!(iter.len(), 173);

        // Consume some elements and check len updates
        for _ in 0..50 {
            iter.next();
        }
        assert_eq!(iter.len(), 123);

        // Consume all remaining
        let remaining: Vec<u32> = iter.collect();
        assert_eq!(remaining.len(), 123);
    }

    #[test]
    fn test_iterator_random_like_sequence() {
        let mut data: Vec<u32> = Vec::new();
        let mut state = 12345u64;

        for _ in 0..5000 {
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            data.push((state >> 16) as u32 & 0xFFFF);
        }

        let encoded = StreamVByte::encode(&data);
        let iter_result: Vec<u32> = encoded.iter().collect();

        assert_eq!(iter_result, data);
    }

    // ============ RANDOM ACCESS TESTS ============

    #[test]
    fn test_random_access_basic() {
        let data: Vec<u32> = (0..100).collect();
        let ra = StreamVByteRandomAccess::new(&data, 16);

        // Test single range
        let mut buffer = vec![0u32; 10];
        ra.get_range(&mut buffer, 10..20);
        assert_eq!(buffer, (10..20).collect::<Vec<u32>>());
    }

    #[test]
    fn test_random_access_basic_u16() {
        let data: Vec<u16> = (0..100).map(|x| x as u16).collect();
        let ra = StreamVByteRandomAccess::new(&data, 16);

        // Test single range
        let mut buffer = vec![0u16; 10];
        ra.get_range(&mut buffer, 10..20);
        assert_eq!(buffer, (10..20).map(|x| x as u16).collect::<Vec<u16>>());
    }

    #[test]
    fn test_random_access_different_block_sizes() {
        let data: Vec<u32> = (0..1000).collect();

        for &block_size in &[4, 8, 16, 32, 64, 128] {
            let ra = StreamVByteRandomAccess::new(&data, block_size);

            // Test multiple ranges
            let test_ranges = vec![0..10, 50..75, 100..150, 500..600, 900..950, 990..1000];

            for range in test_ranges {
                let mut buffer = vec![0u32; range.len()];
                ra.get_range(&mut buffer, range.clone());
                assert_eq!(
                    buffer,
                    range.map(|x| x as u32).collect::<Vec<u32>>(),
                    "Failed for block_size {}",
                    block_size
                );
            }
        }
    }

    #[test]
    fn test_random_access_different_block_sizes_u16() {
        let data: Vec<u16> = (0..1000).map(|x| x as u16).collect();

        // u16 uses 8 values per control byte, so block sizes must be multiples of 8
        for &block_size in &[8, 16, 24, 32, 64, 128] {
            let ra = StreamVByteRandomAccess::new(&data, block_size);

            // Test multiple ranges
            let test_ranges = vec![0..10, 50..75, 100..150, 500..600, 900..950, 990..1000];

            for range in test_ranges {
                let mut buffer = vec![0u16; range.len()];
                ra.get_range(&mut buffer, range.clone());
                assert_eq!(
                    buffer,
                    range.map(|x| x as u16).collect::<Vec<u16>>(),
                    "Failed for block_size {}",
                    block_size
                );
            }
        }
    }

    #[test]
    fn test_random_access_edge_cases() {
        let data: Vec<u32> = (0..100).collect();
        let ra = StreamVByteRandomAccess::new(&data, 16);

        // Test single element
        let mut buffer = vec![0u32; 1];
        ra.get_range(&mut buffer, 42..43);
        assert_eq!(buffer, vec![42]);

        // Test at beginning
        let mut buffer = vec![0u32; 5];
        ra.get_range(&mut buffer, 0..5);
        assert_eq!(buffer, vec![0, 1, 2, 3, 4]);

        // Test at end
        let mut buffer = vec![0u32; 5];
        ra.get_range(&mut buffer, 95..100);
        assert_eq!(buffer, vec![95, 96, 97, 98, 99]);
    }

    #[test]
    fn test_random_access_mixed_value_sizes() {
        let data: Vec<u32> = vec![
            // 1 byte values
            0,
            1,
            127,
            255,
            // 2 byte values
            256,
            1000,
            32767,
            65535,
            // 3 byte values
            65536,
            100000,
            1000000,
            16777215,
            // 4 byte values
            16777216,
            2000000000,
            u32::MAX - 1,
            u32::MAX,
        ];
        let ra = StreamVByteRandomAccess::new(&data, 8);

        // Test various ranges
        let test_cases = vec![
            (0..4, vec![0, 1, 127, 255]),
            (4..8, vec![256, 1000, 32767, 65535]),
            (8..12, vec![65536, 100000, 1000000, 16777215]),
            (12..16, vec![16777216, 2000000000, u32::MAX - 1, u32::MAX]),
            (2..6, vec![127, 255, 256, 1000]),
        ];

        for (range, expected) in test_cases {
            let mut buffer = vec![0u32; range.len()];
            ra.get_range(&mut buffer, range.clone());
            assert_eq!(buffer, expected, "Failed for range {:?}", range);
        }
    }

    #[test]
    fn test_random_access_mixed_value_sizes_u16() {
        let data: Vec<u16> = vec![
            // 1 byte values
            0,
            1,
            127,
            255,
            // 2 byte values
            256,
            1000,
            32767,
            65535,
            // More mixed values
            50,
            500,
            5000,
            50000,
            100,
            10000,
            254,
            u16::MAX,
        ];
        let ra = StreamVByteRandomAccess::new(&data, 8);

        // Test various ranges
        let test_cases = vec![
            (0..4, vec![0u16, 1, 127, 255]),
            (4..8, vec![256u16, 1000, 32767, 65535]),
            (8..12, vec![50u16, 500, 5000, 50000]),
            (12..16, vec![100u16, 10000, 254, u16::MAX]),
            (2..6, vec![127u16, 255, 256, 1000]),
        ];

        for (range, expected) in test_cases {
            let mut buffer = vec![0u16; range.len()];
            ra.get_range(&mut buffer, range.clone());
            assert_eq!(buffer, expected, "Failed for range {:?}", range);
        }
    }

    #[test]
    fn test_random_access_vs_full_decode() {
        let mut data: Vec<u32> = Vec::new();
        let mut state = 12345u64;

        for _ in 0..1000 {
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            data.push((state >> 16) as u32 & 0xFFFF);
        }

        let ra = StreamVByteRandomAccess::new(&data, 64);

        // Test multiple random ranges
        let test_ranges = vec![0..50, 100..200, 300..350, 500..750, 800..900, 950..1000];

        for range in test_ranges {
            let mut buffer = vec![0u32; range.len()];
            ra.get_range(&mut buffer, range.clone());

            let expected = &data[range.clone()];
            assert_eq!(buffer, expected, "Failed for range {:?}", range);
        }
    }

    #[test]
    fn test_random_access_large_ranges() {
        let data: Vec<u32> = (0..10000).map(|i| (i * 17 + 13) % 65536).collect();
        let ra = StreamVByteRandomAccess::new(&data, 128);

        // Test large ranges
        let test_ranges = vec![0..1000, 2000..3500, 5000..7500, 8000..10000];

        for range in test_ranges {
            let mut buffer = vec![0u32; range.len()];
            ra.get_range(&mut buffer, range.clone());

            let expected = &data[range.clone()];
            assert_eq!(buffer, expected, "Failed for range {:?}", range);
        }
    }

    #[test]
    #[should_panic(expected = "Invalid range")]
    fn test_random_access_invalid_range_start_after_end() {
        let data: Vec<u32> = (0..100).collect();
        let ra = StreamVByteRandomAccess::new(&data, 16);
        let mut buffer = vec![0u32; 10];
        ra.get_range(&mut buffer, 50..40); // start > end
    }

    #[test]
    #[should_panic(expected = "Invalid range")]
    fn test_random_access_invalid_range_out_of_bounds() {
        let data: Vec<u32> = (0..100).collect();
        let ra = StreamVByteRandomAccess::new(&data, 16);
        let mut buffer = vec![0u32; 10];
        ra.get_range(&mut buffer, 90..110); // end > data.len()
    }

    #[test]
    #[should_panic(expected = "Output buffer is not large enough")]
    fn test_random_access_buffer_too_small() {
        let data: Vec<u32> = (0..100).collect();
        let ra = StreamVByteRandomAccess::new(&data, 16);
        let mut buffer = vec![0u32; 5]; // buffer too small for range
        ra.get_range(&mut buffer, 10..20); // range needs 10 elements but buffer has 5
    }
}

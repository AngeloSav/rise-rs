#![allow(dead_code)]
use num::PrimInt;
use std::debug_assert;

use std::arch::x86_64::{__m128i, _mm_loadu_si128, _mm_shuffle_epi8, _mm_storeu_si128};

pub trait SVBEncodable: Sized + PrimInt {
    type Reg: AsRef<[Self]> + AsMut<[Self]>;
    const LANES: usize = 16 / std::mem::size_of::<Self>();
    const BYTES: usize = std::mem::size_of::<Self>();
    const CONTROL_BITS: usize = (usize::BITS - (Self::BYTES - 1).leading_zeros()) as usize;
    const N_CONTROL: usize = 8 / Self::CONTROL_BITS;
    const CONTROL_MASK: u8 = (1 << Self::CONTROL_BITS) - 1;
    const MASKS: [Self::Reg; 256];
    const LENGTHS: [u8; 256];

    fn encode_value(v: Self, data: &mut [u8]) -> usize;
    fn decode_value(data: &[u8], length: usize) -> Self;

    #[inline]
    fn encode_control_byte(input: &[Self], control_byte: &mut u8, output: &mut [u8]) -> usize {
        assert!(input.len() <= Self::N_CONTROL);
        let mut encoded_len = 0;
        *control_byte = 0;

        for &v in input {
            let length = Self::encode_value(v, &mut output[encoded_len..]);
            *control_byte <<= Self::CONTROL_BITS;
            *control_byte |= (length - 1) as u8;

            encoded_len += length;
        }

        if input.len() < Self::N_CONTROL {
            *control_byte <<= (Self::N_CONTROL - input.len()) * Self::CONTROL_BITS;
        }

        encoded_len
    }

    /// Decode up to `n` values given a control byte and data byte slice.
    /// Returns the number of bytes read from the data slice.
    #[inline]
    fn decode_control_byte(n: usize, control_byte: u8, data: &[u8], output: &mut [Self]) -> usize {
        debug_assert!(n <= Self::N_CONTROL);

        let mut data_index = 0;
        for i in 0..n {
            let length = ((control_byte >> ((8 - Self::CONTROL_BITS) - i * Self::CONTROL_BITS))
                & Self::CONTROL_MASK)
                + 1;
            let value = Self::decode_value(&data[data_index..], length as usize);
            output[i] = value;
            data_index += length as usize;
        }

        data_index
    }

    /// Decoding SIMD kernel using SSSE3 intrinsics
    ///
    /// Types of this function tries to implement safety guardrails as much as possible. Namely:
    /// `output` - is a reference to the buffer of 4 u32 values;
    /// `input` - is a reference to u8 array of unspecified length (`control_byte` speciefies how much will be decoded);
    //
    /// Technically the encoded length can be calculated from control word directly using horizontal 2-bit sum
    /// ```rust,ignore
    /// let result = *control_byte;
    /// let result = ((result & 0b11001100) >> 2) + (result & 0b00110011);
    /// let result = (result >> 4) + (result & 0b1111) + 4;
    /// ```
    /// Unfortunatley, this approach is slower then memoized length. There is a mention of this approach can be faster
    /// when using `u32` control words, which implies decoding a batch of size 16[^1].
    ///
    /// [^1]: [Bit hacking versus memoization: a Stream VByte example](https://lemire.me/blog/2017/11/28/bit-hacking-versus-memoization-a-stream-vbyte-example/)
    #[inline]
    fn simd_decode(input: &[u8; 16], control_byte: u8, output: &mut Self::Reg) -> usize {
        let mask = &Self::MASKS[control_byte as usize];
        unsafe {
            // Make sure its ok to cast `mask` to `__m128i`
            debug_assert_eq!(core::mem::size_of::<Self>() * Self::LANES, 16);
            let p = <Self::Reg as AsRef<[Self]>>::as_ref(mask).as_ptr();
            let mask = _mm_loadu_si128(p as *const __m128i);
            let input = _mm_loadu_si128(input.as_ptr().cast());
            let answer = _mm_shuffle_epi8(input, mask);
            let q = <Self::Reg as AsMut<[Self]>>::as_mut(output).as_mut_ptr();
            _mm_storeu_si128(q as *mut __m128i, answer);
        }
        Self::LENGTHS[control_byte as usize] as usize
    }
}

const fn lengths_u32() -> [u8; 256] {
    let mut lengths = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        lengths[i] =
            ((i & 0b11) + ((i >> 2) & 0b11) + ((i >> 4) & 0b11) + ((i >> 6) & 0b11)) as u8 + 4;
        i += 1;
    }
    lengths
}

const fn lengths_u16() -> [u8; 256] {
    let mut lengths = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        lengths[i] = ((i & 0b1)
            + ((i >> 1) & 0b1)
            + ((i >> 2) & 0b1)
            + ((i >> 3) & 0b1)
            + ((i >> 4) & 0b1)
            + ((i >> 5) & 0b1)
            + ((i >> 6) & 0b1)
            + ((i >> 7) & 0b1)) as u8
            + 8;
        i += 1;
    }
    lengths
}

/**
Preparing shuffling masks for `pshufb` SSE instructions

`pshufb` (`_mm_shuffle_epi8()`) allows to shuffle bytes around in a `__mm128` register. Shuffle mask consist of 16
bytes. Each byte describe byte index in input register which should be copied to corresponding byte in the output
register. For addressing 16 bytes we need log(16) = 4 bits. So bits 0:3 of each byte are storing input register byte
index. MSB of each byte indicating if corresponding byte in output register should be zeroed out. 4 least significant
bits are non effective if MSB is set.

`pshufb` SSE instruction visualization.

```graph
  Byte offsets:           0        1        2        3        4
                  ┌────────┬────────┬────────┬────────┬────────┬───┐
Input Register:   │   0x03 │   0x15 │   0x22 │   0x19 │   0x08 │...│
                  └────▲───┴────────┴────▲───┴────▲───┴────▲───┴───┘
                       │        ┌────────┘        │        │
                       │        │        ┌─────────────────┘
                       │        │        │        │
                       └───────────────────────────────────┐
                                │        │        │        │
                  ┌────────┬────┴───┬────┴───┬────┴───┬────┴───┬───┐
  Mask Register:  │   0x80 │   0x02 │   0x04 │   0x03 │   0x00 │...│
                  ├────────┼────────┼────────┼────────┼────────┼───┤
Output Register:  │   0x00 │   0x22 │   0x08 │   0x19 │   0x03 │...│
                  └────────┴────────┴────────┴────────┴────────┴───┘
```

See [`_mm_shuffle_epi8()`][_mm_shuffle_epi8] documentation.

NOTE: The code and documentation of this function and the next one are from the nice implementation
by [Denis Bazhenov](https://github.com/bazhenov/svbyte/blob/master/src/lib.rs).

[_mm_shuffle_epi8]: https://www.intel.com/content/www/us/en/docs/intrinsics-guide/index.html#text=shuffle_epi8&ig_expand=6097
*/
const fn shuffle_masks_u32() -> [[u32; 4]; 256] {
    let mut masks = [[0u32; 4]; 256];

    let mut a = 1;
    while a <= 4 {
        let mut b = 1;
        while b <= 4 {
            let mut c = 1;
            while c <= 4 {
                let mut d = 1;
                while d <= 4 {
                    // Loading in reverse order because Intel is Little Endian Machine
                    let mask = [
                        shuffle_mask_u32(a, 0),
                        shuffle_mask_u32(b, a),
                        shuffle_mask_u32(c, a + b),
                        shuffle_mask_u32(d, a + b + c),
                    ];

                    // counting in the index must be 0 based (eg. length of 1 is `00`, not `01`), hence `a - 1`
                    let idx = (a - 1) << 6 | (b - 1) << 4 | (c - 1) << 2 | (d - 1);
                    assert!(a + b + c + d <= 16);
                    masks[idx] = mask;
                    d += 1;
                }
                c += 1;
            }
            b += 1;
        }
        a += 1;
    }
    masks
}

const fn shuffle_masks_u16() -> [[u16; 8]; 256] {
    let mut masks = [[0u16; 8]; 256];

    let mut a = 1;
    while a <= 2 {
        let mut b = 1;
        while b <= 2 {
            let mut c = 1;
            while c <= 2 {
                let mut d = 1;
                while d <= 2 {
                    let mut e = 1;
                    while e <= 2 {
                        let mut f = 1;
                        while f <= 2 {
                            let mut g = 1;
                            while g <= 2 {
                                let mut h = 1;
                                while h <= 2 {
                                    // Loading in reverse order because Intel is Little Endian Machine
                                    let mask = [
                                        shuffle_mask_u16(a, 0),
                                        shuffle_mask_u16(b, a),
                                        shuffle_mask_u16(c, a + b),
                                        shuffle_mask_u16(d, a + b + c),
                                        shuffle_mask_u16(e, a + b + c + d),
                                        shuffle_mask_u16(f, a + b + c + d + e),
                                        shuffle_mask_u16(g, a + b + c + d + e + f),
                                        shuffle_mask_u16(h, a + b + c + d + e + f + g),
                                    ];

                                    // counting in the index must be 0 based (eg. length of 1 is `0`, not `1`), hence `a - 1`
                                    let idx = (a - 1) << 7
                                        | (b - 1) << 6
                                        | (c - 1) << 5
                                        | (d - 1) << 4
                                        | (e - 1) << 3
                                        | (f - 1) << 2
                                        | (g - 1) << 1
                                        | (h - 1);
                                    assert!(a + b + c + d + e + f + g + h <= 16);
                                    masks[idx] = mask;
                                    h += 1;
                                }
                                g += 1;
                            }
                            f += 1;
                        }
                        e += 1;
                    }
                    d += 1;
                }
                c += 1;
            }
            b += 1;
        }
        a += 1;
    }
    masks
}

/**
Prepares shuffle mask for decoding a single `u32` using `pshufb` instruction

`len` parameter is describing the length of decoded `u32` in the input register (1-4). `offset` parameter is
describing the base offset in the register. It is the sum of all previous number lengths loaded in the input register.
*/
const fn shuffle_mask_u32(len: usize, offset: usize) -> u32 {
    const PZ: u8 = 0b10000000;
    assert!(offset < 16, "Offset should be <16");
    let offset = offset as u8;
    let p1 = offset;
    let p2 = offset + 1;
    let p3 = offset + 2;
    let p4 = offset + 3;
    match len {
        1 => u32::from_be_bytes([PZ, PZ, PZ, p1]),
        2 => u32::from_be_bytes([PZ, PZ, p1, p2]),
        3 => u32::from_be_bytes([PZ, p1, p2, p3]),
        4 => u32::from_be_bytes([p1, p2, p3, p4]),
        _ => panic!("Length of u32 is 1..=4 bytes"),
    }
}

const fn shuffle_mask_u16(len: usize, offset: usize) -> u16 {
    const PZ: u8 = 0b10000000;
    assert!(offset < 16, "Offset should be <16");
    let offset = offset as u8;
    let p1 = offset;
    let p2 = offset + 1;
    match len {
        1 => u16::from_be_bytes([PZ, p1]),
        2 => u16::from_be_bytes([p1, p2]),
        _ => panic!("Length of u32 is 1..=4 bytes"),
    }
}

impl SVBEncodable for u32 {
    type Reg = [u32; Self::LANES];

    const MASKS: [[u32; 4]; 256] = shuffle_masks_u32();
    const LENGTHS: [u8; 256] = lengths_u32();

    #[inline]
    fn encode_value(v: Self, data: &mut [u8]) -> usize {
        let bytes = Self::to_be_bytes(v);
        let length = 1.max(std::mem::size_of::<Self>() - v.leading_zeros() as usize / 8);
        data[..length].copy_from_slice(&bytes[Self::BYTES - length..]);

        length
    }

    #[inline]
    fn decode_value(data: &[u8], length: usize) -> Self {
        let mut value_bytes = [0u8; Self::BYTES];
        value_bytes[Self::BYTES - length as usize..].copy_from_slice(&data[..length as usize]);

        let value = u32::from_be_bytes(value_bytes);
        value
    }
}

impl SVBEncodable for u16 {
    type Reg = [u16; Self::LANES];

    const MASKS: [[u16; 8]; 256] = shuffle_masks_u16();
    const LENGTHS: [u8; 256] = lengths_u16();

    #[inline]
    fn encode_value(v: Self, data: &mut [u8]) -> usize {
        let bytes = Self::to_be_bytes(v);
        let length = 1.max(std::mem::size_of::<Self>() - v.leading_zeros() as usize / 8);
        data[..length].copy_from_slice(&bytes[Self::BYTES - length..]);

        length
    }

    #[inline]
    fn decode_value(data: &[u8], length: usize) -> Self {
        let mut value_bytes = [0u8; Self::BYTES];
        value_bytes[Self::BYTES - length as usize..].copy_from_slice(&data[..length as usize]);

        let value = u16::from_be_bytes(value_bytes);
        value
    }
}

/// Decode all the values of type T given a slice of control bytes and data bytes. Decoded values are written to `buffer` slice, which must be large enough to hold `length` values.
/// The function returns the number of bytes read from the data slice.
#[inline]
pub(super) fn decode_slice_aligned<T: SVBEncodable>(
    buffer: &mut [T],
    controls: &[u8],
    data: &[u8],
) -> usize {
    assert!(
        buffer.len() >= controls.len() * T::N_CONTROL,
        "output slice is not large enough"
    );

    let mut iterations = controls.len();

    let mut buffer: *mut T::Reg = buffer.as_mut_ptr().cast();
    let mut control_bytes = controls.as_ptr();
    let mut data_stream = data.as_ptr();
    let data_stream_end = (data.last().unwrap() as *const u8).wrapping_add(1);
    let mut data_stream_offset = 0usize;

    // Decode with loop unrolling
    const UNROLL_FACTOR: usize = 4;
    let n_unrolled_iterations = iterations / UNROLL_FACTOR;

    for _ in 0..n_unrolled_iterations {
        for _ in 0..UNROLL_FACTOR {
            debug_assert!(
                data_stream.wrapping_add(16) <= data_stream_end,
                "At least 16 bytes should be available in the data stream"
            );
            let encoded_len =
                unsafe { T::simd_decode(&*data_stream.cast(), *control_bytes, &mut *buffer) };

            control_bytes = control_bytes.wrapping_add(1);
            buffer = buffer.wrapping_add(1);

            data_stream = data_stream.wrapping_add(encoded_len);
            data_stream_offset += encoded_len;
        }

        iterations -= UNROLL_FACTOR;
    }

    // Tail decode
    for _ in 0..iterations {
        debug_assert!(
            data_stream.wrapping_add(16) <= data_stream_end,
            "At least 16 bytes should be available in the data stream"
        );
        let encoded_len =
            unsafe { T::simd_decode(&*data_stream.cast(), *control_bytes, &mut *buffer) };

        control_bytes = control_bytes.wrapping_add(1);
        buffer = buffer.wrapping_add(1);

        data_stream = data_stream.wrapping_add(encoded_len);
        data_stream_offset += encoded_len;
    }

    data_stream_offset
}

#[inline]
pub(super) fn decode_16_aligned<T: SVBEncodable>(
    buffer: &mut [T; 16],
    controls: &[u8],
    data: &[u8],
) -> usize {
    let mut buffer_ptr = buffer.as_mut_ptr().cast::<T::Reg>();
    let mut control_bytes = controls.as_ptr();
    let mut data_stream = data.as_ptr();

    for _ in 0..(16 / T::LANES) {
        let encoded_len =
            unsafe { T::simd_decode(&*data_stream.cast(), *control_bytes, &mut *buffer_ptr) };

        control_bytes = control_bytes.wrapping_add(1);
        buffer_ptr = buffer_ptr.wrapping_add(1);

        data_stream = data_stream.wrapping_add(encoded_len);
    }
    data_stream as usize - data.as_ptr() as usize
}

use std::usize;

use crate::{
    BitSliceWithOffset, BitVec, EnumeratorFromBitSlice, EstimateSpace, NextGEQ, SequenceEnumerator,
    WriteBitvector,
    bitvector::unary_enum::UnaryEnumerator,
    config,
    utils::{ceil_log2, msb},
};
use epserde::prelude::*;
use num::integer::div_ceil;

/// A compressed representation of a non-decreasing integer sequence.
///
/// Elias-Fano splits each element into a high part (stored in a unary-encoded
/// bit array) and a low part (stored in a dense bit array).  The resulting
/// space usage is approximately `2 + log2(u/n)` bits per element, which is
/// near-optimal for sequences drawn from a universe of size `u`.
///
/// # Construction
///
/// The most convenient entry point is `From<&[u64]>`, which computes `n` and
/// `u` automatically:
///
/// ```
/// use pef::EliasFano;
/// use pef::SequenceEnumerator;
///
/// let ef = EliasFano::from([1u64, 3, 5, 7, 10].as_slice());
/// assert_eq!(ef.len(), 5);
/// let vals: Vec<u64> = ef.iter().collect();
/// assert_eq!(vals, vec![1, 3, 5, 7, 10]);
/// ```
///
/// For lower-level control use [`WriteBitvector::write_bitvector`] directly,
/// which lets you supply `n` and `u` explicitly.
///
/// # Iteration
///
/// Call [`EliasFano::iter`] to get an [`EliasFanoIter`], which implements both
/// [`SequenceEnumerator`] and [`NextGEQ`].
#[derive(Debug, Default, Epserde)]
pub struct EliasFano {
    pub(crate) bv: BitVec,
    pub(crate) n: usize,
    pub(crate) u: u64,
}

const LOG_SAMPLING0: usize = config::EF_LOG_SAMPLING0;
const LOG_SAMPLING1: usize = config::EF_LOG_SAMPLING1;
const LINEAR_SCAN_THRESHOLD: usize = config::EF_LINEAR_SCAN_THRESHOLD;

impl EliasFano {
    /// Returns the number of elements in the sequence
    pub fn len(&self) -> usize {
        self.n
    }

    /// Return an iterator over the encoded sequence.
    ///
    /// The iterator supports both sequential (`next_val`) and random-access
    /// (`move_to_position`, `next_geq`) traversal without copying any data.
    pub fn iter(&self) -> EliasFanoIter<'_> {
        Self::iter_from_slice(self.bv.as_bitslice(), self.n, self.u)
    }

    /// Estimate the number of bits required to encode `n` elements with
    /// universe size `u` using the current sampling parameters.
    pub fn n_bits(u: u64, n: usize) -> usize {
        let n_lo_bits = if u > n as u64 {
            (msb(u / n as u64)) as u64
        } else {
            0
        };
        let higher_bits_len = n as u64 + (u >> (n_lo_bits as usize)) + 2;

        let pointer_size = ceil_log2(higher_bits_len) as u64;
        let n = n as u64;

        (n_lo_bits * n
            + ((higher_bits_len - n) >> LOG_SAMPLING0) * pointer_size
            + (n >> LOG_SAMPLING1) * pointer_size
            + higher_bits_len) as usize
    }
}

impl<'a> From<&'a [u64]> for EliasFano {
    fn from(v: &'a [u64]) -> Self {
        let n = v.len();
        let u = *v.last().unwrap() + 1;
        let bv = Self::write_bitvector(v.iter().copied(), n, u);

        Self { bv, n, u }
    }
}

/// Writes 0-pointer entries for the EF higher-bits array directly into `bv` at offset 0.
/// Extracted from a closure so that `bv` can also be written to by the caller at the same time.
#[inline]
fn ef_set_ptr0(bv: &mut BitVec, begin: u64, end: u64, rank_end: u64, pointer_size: u64) {
    let begin_zeros = begin - rank_end;
    let end_zeros = end - rank_end;

    let mut ptr0 = div_ceil(begin_zeros, 1 << LOG_SAMPLING0);

    while (ptr0 << LOG_SAMPLING0) < end_zeros {
        if ptr0 == 0 {
            ptr0 += 1;
            continue;
        }

        let offset = (ptr0 - 1) * pointer_size;
        bv.set_bits(
            offset as usize,
            pointer_size as usize,
            (ptr0 << LOG_SAMPLING0) + rank_end,
        );

        ptr0 += 1;
    }
}

impl WriteBitvector for EliasFano {
    #[inline]
    fn write_bitvector(seq: impl IntoIterator<Item = u64>, n: usize, u: u64) -> BitVec {
        let n_lo_bits = if u > n as u64 { msb(u / n as u64) } else { 0 };
        let higher_bits_len = n as u64 + (u >> (n_lo_bits as usize)) + 2;

        let pointer_size = ceil_log2(higher_bits_len) as u64;

        // Layout: [ 0ptrs | 1ptrs | lo | hi ]
        let n_0ptrs_bits =
            ((higher_bits_len as usize - n) >> LOG_SAMPLING0) * pointer_size as usize;
        let n_1ptrs_bits = (n >> LOG_SAMPLING1) * pointer_size as usize;
        let offset_lo = n_0ptrs_bits + n_1ptrs_bits;
        let offset_hi = offset_lo + n * n_lo_bits as usize;

        let mut bv = BitVec::with_zeros(offset_hi + higher_bits_len as usize);

        let mut prec_hi = 0u64;
        let mut prec = 0u64;
        let mut lo_pos = offset_lo;

        // +1 to skip the initial sentinel zero
        let mut hi_pos = offset_hi + 1;
        let mut len = 0;

        for (i, el) in seq.into_iter().enumerate() {
            assert!(prec <= el, "Sequence must be non decreasing!");
            assert!(el < u);
            len += 1;

            let to_push = el & ((1 << n_lo_bits) - 1);
            let hi = (el >> n_lo_bits) + i as u64 + 1;

            // Write low bits
            bv.set_bits(lo_pos, n_lo_bits as usize, to_push);
            lo_pos += n_lo_bits as usize;

            // Advance past zero runs in hi (already 0); mark the 1-bit
            hi_pos += ((el >> n_lo_bits) - (prec >> n_lo_bits)) as usize;
            bv.set(hi_pos, true);
            hi_pos += 1;

            if i != 0 && i % (1 << LOG_SAMPLING1) == 0 {
                let ptr1 = i >> LOG_SAMPLING1;
                let off = n_0ptrs_bits + (ptr1 - 1) * pointer_size as usize;
                bv.set_bits(off, pointer_size as usize, hi);
            }

            ef_set_ptr0(&mut bv, prec_hi + 1, hi, i as u64, pointer_size);

            prec = el;
            prec_hi = hi;
        }

        assert!(len != 0, "Sequence is empty");
        assert!(len == n, "n is incorrect");

        ef_set_ptr0(
            &mut bv,
            prec_hi + 1,
            higher_bits_len,
            n as u64,
            pointer_size,
        );

        bv
    }
}

#[derive(Debug, Clone, Default)]
pub struct EliasFanoIter<'a> {
    slice_samples0: BitSliceWithOffset<'a>,
    slice_samples1: BitSliceWithOffset<'a>,
    slice_lo: BitSliceWithOffset<'a>,
    slice_hi: BitSliceWithOffset<'a>,
    unary_enumerator: UnaryEnumerator<'a>,
    value: u64,
    n_bits_lo: usize,
    lo_bitmask: u64,
    pointer_size: usize,
    position: usize,
    len: usize,
    u: u64,
}

impl EliasFanoIter<'_> {
    const LINEAR_SCAN_THRESHOLD: usize = LINEAR_SCAN_THRESHOLD;

    #[inline]
    fn read_low(&self) -> u64 {
        let idx = self.position * self.n_bits_lo;
        let lo = unsafe { self.slice_lo.get_word56(idx) } & self.lo_bitmask;
        lo
    }

    #[inline]
    fn read_next(&mut self) -> u64 {
        let high = self.unary_enumerator.next_one() as u64;
        ((high - self.position as u64 - 1) << self.n_bits_lo as u64) | self.read_low()
    }

    #[inline]
    fn value(&self) -> (u64, usize) {
        (self.value, self.position)
    }

    #[cold]
    #[inline(never)]
    fn slow_next_geq(&mut self, lower_bound: u64) -> (u64, usize) {
        if core::intrinsics::unlikely(lower_bound >= self.u) {
            return self.move_to_position(self.len);
        }

        let hi_lower_bound = (lower_bound >> self.n_bits_lo) as u64;
        let cur_hi = self.value >> self.n_bits_lo;

        let to_skip;
        if lower_bound > self.value && ((hi_lower_bound - cur_hi) >> LOG_SAMPLING0) == 0 {
            // println!("FAST skip in slow_next_geq");
            to_skip = (hi_lower_bound - cur_hi) as usize;
        } else {
            // println!("SLOW: darray");
            let ptr = hi_lower_bound >> LOG_SAMPLING0;
            let hi_pos = if ptr == 0 {
                0
            } else {
                unsafe {
                    self.slice_samples0
                        .get_word56((ptr - 1) as usize * self.pointer_size)
                        & ((1 << self.pointer_size) - 1)
                }
            };
            let hi_rank0 = (ptr as usize) << LOG_SAMPLING0;

            self.unary_enumerator = UnaryEnumerator::with_pos(&self.slice_hi, hi_pos as usize);

            to_skip = hi_lower_bound as usize - hi_rank0;
        }

        self.unary_enumerator.skip0(to_skip);
        self.position = self.unary_enumerator.position() - hi_lower_bound as usize;

        let mut low_idx = self.position * self.n_bits_lo;
        let mut high_base = self.position as u64 + 1;

        loop {
            if core::intrinsics::unlikely(self.position == self.len) {
                self.value = self.u;
                return self.value();
            }

            let high_index = self.unary_enumerator.next_one() as u64;
            let high_val = high_index - high_base;

            let lo = unsafe { self.slice_lo.get_word56(low_idx) } & self.lo_bitmask;
            high_base += 1;
            low_idx += self.n_bits_lo;

            let val = (high_val << self.n_bits_lo) | lo;

            // let val = self.read_next();

            if val >= lower_bound {
                self.value = val;
                return self.value();
            }

            self.position += 1;
        }
    }

    #[cold]
    #[inline(never)]
    fn slow_move(&mut self, pos: usize) -> (u64, usize) {
        if core::intrinsics::unlikely(pos >= self.len) {
            self.position = pos;
            self.value = self.u;
            return self.value();
        }

        let skip = pos.wrapping_sub(self.position);
        let to_skip;

        if pos > self.position && skip >> LOG_SAMPLING1 == 0 {
            to_skip = skip - 1;
        } else {
            let ptr = pos >> LOG_SAMPLING1;
            let hi_pos = if ptr == 0 {
                0
            } else {
                unsafe {
                    self.slice_samples1
                        .get_word56((ptr - 1) as usize * self.pointer_size)
                        & ((1 << self.pointer_size) - 1)
                }
            };
            let hi_rank1 = (ptr as usize) << LOG_SAMPLING1;

            self.unary_enumerator = UnaryEnumerator::with_pos(&self.slice_hi, hi_pos as usize);
            to_skip = pos - hi_rank1;
        }

        self.unary_enumerator.skip1(to_skip);
        self.position = pos;
        self.value = self.read_next();

        self.value()
    }
}

impl SequenceEnumerator for EliasFanoIter<'_> {
    fn next_val(&mut self) -> (u64, usize) {
        // NOT STARTED
        if core::intrinsics::unlikely(self.position == usize::MAX) {
            return self.move_to_position(0);
        }

        self.position += 1;

        if core::intrinsics::likely(self.position < self.len) {
            self.value = self.read_next();
        } else {
            self.value = self.u;
        }

        self.value()
    }

    fn move_to_position(&mut self, pos: usize) -> (u64, usize) {
        // debug_assert!(pos <= self.len);

        if pos == self.position {
            return self.value();
        }

        let skip = pos.wrapping_sub(self.position);
        if core::intrinsics::likely(pos > self.position && skip <= Self::LINEAR_SCAN_THRESHOLD) {
            self.position = pos;
            // println!("skip linear scan: {}", skip);

            if core::intrinsics::unlikely(self.position == self.len) {
                self.value = self.u;
            } else {
                for _ in 0..skip {
                    self.unary_enumerator.next_one();
                }

                self.value = (self.unary_enumerator.position() as u64 - self.position as u64 - 1)
                    << self.n_bits_lo as u64
                    | self.read_low();
            }
            return self.value();
        }

        self.slow_move(pos)
    }

    fn len(&self) -> usize {
        self.len
    }
}

impl NextGEQ for EliasFanoIter<'_> {
    #[inline(always)]
    fn next_geq(&mut self, lower_bound: u64) -> (u64, usize) {
        if lower_bound == self.value {
            return self.value();
        }

        // NOT STARTED
        if core::intrinsics::unlikely(self.position == usize::MAX) {
            self.move_to_position(0);
        }

        let high_lower_bound = (lower_bound >> self.n_bits_lo) as u64;
        let cur_hi = self.value >> self.n_bits_lo;

        if core::intrinsics::likely(
            lower_bound > self.value
                && (high_lower_bound - cur_hi) <= Self::LINEAR_SCAN_THRESHOLD as u64,
        ) {
            // println!("FAST LINEAR scan in next_geq");
            let mut val;
            let mut high_base = self.position as u64 + 2;
            let mut low_idx = (self.position + 1) * self.n_bits_lo;
            loop {
                self.position += 1;
                if core::intrinsics::likely(self.position < self.len) {
                    // val = self.read_next();
                    let hi = self.unary_enumerator.next_one() as u64 - high_base;
                    let lo = unsafe { self.slice_lo.get_word56(low_idx) } & self.lo_bitmask;
                    high_base += 1;
                    low_idx += self.n_bits_lo;

                    val = (hi << self.n_bits_lo as u64) | lo;
                } else {
                    val = self.u;
                    break;
                }

                if val >= lower_bound {
                    break;
                }
            }

            self.value = val;
            return self.value();
        } else {
            return self.slow_next_geq(lower_bound);
        }
    }
}

impl Iterator for EliasFanoIter<'_> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        let val = self.next_val().0;
        if val == self.u {
            return None;
        }
        Some(val)
    }
}

impl EstimateSpace for EliasFano {
    fn bitsize(u: u64, n: usize) -> usize {
        Self::n_bits(u, n)
    }
}

impl<'a> EnumeratorFromBitSlice<'a> for EliasFano {
    type IterType = EliasFanoIter<'a>;

    fn iter_from_slice(bv: BitSliceWithOffset<'a>, n: usize, u: u64) -> Self::IterType {
        let n_lo_bits = if u > n as u64 {
            msb(u / n as u64) as u64
        } else {
            0
        };
        // println!("n_lo_bits: {}", n_lo_bits);

        let higher_bits_len = n as u64 + (u >> (n_lo_bits as usize)) + 2;
        let pointer_size = ceil_log2(higher_bits_len) as usize;

        let mut start_split = 0;
        let mut end_split =
            ((higher_bits_len as usize - n as usize) >> LOG_SAMPLING0) * pointer_size;

        let slice_samples0 = bv.slice(start_split, end_split);

        start_split = end_split;
        end_split += (n >> LOG_SAMPLING1) * pointer_size;
        let slice_samples1 = bv.slice(start_split, end_split);

        start_split = end_split;
        end_split += n * n_lo_bits as usize;
        let slice_lo = bv.slice(start_split, end_split);

        start_split = end_split;
        end_split += higher_bits_len as usize;
        let slice_hi = bv.slice(start_split, end_split);

        let lo_bitmask = (1 << n_lo_bits) - 1;

        let unary_enumerator = UnaryEnumerator::with_pos(&slice_hi, 0);

        EliasFanoIter {
            slice_samples0,
            slice_samples1,
            slice_lo,
            unary_enumerator,
            slice_hi,
            n_bits_lo: n_lo_bits as usize,
            lo_bitmask,
            pointer_size,
            position: usize::MAX, // so that the first next_val() sets it to 0
            len: n as usize,
            u,
            value: u,
        }
    }
}

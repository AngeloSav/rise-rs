use core::slice;
use std::mem;

use num::integer::div_ceil;
use serde::{Deserialize, Serialize};

use crate::{
    bitvector::bitvector_collection::BitVectorCollection,
    space_usage::SpaceUsage,
    utils::{ceil_log2, msb},
    BitSliceWithOffset, BitVec, EnumeratorFromBitSlice, EstimateSpace,
    IncreasingSequenceEnumerator, ToBitvector, WriteBitvector,
};

pub mod all_ones_seq;
pub mod indexed_seq;
pub mod opt_partition;
pub mod ranked_bv;
pub mod uniform_partitioned_seq;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct EliasFano {
    bv: BitVec,
    n: usize,
    u: u64,
}

const LOG_SAMPLING0: usize = 9;
const LOG_SAMPLING1: usize = 8;
const LINEAR_SCAN_THRESHOLD: usize = 8;

impl EliasFano {
    /// Returns the number of elements in the sequence
    pub fn len(&self) -> usize {
        self.n
    }

    pub fn iter(&self) -> EliasFanoIter {
        Self::iter_from_slice_with_data(self.bv.as_bitslice(), self.n, self.u)
    }

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
        let bv = Self::write_bitvector(v, n, u);

        Self { bv, n, u }
    }
}

impl WriteBitvector for EliasFano {
    #[inline]
    fn write_bitvector(seq: &[u64], n: usize, u: u64) -> BitVec {
        assert!(!seq.is_empty(), "Sequence is empty");
        assert!(seq.len() == n, "n is incorrect");

        let n_lo_bits = if u > n as u64 { msb(u / n as u64) } else { 0 };
        let higher_bits_len = n as u64 + (u >> (n_lo_bits as usize)) + 2;

        let pointer_size = ceil_log2(higher_bits_len) as u64;

        let mut bv_lo = BitVec::new();
        let mut bv_hi = BitVec::new();
        let mut bv_0ptrs = BitVec::with_zeros(
            ((higher_bits_len as usize - n) >> LOG_SAMPLING0) * pointer_size as usize,
        );
        let mut bv_1ptrs = BitVec::with_zeros((n >> LOG_SAMPLING1) * pointer_size as usize);

        let mut set_ptr0 = |begin: u64, end: u64, rank_end: u64| {
            let begin_zeros = begin - rank_end;
            let end_zeros = end - rank_end;

            let mut ptr0 = div_ceil(begin_zeros, 1 << LOG_SAMPLING0);

            while (ptr0 << LOG_SAMPLING0) < end_zeros {
                if ptr0 == 0 {
                    ptr0 += 1;
                    continue;
                }

                let offset = (ptr0 - 1) * pointer_size;
                bv_0ptrs.set_bits(
                    offset as usize,
                    pointer_size as usize,
                    (ptr0 << LOG_SAMPLING0) + rank_end,
                );

                ptr0 += 1;
            }
        };

        let mut prec_hi = 0;
        let mut prec = 0;
        for (i, &el) in seq.into_iter().enumerate() {
            assert!(prec <= el, "Sequence must be non decreasing!");
            let to_push = el & ((1 << n_lo_bits) - 1);
            let hi = (el >> n_lo_bits) + i as u64 + 1;
            // println!("to push  {:0>10b}", to_push);
            bv_lo.append_bits(to_push, n_lo_bits as usize);

            bv_hi.extend_with_zeros(((el >> n_lo_bits) - (prec >> n_lo_bits)) as usize);
            bv_hi.push(true);

            if i != 0 && i % (1 << LOG_SAMPLING1) == 0 {
                let ptr1 = i >> LOG_SAMPLING1;
                let off = (ptr1 - 1) * pointer_size as usize;
                bv_1ptrs.set_bits(off, pointer_size as usize, hi);
            }

            set_ptr0(prec_hi + 1, hi, i as u64);

            prec = el;
            prec_hi = hi;
        }

        set_ptr0(prec_hi, higher_bits_len, n as u64);
        bv_hi.push(false);

        // println!("---------------");
        let mut bv = BitVectorCollection::with_capacity(
            bv_hi.len() + bv_lo.len() + bv_0ptrs.len() + bv_1ptrs.len(),
            4,
        );
        bv.push(bv_0ptrs);
        // println!(
        //     "len: {} | n_bits: {} ({} u64)",
        //     bv.bv.data.len(),
        //     bv.bv.n_bits,
        //     bv.bv.n_bits / 64
        // );
        bv.push(bv_1ptrs);
        // println!(
        //     "len: {} | n_bits: {} ({} u64)",
        //     bv.bv.data.len(),
        //     bv.bv.n_bits,
        //     bv.bv.n_bits / 64
        // );
        bv.push(bv_lo);
        // println!("pushed lo");
        // println!(
        //     "len: {} | n_bits: {} ({} u64)",
        //     bv.bv.data.len(),
        //     bv.bv.n_bits,
        //     bv.bv.n_bits / 64
        // );
        bv_hi.extend_with_zeros(higher_bits_len as usize - bv_hi.len());
        bv.push(bv_hi);
        // println!("pushed hi");
        // println!(
        //     "len: {} | n_bits: {} ({} u64)",
        //     bv.bv.data.len(),
        //     bv.bv.n_bits,
        //     bv.bv.n_bits / 64
        // );

        bv.bv
    }
}

#[derive(Debug, Default)]
pub struct EliasFanoIter<'a> {
    slice_samples: BitSliceWithOffset<'a>,
    slice_samples1: BitSliceWithOffset<'a>,
    slice_lo: BitSliceWithOffset<'a>,
    slice_hi: BitSliceWithOffset<'a>,
    n_bits_lo: usize,
    pointer_size: usize,
    position: usize,
    i_hi: usize,
    len: usize,
    u: u64,
    cur_value: u64,
}

impl EliasFanoIter<'_> {
    const LINEAR_SCAN_THRESHOLD: usize = 8;

    #[cold]
    #[inline(never)]
    fn slow_next_geq(&mut self, lower_bound: u64) -> Option<(u64, usize)> {
        if lower_bound >= self.u {
            return self.move_to_position(self.len);
        }

        let hi_lower_bound = (lower_bound >> self.n_bits_lo) as usize;
        let cur_hi = self.i_hi - self.position;

        let to_skip;
        if lower_bound > self.cur_value && (hi_lower_bound as usize - cur_hi) >> LOG_SAMPLING0 == 0
        {
            to_skip = hi_lower_bound as usize - cur_hi;
        } else {
            let ptr = hi_lower_bound >> LOG_SAMPLING0;
            let hi_pos = if ptr == 0 {
                0
            } else {
                unsafe {
                    self.slice_samples.get_bits_unchecked(
                        (ptr - 1) as usize * self.pointer_size,
                        self.pointer_size,
                    )
                }
            };
            let hi_rank0 = (ptr as usize) << LOG_SAMPLING0;

            to_skip = hi_lower_bound - hi_rank0;
            self.i_hi = hi_pos as usize;
        }

        // this is the old, slow way to skip zeros
        // for _ in 0..to_skip {
        //     self.i_hi = self.slice_hi.next_zero(self.i_hi)? + 1;
        // }

        if to_skip != 0 {
            self.i_hi = self.slice_hi.skip_zeros(self.i_hi, to_skip - 1)? + 1
        };

        self.position = self.i_hi - hi_lower_bound;
        // self.hi_ctr = hi_lower_bound;

        let (mut val, mut pos) = self.next_val()?;
        while val < lower_bound {
            (val, pos) = self.next_val()?;
        }

        Some((val, pos))
    }

    fn slow_move(&mut self, pos: usize) -> Option<(u64, usize)> {
        if pos >= self.len {
            self.position = self.len;
            return None;
        }

        let skip: isize = pos as isize - self.position as isize + 1;
        let to_skip;

        if pos >= self.position && skip >> LOG_SAMPLING1 == 0 {
            to_skip = skip as usize - 1;
        } else {
            let ptr = pos >> LOG_SAMPLING1;
            let hi_pos = if ptr == 0 {
                0
            } else {
                unsafe {
                    self.slice_samples1.get_bits_unchecked(
                        (ptr - 1) as usize * self.pointer_size,
                        self.pointer_size,
                    ) - 1
                }
            };
            let hi_rank = (ptr as usize) << LOG_SAMPLING1;

            to_skip = pos - hi_rank;
            self.i_hi = hi_pos as usize;
        }

        if to_skip != 0 {
            self.i_hi = self.slice_hi.skip_ones(self.i_hi, to_skip - 1)? + 1;
        }
        self.position = pos;

        // self.hi_ctr = self.i_hi - self.position;
        self.next_val()
    }
}

impl IncreasingSequenceEnumerator for EliasFanoIter<'_> {
    fn next_val(&mut self) -> Option<(u64, usize)> {
        if core::intrinsics::likely(self.position < self.len) {
            let lo = self
                .slice_lo
                .get_bits(self.position * self.n_bits_lo, self.n_bits_lo)
                .unwrap();

            self.i_hi = unsafe { self.slice_hi.next_one_unchecked(self.i_hi) };

            let hi = ((self.i_hi - self.position) << self.n_bits_lo) as u64;

            self.position += 1;
            self.i_hi += 1;

            self.cur_value = hi | lo;
            Some((self.cur_value, self.position - 1))
        } else {
            None
        }
    }

    fn next_geq(&mut self, lower_bound: u64) -> Option<(u64, usize)> {
        // let lb_hi = lower_bound >> self.n_bits_lo;
        // let hi_diff = lb_hi - self.hi_ctr as u64;

        if lower_bound == self.cur_value && self.position != 0 {
            return Some((self.cur_value, self.position - 1));
        }

        let hi_lower_bound = (lower_bound >> self.n_bits_lo) as usize;
        let cur_hi = self.i_hi - self.position;

        if self.position == 0
            || (self.cur_value < lower_bound
                && (hi_lower_bound as usize - cur_hi) <= Self::LINEAR_SCAN_THRESHOLD)
        {
            let (mut val, mut pos) = self.next_val()?;
            while val < lower_bound {
                (val, pos) = self.next_val()?;
            }
            Some((val, pos))
        } else {
            //slow next geq
            self.slow_next_geq(lower_bound)
        }
    }

    fn move_to_position(&mut self, pos: usize) -> Option<(u64, usize)> {
        let skip: isize = pos as isize - self.position as isize + 1;

        if self.position <= pos && skip <= LINEAR_SCAN_THRESHOLD as isize {
            let mut skipped = 1;
            while skipped < skip {
                self.next_val()?;
                skipped += 1;
            }
            return self.next_val();
        }

        return self.slow_move(pos);
    }

    fn current_position(&self) -> usize {
        self.position - 1
    }

    fn len(&self) -> usize {
        self.len
    }
}

impl Iterator for EliasFanoIter<'_> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.next_val()?.0)
    }
}

impl ToBitvector for EliasFano {
    fn to_bv(&self) -> BitVec {
        let mut bv = BitVec::new();
        bv.append_gamma(self.n as u64);
        bv.append_gamma(self.u);
        bv.concat(&self.bv);
        bv
    }
}

impl EstimateSpace for EliasFano {
    fn bitsize(u: u64, n: usize) -> usize {
        let n_lo_bits = msb(u / n as u64) + 1;

        let n_ones = (u >> n_lo_bits) as usize;
        n + n_ones + n * n_lo_bits as usize
    }
}

impl<'a> EnumeratorFromBitSlice<'a, EliasFanoIter<'a>> for EliasFano {
    fn iter_from_slice(bv: BitSliceWithOffset<'a>) -> EliasFanoIter<'a> {
        let (n, next_pos) = unsafe { bv.get_gamma_unchecked(0) };
        let (u, next_pos) = unsafe { bv.get_gamma_unchecked(next_pos) };
        Self::iter_from_slice_with_data(bv.split_at(next_pos).1, n as usize, u)
    }

    fn iter_from_slice_with_data(
        bv: BitSliceWithOffset<'a>,
        n: usize,
        u: u64,
    ) -> EliasFanoIter<'a> {
        let n_lo_bits = if u > n as u64 {
            msb(u / n as u64) as u64
        } else {
            0
        };
        // println!("n_lo_bits: {}", n_lo_bits);

        let higher_bits_len = n as u64 + (u >> (n_lo_bits as usize)) + 2;
        let pointer_size = ceil_log2(higher_bits_len) as usize;

        let (slice_samples, slice_remainder) =
            bv.split_at(((higher_bits_len as usize - n as usize) >> LOG_SAMPLING0) * pointer_size);

        let (slice_samples1, slice_remainder) =
            slice_remainder.split_at((n >> LOG_SAMPLING1) * pointer_size);

        let (slice_lo, slice_hi) = slice_remainder.split_at((n as u64 * n_lo_bits) as usize);
        // println!("ok second split");

        EliasFanoIter {
            slice_samples,
            slice_samples1,
            slice_lo,
            slice_hi,
            n_bits_lo: n_lo_bits as usize,
            pointer_size,
            position: 0,
            i_hi: 0,
            len: n as usize,
            cur_value: 0,
            u,
        }
    }
}

impl SpaceUsage for EliasFano {
    fn space_usage_byte(&self) -> usize {
        self.bv.len() / 8 + 8 + 2 * mem::size_of::<usize>()
    }
}

mod tests;

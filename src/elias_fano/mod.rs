use std::mem;

use num::integer::div_ceil;
use serde::{Deserialize, Serialize};

use crate::{
    bitvector::bitvector_collection::BitVectorCollection,
    space_usage::SpaceUsage,
    utils::{ceil_log2, gamma_size, msb},
    BitSliceWithOffset, BitVec, BitVecCollection, EnumeratorFromBitSlice, EstimateSpace,
    IncreasingSequenceEnumerator, ToBitvector, WriteBitvector,
};

pub mod all_ones_seq;
pub mod indexed_seq;
pub mod opt_partition;
pub mod ranked_bv;
pub mod uniform_partitioned_seq;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct EliasFano {
    bv: BitVecCollection,
    n: usize,
    u: u64,
    pointer_size: usize,
    n_lo_bits: usize,
}

const LOG_SAMPLING0: usize = 9;

impl EliasFano {
    /// Returns the number of elements in the sequence
    pub fn len(&self) -> usize {
        self.n
    }

    pub fn iter(&self) -> EliasFanoIter {
        EliasFanoIter {
            slice_samples: self.bv.get(0),
            slice_lo: self.bv.get(1),
            slice_hi: self.bv.get(2),
            n_bits_lo: self.n_lo_bits,
            pointer_size: self.pointer_size,
            position: 0,
            hi_ctr: 0,
            i_hi: 0,
            len: self.len(),
            cur_value: 0,
        }
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

        (n_lo_bits * n + ((higher_bits_len - n) >> LOG_SAMPLING0) * pointer_size + higher_bits_len)
            as usize
    }
}

impl<'a> From<&'a [u64]> for EliasFano {
    fn from(v: &'a [u64]) -> Self {
        assert!(!v.is_empty(), "Sequence is empty");

        let u = *v.last().unwrap();
        let n = v.len();

        // let n_bits = msb(u) + 1;
        let n_lo_bits = msb(u / v.len() as u64) + 1;
        let higher_bits_len = n as u64 + (u >> (n_lo_bits as usize)) + 2;

        let pointer_size = ceil_log2(higher_bits_len) as u64;

        let mut bv_lo = BitVec::new();
        let mut bv_hi = BitVec::new();
        let mut bv_ptrs = BitVec::with_zeros(
            ((higher_bits_len as usize - n) >> LOG_SAMPLING0) * pointer_size as usize,
        );

        let mut set_ptr0 = |begin: u64, end: u64, rank_end: u64| {
            let begin_zeros = begin - rank_end;
            let end_zeros = end - rank_end;

            let mut ptr0 = div_ceil(begin_zeros, 1 << LOG_SAMPLING0);

            while (ptr0 << LOG_SAMPLING0) < end_zeros {
                if ptr0 == 0 {
                    continue;
                }

                let offset = (ptr0 - 1) * pointer_size;
                bv_ptrs.set_bits(
                    offset as usize,
                    pointer_size as usize,
                    (ptr0 << LOG_SAMPLING0) + rank_end,
                );

                ptr0 += 1;
            }
        };

        let mut prec_hi = 0;
        let mut prec = 0;
        for (i, &el) in v.into_iter().enumerate() {
            assert!(prec <= el, "Sequence must be non decreasing!");
            let to_push = el & ((1 << n_lo_bits) - 1);
            let hi = (el >> n_lo_bits) + i as u64 + 1;
            // println!("to push  {:0>10b}", to_push);
            bv_lo.append_bits(to_push, n_lo_bits as usize);

            bv_hi.extend_with_zeros(((el >> n_lo_bits) - (prec >> n_lo_bits)) as usize);
            bv_hi.push(true);

            set_ptr0(prec_hi + 1, hi, i as u64);

            prec = el;
            prec_hi = hi;
        }

        set_ptr0(prec_hi, higher_bits_len, n as u64);
        bv_hi.push(false);

        // println!("---------------");
        let mut bv = BitVectorCollection::with_capacity(bv_hi.len() + bv_lo.len(), 2);
        bv.push(bv_ptrs);
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
        bv.push(bv_hi);
        // println!("pushed hi");
        // println!(
        //     "len: {} | n_bits: {} ({} u64)",
        //     bv.bv.data.len(),
        //     bv.bv.n_bits,
        //     bv.bv.n_bits / 64
        // );

        Self {
            bv,
            n,
            u,
            pointer_size: pointer_size as usize,
            n_lo_bits: n_lo_bits as usize,
        }
    }
}

impl WriteBitvector for EliasFano {
    fn write_bitvector(seq: &[u64], n: usize, u: u64) -> BitVec {
        assert!(!seq.is_empty(), "Sequence is empty");

        let n_lo_bits = if u > n as u64 { msb(u / n as u64) } else { 0 };
        let higher_bits_len = n as u64 + (u >> (n_lo_bits as usize)) + 2;

        let pointer_size = ceil_log2(higher_bits_len) as u64;

        let mut bv_lo = BitVec::new();
        let mut bv_hi = BitVec::new();
        let mut bv_ptrs = BitVec::with_zeros(
            ((higher_bits_len as usize - n) >> LOG_SAMPLING0) * pointer_size as usize,
        );

        let mut set_ptr0 = |begin: u64, end: u64, rank_end: u64| {
            let begin_zeros = begin - rank_end;
            let end_zeros = end - rank_end;

            let mut ptr0 = div_ceil(begin_zeros, 1 << LOG_SAMPLING0);

            while (ptr0 << LOG_SAMPLING0) < end_zeros {
                if ptr0 == 0 {
                    continue;
                }

                let offset = (ptr0 - 1) * pointer_size;
                bv_ptrs.set_bits(
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

            set_ptr0(prec_hi + 1, hi, i as u64);

            prec = el;
            prec_hi = hi;
        }

        set_ptr0(prec_hi, higher_bits_len, n as u64);
        bv_hi.push(false);

        // println!("---------------");
        let mut bv = BitVectorCollection::with_capacity(bv_hi.len() + bv_lo.len(), 2);
        bv.push(bv_ptrs);
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
    slice_lo: BitSliceWithOffset<'a>,
    slice_hi: BitSliceWithOffset<'a>,
    n_bits_lo: usize,
    pointer_size: usize,
    position: usize,
    hi_ctr: usize,
    i_hi: usize,
    len: usize,
    cur_value: u64,
}

impl EliasFanoIter<'_> {
    const LINEAR_SCAN_THRESHOLD: usize = 8;
}

impl IncreasingSequenceEnumerator for EliasFanoIter<'_> {
    fn next_val(&mut self) -> Option<(u64, usize)> {
        if self.position < self.len {
            let lo = self
                .slice_lo
                .get_bits(self.position * self.n_bits_lo, self.n_bits_lo)
                .unwrap();

            // while !self.slice_hi.get(self.i_hi + self.hi_ctr).expect("hi") {
            //     self.hi_ctr += 1;
            // }

            let new_pos = unsafe { self.slice_hi.next_one_unchecked(self.i_hi) };
            self.hi_ctr += new_pos - self.i_hi;
            self.i_hi = new_pos;

            self.position += 1;
            self.i_hi += 1;

            let hi = (self.hi_ctr << self.n_bits_lo) as u64;

            self.cur_value = hi | lo;
            Some((self.cur_value, self.position - 1))
        } else {
            None
        }
    }

    #[inline(always)]
    fn next_geq(&mut self, lower_bound: u64) -> Option<(u64, usize)> {
        // let lb_hi = lower_bound >> self.n_bits_lo;
        // let hi_diff = lb_hi - self.hi_ctr as u64;

        // naive way
        // let mut val = self.cur_value;
        // if i > self.cur_value {
        //     while val < i {
        //         val = self.next_val()?.0
        //     }
        // }
        // Some((val, self.i))

        let hi_lower_bound = (lower_bound >> self.n_bits_lo) as usize;
        let cur_hi = self.hi_ctr;
        let hi_diff = hi_lower_bound as usize - cur_hi;

        if self.cur_value < lower_bound && hi_diff <= Self::LINEAR_SCAN_THRESHOLD {
            let (mut val, mut pos) = self.next_val()?;
            while val < lower_bound {
                (val, pos) = self.next_val()?;
            }
            Some((val, pos))
        } else {
            //slow next geq
            let to_skip;
            if lower_bound > self.cur_value && hi_diff >> LOG_SAMPLING0 == 0 {
                to_skip = hi_diff;
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

            //TODO: fast skip0
            for _ in 0..to_skip {
                self.i_hi = self.slice_hi.next_zero(self.i_hi)? + 1;
            }

            self.position = self.i_hi - hi_lower_bound;
            self.hi_ctr = hi_lower_bound;

            let (mut val, mut pos) = self.next_val()?;
            while val < lower_bound {
                (val, pos) = self.next_val()?;
            }

            Some((val, pos))
        }
    }

    fn move_to_position(&mut self, _pos: usize) {
        todo!()
    }

    fn position(&self) -> usize {
        self.position
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
        let mut bvr = BitVec::new();
        // println!("pushing n = {}", self.n);
        bvr.append_gamma(self.n as u64);
        // println!("pushing u = {}", self.u);
        bvr.append_gamma(self.u);
        bvr.concat(&self.bv.bv);
        bvr
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
        let (n, pos) = unsafe { bv.get_gamma_unchecked(0) };
        // let n_len = gamma_size(n);

        // println!("n: {} | n_len {} | pos {}", n, n_len, pos);

        let (u, pos) = unsafe { bv.get_gamma_unchecked(pos) };
        // let u_len = gamma_size(u);

        // println!("bv len = {}", bv.len());
        // println!("u: {} | u gamma len: {}", u, u_len);

        let n_lo_bits = msb(u / n) as u64 + 1;
        // println!("n_lo_bits: {}", n_lo_bits);

        let higher_bits_len = n as u64 + (u >> (n_lo_bits as usize)) + 2;
        let pointer_size = ceil_log2(higher_bits_len) as usize;
        // let start_bits = n_len + u_len;

        // println!("splitting at bit n {}", start_bits);
        let (_, data) = bv.split_at(pos);
        // println!("ok first split");
        // println!("data len: {}", data.len());
        // println!("splitting at bit n {}", n * n_lo_bits);

        let (slice_samples, slice_remainder) = data
            .split_at(((higher_bits_len as usize - n as usize) >> LOG_SAMPLING0) * pointer_size);
        let (slice_lo, slice_hi) = slice_remainder.split_at((n * n_lo_bits) as usize);
        // println!("ok second split");

        EliasFanoIter {
            slice_samples,
            slice_lo,
            slice_hi,
            n_bits_lo: n_lo_bits as usize,
            pointer_size,
            position: 0,
            hi_ctr: 0,
            i_hi: 0,
            len: n as usize,
            cur_value: 0,
        }
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
        let (slice_lo, slice_hi) = slice_remainder.split_at((n as u64 * n_lo_bits) as usize);
        // println!("ok second split");

        EliasFanoIter {
            slice_samples,
            slice_lo,
            slice_hi,
            n_bits_lo: n_lo_bits as usize,
            pointer_size,
            position: 0,
            hi_ctr: 0,
            i_hi: 0,
            len: n as usize,
            cur_value: 0,
        }
    }
}

impl SpaceUsage for EliasFano {
    fn space_usage_byte(&self) -> usize {
        self.bv.n_bits() / 8 + 8 + 2 * mem::size_of::<usize>()
    }
}

mod tests;

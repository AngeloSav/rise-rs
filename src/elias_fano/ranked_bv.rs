use std::str::SplitInclusive;

use num::integer::div_ceil;

use crate::{
    bitvector::bitvector_collection::BitVectorCollection, utils::ceil_log2, AccessBin,
    BitSliceWithOffset, BitVec, EnumeratorFromBitSlice, EstimateSpace,
    IncreasingSequenceEnumerator, ToBitvector, WriteBitvector,
};

#[derive(Debug)]
pub struct RankedBv {
    bv: BitVec,
    n: usize,
    u: u64,
}

const LOG_RANK_SAMPLING: usize = 9; // length of buckets
const LOG_SAMPLING1: usize = 8;
const LINEAR_SCAN_THRESHOLD: usize = 8;

impl RankedBv {
    /// Returns the number of elements in the sequence
    pub fn len(&self) -> usize {
        self.n
    }

    pub fn iter(&self) -> RankedBvIter<'_> {
        Self::iter_from_slice_with_data(self.bv.as_bitslice(), self.n, self.u)
    }
}

impl<'a> From<&'a [u64]> for RankedBv {
    fn from(v: &'a [u64]) -> Self {
        let n = v.len();
        let u = *v.last().unwrap();

        let bv = Self::write_bitvector(&v, n, u);
        RankedBv { bv, n, u }
    }
}

impl ToBitvector for RankedBv {
    fn to_bv(&self) -> BitVec {
        let mut bv = BitVec::new();
        bv.append_gamma(self.n as u64);
        bv.append_gamma(self.u);
        bv.concat(&self.bv);
        bv
    }
}

impl WriteBitvector for RankedBv {
    fn write_bitvector(seq: &[u64], n: usize, u: u64) -> BitVec {
        let rank_sample_size = ceil_log2(n + 1) as u64;
        let pointer_size = ceil_log2(u);

        let mut bv = BitVec::with_zeros(1 + u as usize);
        let mut samples =
            BitVec::with_zeros((u as usize >> LOG_RANK_SAMPLING) * rank_sample_size as usize);
        let mut samples1 =
            BitVec::with_zeros((n as usize >> LOG_SAMPLING1) * pointer_size as usize);

        let mut set_rank_samples = |begin: u64, end: u64, rank: u64| {
            let mut sample = div_ceil(begin, 1 << LOG_RANK_SAMPLING);
            while (sample << LOG_RANK_SAMPLING) < end {
                if sample == 0 {
                    continue;
                }
                let offset = (sample - 1) * rank_sample_size;
                // println!("writing {} {} {}", offset, rank_sample_size, rank);
                samples.set_bits(offset as usize, rank_sample_size as usize, rank);

                sample += 1;
            }
        };

        let mut prec = 0;
        for (i, &el) in seq.into_iter().enumerate() {
            assert!(i == 0 || prec < el, "Sequence must be strictly increasing!");
            bv.set(el as usize, true);

            if i != 0 && i % (1 << LOG_SAMPLING1) == 0 {
                let ptr1 = i >> LOG_SAMPLING1;
                let off = (ptr1 - 1) * pointer_size as usize;
                samples1.set_bits(off, pointer_size as usize, el);
            }

            set_rank_samples(prec + 1, el + 1, i as u64);
            prec = el;
        }

        set_rank_samples(prec + 1, u, n as u64);

        let mut bvc = BitVectorCollection::with_capacity(bv.len() + samples.len(), 2);
        bvc.push(bv);
        bvc.push(samples);
        bvc.push(samples1);

        bvc.bv
    }
}

impl<'a> EnumeratorFromBitSlice<'a, RankedBvIter<'a>> for RankedBv {
    fn iter_from_slice(bv: BitSliceWithOffset<'a>) -> RankedBvIter<'a> {
        let (n, next_pos) = unsafe { bv.get_gamma_unchecked(0) };
        let (u, next_pos) = unsafe { bv.get_gamma_unchecked(next_pos) };
        let bv = bv.split_at(next_pos).1;

        Self::iter_from_slice_with_data(bv, n as usize, u)
    }

    fn iter_from_slice_with_data(bv: BitSliceWithOffset<'a>, n: usize, u: u64) -> RankedBvIter<'a> {
        let rank_sample_size = ceil_log2(n + 1) as usize;
        let pointer_size = ceil_log2(u) as usize;

        let start_samples = u as usize + 1;
        let end_samples = start_samples + (u as usize >> LOG_RANK_SAMPLING) * rank_sample_size;

        let data_slice = bv.slice(0, start_samples); //maybe not +1?
        let sample_slice = bv.slice(start_samples, end_samples);
        let samples1_slice = bv.split_at(end_samples).1;

        RankedBvIter {
            data: data_slice,
            samples: sample_slice,
            samples1: samples1_slice,
            rank_sample_size,
            pointer_size,
            value: 0,
            position: 0,
            u,
            n,
        }
    }
}

impl EstimateSpace for RankedBv {
    fn bitsize(u: u64, n: usize) -> usize {
        let rank_sample_size = ceil_log2(n + 1) as usize;
        let sample_space = (u as usize >> LOG_RANK_SAMPLING) * rank_sample_size as usize;
        u as usize + sample_space
    }
}

#[derive(Debug)]
pub struct RankedBvIter<'a> {
    data: BitSliceWithOffset<'a>,
    samples: BitSliceWithOffset<'a>,
    samples1: BitSliceWithOffset<'a>,
    rank_sample_size: usize,
    pointer_size: usize,
    value: usize,
    position: usize,
    u: u64,
    n: usize,
}

impl RankedBvIter<'_> {
    const LINEAR_SCAN_THRESHOLD: u64 = 8;

    fn slow_move(&mut self, pos: usize) -> Option<(u64, usize)> {
        if pos >= self.n {
            self.position = self.n;
            return None;
        }

        let skip: isize = pos as isize - self.position as isize + 1;
        let to_skip;
        if pos >= self.position && skip >> LOG_SAMPLING1 == 0 {
            to_skip = skip as usize - 1;
        } else {
            let ptr = pos >> LOG_SAMPLING1;
            let ptr_pos = if ptr == 0 {
                0
            } else {
                unsafe {
                    self.samples1.get_bits_unchecked(
                        (ptr - 1) as usize * self.pointer_size,
                        self.pointer_size,
                    )
                }
            };

            self.value = ptr_pos as usize;
            to_skip = pos - ((ptr as usize) << LOG_SAMPLING1);
        }

        if to_skip != 0 {
            self.value = self.data.skip_ones(self.value, to_skip - 1)? + 1;
        }
        self.position = pos;

        self.next_val()
    }
}

impl IncreasingSequenceEnumerator for RankedBvIter<'_> {
    fn next_val(&mut self) -> Option<(u64, usize)> {
        if self.value >= self.u as usize {
            None
        } else {
            let new_pos = unsafe { self.data.next_one_unchecked(self.value) };
            self.value = new_pos + 1;

            self.position += 1;
            Some((new_pos as u64, self.position - 1))
        }
    }

    fn next_geq(&mut self, lower_bound: u64) -> Option<(u64, usize)> {
        // println!("lower bound is {}", lower_bound);
        // println!("self.value {}", self.value);
        if lower_bound + 1 == self.value as u64 {
            // println!("here 1");
            return Some((self.value as u64 - 1, self.position - 1));
        }

        // let diff = lower_bound - self.value as u64;
        if lower_bound > self.value as u64
            && (lower_bound - self.value as u64) < Self::LINEAR_SCAN_THRESHOLD
        {
            // println!("here 2");
            let (mut val, mut pos) = self.next_val()?;
            while val < lower_bound {
                (val, pos) = self.next_val()?;
            }
            Some((val, pos))
        } else {
            // println!("here 3");
            //slow next_geq
            if lower_bound >= self.u as u64 {
                return self.move_to_position(self.n);
            }

            // let skip = lower_bound - self.value as u64;
            let begin;
            if lower_bound > self.value as u64
                && (lower_bound - self.value as u64) >> LOG_RANK_SAMPLING == 0
            {
                // println!("here again 1");
                begin = self.value;
            } else {
                // println!("here again 2");
                let block = (lower_bound >> LOG_RANK_SAMPLING) as usize;
                self.position = if block == 0 {
                    0
                } else {
                    unsafe {
                        self.samples.get_bits_unchecked(
                            (block - 1) * self.rank_sample_size,
                            self.rank_sample_size,
                        ) as usize
                    }
                };

                begin = block << LOG_RANK_SAMPLING;
                // println!("ones up until {} : {}", begin, self.position);
            }

            self.position += self.data.rank_range(begin, lower_bound as usize);

            self.value = lower_bound as usize;

            self.next_val()
        }
    }

    fn move_to_position(&mut self, pos: usize) -> Option<(u64, usize)> {
        // println!("pos is : {}", self.position);
        let skip = pos as isize - self.position as isize + 1;

        if self.position <= pos && skip <= LINEAR_SCAN_THRESHOLD as isize {
            let mut skipped = 1;
            while skipped < skip {
                self.next_val()?;
                skipped += 1;
            }

            return self.next_val();
        }

        self.slow_move(pos)
    }

    fn current_position(&self) -> usize {
        self.position - 1
    }

    fn len(&self) -> usize {
        self.n
    }
}

impl Iterator for RankedBvIter<'_> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.next_val()?.0)
    }
}

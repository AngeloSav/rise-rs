use num::integer::div_ceil;

use crate::{
    bitvector::bitvector_collection::BitVectorCollection, utils::ceil_log2, BitSliceWithOffset,
    BitVec, BitVecCollection, EnumeratorFromBitSlice, EstimateSpace, IncreasingSequenceEnumerator,
    ToBitvector,
};

use super::gamma_size;

#[derive(Debug)]
pub struct RankedBv {
    bv: BitVecCollection,
    n: usize,
    u: u64,
}

const LOG_RANK_SAMPLING: usize = 6; // length of buckets

impl RankedBv {
    /// Returns the number of elements in the sequence
    pub fn len(&self) -> usize {
        self.n
    }

    pub fn iter(&self) -> RankedBvIter<'_> {
        RankedBvIter {
            data: self.bv.get(0),
            samples: self.bv.get(1),
            pos: 0,
            i: 0,
            len: self.u as usize,
        }
    }
}

impl From<Vec<u64>> for RankedBv {
    fn from(v: Vec<u64>) -> Self {
        let u = *v.last().unwrap();
        let n = v.len();

        let rank_sample_size = ceil_log2(n + 1) as u64;

        let mut bv = BitVec::with_zeros(1 + u as usize);
        let mut samples =
            BitVec::with_zeros((u as usize >> LOG_RANK_SAMPLING) * rank_sample_size as usize);

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
        for (i, el) in v.into_iter().enumerate() {
            assert!(i == 0 || prec < el, "Sequence must be strictly increasing!");
            bv.set(el as usize, true);

            set_rank_samples(prec + 1, el + 1, i as u64);
            prec = el;
        }

        set_rank_samples(prec + 1, u, n as u64);

        let mut bvc = BitVectorCollection::with_capacity(bv.len() + samples.len(), 2);
        bvc.push(bv);
        bvc.push(samples);

        Self { bv: bvc, n, u }
    }
}

impl ToBitvector for RankedBv {
    fn to_bv(&self) -> BitVec {
        let mut bv = BitVec::new();
        bv.append_gamma(self.n as u64);
        bv.append_gamma(self.u as u64);
        bv.concat(&self.bv.bv);
        bv
    }
}

impl<'a> EnumeratorFromBitSlice<'a, RankedBvIter<'a>> for RankedBv {
    fn iter_from_slice(bv: BitSliceWithOffset<'a>) -> RankedBvIter<'a> {
        let n = unsafe { bv.get_gamma_unchecked(0) }.0;
        let u = unsafe { bv.get_gamma_unchecked(gamma_size(n)) }.0;
        let start_data = gamma_size(n) + gamma_size(u);
        let rank_sample_size = ceil_log2(n + 1) as u64;

        let start_samples = start_data + u as usize + 1;
        let end_samples =
            start_samples + (u as usize >> LOG_RANK_SAMPLING) * rank_sample_size as usize;

        let data_slice = bv.slice(start_data, start_samples); //maybe not +1?
        let sample_slice = bv.slice(start_samples, end_samples);

        RankedBvIter {
            data: data_slice,
            samples: sample_slice,
            pos: 0,
            i: 0,
            len: u as usize,
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
    pos: usize,
    i: usize,
    len: usize,
}

impl IncreasingSequenceEnumerator for RankedBvIter<'_> {
    fn next_val(&mut self) -> Option<(u64, usize)> {
        if self.pos > self.len {
            None
        } else {
            let new_pos = unsafe { self.data.next_one_unchecked(self.pos) };
            self.pos = new_pos + 1;

            self.i += 1;
            Some((new_pos as u64, self.i))
        }
    }

    fn next_geq(&mut self, i: u64) -> Option<(u64, usize)> {
        todo!()
    }

    fn move_to_position(&mut self, pos: usize) {
        todo!()
    }

    fn position(&self) -> usize {
        todo!()
    }
}

impl Iterator for RankedBvIter<'_> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.next_val()?.0)
    }
}

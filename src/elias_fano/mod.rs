use std::mem;

use crate::{
    bitvector::bitvector_collection::BitVectorCollection, space_usage::SpaceUsage, utils::msb,
    BitSliceWithOffset, BitVec, BitVecCollection, EnumeratorFromBitSlice,
    IncreasingSequenceEnumerator, ToBitvector,
};

pub mod uniform_partitioned_seq;

#[derive(Debug, Default)]
pub struct EliasFano {
    bv: BitVecCollection,
    n: usize,
    u: u64,
    n_lo_bits: usize,
}

impl EliasFano {
    /// Returns the number of elements in the sequence
    pub fn len(&self) -> usize {
        self.n
    }

    pub fn iter(&self) -> EliasFanoIter {
        EliasFanoIter {
            slice_lo: self.bv.get(0),
            slice_hi: self.bv.get(1),
            n_bits_lo: self.n_lo_bits,
            i: 0,
            hi_ctr: 0,
            i_hi: 0,
            len: self.len(),
            cur_value: 0,
        }
    }
}

impl From<Vec<u64>> for EliasFano {
    fn from(v: Vec<u64>) -> Self {
        assert!(!v.is_empty(), "Sequence is empty");

        let mut bv_lo = BitVec::new();
        let mut bv_hi = BitVec::new();

        let u = *v.last().unwrap();
        let n = v.len();

        // let n_bits = msb(u) + 1;
        let n_lo_bits = msb(u / v.len() as u64) + 1;

        let mut prec = 0;
        for el in v {
            assert!(prec <= el, "Sequence must be non decreasing!");
            let to_push = el & ((1 << n_lo_bits) - 1);
            // println!("to push  {:0>10b}", to_push);
            bv_lo.append_bits(to_push, n_lo_bits as usize);

            bv_hi.extend_with_zeros(((el >> n_lo_bits) - (prec >> n_lo_bits)) as usize);
            bv_hi.push(true);

            prec = el;
        }
        bv_hi.push(false);

        // println!("---------------");
        let mut bv = BitVectorCollection::with_capacity(bv_hi.len() + bv_lo.len(), 2);
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
            n_lo_bits: n_lo_bits as usize,
        }
    }
}

#[derive(Debug, Default)]
pub struct EliasFanoIter<'a> {
    slice_lo: BitSliceWithOffset<'a>,
    slice_hi: BitSliceWithOffset<'a>,
    n_bits_lo: usize,
    i: usize,
    hi_ctr: usize,
    i_hi: usize,
    len: usize,
    cur_value: u64,
}

impl IncreasingSequenceEnumerator for EliasFanoIter<'_> {
    fn next_val(&mut self) -> Option<(u64, usize)> {
        if self.i < self.len {
            let lo = self
                .slice_lo
                .get_bits(self.i * self.n_bits_lo, self.n_bits_lo)
                .unwrap();

            // while !self.slice_hi.get(self.i_hi + self.hi_ctr).expect("hi") {
            //     self.hi_ctr += 1;
            // }

            let new_pos = unsafe { self.slice_hi.next_one_unchecked(self.i_hi) };
            self.hi_ctr += new_pos - self.i_hi;
            self.i_hi = new_pos;

            self.i += 1;
            self.i_hi += 1;

            let hi = (self.hi_ctr << self.n_bits_lo) as u64;

            self.cur_value = hi | lo;
            Some((self.cur_value, self.i))
        } else {
            None
        }
    }

    fn next_geq(&mut self, i: u64) -> Option<(u64, usize)> {
        // let lb_hi = lower_bound >> self.n_bits_lo;
        // let hi_diff = lb_hi - self.hi_ctr as u64;

        let mut val = self.cur_value;
        if i > self.cur_value {
            while val < i {
                val = self.next_val()?.0
            }
        }
        Some((val, self.i))
    }

    fn move_to_position(&mut self, _pos: usize) {
        todo!()
    }

    fn position(&self) -> usize {
        self.i
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

impl<'a> EnumeratorFromBitSlice<'a, EliasFanoIter<'a>> for EliasFano {
    fn iter_from_slice(bv: BitSliceWithOffset<'a>) -> EliasFanoIter<'a> {
        let (n, pos) = unsafe { bv.get_gamma_unchecked(0) };
        let n_len = gamma_size(n);

        // println!("n: {} | n_len {} | pos {}", n, n_len, pos);

        let (u, _) = unsafe { bv.get_gamma_unchecked(n_len) };
        let u_len = gamma_size(u);

        // println!("bv len = {}", bv.len());
        // println!("u: {} | u gamma len: {}", u, u_len);

        let n_lo_bits = msb(u / n) as u64 + 1;
        // println!("n_lo_bits: {}", n_lo_bits);

        let start_bits = n_len + u_len;

        // println!("splitting at bit n {}", start_bits);
        let (_, data) = bv.split_at(start_bits);
        // println!("ok first split");
        // println!("data len: {}", data.len());
        // println!("splitting at bit n {}", n * n_lo_bits);
        let (slice_lo, slice_hi) = data.split_at((n * n_lo_bits) as usize);
        // println!("ok second split");

        EliasFanoIter {
            slice_lo,
            slice_hi,
            n_bits_lo: n_lo_bits as usize,
            i: 0,
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

fn gamma_size(n: u64) -> usize {
    (msb(n + 1) * 2 + 1) as usize
}

mod tests;

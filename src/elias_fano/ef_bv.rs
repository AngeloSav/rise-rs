use crate::{utils::msb, BitVec};

pub struct EliasFano2 {
    bv_hi: BitVec,
    bv_lo: BitVec,
    n: usize,
    n_lo_bits: usize,
}

impl EliasFano2 {
    /// Returns the number of elements in the sequence
    pub fn len(&self) -> usize {
        self.n
    }

    pub fn iter(&self) -> EliasFanoIter2 {
        EliasFanoIter2 {
            slice_lo: &self.bv_lo,
            slice_hi: &self.bv_hi,
            n_bits_lo: self.n_lo_bits,
            i: 0,
            hi_ctr: 0,
            i_hi: 0,
            len: self.len(),
        }
    }
}

impl From<Vec<u64>> for EliasFano2 {
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

        Self {
            bv_lo,
            bv_hi,
            n,
            n_lo_bits: n_lo_bits as usize,
        }
    }
}

// this WORKS
pub struct EliasFanoIter2<'a> {
    slice_lo: &'a BitVec,
    slice_hi: &'a BitVec,
    n_bits_lo: usize,
    i: usize,
    hi_ctr: usize,
    i_hi: usize,
    len: usize,
}

impl Iterator for EliasFanoIter2<'_> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        if self.i < self.len {
            let lo = self
                .slice_lo
                .get_bits(self.i * self.n_bits_lo, self.n_bits_lo)
                .unwrap();

            // while self.slice_hi.get(self.i_hi + self.hi_ctr).expect("hi") != true {
            //     self.hi_ctr += 1;
            // }

            let new_pos = self.slice_hi.next_one(self.i_hi)?;
            self.hi_ctr += new_pos - self.i_hi;
            self.i_hi = new_pos;

            self.i += 1;
            self.i_hi += 1;

            let hi = (self.hi_ctr << self.n_bits_lo) as u64;

            Some(hi | lo)
        } else {
            None
        }
    }
}

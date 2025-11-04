use crate::{BitVec, EnumeratorFromBitSlice, EstimateSpace, SequenceEnumerator, WriteBitvector};

use super::{EliasFano, EliasFanoIter};

#[derive(Debug)]
pub struct StrictEliasFano {
    ef: EliasFano,
}

impl StrictEliasFano {
    /// Returns the number of elements in the sequence
    pub fn len(&self) -> usize {
        self.ef.len()
    }

    pub fn iter(&self) -> StrictEliasFanoIter<'_> {
        StrictEliasFanoIter {
            it: self.ef.iter(),
            cur_value: (self.ef.u, self.ef.len()),
        }
    }
}

impl<'a> From<&'a [u64]> for StrictEliasFano {
    fn from(v: &'a [u64]) -> Self {
        let v: Vec<_> = v
            .iter()
            .enumerate()
            .map(|(i, &x)| {
                x.checked_sub(i as u64)
                    .expect("Sequence should be strictly increasing!")
            })
            .collect();
        let ef = EliasFano::from(v.as_ref());

        Self { ef }
    }
}

impl WriteBitvector for StrictEliasFano {
    fn write_bitvector(seq: &[u64], n: usize, u: u64) -> BitVec {
        assert!(u >= n as u64);
        let new_u = u - n as u64 + 1;

        let v: Vec<_> = seq
            .iter()
            .enumerate()
            .map(|(i, &x)| {
                x.checked_sub(i as u64)
                    .expect("Sequence should be strictly increasing!")
            })
            .collect();

        EliasFano::write_bitvector(v.as_ref(), v.len(), new_u)
    }
}

impl<'a> EnumeratorFromBitSlice<'a> for StrictEliasFano {
    type IterType = StrictEliasFanoIter<'a>;

    fn iter_from_slice(bv: crate::BitSliceWithOffset<'a>, n: usize, u: u64) -> Self::IterType {
        let new_u = u - n as u64 + 1;

        StrictEliasFanoIter {
            it: EliasFano::iter_from_slice(bv, n, new_u),
            cur_value: (new_u, n),
        }
    }
}

#[derive(Debug)]
pub struct StrictEliasFanoIter<'a> {
    it: EliasFanoIter<'a>,
    cur_value: (u64, usize),
}

impl SequenceEnumerator for StrictEliasFanoIter<'_> {
    fn next_val(&mut self) -> (u64, usize) {
        let (val, pos) = self.it.next_val();
        self.cur_value = (val + pos as u64, pos);
        self.cur_value
    }

    fn move_to_position(&mut self, pos: usize) -> (u64, usize) {
        let (val, pos) = self.it.move_to_position(pos);
        self.cur_value = (val + pos as u64, pos);
        self.cur_value
    }

    fn len(&self) -> usize {
        self.it.len()
    }
}

impl Iterator for StrictEliasFanoIter<'_> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        let (val, pos) = self.next_val();
        if pos == self.len() {
            return None;
        }
        Some(val)
    }
}

impl EstimateSpace for StrictEliasFano {
    fn bitsize(u: u64, n: usize) -> usize {
        assert!(u >= n as u64);
        EliasFano::bitsize(u + 1 - n as u64, n)
    }
}

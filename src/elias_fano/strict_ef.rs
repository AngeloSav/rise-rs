use crate::{
    BitVec, EnumeratorFromBitSlice, EstimateSpace, SequenceEnumerator, ToBitvector, WriteBitvector,
};

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

    pub fn iter(&self) -> StrictEliasFanoIter {
        StrictEliasFanoIter {
            it: self.ef.iter(),
            cur_value: None,
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

impl ToBitvector for StrictEliasFano {
    fn to_bv(&self) -> BitVec {
        todo!()
    }
}

impl WriteBitvector for StrictEliasFano {
    fn write_bitvector(seq: &[u64], n: usize, u: u64) -> BitVec {
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

    fn iter_from_slice(_bv: crate::BitSliceWithOffset<'a>) -> Self::IterType {
        todo!()
    }

    fn iter_from_slice_with_data(
        bv: crate::BitSliceWithOffset<'a>,
        n: usize,
        u: u64,
    ) -> Self::IterType {
        let new_u = u - n as u64 + 1;

        StrictEliasFanoIter {
            it: EliasFano::iter_from_slice_with_data(bv, n, new_u),
            cur_value: None,
        }
    }
}

#[derive(Debug)]
pub struct StrictEliasFanoIter<'a> {
    it: EliasFanoIter<'a>,
    cur_value: Option<(u64, usize)>,
}

impl SequenceEnumerator for StrictEliasFanoIter<'_> {
    fn next_val(&mut self) -> Option<(u64, usize)> {
        let (val, pos) = self.it.next_val()?;
        self.cur_value = Some((val + pos as u64, pos));
        self.cur_value
    }

    // fn next_geq(&mut self, lower_bound: u64) -> Option<(u64, usize)> {
    //     //NOTE: this is not an effiecient implementation of nextGEQ, but works well in the context of partitioned EF
    //     // let lb_new = lower_bound.saturating_sub(self.it.len() as u64);
    //     // let (mut val, mut pos) = self.it.next_geq(lb_new)?;
    //     // val = val + pos as u64;

    //     // while val < lower_bound {
    //     //     (val, pos) = self.next_val()?;
    //     // }

    //     // Some((val, pos))

    //     // we dont reset in case of equality because the sequence is strictly increasing
    //     if self.cur_value.is_none_or(|(x, _)| x > lower_bound) {
    //         self.cur_value = self
    //             .it
    //             .next_geq(lower_bound.saturating_sub(self.len() as u64))
    //             .map(|(val, pos)| (val + pos as u64, pos))
    //     }

    //     while self.cur_value.is_some_and(|(x, _)| x < lower_bound) {
    //         self.next_val();
    //     }

    //     self.cur_value
    // }

    fn move_to_position(&mut self, pos: usize) -> Option<(u64, usize)> {
        let (val, pos) = self.it.move_to_position(pos)?;
        self.cur_value = Some((val + pos as u64, pos));
        self.cur_value
    }

    fn len(&self) -> usize {
        self.it.len()
    }
}

impl Iterator for StrictEliasFanoIter<'_> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.next_val()?.0)
    }
}

impl EstimateSpace for StrictEliasFano {
    fn bitsize(u: u64, n: usize) -> usize {
        EliasFano::bitsize(u - n as u64 + 1, n)
    }
}

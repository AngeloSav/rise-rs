use std::marker::PhantomData;

use epserde::prelude::*;

use crate::{
    BitSliceWithOffset, BitVec, EnumeratorFromBitSlice, SequenceEnumerator, WriteBitvector,
    indexes::freq_index::FreqList,
};

#[derive(Debug, Default, Epserde)]
pub struct PositiveSequence<BaseSequence> {
    bv: BitVec,
    n: usize,
    u: u64,
    _phantom: PhantomData<BaseSequence>,
}

impl<'a, BaseSequence> PositiveSequence<BaseSequence> where BaseSequence: FreqList {}

impl<'a, BaseSequence> WriteBitvector for PositiveSequence<BaseSequence>
where
    BaseSequence: FreqList,
{
    fn write_bitvector(seq: impl IntoIterator<Item = u64>, n: usize, _u: u64) -> BitVec {
        // we can discard u as we build a new seqeunce
        let psum = seq
            .into_iter()
            .scan(0, |s, el| {
                *s += el;
                Some(*s)
            })
            .collect::<Vec<_>>();

        assert!(psum.len() == n);
        // let n = psum.len();

        let u = *psum.last().unwrap() + 1;

        let mut bv = BitVec::new();
        bv.append_gamma_nonzero(u);

        bv.concat(BaseSequence::write_bitvector(psum.into_iter(), n, u));

        bv
    }
}

impl<'a, BaseSequence> From<&'a [u64]> for PositiveSequence<BaseSequence>
where
    BaseSequence: FreqList,
{
    fn from(_value: &'a [u64]) -> Self {
        todo!()
    }
}

impl<'a, BaseSequence> EnumeratorFromBitSlice<'a> for PositiveSequence<BaseSequence>
where
    BaseSequence: FreqList,
{
    type IterType = PositiveSequenceIter<'a, BaseSequence>;

    fn iter_from_slice(bv: BitSliceWithOffset<'a>, n: usize, _u: u64) -> Self::IterType {
        let (u, next_pos) = unsafe { bv.get_gamma_nonzero_unchecked(0) };
        // println!("u: {}, n: {}", u, n);
        let bv = bv.split_at(next_pos).1;
        let it = BaseSequence::iter_from_slice(bv, n, u);
        PositiveSequenceIter {
            it,
            prev: 0,
            pos: 0,
        }
    }
}

#[derive(Debug)]
pub struct PositiveSequenceIter<'a, BaseSequence>
where
    BaseSequence: FreqList,
{
    it: <BaseSequence as EnumeratorFromBitSlice<'a>>::IterType,
    prev: u64,
    pos: usize,
}

impl<'a, BaseSequence> SequenceEnumerator for PositiveSequenceIter<'a, BaseSequence>
where
    BaseSequence: FreqList,
{
    fn next_val(&mut self) -> (u64, usize) {
        let (cur, pos) = self.it.next_val();
        let actual_val = cur - self.prev;
        self.prev = cur;
        self.pos = pos + 1;
        (actual_val, pos)
    }

    fn move_to_position(&mut self, pos: usize) -> (u64, usize) {
        if core::intrinsics::likely(pos != self.pos) {
            if pos == 0 {
                let (cur, pos) = self.it.move_to_position(0);
                self.prev = cur;
                self.pos = pos + 1;
                return (cur, pos);
            } else {
                self.prev = self.it.move_to_position(pos - 1).0
            }
        }
        self.next_val()
    }

    fn len(&self) -> usize {
        self.it.len()
    }
}

impl<'a, BaseSequence> Iterator for PositiveSequenceIter<'a, BaseSequence>
where
    BaseSequence: FreqList,
{
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        let (val, pos) = self.next_val();
        if pos == self.len() {
            return None;
        }
        Some(val)
    }
}

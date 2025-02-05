use std::{marker::PhantomData, mem};

use serde::{Deserialize, Serialize};

use crate::{
    indexes::freq_index::PostingList, space_usage::SpaceUsage, utils::ceil_log2,
    BitSliceWithOffset, BitVec, EnumeratorFromBitSlice, IncreasingSequenceEnumerator, ToBitvector,
    WriteBitvector,
};

use super::{EliasFano, EliasFanoIter};

#[derive(Debug, Serialize, Deserialize)]
pub struct UniformPartitionedSequence<BaseSequence> {
    n: usize,
    u: u64,
    bv: BitVec,
    _phantom: PhantomData<BaseSequence>,
}

const PARTITION_SIZE: usize = 128;

impl<'a, BaseSequence> UniformPartitionedSequence<BaseSequence>
where
    BaseSequence: PostingList<'a>,
{
    pub fn len(&self) -> usize {
        self.n
    }

    pub fn iter(&'a self) -> UniformPartitionedSeqIter<'a, BaseSequence> {
        Self::iter_from_slice_with_data(self.bv.as_bitslice(), self.n, self.u)
    }
}

impl<'a, 'b, BaseSequence> From<&'b [u64]> for UniformPartitionedSequence<BaseSequence>
where
    BaseSequence: PostingList<'a>,
{
    fn from(v: &'b [u64]) -> Self {
        let n = v.len();
        let u = *v.last().unwrap() + 1;
        let bv = Self::write_bitvector(v, n, u);

        Self {
            bv,
            n,
            u,
            _phantom: PhantomData,
        }
    }
}

impl<'a, BaseSequence> ToBitvector for UniformPartitionedSequence<BaseSequence>
where
    BaseSequence: PostingList<'a>,
{
    fn to_bv(&self) -> BitVec {
        let mut bv = BitVec::new();
        bv.append_gamma(self.n as u64);
        bv.append_gamma(self.u);
        bv.concat(&self.bv);
        bv
    }
}

impl<'a, BaseSequence> WriteBitvector for UniformPartitionedSequence<BaseSequence>
where
    BaseSequence: PostingList<'a> + WriteBitvector,
{
    fn write_bitvector(seq: &[u64], n: usize, u: u64) -> BitVec {
        assert!(n > 0);
        let mut bv = BitVec::new();
        let n_partitions = usize::div_ceil(n, PARTITION_SIZE);

        bv.append_gamma(n_partitions as u64);

        if n_partitions == 1 {
            let cur_base = seq[0];
            let cur_partition = seq.iter().map(|&x| x - cur_base).collect::<Vec<_>>();

            let universe_bits = ceil_log2(u) as usize;
            bv.append_bits(cur_base, universe_bits);

            if n > 1 {
                if cur_base + *cur_partition.last().unwrap() + 1 == u {
                    bv.append_delta(0);
                } else {
                    bv.append_delta(*cur_partition.last().unwrap());
                }
            }

            bv.concat(BaseSequence::write_bitvector(
                &cur_partition,
                cur_partition.len(),
                *cur_partition.last().unwrap() + 1,
            ));
        } else {
            let mut cur_partition: Vec<u64>;
            let mut upper_bounds: Vec<u64> = Vec::new();
            let mut bv_sequences: BitVec = BitVec::new();

            let mut endpoints = Vec::new();
            let mut it = seq.into_iter();

            let mut cur_base = seq[0];
            upper_bounds.push(cur_base);

            for _ in 0..n_partitions {
                cur_partition = (&mut it).take(PARTITION_SIZE).copied().collect();

                // let cur_base = cur_partition[0];
                // upper_bounds.push(cur_base);
                let new_ub = *cur_partition.last().unwrap();

                for el in cur_partition.iter_mut() {
                    *el -= cur_base;
                }

                // println!(
                //     "NEW SEQ n {} | u {}",
                //     cur_partition.len(),
                //     *cur_partition.last().unwrap() + 1
                // );
                bv_sequences.concat(BaseSequence::write_bitvector(
                    &cur_partition,
                    cur_partition.len(),
                    *cur_partition.last().unwrap() + 1,
                ));

                upper_bounds.push(new_ub);
                cur_base = new_ub + 1;
                endpoints.push(bv_sequences.len());
            }

            // println!("ubs : {:?}", upper_bounds);
            // println!("ubs len: {:?}", upper_bounds.len());
            let bv_upper_bounds = EliasFano::write_bitvector(&upper_bounds, n_partitions + 1, u);
            let endpoint_bits = ceil_log2(bv_sequences.len() + 1);
            bv.append_gamma(endpoint_bits as u64);

            // println!(
            //     "ubs START: {:?} | n {} | u {}",
            //     bv.len(),
            //     n_partitions,
            //     u + 1
            // );
            bv.concat(bv_upper_bounds);

            // println!("bvlen so far: {}", bv.len());

            // println!("endpoints: {:?}", endpoints);
            for e in endpoints {
                bv.append_bits(e as u64, endpoint_bits as usize);
            }

            // println!("sequences start @ {}", bv.len());

            bv.concat(bv_sequences);
        }

        bv
    }
}

impl<'a, BaseSequence> EnumeratorFromBitSlice<'a> for UniformPartitionedSequence<BaseSequence>
where
    BaseSequence: PostingList<'a>,
{
    type IterType = UniformPartitionedSeqIter<'a, BaseSequence>;

    fn iter_from_slice(bv: BitSliceWithOffset<'a>) -> Self::IterType {
        let (n, next_pos) = unsafe { bv.get_gamma_unchecked(0) };
        let (u, next_pos) = unsafe { bv.get_gamma_unchecked(next_pos) };
        Self::iter_from_slice_with_data(bv.split_at(next_pos).1, n as usize, u)
    }

    fn iter_from_slice_with_data(bv: BitSliceWithOffset<'a>, n: usize, u: u64) -> Self::IterType {
        let (n_partitions, mut next_pos) = unsafe { bv.get_gamma_unchecked(0) };
        let n_partitions = n_partitions as usize;

        if n_partitions == 1 {
            let universe_bits = ceil_log2(u);
            let cur_base = unsafe { bv.get_bits_unchecked(next_pos, universe_bits as usize) };

            let mut ub = 0;
            if n > 1 {
                let (universe_delta, np) =
                    unsafe { bv.get_delta_unchecked(next_pos + universe_bits as usize) };
                ub = if universe_delta != 0 {
                    universe_delta
                } else {
                    u - cur_base - 1
                };
                next_pos = np;
            }
            let cur_sequence =
                BaseSequence::iter_from_slice_with_data(bv.split_at(next_pos).1, n, ub + 1);

            return UniformPartitionedSeqIter {
                position: 0,
                cur_base,
                cur_ub: u,
                cur_begin: 0,
                cur_end: n,
                cur_partition: 0,
                upper_bounds: EliasFanoIter::default(),
                n_partitions: 1,
                endpoints: Vec::default(),
                sequences: BitSliceWithOffset::default(),
                cur_sequence,
                cur_value: 0,
                _phantom: PhantomData,
                len: n,
                universe: u,
            };
        } else {
            let (endpoint_bits, np) = unsafe { bv.get_gamma_unchecked(next_pos) };
            next_pos = np;
            // println!(
            //     "ubs START: {:?} | n {} | u {}",
            //     next_pos,
            //     n_partitions,
            //     u + 1
            // );
            let mut upper_bounds =
                EliasFano::iter_from_slice_with_data(bv.split_at(next_pos).1, n_partitions + 1, u);
            next_pos += EliasFano::n_bits(u, n_partitions + 1);

            // println!("next_pos {:?}", next_pos);

            let mut endpoints = vec![0];
            if endpoint_bits != 0 {
                for idx in (next_pos..)
                    .step_by(endpoint_bits as usize)
                    .take(n_partitions)
                {
                    endpoints.push(bv.get_bits(idx, endpoint_bits as usize).unwrap() as usize);
                }
            } else {
                for _ in 0..n_partitions {
                    endpoints.push(0);
                }
            }

            // println!("endpoints: {:?}", endpoints);

            // println!(
            //     "sequences start @ {}",
            //     next_pos + endpoint_bits as usize * (n_partitions)
            // );
            let sequences = bv
                .split_at(next_pos + endpoint_bits as usize * (n_partitions))
                .1;

            // println!("sequences len: {:?}", sequences.len());

            // println!("ubs: {:?}", upper_bounds.collect::<Vec<_>>());
            // todo!();

            let cur_base = upper_bounds.next().unwrap();
            let cur_ub = upper_bounds.next().unwrap();
            let cur_begin = 0;
            let cur_end = 1 * PARTITION_SIZE;

            // println!(
            //     "NEW SEQ n {} | u {}",
            //     cur_end - cur_begin,
            //     cur_ub - cur_base + 1
            // );
            let cur_sequence = BaseSequence::iter_from_slice_with_data(
                sequences.slice(endpoints[0], endpoints[1]),
                cur_end,
                cur_ub - cur_base + 1,
            );

            // println!("cur seq: {:?}", cur_sequence.collect::<Vec<_>>());
            // todo!();

            UniformPartitionedSeqIter {
                position: 0,
                cur_partition: 0,
                cur_base,
                cur_ub,
                cur_begin,
                cur_end,
                upper_bounds,
                n_partitions: n_partitions as usize,
                endpoints,
                sequences,
                cur_sequence,
                cur_value: 0,
                len: n,
                _phantom: PhantomData,
                universe: u,
            }
        }
    }
}

#[derive(Debug)]
pub struct UniformPartitionedSeqIter<'a, BaseSequence>
where
    BaseSequence: PostingList<'a>,
{
    position: usize,
    cur_partition: usize,
    cur_base: u64,
    upper_bounds: EliasFanoIter<'a>,
    n_partitions: usize,
    endpoints: Vec<usize>,
    sequences: BitSliceWithOffset<'a>,
    cur_sequence: BaseSequence::IterType,
    cur_value: u64,
    _phantom: PhantomData<BaseSequence>,
    cur_ub: u64,
    cur_begin: usize,
    cur_end: usize,
    len: usize,
    universe: u64,
}

impl<'a, BaseSequence> UniformPartitionedSeqIter<'a, BaseSequence>
where
    BaseSequence: PostingList<'a>,
{
    #[cold]
    fn switch_partition(&mut self, part: usize) {
        assert!(self.n_partitions > 1);

        self.cur_partition = part;
        self.cur_begin = self.cur_partition * PARTITION_SIZE;
        self.cur_end = self.len.min((self.cur_partition + 1) * PARTITION_SIZE);

        //get bounds of this
        self.cur_base =
            self.upper_bounds.move_to_position(part).unwrap().0 + if part == 0 { 0 } else { 1 };
        self.cur_ub = self.upper_bounds.next().unwrap_or(self.universe);

        self.cur_sequence = BaseSequence::iter_from_slice_with_data(
            self.sequences.slice(
                self.endpoints[self.cur_partition],
                self.endpoints[self.cur_partition + 1],
            ),
            self.cur_end - self.cur_begin,
            self.cur_ub - self.cur_base + 1,
        );
    }

    #[cold]
    fn slow_next_geq(&mut self, lower_bound: u64) -> Option<(u64, usize)> {
        if self.n_partitions == 1 {
            if lower_bound < self.cur_base {
                return self.move_to_position(0);
            } else {
                return self.move_to_position(self.len);
            }
        }

        let ub_res = self.upper_bounds.next_geq(lower_bound);

        if ub_res.is_none() {
            return self.move_to_position(self.len);
        }

        let (_ub_val, ub_pos) = ub_res.unwrap();

        if ub_pos == 0 {
            return self.move_to_position(0);
        }

        self.switch_partition(ub_pos - 1);
        // let (val, pos) = self
        //     .cur_sequence
        //     .next_geq(0.max(lower_bound as i64 - self.cur_base as i64) as u64)?;

        // self.position = self.cur_begin + pos + 1;
        // Some((val + self.cur_base, self.position - 1))
        self.next_geq(lower_bound)
    }

    #[cold]
    fn slow_move(&mut self, pos: usize) -> Option<(u64, usize)> {
        if pos >= self.len {
            if self.n_partitions > 1 {
                self.switch_partition(self.n_partitions - 1);
            }
            return self.cur_sequence.move_to_position(self.cur_end);
        }

        let part = pos / PARTITION_SIZE;
        self.switch_partition(part);

        let (val, pos) = self.cur_sequence.move_to_position(pos - self.cur_begin)?;
        self.position = pos + self.cur_begin;
        Some((val + self.cur_base, self.position - 1))
    }
}

impl<'a, BaseSequence> IncreasingSequenceEnumerator for UniformPartitionedSeqIter<'a, BaseSequence>
where
    BaseSequence: PostingList<'a>,
{
    fn next_val(&mut self) -> Option<(u64, usize)> {
        self.position += 1;

        if let Some(x) = self.cur_sequence.next() {
            self.cur_value = x + self.cur_base;
            Some((self.cur_value, self.position - 1))
        } else if self.cur_partition < self.n_partitions - 1 && self.n_partitions != 1 {
            // go to next partition, if any
            self.switch_partition(self.cur_partition + 1);

            self.cur_value = self.cur_base + self.cur_sequence.next_val().unwrap().0;
            Some((self.cur_value, self.position - 1))
        } else {
            None
        }
    }

    fn next_geq(&mut self, lower_bound: u64) -> Option<(u64, usize)> {
        if lower_bound >= self.cur_base && lower_bound <= self.cur_ub {
            let (val, pos) = self.cur_sequence.next_geq(lower_bound - self.cur_base)?;
            self.position = self.cur_begin + pos as usize + 1;
            Some((val + self.cur_base, self.position - 1))
        } else {
            self.slow_next_geq(lower_bound)
        }
    }

    fn move_to_position(&mut self, pos: usize) -> Option<(u64, usize)> {
        self.position = pos;

        if self.position >= self.cur_begin && self.position < self.cur_end {
            let (val, _pos) = self.cur_sequence.move_to_position(pos - self.cur_begin)?;
            return Some((self.cur_base + val, self.position));
        }

        self.slow_move(pos)
    }

    fn len(&self) -> usize {
        self.len
    }
}

impl<'a, BaseSequence> Iterator for UniformPartitionedSeqIter<'a, BaseSequence>
where
    BaseSequence: PostingList<'a>,
{
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.next_val()?.0)
    }
}

impl<T> SpaceUsage for UniformPartitionedSequence<T> {
    fn space_usage_byte(&self) -> usize {
        self.bv.len() / 8 + mem::size_of::<usize>() + mem::size_of::<u64>()
    }
}

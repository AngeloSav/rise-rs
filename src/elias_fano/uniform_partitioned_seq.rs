use std::marker::PhantomData;

use epserde::Epserde;

use crate::{
    indexes::freq_index::{DocList, FreqList},
    utils::ceil_log2,
    BitSliceWithOffset, BitVec, EnumeratorFromBitSlice, NextGEQ, SequenceEnumerator,
    WriteBitvector,
};

use super::{EliasFano, EliasFanoIter};

#[derive(Debug, Epserde)]
pub struct UniformPartitionedSequence<BaseSequence> {
    n: usize,
    u: u64,
    bv: BitVec,
    _phantom: PhantomData<BaseSequence>,
}

const PARTITION_SIZE: usize = 128;

impl<'a, BaseSequence> UniformPartitionedSequence<BaseSequence>
where
    BaseSequence: FreqList,
{
    pub fn len(&self) -> usize {
        self.n
    }

    pub fn iter(&'a self) -> UniformPartitionedSeqIter<'a, BaseSequence> {
        Self::iter_from_slice(self.bv.as_bitslice(), self.n, self.u)
    }
}

impl<'a, BaseSequence> From<&'a [u64]> for UniformPartitionedSequence<BaseSequence>
where
    BaseSequence: FreqList,
{
    fn from(v: &'a [u64]) -> Self {
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

impl<BaseSequence> WriteBitvector for UniformPartitionedSequence<BaseSequence>
where
    BaseSequence: FreqList + WriteBitvector,
{
    fn write_bitvector(seq: &[u64], n: usize, u: u64) -> BitVec {
        debug_assert!(n > 0);
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

                bv_sequences.concat(BaseSequence::write_bitvector(
                    &cur_partition,
                    cur_partition.len(),
                    *cur_partition.last().unwrap() + 1,
                ));

                upper_bounds.push(new_ub);
                cur_base = new_ub + 1;
                endpoints.push(bv_sequences.len());
            }

            let bv_upper_bounds = EliasFano::write_bitvector(&upper_bounds, n_partitions + 1, u);
            let endpoint_bits = ceil_log2(bv_sequences.len() + 1);
            bv.append_gamma(endpoint_bits as u64);

            bv.concat(bv_upper_bounds);

            for e in endpoints {
                bv.append_bits(e as u64, endpoint_bits as usize);
            }

            bv.concat(bv_sequences);
        }

        bv
    }
}

fn get_endpoint<'a>(bv: &BitSliceWithOffset<'a>, idx: usize, endpoint_bits: usize) -> usize {
    if idx == 0 {
        0
    } else {
        unsafe { bv.get_word56((idx - 1) * endpoint_bits) as usize & ((1 << endpoint_bits) - 1) }
    }
}
impl<'a, BaseSequence> EnumeratorFromBitSlice<'a> for UniformPartitionedSequence<BaseSequence>
where
    BaseSequence: FreqList,
{
    type IterType = UniformPartitionedSeqIter<'a, BaseSequence>;

    fn iter_from_slice(bv: BitSliceWithOffset<'a>, n: usize, u: u64) -> Self::IterType {
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
            let cur_sequence = BaseSequence::iter_from_slice(bv.split_at(next_pos).1, n, ub + 1);

            return UniformPartitionedSeqIter {
                position: 0,
                cur_base,
                cur_ub: cur_base + ub,
                cur_begin: 0,
                cur_end: n,
                cur_partition: 0,
                upper_bounds: EliasFanoIter::default(),
                n_partitions: 1,
                endpoints: BitSliceWithOffset::default(),
                endpoint_bits: 0,
                sequences: BitSliceWithOffset::default(),
                cur_sequence,
                cur_value: 0,
                _phantom: PhantomData,
                len: n,
                universe: u,
            };
        } else {
            let (endpoint_bits, np) = unsafe { bv.get_gamma_unchecked(next_pos) };
            let endpoint_bits = endpoint_bits as usize;
            next_pos = np;

            let mut upper_bounds =
                EliasFano::iter_from_slice(bv.split_at(next_pos).1, n_partitions + 1, u);
            next_pos += EliasFano::n_bits(u, n_partitions + 1);

            // let mut endpoints = vec![0];
            // for idx in (next_pos..)
            //     .step_by(endpoint_bits as usize)
            //     .take(n_partitions)
            // {
            //     endpoints.push(
            //         unsafe { bv.get_word56(idx as usize) as usize } & ((1 << endpoint_bits) - 1),
            //     );
            // }

            let endpoints = bv.slice(next_pos, next_pos + endpoint_bits as usize * n_partitions);

            let sequences = bv
                .split_at(next_pos + endpoint_bits as usize * n_partitions)
                .1;

            let cur_base = upper_bounds.next().unwrap();
            let cur_ub = upper_bounds.next().unwrap();
            let cur_begin = 0;
            let cur_end = 1 * PARTITION_SIZE;

            let start_endpoint = get_endpoint(&endpoints, 0, endpoint_bits);
            let end_endpoint = get_endpoint(&endpoints, 1, endpoint_bits);
            let cur_sequence = BaseSequence::iter_from_slice(
                sequences.slice(start_endpoint, end_endpoint),
                cur_end,
                cur_ub - cur_base + 1,
            );

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
                endpoint_bits,
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
    BaseSequence: FreqList,
{
    position: usize,
    cur_partition: usize,
    cur_base: u64,
    upper_bounds: EliasFanoIter<'a>,
    n_partitions: usize,
    endpoints: BitSliceWithOffset<'a>,
    endpoint_bits: usize,
    sequences: BitSliceWithOffset<'a>,
    cur_sequence: <BaseSequence as EnumeratorFromBitSlice<'a>>::IterType,
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
    BaseSequence: FreqList,
{
    #[cold]
    fn switch_partition(&mut self, part: usize) {
        debug_assert!(self.n_partitions > 1);

        self.cur_partition = part;
        self.cur_begin = self.cur_partition * PARTITION_SIZE;
        self.cur_end = self.len.min((self.cur_partition + 1) * PARTITION_SIZE);

        //get bounds of this
        self.cur_base = self.upper_bounds.move_to_position(part).0 + if part == 0 { 0 } else { 1 };
        self.cur_ub = self.upper_bounds.next().unwrap_or(self.universe);

        self.cur_sequence = BaseSequence::iter_from_slice(
            self.sequences.slice(
                get_endpoint(&self.endpoints, self.cur_partition, self.endpoint_bits),
                get_endpoint(&self.endpoints, self.cur_partition + 1, self.endpoint_bits),
            ),
            self.cur_end - self.cur_begin,
            self.cur_ub - self.cur_base + 1,
        );
    }

    /// Called only from `next_val` when advancing to the immediately next partition.
    /// Avoids the backward `move_to_position` seek that the general `switch_partition` does:
    /// instead of re-seeking the EF upper-bounds iterator, it advances it by one step,
    /// and reuses `cur_ub` (already known) as the new `cur_base`.
    #[inline]
    fn switch_partition_next(&mut self) {
        debug_assert!(self.n_partitions > 1);
        self.cur_partition += 1;
        self.cur_begin = self.cur_end;
        self.cur_end = self.len.min(self.cur_begin + PARTITION_SIZE);

        // The previous cur_ub is the upper bound of the partition we just left;
        // the new base is one past it.
        self.cur_base = self.cur_ub + 1;
        // Advance EF iterator by one step instead of seeking backward.
        self.cur_ub = self.upper_bounds.next_val().0;

        self.cur_sequence = BaseSequence::iter_from_slice(
            self.sequences.slice(
                get_endpoint(&self.endpoints, self.cur_partition, self.endpoint_bits),
                get_endpoint(&self.endpoints, self.cur_partition + 1, self.endpoint_bits),
            ),
            self.cur_end - self.cur_begin,
            self.cur_ub - self.cur_base + 1,
        );
    }

    #[cold]
    fn slow_move(&mut self, pos: usize) -> (u64, usize) {
        debug_assert!(pos <= self.len);
        if pos == self.len {
            if self.n_partitions > 1 {
                self.switch_partition(self.n_partitions - 1);
            }
            self.cur_sequence.move_to_position(self.cur_end);
            return (self.universe, self.len);
        }

        let part = pos / PARTITION_SIZE;
        self.switch_partition(part);

        let (val, _pos) = self.cur_sequence.move_to_position(pos - self.cur_begin);

        (val + self.cur_base, self.position - 1)
    }
}

impl<'a, BaseSequence> UniformPartitionedSeqIter<'a, BaseSequence>
where
    BaseSequence: DocList,
{
    #[cold]
    fn slow_next_geq(&mut self, lower_bound: u64) -> (u64, usize) {
        if self.n_partitions == 1 {
            // move_to_position already returns values in the global domain (cur_base added
            // internally), so we must NOT add cur_base again here.
            if lower_bound < self.cur_base {
                return self.move_to_position(0);
            } else {
                return self.move_to_position(self.len);
            }
        }

        let (_ub_val, ub_pos) = self.upper_bounds.next_geq(lower_bound);

        if ub_pos == 0 {
            return self.move_to_position(0);
        }

        if ub_pos >= self.upper_bounds.len() {
            return self.move_to_position(self.len);
        }

        self.switch_partition(ub_pos - 1);

        self.next_geq(lower_bound)
    }
}

impl<'a, BaseSequence> SequenceEnumerator for UniformPartitionedSeqIter<'a, BaseSequence>
where
    BaseSequence: FreqList,
{
    fn next_val(&mut self) -> (u64, usize) {
        self.position += 1;

        // we still have elems in the current partition
        if self.position - 1 < self.cur_end {
            self.cur_value = self.cur_sequence.next_val().0 + self.cur_base;
            return (self.cur_value, self.position - 1);
        }

        if self.position - 1 >= self.len {
            return (self.universe, self.len);
        }

        // go to next partition, if any
        self.switch_partition_next();

        self.cur_value = self.cur_sequence.next_val().0 + self.cur_base;
        (self.cur_value, self.position - 1)
    }

    fn move_to_position(&mut self, pos: usize) -> (u64, usize) {
        self.position = pos + 1;

        if self.position - 1 >= self.cur_begin && self.position - 1 < self.cur_end {
            let (val, _pos) = self.cur_sequence.move_to_position(pos - self.cur_begin);
            return (self.cur_base + val, self.position - 1);
        }

        self.slow_move(pos)
    }

    fn len(&self) -> usize {
        self.len
    }
}

impl<'a, BaseSequence> NextGEQ for UniformPartitionedSeqIter<'a, BaseSequence>
where
    BaseSequence: DocList,
{
    fn next_geq(&mut self, lower_bound: u64) -> (u64, usize) {
        if lower_bound >= self.cur_base && lower_bound <= self.cur_ub {
            // println!(
            //     "fast next geq in partition {}/{}, universe {}, lower_bound {}, cur_base {}, len {}, cur_seq len {}",
            //     self.cur_partition, self.n_partitions, self.universe, lower_bound, self.cur_base, self.len, self.cur_sequence.len()
            // );
            let (val, pos) = self.cur_sequence.next_geq(lower_bound - self.cur_base);
            self.position = self.cur_begin + pos as usize + 1;
            (val + self.cur_base, self.position - 1)
        } else {
            // println!("slow next geq");
            self.slow_next_geq(lower_bound)
        }
    }
}

impl<'a, BaseSequence> Iterator for UniformPartitionedSeqIter<'a, BaseSequence>
where
    BaseSequence: FreqList,
{
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        let val = self.next_val().0;
        if val == self.universe {
            return None;
        }
        Some(val)
    }
}

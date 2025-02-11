use core::panic;
use std::{marker::PhantomData, mem};

use serde::{Deserialize, Serialize};

use crate::{
    indexes::freq_index::PostingList, space_usage::SpaceUsage, utils::ceil_log2,
    BitSliceWithOffset, BitVec, CostWindow, EnumeratorFromBitSlice, IncreasingSequenceEnumerator,
    PartitionableSequence, ToBitvector, WriteBitvector,
};

use super::{EliasFano, EliasFanoIter};

#[derive(Debug, Serialize, Deserialize)]
pub struct OptPartitionedSequence<BaseSequence> {
    n: usize,
    u: u64,
    bv: BitVec,
    _phantom: PhantomData<BaseSequence>,
}

impl<'a, BaseSequence> OptPartitionedSequence<BaseSequence>
where
    BaseSequence: PostingList<'a> + for<'b> PartitionableSequence<'b>,
{
    pub fn len(&self) -> usize {
        self.n
    }

    pub fn iter(&'a self) -> OptPartitionedSeqIter<'a, BaseSequence> {
        Self::iter_from_slice_with_data(self.bv.as_bitslice(), self.n, self.u)
    }
}

const EPS1: f64 = 0.0;
const EPS2: f64 = 0.3;

impl<'a, BaseSequence> From<&[u64]> for OptPartitionedSequence<BaseSequence>
where
    BaseSequence: PostingList<'a> + for<'b> PartitionableSequence<'b>,
{
    fn from(v: &[u64]) -> Self {
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

impl<'a, BaseSequence> ToBitvector for OptPartitionedSequence<BaseSequence>
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

impl<'a, BaseSequence> WriteBitvector for OptPartitionedSequence<BaseSequence>
where
    BaseSequence: PostingList<'a> + for<'b> PartitionableSequence<'b>,
{
    // serialization is done in the following way:
    // If only 1 partition:  | 1 | serialized  BaseSequence |
    // Else:                 | n partitions | bitlen of endpoints | list of endpoints | len of (upper bounds sequence) | elias_fano encoded upper bounds | serialized BaseSequences |

    fn write_bitvector(seq: &[u64], n: usize, u: u64) -> BitVec {
        assert!(n > 0);
        let mut bv = BitVec::new();

        let (_, partitions) = optimal_partition::<BaseSequence::CW>(&seq, EPS1, EPS2);
        let n_partitions = partitions.len();

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

            for part_size in partitions.iter().scan(0u64, |s, &x| {
                let t = x - *s;
                *s = x;
                Some(t as usize)
            }) {
                cur_partition = (&mut it).take(part_size).copied().collect();

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

            let bv_sizes = EliasFano::write_bitvector(&partitions, n_partitions, n as u64 + 1);
            bv.concat(bv_sizes);

            for e in endpoints {
                bv.append_bits(e as u64, endpoint_bits as usize);
            }

            bv.concat(bv_sequences);
        }

        bv
    }
}

impl<'a, BaseSequence> EnumeratorFromBitSlice<'a> for OptPartitionedSequence<BaseSequence>
where
    BaseSequence: PostingList<'a> + for<'b> PartitionableSequence<'b>,
{
    type IterType = OptPartitionedSeqIter<'a, BaseSequence>;
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

            return OptPartitionedSeqIter {
                position: 0,
                cur_base,
                cur_ub: cur_base + ub,
                cur_begin: 0,
                cur_end: n,
                cur_partition: 0,
                upper_bounds: EliasFanoIter::default(),
                sizes: EliasFanoIter::default(),
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

            let mut upper_bounds =
                EliasFano::iter_from_slice_with_data(bv.split_at(next_pos).1, n_partitions + 1, u);
            next_pos += EliasFano::n_bits(u, n_partitions + 1);

            // println!("sizes start : {}", next_pos);
            let mut sizes = EliasFano::iter_from_slice_with_data(
                bv.split_at(next_pos).1,
                n_partitions,
                n as u64 + 1,
            );
            next_pos += EliasFano::n_bits(n as u64 + 1, n_partitions);

            let mut endpoints = vec![0];
            for idx in (next_pos..)
                .step_by(endpoint_bits as usize)
                .take(n_partitions)
            {
                endpoints.push(bv.get_bits(idx, endpoint_bits as usize).unwrap() as usize);
            }

            let sequences = bv
                .split_at(next_pos + endpoint_bits as usize * (n_partitions))
                .1;

            let cur_base = upper_bounds.next().unwrap();
            let cur_ub = upper_bounds.next().unwrap();
            let cur_begin = 0 as usize;
            let cur_end = sizes.next().unwrap() as usize;

            let cur_sequence = BaseSequence::iter_from_slice_with_data(
                sequences.slice(endpoints[0], endpoints[1]),
                cur_end as usize,
                cur_ub - cur_base + 1,
            );

            OptPartitionedSeqIter {
                position: 0,
                cur_partition: 0,
                cur_base,
                cur_ub,
                cur_begin,
                cur_end,
                sizes,
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
pub struct OptPartitionedSeqIter<'a, BaseSequence>
where
    BaseSequence: PostingList<'a> + for<'b> PartitionableSequence<'b>,
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
    sizes: EliasFanoIter<'a>,
}

impl<'a, BaseSequence> OptPartitionedSeqIter<'a, BaseSequence>
where
    BaseSequence: PostingList<'a> + for<'b> PartitionableSequence<'b>,
{
    #[cold]
    fn switch_partition(&mut self, part: usize) {
        assert!(self.n_partitions > 1);

        self.cur_partition = part;

        if part == 0 {
            self.cur_begin = 0;
            self.cur_end = self.sizes.move_to_position(part).unwrap().0 as usize;
        } else {
            self.cur_begin = self.sizes.move_to_position(part - 1).unwrap().0 as usize;
            self.cur_end = self.sizes.next().unwrap() as usize;
        }

        //get bounds of this
        self.cur_base =
            self.upper_bounds.move_to_position(part).unwrap().0 + if part == 0 { 0 } else { 1 };
        self.cur_ub = self.upper_bounds.next().unwrap_or(self.universe);

        // without using a vec for endpoints
        // let mask = (1 << self.endpoint_bits) - 1;

        // let start_p = if self.cur_partition == 0 {
        //     0
        // } else {
        //     unsafe {
        //         self.endpoints_slice
        //             .get_word56((self.cur_partition - 1) * self.endpoint_bits)
        //             & mask
        //     }
        // };
        // let end_p = unsafe {
        //     self.endpoints_slice
        //         .get_word56((self.cur_partition) * self.endpoint_bits)
        //         & mask
        // };

        // self.cur_sequence = BaseSequence::iter_from_slice_with_data(
        //     self.sequences.slice(start_p as usize, end_p as usize),
        //     self.cur_end - self.cur_begin,
        //     self.cur_ub - self.cur_base + 1,
        // );

        //using a vec saves ~1ms from execution times of or
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

        let (_ub_val, ub_pos) = unsafe { ub_res.unwrap_unchecked() };

        if ub_pos == 0 {
            return self.move_to_position(0);
        }

        self.switch_partition(ub_pos - 1);

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

        let (_end, part) = self.sizes.next_geq(pos as u64 + 1).unwrap();
        self.switch_partition(part);

        let (val, _pos) = self.cur_sequence.move_to_position(pos - self.cur_begin)?;
        Some((val + self.cur_base, self.position - 1))
    }
}

impl<'a, BaseSequence> IncreasingSequenceEnumerator for OptPartitionedSeqIter<'a, BaseSequence>
where
    BaseSequence: PostingList<'a> + for<'b> PartitionableSequence<'b>,
{
    fn next_val(&mut self) -> Option<(u64, usize)> {
        self.position += 1;

        if let Some(x) = self.cur_sequence.next() {
            self.cur_value = x + self.cur_base;
            Some((self.cur_value, self.position - 1))
        } else if self.cur_partition < self.n_partitions - 1 {
            // go to next partition, if any
            self.switch_partition(self.cur_partition + 1);

            self.cur_value = self.cur_base + self.cur_sequence.next().unwrap();
            Some((self.cur_value, self.position - 1))
        } else {
            None
        }
    }

    fn next_geq(&mut self, lower_bound: u64) -> Option<(u64, usize)> {
        // println!("nextgeq");
        if lower_bound >= self.cur_base && lower_bound <= self.cur_ub {
            // println!("here");
            let (val, pos) = self
                .cur_sequence
                .next_geq(lower_bound - self.cur_base)
                .unwrap_or_else(|| {
                    panic!(
                        "partition {}/{}
                sequence len {}
                lower bound: {}
                cur_base: {}
                cur_ub: {}
                cur_sequence: {:?}
                serching lb in seq: {}
                ",
                        self.cur_partition,
                        self.n_partitions,
                        self.len,
                        lower_bound,
                        self.cur_base,
                        self.cur_ub,
                        self.cur_sequence,
                        lower_bound - self.cur_base
                    );
                });
            self.position = self.cur_begin + pos as usize + 1;
            Some((val + self.cur_base, self.position - 1))
        } else {
            // println!("here2");
            self.slow_next_geq(lower_bound)
        }
    }

    fn move_to_position(&mut self, pos: usize) -> Option<(u64, usize)> {
        self.position = pos + 1;

        if self.position >= self.cur_begin && self.position < self.cur_end {
            let (val, _pos) = self.cur_sequence.move_to_position(pos - self.cur_begin)?;
            return Some((self.cur_base + val, self.position - 1));
        }

        self.slow_move(pos)
    }

    fn len(&self) -> usize {
        self.len
    }
}

impl<'a, BaseSequence> Iterator for OptPartitionedSeqIter<'a, BaseSequence>
where
    BaseSequence: PostingList<'a> + for<'b> PartitionableSequence<'b>,
{
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.next_val()?.0)
    }
}

impl<T> SpaceUsage for OptPartitionedSequence<T> {
    fn space_usage_byte(&self) -> usize {
        self.bv.len() / 8 + mem::size_of::<usize>() + mem::size_of::<u64>()
    }
}

/// returns a pair (optimal cost, vector of positions) that are the optimal starting point for each block
pub fn optimal_partition<'a, 'b, T: CostWindow<'a>>(
    sequence: &'b [u64],
    eps1: f64,
    eps2: f64,
) -> (usize, Vec<u64>)
where
    'b: 'a,
{
    assert!(!sequence.is_empty(), "sequence is empty");
    let single_block_cost = T::single_block_cost(sequence);

    let mut min_cost = vec![single_block_cost; sequence.len() + 1];
    min_cost[0] = 0;

    let mut windows = Vec::new();
    let cost_lb = T::minimum_cost(sequence); // minimum cost
    let mut cost_bound = cost_lb;

    //initialize windows
    while eps1 == 0.0 || cost_bound < (cost_lb as f64 / eps1) as usize {
        windows.push(T::new(sequence, cost_bound));
        if cost_bound >= single_block_cost {
            break;
        }
        cost_bound = ((cost_bound as f64) * (1.0 + eps2)) as usize;
    }

    let mut path = vec![0u64; sequence.len() + 1];
    for i in 0..sequence.len() {
        let mut last_end = i + 1;
        for window in windows.iter_mut() {
            assert_eq!(window.start(), i);

            while window.end() < last_end {
                window.advance_end();
            }

            let mut window_cost;
            loop {
                window_cost = window.window_cost();
                if min_cost[i] + window_cost < min_cost[window.end()] {
                    min_cost[window.end()] = min_cost[i] + window_cost;
                    path[window.end()] = i as u64;
                }

                last_end = window.end();
                if window.end() == sequence.len() {
                    break;
                }
                if window_cost >= window.cost_upper_bound() {
                    break;
                }
                window.advance_end();
            }
            window.advance_start();
        }
    }

    let mut partition = Vec::new();

    let mut cur_pos = sequence.len() as u64;
    while cur_pos != 0 {
        partition.push(cur_pos);
        cur_pos = path[cur_pos as usize] as u64;
    }

    partition.reverse();
    (min_cost[sequence.len()], partition)
}

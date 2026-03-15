use std::marker::PhantomData;

use epserde::Epserde;

use crate::{
    BitSliceWithOffset, BitVec, EnumeratorFromBitSlice, NextGEQ, SequenceEnumerator,
    WriteBitvector,
    indexes::freq_index::{DocList, FreqList},
    utils::{ceil_log2, prefetch_bitslice_word},
};

use super::{EliasFano, EliasFanoIter};

// ── Partitioning traits ───────────────────────────────────────────────────────

/// A sliding window used by [`optimal_partition`] to evaluate the cost of
/// encoding a contiguous sub-sequence.
///
/// A `CostWindow` is parameterised by an upper bound on acceptable cost; the
/// dynamic-programming algorithm grows and shrinks the window while probing
/// candidate partition boundaries.
pub trait CostWindow<'a> {
    /// Create a new window starting at index 0 with the given cost upper bound.
    fn new(sequence: &'a [u64], cost_upper_bound: usize) -> Self;
    /// Universe of the current window (max value + 1).
    fn universe(&self) -> u64;
    /// Number of elements in the current window.
    fn size(&self) -> usize;

    /// Encoding cost (in bits) for the current window.
    fn window_cost(&self) -> usize;
    /// Cost of encoding the entire `sequence` as a single block.
    fn single_block_cost(sequence: &[u64]) -> usize;
    /// Lower bound on the encoding cost of any single element (used to seed
    /// the DP cost ladder).
    fn minimum_cost(sequence: &[u64]) -> usize;

    /// Advance the start of the window by one element.
    fn advance_start(&mut self);
    /// Advance the end of the window by one element.
    fn advance_end(&mut self);
    /// Inclusive start index of the current window.
    fn start(&self) -> usize;
    /// Exclusive end index of the current window.
    fn end(&self) -> usize;
    /// The cost upper bound this window was constructed with.
    fn cost_upper_bound(&self) -> usize;
}

/// Marker trait: the implementing sequence type can be split into independent
/// partitions, each encoded separately.
///
/// The associated type [`CostWindow`] is used by [`optimal_partition`] to
/// evaluate encoding costs without performing actual encoding.
pub trait PartitionableSequence<'a> {
    type CW: CostWindow<'a>;
}

#[derive(Debug, Epserde)]
pub struct OptPartitionedSequence<BaseSequence> {
    n: usize,
    u: u64,
    bv: BitVec,
    _phantom: PhantomData<BaseSequence>,
}

impl<'a, BaseSequence> OptPartitionedSequence<BaseSequence>
where
    BaseSequence: FreqList + for<'b> PartitionableSequence<'b>,
{
    pub fn len(&self) -> usize {
        self.n
    }

    pub fn iter(&'a self) -> OptPartitionedSeqIter<'a, BaseSequence> {
        Self::iter_from_slice(self.bv.as_bitslice(), self.n, self.u)
    }
}

const EPS1: f64 = 0.0;
const EPS2: f64 = 0.3;

impl<BaseSequence> From<&[u64]> for OptPartitionedSequence<BaseSequence>
where
    BaseSequence: FreqList + for<'b> PartitionableSequence<'b>,
{
    fn from(v: &[u64]) -> Self {
        let n = v.len();
        let u = *v.last().unwrap() + 1;
        let bv = Self::write_bitvector(v.iter().copied(), n, u);

        Self {
            bv,
            n,
            u,
            _phantom: PhantomData,
        }
    }
}

impl<BaseSequence> WriteBitvector for OptPartitionedSequence<BaseSequence>
where
    BaseSequence: FreqList + for<'b> PartitionableSequence<'b>,
{
    // serialization is done in the following way:
    // If only 1 partition:  | 1 | serialized  BaseSequence |
    // Else:                 | n partitions | bitlen of endpoints | list of endpoints | len of (upper bounds sequence) | elias_fano encoded upper bounds | serialized BaseSequences |

    fn write_bitvector(seq: impl IntoIterator<Item = u64>, n: usize, u: u64) -> BitVec {
        assert!(n > 0);
        let seq: Vec<u64> = seq.into_iter().collect();
        assert!(seq.len() == n, "Sequence length mismatch!");
        let mut bv = BitVec::new();

        // log::info!("partitioning sequence of length {} and universe {}", n, u);
        let (_, partitions) = optimal_partition::<BaseSequence::CW>(&seq, EPS1, EPS2);
        let n_partitions = partitions.len();
        // log::info!("done");

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
                cur_partition.iter().copied(),
                cur_partition.len(),
                *cur_partition.last().unwrap() + 1,
            ));
        } else {
            let mut cur_partition: Vec<u64>;
            let mut upper_bounds: Vec<u64> = Vec::new();
            let mut bv_sequences: BitVec = BitVec::new();

            let mut endpoints = Vec::new();
            let mut cur_base = seq[0];
            let mut it = seq.into_iter();
            upper_bounds.push(cur_base);

            for part_size in partitions.iter().scan(0u64, |s, &x| {
                let t = x - *s;
                *s = x;
                Some(t as usize)
            }) {
                cur_partition = (&mut it).take(part_size).collect();

                // let cur_base = cur_partition[0];
                // upper_bounds.push(cur_base);
                let new_ub = *cur_partition.last().unwrap();

                for el in cur_partition.iter_mut() {
                    *el -= cur_base;
                }

                bv_sequences.concat(BaseSequence::write_bitvector(
                    cur_partition.iter().copied(),
                    cur_partition.len(),
                    *cur_partition.last().unwrap() + 1,
                ));

                upper_bounds.push(new_ub);
                cur_base = new_ub + 1;
                endpoints.push(bv_sequences.len());
            }

            let bv_upper_bounds =
                EliasFano::write_bitvector(upper_bounds.iter().copied(), n_partitions + 1, u + 1);
            let endpoint_bits = ceil_log2(bv_sequences.len() + 1);
            bv.append_gamma(endpoint_bits as u64);

            bv.concat(bv_upper_bounds);

            let bv_sizes = EliasFano::write_bitvector(
                partitions[0..&partitions.len() - 1].iter().copied(), // we dont need the last element
                n_partitions - 1,
                n as u64,
            );
            bv.concat(bv_sizes);

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

impl<'a, BaseSequence> EnumeratorFromBitSlice<'a> for OptPartitionedSequence<BaseSequence>
where
    BaseSequence: FreqList + for<'b> PartitionableSequence<'b>,
{
    type IterType = OptPartitionedSeqIter<'a, BaseSequence>;

    fn iter_from_slice(bv: BitSliceWithOffset<'a>, n: usize, u: u64) -> Self::IterType {
        let (n_partitions, mut next_pos) = unsafe { bv.get_gamma_unchecked(0) };
        let n_partitions = n_partitions as usize;

        if n_partitions == 1 {
            let universe_bits = ceil_log2(u);
            let cur_base = unsafe { bv.get_bits_unchecked(next_pos, universe_bits as usize) };

            next_pos += universe_bits as usize;
            let mut ub = 0;
            if n > 1 {
                let (universe_delta, np) = unsafe { bv.get_delta_unchecked(next_pos as usize) };
                ub = if universe_delta != 0 {
                    universe_delta
                } else {
                    u - cur_base - 1
                };
                next_pos = np;
            }
            let cur_sequence = BaseSequence::iter_from_slice(bv.split_at(next_pos).1, n, ub + 1);

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
                EliasFano::iter_from_slice(bv.split_at(next_pos).1, n_partitions + 1, u + 1);
            next_pos += EliasFano::n_bits(u + 1, n_partitions + 1);

            // println!("sizes start : {}", next_pos);
            let mut sizes =
                EliasFano::iter_from_slice(bv.split_at(next_pos).1, n_partitions - 1, n as u64);
            next_pos += EliasFano::n_bits(n as u64, n_partitions - 1);

            // let mut endpoints = vec![0];
            // for idx in (next_pos..)
            //     .step_by(endpoint_bits as usize)
            //     .take(n_partitions)
            // {
            //     endpoints.push(bv.get_bits(idx, endpoint_bits as usize).unwrap() as usize);
            // }

            let endpoints = bv.slice(next_pos, next_pos + endpoint_bits as usize * n_partitions);

            let sequences = bv
                .split_at(next_pos + endpoint_bits as usize * n_partitions)
                .1;

            let cur_base = upper_bounds.next().unwrap();
            let cur_ub = upper_bounds.next().unwrap();
            let cur_begin = 0 as usize;
            let cur_end = sizes.next().unwrap() as usize;

            let start_endpoint = get_endpoint(&endpoints, 0, endpoint_bits);
            let end_endpoint = get_endpoint(&endpoints, 1, endpoint_bits);

            let cur_sequence = BaseSequence::iter_from_slice(
                sequences.slice(start_endpoint, end_endpoint),
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
                endpoint_bits: endpoint_bits,
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
    BaseSequence: FreqList + for<'b> PartitionableSequence<'b>,
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
    sizes: EliasFanoIter<'a>,
}

impl<'a, BaseSequence> OptPartitionedSeqIter<'a, BaseSequence>
where
    BaseSequence: FreqList + for<'b> PartitionableSequence<'b>,
{
    #[cold]
    fn switch_partition(&mut self, part: usize) {
        debug_assert!(self.n_partitions > 1);
        debug_assert!(self.n_partitions > part);

        self.cur_partition = part;

        if part == 0 {
            self.cur_begin = 0;
            self.cur_end = self.sizes.move_to_position(part).0 as usize;
        } else {
            self.cur_begin = self.sizes.move_to_position(part - 1).0 as usize;
            self.cur_end = self.sizes.next().unwrap_or(self.len as u64) as usize;
        }

        //get bounds of this
        self.cur_base = self.upper_bounds.move_to_position(part).0 + if part == 0 { 0 } else { 1 };
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

        let start_endpoint = get_endpoint(&self.endpoints, self.cur_partition, self.endpoint_bits);
        let end_endpoint =
            get_endpoint(&self.endpoints, self.cur_partition + 1, self.endpoint_bits);

        prefetch_bitslice_word(&self.sequences, start_endpoint);
        prefetch_bitslice_word(&self.sequences, end_endpoint);

        self.cur_sequence = BaseSequence::iter_from_slice(
            self.sequences.slice(start_endpoint, end_endpoint),
            self.cur_end - self.cur_begin,
            self.cur_ub - self.cur_base + 1,
        );
    }

    /// Called only from `next_val` when advancing to the immediately next partition.
    /// Avoids the backward `move_to_position` seeks that the general `switch_partition` does:
    /// instead of re-seeking both the `upper_bounds` and `sizes` EF iterators, it advances
    /// each by one step and reuses already-known values for begin/base.
    #[inline]
    fn switch_partition_next(&mut self) {
        debug_assert!(self.n_partitions > 1);
        self.cur_partition += 1;

        // The previous cur_end is the begin of the next partition.
        self.cur_begin = self.cur_end;
        // Advance the EF sizes iterator by one step instead of seeking backward.
        self.cur_end = self.sizes.next_val().0 as usize;

        // The previous cur_ub is the upper bound we just left; new base is one past it.
        self.cur_base = self.cur_ub + 1;
        // Advance EF upper-bounds iterator by one step instead of seeking backward.
        self.cur_ub = self.upper_bounds.next_val().0;

        let start_endpoint = get_endpoint(&self.endpoints, self.cur_partition, self.endpoint_bits);
        let end_endpoint =
            get_endpoint(&self.endpoints, self.cur_partition + 1, self.endpoint_bits);

        prefetch_bitslice_word(&self.sequences, start_endpoint);
        prefetch_bitslice_word(&self.sequences, end_endpoint);

        self.cur_sequence = BaseSequence::iter_from_slice(
            self.sequences.slice(start_endpoint, end_endpoint),
            self.cur_end - self.cur_begin,
            self.cur_ub - self.cur_base + 1,
        );
    }

    #[cold]
    fn slow_move(&mut self, pos: usize) -> (u64, usize) {
        if pos >= self.len {
            if self.n_partitions > 1 {
                self.switch_partition(self.n_partitions - 1);
            }
            self.cur_sequence.move_to_position(self.cur_end);
            return (self.universe, self.len);
        }

        let part = self.sizes.next_geq(pos as u64 + 1).1;
        self.switch_partition(part);

        let (val, _pos) = self.cur_sequence.move_to_position(pos - self.cur_begin);
        (val + self.cur_base, self.position - 1)
    }
}

impl<'a, BaseSequence> OptPartitionedSeqIter<'a, BaseSequence>
where
    BaseSequence: DocList + for<'b> PartitionableSequence<'b>,
{
    #[cold]
    fn slow_next_geq(&mut self, lower_bound: u64) -> (u64, usize) {
        if self.n_partitions == 1 {
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

        if ub_pos == self.upper_bounds.len() {
            return self.move_to_position(self.len);
        }

        self.switch_partition(ub_pos - 1);

        self.next_geq(lower_bound)
    }
}

impl<'a, BaseSequence> SequenceEnumerator for OptPartitionedSeqIter<'a, BaseSequence>
where
    BaseSequence: FreqList + for<'b> PartitionableSequence<'b>,
{
    fn next_val(&mut self) -> (u64, usize) {
        self.position += 1;

        if self.position - 1 < self.cur_end {
            self.cur_value = self.cur_sequence.next_val().0 + self.cur_base;
            return (self.cur_value, self.position - 1);
        }

        if self.position - 1 >= self.len {
            return (self.universe, self.len);
        }

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

impl<'a, BaseSequence> NextGEQ for OptPartitionedSeqIter<'a, BaseSequence>
where
    BaseSequence: DocList + for<'b> PartitionableSequence<'b>,
{
    fn next_geq(&mut self, lower_bound: u64) -> (u64, usize) {
        // println!("nextgeq");
        if core::intrinsics::likely(lower_bound >= self.cur_base && lower_bound <= self.cur_ub) {
            // println!("here");
            let (val, pos) = self.cur_sequence.next_geq(lower_bound - self.cur_base);
            // .unwrap_or_else(|| {
            //     panic!(
            //         "partition {}/{}
            // sequence len {}
            // lower bound: {}
            // cur_base: {}
            // cur_ub: {}
            // cur_sequence: {:?}
            // serching lb in seq: {}
            // ",
            //         self.cur_partition,
            //         self.n_partitions,
            //         self.len,
            //         lower_bound,
            //         self.cur_base,
            //         self.cur_ub,
            //         self.cur_sequence,
            //         lower_bound - self.cur_base
            //     );
            // });
            self.position = self.cur_begin + pos as usize + 1;
            (val + self.cur_base, self.position - 1)
        } else {
            // println!("here2");
            self.slow_next_geq(lower_bound)
        }
    }
}

impl<'a, BaseSequence> Iterator for OptPartitionedSeqIter<'a, BaseSequence>
where
    BaseSequence: FreqList + for<'b> PartitionableSequence<'b>,
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

/// returns a pair (optimal cost, vector of positions) that are the optimal starting point for each block
pub fn optimal_partition<'a, 'b, T: CostWindow<'a>>(
    sequence: &'b [u64],
    eps1: f64,
    eps2: f64,
) -> (usize, Vec<u64>)
where
    'b: 'a,
{
    debug_assert!(!sequence.is_empty(), "sequence is empty");
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
            debug_assert!(window.start() == i);

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

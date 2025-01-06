use std::{arch::x86_64::_SIDD_MASKED_NEGATIVE_POLARITY, marker::PhantomData, mem};

use serde::{Deserialize, Serialize};

use crate::{
    indexes::freq_index::PostingList,
    space_usage::SpaceUsage,
    utils::{ceil_log2, gamma_size},
    BitSliceWithOffset, BitVec, BitVecCollection, EnumeratorFromBitSlice,
    IncreasingSequenceEnumerator, ToBitvector, WriteBitvector,
};

use super::{EliasFano, EliasFanoIter};

#[derive(Debug, Serialize, Deserialize)]
pub struct UniformPartitionedSequence<BaseSequence, BSIter> {
    n: usize,
    n_partitions: usize,
    bv_upper_bounds: EliasFano,
    bv_sequences: BitVecCollection,
    endpoints: Vec<usize>,
    _phantom: PhantomData<(BaseSequence, BSIter)>,
}

const PARTITION_SIZE: usize = 512;

impl<'a, BaseSequence, BaseSequenceIter> UniformPartitionedSequence<BaseSequence, BaseSequenceIter>
where
    BaseSequence: PostingList<'a, BaseSequenceIter>,
    BaseSequenceIter: IncreasingSequenceEnumerator,
{
    pub fn len(&self) -> usize {
        self.n
    }

    pub fn iter(&'a self) -> UniformPartitionedSeqIter<'a, BaseSequence, BaseSequenceIter> {
        let first_seq = self.bv_sequences.get(0);

        let mut upper_bounds = if self.n_partitions == 1 {
            EliasFanoIter::default()
        } else {
            self.bv_upper_bounds.iter()
        };
        let cur_base = if self.n_partitions == 1 {
            0
        } else {
            upper_bounds.next().unwrap()
        };

        let mut endpoints = vec![0];
        endpoints.extend(self.endpoints.clone());

        UniformPartitionedSeqIter {
            position: 0,
            cur_base,
            cur_ub: todo!(),
            cur_partition: 0,
            upper_bounds,
            n_partitions: self.n_partitions,
            endpoints,
            sequences: BitSliceWithOffset::new(&self.bv_sequences.bv, 0),
            cur_sequence: BaseSequence::iter_from_slice(first_seq),
            cur_value: 0,
            _phantom: PhantomData,
            cur_begin: todo!(),
            cur_end: todo!(),
            len: todo!(),
            universe: todo!(),
        }
    }
}

impl<'a, 'b, BaseSequence, BaseSequenceIter> From<&'b [u64]>
    for UniformPartitionedSequence<BaseSequence, BaseSequenceIter>
where
    BaseSequence: PostingList<'a, BaseSequenceIter>,
    BaseSequenceIter: IncreasingSequenceEnumerator,
{
    fn from(v: &'b [u64]) -> Self {
        let n = v.len();
        let n_partitions = usize::div_ceil(n, PARTITION_SIZE);
        let mut bv_sequences = BitVecCollection::default();

        let mut cur_partition = Vec::new();
        let mut bv_upper_bounds = Vec::new();
        if n_partitions == 1 {
            bv_sequences.push(BaseSequence::from(v).to_bv());

            Self {
                n_partitions,
                n,
                bv_upper_bounds: EliasFano::default(),
                bv_sequences,
                endpoints: vec![0],
                _phantom: PhantomData,
            }
        } else {
            let mut endpoints = Vec::new();
            let mut it = v.into_iter();
            for _ in 0..n_partitions {
                cur_partition = (&mut it).take(PARTITION_SIZE).copied().collect();

                let cur_base = cur_partition[0];
                for el in cur_partition.iter_mut() {
                    *el -= cur_base;
                }

                bv_upper_bounds.push(cur_base);
                bv_sequences.push(BaseSequence::from(&cur_partition).to_bv());
                endpoints.push(bv_sequences.n_bits());
            }

            Self {
                n_partitions,
                n,
                bv_upper_bounds: EliasFano::from(bv_upper_bounds.as_slice()),
                bv_sequences,
                endpoints,
                _phantom: PhantomData,
            }
        }

        // Self {
        //     n_partitions,
        //     n,
        //     bv_upper_bounds: EliasFano::from(bv_upper_bounds),
        //     bv_sequences,
        //     endpoints,
        //     _phantom: PhantomData,
        // }
    }
}

impl<'a, BaseSequence, BaseSequenceIter> ToBitvector
    for UniformPartitionedSequence<BaseSequence, BaseSequenceIter>
where
    BaseSequence: PostingList<'a, BaseSequenceIter>,
    BaseSequenceIter: IncreasingSequenceEnumerator,
{
    // serialization is done in the following way:
    // If only 1 partition:  | 1 | serialized  BaseSequence |
    // Else:                 | n partitions | bitlen of endpoints | list of endpoints | len of (upper bounds sequence) | elias_fano encoded upper bounds | serialized BaseSequences |

    // the number of partitions is encoded using delta (for now i use gamma)
    // each endpoint is expressed using `bitlen` bits
    fn to_bv(&self) -> BitVec {
        let mut bv = BitVec::new();
        // println!("n_partititions: {}", self.n_partitions);

        if self.n_partitions == 1 {
            bv.append_gamma(self.n_partitions as u64);
            bv.concat(&self.bv_sequences.bv);
        } else {
            let endpoint_bitlen = ceil_log2(*self.endpoints.last().unwrap() + 1);

            // println!("endpoint last = {}", *self.endpoints.last().unwrap());
            // println!("bitlen = {}", endpoint_bitlen);
            bv.append_gamma(self.n_partitions as u64);
            bv.append_gamma(endpoint_bitlen as u64);
            for &e in &self.endpoints {
                // println!("writing {}", e);
                bv.append_bits(e as u64, endpoint_bitlen as usize);
            }
            // println!("done");
            let upper_bounds_bv = self.bv_upper_bounds.to_bv();
            bv.append_gamma(upper_bounds_bv.len() as u64);
            // println!("ub start at: {} bits", bv.len());
            bv.concat(upper_bounds_bv);
            // println!("sequences start at: {} bits", bv.len());
            bv.concat(&self.bv_sequences.bv);
        }
        // println!("final len: {}", bv.len());

        bv
    }
}

impl<'a, BaseSequence, BaseSequenceIter> WriteBitvector
    for UniformPartitionedSequence<BaseSequence, BaseSequenceIter>
where
    BaseSequence: PostingList<'a, BaseSequenceIter> + WriteBitvector,
    BaseSequenceIter: IncreasingSequenceEnumerator,
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

impl<'a, BaseSequence, BaseSequenceIter>
    EnumeratorFromBitSlice<'a, UniformPartitionedSeqIter<'a, BaseSequence, BaseSequenceIter>>
    for UniformPartitionedSequence<BaseSequence, BaseSequenceIter>
where
    BaseSequence: PostingList<'a, BaseSequenceIter>,
    BaseSequenceIter: IncreasingSequenceEnumerator,
{
    fn iter_from_slice(
        bv: BitSliceWithOffset<'a>,
    ) -> UniformPartitionedSeqIter<'a, BaseSequence, BaseSequenceIter> {
        let (n_partitions, _) = unsafe { bv.get_gamma_unchecked(0) };
        // println!("n_parts: {}", n_partitions);
        if n_partitions == 1 {
            let cur_sequence =
                BaseSequence::iter_from_slice(bv.split_at(gamma_size(n_partitions)).1);

            UniformPartitionedSeqIter {
                position: 0,
                cur_base: 0,
                cur_ub: todo!(),
                cur_partition: 0,
                upper_bounds: EliasFanoIter::default(),
                n_partitions: 1,
                endpoints: Vec::default(),
                sequences: BitSliceWithOffset::default(),
                cur_sequence,
                cur_value: 0,
                _phantom: PhantomData,
                cur_begin: todo!(),
                cur_end: todo!(),
                len: todo!(),
                universe: todo!(),
            }
        } else {
            let (endpoint_bitlen, _) = unsafe { bv.get_gamma_unchecked(gamma_size(n_partitions)) };

            let start_endpoints = gamma_size(n_partitions) + gamma_size(endpoint_bitlen);

            let mut endpoints = vec![0];
            for idx in (start_endpoints..)
                .step_by(endpoint_bitlen as usize)
                .take(n_partitions as usize)
            {
                endpoints.push(bv.get_bits(idx, endpoint_bitlen as usize).unwrap() as usize);
            }

            let (ef_ub_size, _) = unsafe {
                bv.get_gamma_unchecked(start_endpoints + (n_partitions * endpoint_bitlen) as usize)
            };
            // println!("endpoints bitlen :{:?}", endpoint_bitlen);
            // println!("endpoints:{:?}", endpoints);

            let sep = start_endpoints
                + (n_partitions * endpoint_bitlen) as usize
                + gamma_size(ef_ub_size);
            // println!("ub: {} - {}", sep, sep + ef_ub_size as usize);

            let mut upper_bounds =
                EliasFano::iter_from_slice(bv.slice(sep, sep + ef_ub_size as usize));
            // println!("ub: {:?}", upper_bounds);

            let start_sequences = sep + ef_ub_size as usize;
            // println!("sequences start at: {}", start_sequences);

            let sequences = bv.split_at(start_sequences).1;
            // println!("sequences: {:?}", sequences);
            let cur_sequence =
                BaseSequence::iter_from_slice(sequences.slice(endpoints[0], endpoints[1]));

            UniformPartitionedSeqIter {
                position: 0,
                cur_partition: 0,
                cur_base: upper_bounds.next().unwrap(),
                cur_ub: todo!(),
                upper_bounds,
                n_partitions: n_partitions as usize,
                endpoints,
                sequences,
                cur_sequence,
                cur_value: 0,
                _phantom: PhantomData,
                cur_begin: todo!(),
                cur_end: todo!(),
                len: todo!(),
                universe: todo!(),
            }
        }
    }

    fn iter_from_slice_with_data(
        bv: BitSliceWithOffset<'a>,
        n: usize,
        u: u64,
    ) -> UniformPartitionedSeqIter<'a, BaseSequence, BaseSequenceIter> {
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
                cur_base: 0,
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
                len: todo!(),
                universe: todo!(),
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

pub struct UniformPartitionedSeqIter<'a, BaseSequence, BaseSequenceIter> {
    position: usize,
    cur_partition: usize,
    cur_base: u64,
    upper_bounds: EliasFanoIter<'a>,
    n_partitions: usize,
    endpoints: Vec<usize>,
    sequences: BitSliceWithOffset<'a>,
    cur_sequence: BaseSequenceIter,
    cur_value: u64,
    _phantom: PhantomData<BaseSequence>,
    cur_ub: u64,
    cur_begin: usize,
    cur_end: usize,
    len: usize,
    universe: u64,
}

impl<'a, BaseSequence, BaseSequenceIter> IncreasingSequenceEnumerator
    for UniformPartitionedSeqIter<'a, BaseSequence, BaseSequenceIter>
where
    BaseSequence: PostingList<'a, BaseSequenceIter>,
    BaseSequenceIter: IncreasingSequenceEnumerator,
{
    fn next_val(&mut self) -> Option<(u64, usize)> {
        self.position += 1;

        if let Some(x) = self.cur_sequence.next() {
            self.cur_value = x + self.cur_base;
            Some((self.cur_value, self.position))
        } else if self.cur_partition < self.n_partitions - 1 && self.n_partitions != 1 {
            // go to next partition, if any
            self.cur_partition += 1;

            self.cur_base = self.cur_ub + 1;
            self.cur_ub = self.upper_bounds.next().unwrap_or(self.universe);
            self.cur_begin = self.cur_end;
            self.cur_end = self.len.min((self.cur_partition + 1) * PARTITION_SIZE);

            // println!(
            //     "NEW SEQ n {} | u {}",
            //     self.cur_end - self.cur_begin,
            //     self.cur_ub - self.cur_base + 1
            // );

            self.cur_sequence = BaseSequence::iter_from_slice_with_data(
                self.sequences.slice(
                    self.endpoints[self.cur_partition],
                    self.endpoints[self.cur_partition + 1],
                ),
                self.cur_end - self.cur_begin,
                self.cur_ub - self.cur_base + 1,
            );

            // self.cur_base = self
            //     .upper_bounds
            //     .next()
            //     .expect("upper bounds is shorter than n partitions");

            self.cur_value = self.cur_base + self.cur_sequence.next().unwrap();
            Some((self.cur_value, self.position))
        } else {
            None
        }
    }

    fn next_geq(&mut self, i: u64) -> Option<(u64, usize)> {
        let mut val = self.cur_value;
        if i > self.cur_value {
            while val < i {
                val = self.next_val()?.0
            }
        }
        Some((val, self.position))
    }

    fn move_to_position(&mut self, pos: usize) {
        todo!()
    }

    fn position(&self) -> usize {
        todo!()
    }
}

impl<'a, BaseSequence, BaseSequenceIter> Iterator
    for UniformPartitionedSeqIter<'a, BaseSequence, BaseSequenceIter>
where
    BaseSequence: PostingList<'a, BaseSequenceIter>,
    BaseSequenceIter: IncreasingSequenceEnumerator,
{
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.next_val()?.0)
    }
}

impl<T, S> SpaceUsage for UniformPartitionedSequence<T, S> {
    fn space_usage_byte(&self) -> usize {
        self.bv_sequences.n_bits() / 8
            + self.bv_upper_bounds.space_usage_byte()
            + mem::size_of::<usize>() * self.endpoints.len()
            + 2 * mem::size_of::<usize>()
    }
}

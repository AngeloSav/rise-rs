use std::{marker::PhantomData, mem};

use serde::{Deserialize, Serialize};

use crate::{
    indexes::freq_index::PostingList, space_usage::SpaceUsage, utils::ceil_log2,
    BitSliceWithOffset, BitVec, BitVecCollection, EnumeratorFromBitSlice,
    IncreasingSequenceEnumerator, ToBitvector,
};

use super::{gamma_size, EliasFano, EliasFanoIter};

#[derive(Debug, Serialize, Deserialize)]
pub struct UniformPartitionedSequence<BaseSequence, BSIter, const PARTITION_SIZE: usize = 256> {
    n: usize,
    n_partitions: usize,
    bv_upper_bounds: EliasFano,
    bv_sequences: BitVecCollection,
    endpoints: Vec<usize>,
    _phantom: PhantomData<(BaseSequence, BSIter)>,
}

impl<'a, BaseSequence, BaseSequenceIter, const PARTITION_SIZE: usize>
    UniformPartitionedSequence<BaseSequence, BaseSequenceIter, PARTITION_SIZE>
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

        UniformPartitionedSeqIter {
            position: 0,
            cur_base,
            cur_partition: 0,
            upper_bounds,
            n_partitions: self.n_partitions,
            endpoints: self.endpoints.clone(),
            sequences: BitSliceWithOffset::new(&self.bv_sequences.bv, 0),
            cur_sequence: BaseSequence::iter_from_slice(first_seq),
            cur_value: 0,
            _phantom: PhantomData,
        }
    }
}

impl<'a, BaseSequence, BaseSequenceIter, const PARTITION_SIZE: usize> From<Vec<u64>>
    for UniformPartitionedSequence<BaseSequence, BaseSequenceIter, PARTITION_SIZE>
where
    BaseSequence: PostingList<'a, BaseSequenceIter>,
    BaseSequenceIter: IncreasingSequenceEnumerator,
{
    fn from(v: Vec<u64>) -> Self {
        let n = v.len();
        let n_partitions = usize::div_ceil(n, PARTITION_SIZE);
        let mut bv_sequences = BitVecCollection::default();

        let mut cur_partition = Vec::new();
        let mut bv_upper_bounds = Vec::new();
        if n_partitions == 1 {
            cur_partition.extend(v);
            bv_sequences.push(BaseSequence::from(cur_partition).to_bv());

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
                cur_partition = (&mut it).take(PARTITION_SIZE).collect();

                let cur_base = cur_partition[0];
                for el in cur_partition.iter_mut() {
                    *el -= cur_base;
                }

                bv_upper_bounds.push(cur_base);
                bv_sequences.push(BaseSequence::from(cur_partition).to_bv());
                endpoints.push(bv_sequences.n_bits());
            }

            Self {
                n_partitions,
                n,
                bv_upper_bounds: EliasFano::from(bv_upper_bounds),
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

impl<'a, BaseSequence, BaseSequenceIter, const PARTITION_SIZE: usize> ToBitvector
    for UniformPartitionedSequence<BaseSequence, BaseSequenceIter, PARTITION_SIZE>
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

impl<'a, BaseSequence, BaseSequenceIter, const PARTITION_SIZE: usize>
    EnumeratorFromBitSlice<'a, UniformPartitionedSeqIter<'a, BaseSequence, BaseSequenceIter>>
    for UniformPartitionedSequence<BaseSequence, BaseSequenceIter, PARTITION_SIZE>
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
                cur_partition: 0,
                upper_bounds: EliasFanoIter::default(),
                n_partitions: 1,
                endpoints: Vec::default(),
                sequences: BitSliceWithOffset::default(),
                cur_sequence,
                cur_value: 0,
                _phantom: PhantomData,
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
                upper_bounds,
                n_partitions: n_partitions as usize,
                endpoints,
                sequences,
                cur_sequence,
                cur_value: 0,
                _phantom: PhantomData,
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
            // go to next value, if any
            self.cur_partition += 1;

            self.cur_sequence = BaseSequence::iter_from_slice(self.sequences.slice(
                self.endpoints[self.cur_partition],
                self.endpoints[self.cur_partition + 1],
            ));
            self.cur_base = self
                .upper_bounds
                .next()
                .expect("upper bounds is shorter than n partitions");

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

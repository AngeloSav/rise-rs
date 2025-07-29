use crate::{
    elias_fano::{indexed_seq::StrictSequence, opt_partition::OptPartitionedSequence},
    gen_sequences::{gen_positive_sequence, gen_strictly_increasing_sequence},
    BitVector, EliasFano, EnumeratorFromBitSlice, SequenceEnumerator, WriteBitvector,
};

use super::positive_sequence::PositiveSequence;

#[test]
fn increasing_sequence() {
    let v = [1, 4, 43, 0, 5, 321];

    type TY = PositiveSequence<EliasFano>;

    let s = TY::write_bitvector(&v, v.len(), 0);
    let it = TY::iter_from_slice(s.as_bitslice(), v.len(), 0);

    println!("{:?}", it.collect::<Vec<_>>());
}

#[test]
fn increasing_sequence_opt() {
    let v: Vec<u64> = gen_strictly_increasing_sequence(1 << 15, 1 << 22)
        .into_iter()
        .map(|x| x as u64)
        .collect();

    type TY = PositiveSequence<OptPartitionedSequence<StrictSequence>>;

    let s: crate::BitVector<Vec<u64>> = TY::write_bitvector(v.as_slice(), v.len(), 0);
    let mut it = TY::iter_from_slice(s.as_bitslice(), v.len(), 0);

    println!("{:?} == {:?}", it.move_to_position(10), v[10]);
    println!("{:?} == {:?}", it.move_to_position(11), v[11]);
    println!("{:?} == {:?}", it.move_to_position(10), v[10]);
    println!("{:?} == {:?}", it.move_to_position(300), v[300]);
    println!("{:?} == {:?}", it.move_to_position(0), v[0]);
}

#[test]
fn test_random_opt() {
    let v: Vec<u64> = gen_positive_sequence(1 << 13, 1 << 15)
        .into_iter()
        .map(|x| x as u64 + 1)
        .collect();

    type TY = PositiveSequence<OptPartitionedSequence<StrictSequence>>;

    let s: BitVector<Vec<u64>> = TY::write_bitvector(v.as_slice(), v.len(), 0);
    let it = TY::iter_from_slice(s.as_bitslice(), v.len(), 0);

    assert_eq!(v, it.collect::<Vec<u64>>());
}

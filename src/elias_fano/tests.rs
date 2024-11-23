use crate::{
    elias_fano::EliasFano, gen_sequences::gen_strictly_increasing_sequence, utils::msb,
    IncreasingSequenceEnumerator,
};

use super::uniform_partitioned_seq::UniformPartitionedSequence;

#[test]
fn create_ef() {
    let v = vec![2, 3, 5, 7, 11, 13, 14, 256, 1024, 10000];

    let ef = EliasFano::from(v.clone());
    println!("{:?}", ef.bv);

    for b in ef.iter() {
        println!("{}", b);
    }
}

#[test]
fn next_geq_test() {
    let v = vec![2, 3, 5, 7, 11, 13, 14, 256, 1024, 10000];

    let ef = EliasFano::from(v.clone());
    let mut efi = ef.iter();
    println!("{:?}", efi.next_geq(3000));
}

#[test]
fn test_ef_iter_random() {
    let v = gen_strictly_increasing_sequence(1 << 12, 1 << 22)
        .iter()
        .map(|&x| x as u64)
        .collect::<Vec<_>>();

    let ef = EliasFano::from(v.clone());

    for (&a, b) in v.iter().zip(ef.iter()) {
        assert_eq!(a, b);
    }
}

#[test]
fn pg() {
    let a: UniformPartitionedSequence<EliasFano, _, 33> =
        UniformPartitionedSequence::from((0..).step_by(7).take(128).collect::<Vec<_>>());

    for e in a.iter() {
        println!("{:?}", e);
    }
}

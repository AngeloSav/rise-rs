use crate::{
    EnumeratorFromBitSlice, NextGEQ, SequenceEnumerator, WriteBitvector,
    elias_fano::{EliasFano, opt_partition::OptPartitionedSequence},
    gen_sequences::{gen_positive_sequence, gen_strictly_increasing_sequence},
    indexes::freq_index::{DocList, FreqList},
    positive_sequences::positive_sequence::PositiveSequence,
};

use super::{
    all_ones_seq::AllOnes,
    indexed_seq::{IndexedSequence, StrictSequence},
    ranked_bv::RankedBv,
    strict_ef::StrictEliasFano,
    uniform_partitioned_seq::UniformPartitionedSequence,
};

#[test]
fn create_ef() {
    let v: Vec<u64> = vec![2, 3, 5, 7, 11, 13, 14, 256, 1024, 10000];

    let ef = EliasFano::from(v.clone().as_slice());
    println!("{:?}", ef.bv);

    for (a, b) in ef.iter().zip(v.clone()) {
        println!("{}", b);
        assert_eq!(a, b);
    }

    let mut it = ef.iter();

    for i in 0..v.len() {
        println!("{:?}", it.move_to_position(i));
    }
}

#[test]
fn test_ef_iter_random() {
    let v = gen_strictly_increasing_sequence(1 << 12, 1 << 22)
        .iter()
        .map(|&x| x as u64)
        .collect::<Vec<_>>();

    let ef = EliasFano::from(v.clone().as_slice());

    for (&a, b) in v.iter().zip(ef.iter()) {
        assert_eq!(a, b);
    }
}

#[test]
fn test_strict_ef_iter_random() {
    let v = gen_strictly_increasing_sequence(1 << 12, 1 << 22)
        .iter()
        .map(|&x| x as u64)
        .collect::<Vec<_>>();

    let ef = StrictEliasFano::from(v.clone().as_slice());

    for (&a, b) in v.iter().zip(ef.iter()) {
        assert_eq!(a, b);
    }
}

#[test]
fn test_opt_iter_random() {
    let v = gen_strictly_increasing_sequence(1 << 12, 1 << 22)
        .iter()
        .map(|&x| x as u64)
        .collect::<Vec<_>>();

    let ef = <OptPartitionedSequence<IndexedSequence>>::from(v.clone().as_slice());

    for (&a, b) in v.iter().zip(ef.iter()) {
        assert_eq!(a, b);
    }
}

#[test]
fn test_uniform_iter_random() {
    let v = gen_strictly_increasing_sequence(1 << 12, 1 << 22)
        .iter()
        .map(|&x| x as u64)
        .collect::<Vec<_>>();

    let ef = <UniformPartitionedSequence<IndexedSequence>>::from(v.clone().as_slice());

    for (&a, b) in v.iter().zip(ef.iter()) {
        assert_eq!(a, b);
    }
}

#[test]
fn test_ef_small() {
    let v = vec![0, 1, 2, 3, 4, 5, 6, 61, 127, 200, 290, 1024, 1027];
    let a: EliasFano = EliasFano::from(v.clone().as_slice());

    for (a, b) in a.iter().zip(v) {
        assert!(a == b);
        println!("{:?}", a);
    }

    let mut it = a.iter();
    assert_eq!(it.next_val(), (0, 0));
    assert_eq!(it.next_geq(30), (61, 7));
}

#[test]
fn test_strictef_small() {
    let v = vec![0, 1, 2, 3, 4, 5, 6, 61, 127, 200, 290, 1024, 1027];
    let a = StrictEliasFano::from(v.clone().as_slice());

    for (a, b) in a.iter().zip(v) {
        assert!(a == b);
        println!("{:?}", a);
    }

    let mut it = a.iter();
    assert_eq!(it.next_val(), (0, 0));
}

#[test]
fn test_ranked_bv_small() {
    let v = vec![1, 2, 3, 4, 5, 6, 61, 62, 127, 200, 290];
    let a: RankedBv = RankedBv::from(v.clone().as_slice());

    for (a, b) in a.iter().zip(v) {
        assert!(a == b);
        println!("{:?}", a);
    }

    let mut it = a.iter();
    assert_eq!(it.next_val(), (1, 0));
    assert_eq!(it.next_geq(3), (3, 2));
    assert_eq!(it.next_geq(6), (6, 5));
    assert_eq!(it.next_geq(8).0, 61);
    assert_eq!(it.next_geq(199).0, 200);
}

#[test]
fn test_ranked_bv_small_new() {
    let v = vec![1, 2, 3, 4, 5, 6, 61, 62, 127, 200, 290];
    let a = RankedBv::write_bitvector(v.clone().as_slice(), v.len(), *v.last().unwrap() + 1);

    for (a, b) in
        RankedBv::iter_from_slice(a.as_bitslice(), v.len(), *v.last().unwrap() + 1).zip(v.clone())
    {
        assert!(a == b);
        println!("{:?}", a);
    }

    let mut it = RankedBv::iter_from_slice(a.as_bitslice(), v.len(), *v.last().unwrap() + 1);
    assert_eq!(it.next_val(), (1, 0));
    assert_eq!(it.next_geq(3), (3, 2));
    assert_eq!(it.next_geq(6), (6, 5));
    assert_eq!(it.next_geq(8).0, 61);
    assert_eq!(it.next_geq(199).0, 200);
}

#[test]
fn test_all_ones_small() {
    let v = vec![0, 1, 2, 3, 4, 5, 6];
    // let v = (0..=170).collect::<Vec<_>>();
    let a: AllOnes = AllOnes::from(v.clone().as_slice());

    for (a, b) in a.iter().zip(v) {
        assert!(a == b);
        println!("{:?}", a);
    }
}

#[test]
fn test_all_ones_small_new() {
    let v = vec![0, 1, 2, 3, 4, 5, 6];
    // let v = (0..=170).collect::<Vec<_>>();
    let a = AllOnes::write_bitvector(&v, v.len(), *v.last().unwrap() + 1);

    for (a, b) in AllOnes::iter_from_slice(a.as_bitslice(), v.len(), *v.last().unwrap() + 1).zip(v)
    {
        assert!(a == b);
        println!("{:?}", a);
    }
}

fn test_nextgeq<TY: DocList>() {
    let v = gen_strictly_increasing_sequence((1 << 13) + 100, 1 << 32)
        .iter()
        .map(|&x| x as u64)
        .collect::<Vec<_>>();

    let queries = gen_strictly_increasing_sequence(1 << 10, 1 << 32)
        .iter()
        .map(|&x| x as u64)
        .collect::<Vec<_>>();

    let binding = v.clone();
    let universe = *binding.last().unwrap() + 1;

    let x = TY::write_bitvector(binding.as_slice(), binding.len(), universe);

    let v_it = v.clone().into_iter();
    let mut it = TY::iter_from_slice(x.as_bitslice(), binding.len(), binding.last().unwrap() + 1);

    // it.move_to_position(0);

    for q in queries {
        let a = v_it
            .clone()
            .skip_while(|&x| x < q)
            .next()
            .unwrap_or(universe);
        let b = it.next_geq(q).0;

        assert_eq!(
            b,
            a,
            "query = {} | universe = {} | q > u = {} | {} Exists in sequence? {}",
            q,
            universe,
            q > universe,
            b,
            v.contains(&b)
        );
    }
}

#[test]
fn test_nextgeq_ef_random() {
    test_nextgeq::<EliasFano>();
}

#[test]
fn test_nextgeq_rbv_random() {
    test_nextgeq::<RankedBv>();
}

#[test]
fn test_nextgeq_indexed_random() {
    test_nextgeq::<IndexedSequence>();
}

#[test]
fn test_nextgeq_upis_random() {
    test_nextgeq::<UniformPartitionedSequence<IndexedSequence>>();
}

#[test]
fn test_nextgeq_opt_random() {
    test_nextgeq::<OptPartitionedSequence<IndexedSequence>>();
}

fn test_collect<TY: for<'a> FreqList>() {
    let v = gen_positive_sequence((1 << 13) + 100, 1 << 12)
        .iter()
        .map(|&x| x as u64)
        .collect::<Vec<_>>();

    let binding = v.clone();
    let x = TY::write_bitvector(
        binding.as_slice(),
        binding.len(),
        *binding.last().unwrap() + 1,
    );

    let it = TY::iter_from_slice(x.as_bitslice(), binding.len(), binding.last().unwrap() + 1);

    let collected: Vec<_> = it.collect();
    assert_eq!(v, collected);
}

#[test]
fn test_collect_positive_sequence_ef() {
    test_collect::<PositiveSequence<EliasFano>>();
}

#[test]
fn test_collect_positive_sequence_sef() {
    test_collect::<PositiveSequence<StrictEliasFano>>();
}

#[test]
fn test_collect_positive_sequence_sseq() {
    test_collect::<PositiveSequence<StrictSequence>>();
}

#[test]
fn test_collect_positive_sequence_opt() {
    test_collect::<PositiveSequence<OptPartitionedSequence<StrictSequence>>>();
}

#[test]
#[should_panic]
fn pg2() {
    let n = 78012;
    let mut v = vec![1; n];
    v[300] = 2;
    v[500] = 0;

    type TY = PositiveSequence<OptPartitionedSequence<StrictSequence>>;
    let s = TY::write_bitvector(v.as_slice(), v.len(), 0);
    let mut it = TY::iter_from_slice(s.as_bitslice(), n, 0);

    let i = 0;
    println!("it[{}] = {:?}", i, it.move_to_position(i));

    println!("it[{}] = {:?}", i, it.move_to_position(299));
    println!("it[{}] = {:?}", i, it.next_val());
    println!("it[{}] = {:?}", i, it.next_val());

    it.move_to_position(0);

    println!("{:?}", it);
}

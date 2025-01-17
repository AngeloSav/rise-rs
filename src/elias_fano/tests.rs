use crate::{
    bitvector::bitvector_collection::BitVectorCollection,
    elias_fano::{
        indexed_seq::{IndexSeqCostWindow, IndexedSequenceIter},
        opt_partition::{optimal_partition, OptPartitionedSequence},
        uniform_partitioned_seq::UniformPartitionedSeqIter,
        EliasFano,
    },
    gen_sequences::gen_strictly_increasing_sequence,
    utils::{gamma_size, msb, select_in_word},
    BitVec, CostWindow, EliasFanoIter, EnumeratorFromBitSlice, IncreasingSequenceEnumerator,
    ToBitvector, WriteBitvector,
};

use super::{
    all_ones_seq::AllOnes, indexed_seq::IndexedSequence, ranked_bv::RankedBv,
    uniform_partitioned_seq::UniformPartitionedSequence,
};

#[test]
fn create_ef() {
    let v: Vec<u64> = vec![2, 3, 5, 7, 11, 13, 14, 256, 1024, 10000];

    let ef = EliasFano::from(v.clone().as_slice());
    println!("{:?}", ef.bv);

    for b in ef.iter() {
        println!("{}", b);
    }
}

#[test]
fn next_geq_test() {
    let v = vec![2, 3, 5, 7, 11, 13, 14, 256, 1024, 10000];

    let ef = EliasFano::from(v.clone().as_slice());
    let mut efi = ef.iter();
    println!("{:?}", efi.next_geq(3000));
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
fn test_ef_small() {
    let v = vec![0, 1, 2, 3, 4, 5, 6, 61, 127, 200, 290, 1024, 1027];
    let a: EliasFano = EliasFano::from(v.clone().as_slice());

    for (a, b) in a.iter().zip(v) {
        assert!(a == b);
        println!("{:?}", a);
    }

    let mut it = a.iter();
    assert_eq!(it.next_val(), Some((0, 0)));
    assert_eq!(it.next_geq(30), Some((61, 7)));
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
    assert_eq!(it.next_val(), Some((1, 0)));
    assert_eq!(it.next_geq(3), Some((3, 2)));
    assert_eq!(it.next_geq(6), Some((6, 5)));
    assert_eq!(it.next_geq(8).unwrap().0, 61);
    assert_eq!(it.next_geq(199).unwrap().0, 200);
}

#[test]
fn test_ranked_bv_small_new() {
    let v = vec![1, 2, 3, 4, 5, 6, 61, 62, 127, 200, 290];
    let a = RankedBv::write_bitvector(v.clone().as_slice(), v.len(), *v.last().unwrap() + 1);

    for (a, b) in
        RankedBv::iter_from_slice_with_data(a.as_bitslice(), v.len(), *v.last().unwrap() + 1)
            .zip(v.clone())
    {
        assert!(a == b);
        println!("{:?}", a);
    }

    let mut it =
        RankedBv::iter_from_slice_with_data(a.as_bitslice(), v.len(), *v.last().unwrap() + 1);
    assert_eq!(it.next_val(), Some((1, 0)));
    assert_eq!(it.next_geq(3), Some((3, 2)));
    assert_eq!(it.next_geq(6), Some((6, 5)));
    assert_eq!(it.next_geq(8).unwrap().0, 61);
    assert_eq!(it.next_geq(199).unwrap().0, 200);
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

    for (a, b) in
        AllOnes::iter_from_slice_with_data(a.as_bitslice(), v.len(), *v.last().unwrap() + 1).zip(v)
    {
        assert!(a == b);
        println!("{:?}", a);
    }
}

#[test]
fn pg() {
    // let v = vec![1, 2, 3, 4, 5, 6, 10, 10000];
    // let v = (0..=4000).collect::<Vec<_>>();
    let v = gen_strictly_increasing_sequence(1 << 12, 1 << 22)
        .iter()
        .map(|&x| x as u64)
        .collect::<Vec<_>>();
    type TY<'a> = OptPartitionedSequence<IndexedSequence, IndexedSequenceIter<'a>>;
    // type TY<'a> = AllOnes;

    let binding = v.clone();
    let x = TY::write_bitvector(binding.as_slice(), v.len(), *v.last().unwrap());

    println!("{:?}", x);

    let mut bv = BitVectorCollection::with_capacity(0, 0);
    bv.push(x);

    let it = TY::iter_from_slice_with_data(bv.get(0), v.len(), *v.last().unwrap());

    for (a, b) in it.zip(v) {
        println!("{:?}", a);
        assert!(a == b);
    }
}

#[test]
fn pg2() {
    // let v = vec![1, 2, 3, 4, 5, 6, 10, 10000];
    // let v = (0..=4000).collect::<Vec<_>>();
    let mut v = gen_strictly_increasing_sequence(1 << 12, 1 << 12)
        .iter()
        .map(|&x| x as u64)
        .collect::<Vec<_>>();
    // type TY<'a> = UniformPartitionedSequence<IndexedSequence, IndexedSequenceIter<'a>>;
    type TY<'a> = EliasFano;

    v.extend(v.clone().iter().map(|x| x + 10000));

    let x = TY::from(v.clone().as_slice());

    // println!("{:?}", x);

    let mut bv = BitVectorCollection::with_capacity(0, 0);
    bv.push(x.to_bv());

    let mut it = TY::iter_from_slice(bv.get(0));

    let lb = 100;

    for i in TY::iter_from_slice(bv.get(0)).take(20) {
        println!("{}", i);
    }

    let a = it.next_geq(lb);
    println!("{:?}", a);

    assert_eq!(
        Some(a.unwrap().0),
        TY::iter_from_slice(bv.get(0))
            .skip_while(|x| x < &lb)
            .next()
    );

    println!(
        "{:?}",
        optimal_partition::<IndexSeqCostWindow>(&v, 0.0, 0.3)
    );

    println!("{:?}", IndexSeqCostWindow::single_block_cost(&v))
}

#[test]
fn pg3() {
    // let v = vec![1, 2, 3, 4, 5, 6, 10, 10000];
    // let v = (0..=4000).collect::<Vec<_>>();
    let v = gen_strictly_increasing_sequence((1 << 12) + 100, 1 << 22)
        .iter()
        .map(|&x| x as u64)
        .collect::<Vec<_>>();
    // type TY<'a> = OptPartitionedSequence<IndexedSequence, IndexedSequenceIter<'a>>;
    // type TY<'a> = UniformPartitionedSequence<EliasFano, EliasFanoIter<'a>>;
    type TY<'a> = UniformPartitionedSequence<IndexedSequence, IndexedSequenceIter<'a>>;
    // type TY<'a> = EliasFano;

    let binding = v.clone();
    let x = TY::write_bitvector(
        binding.as_slice(),
        binding.len(),
        *binding.last().unwrap() + 1,
    );

    // println!("{:?}", x);

    let mut bv = BitVectorCollection::with_capacity(0, 0);
    bv.push(x);

    let it = TY::iter_from_slice_with_data(bv.get(0), binding.len(), *binding.last().unwrap() + 1);

    for (a, b) in it.zip(v) {
        // println!("{:?} {}", a, b);
        assert!(a == b);
    }
}

#[test]
fn pg4() {
    // let v = vec![1, 2, 3, 4, 5, 6, 10, 10000];
    // let v = (0..=4000).collect::<Vec<_>>();
    let v = gen_strictly_increasing_sequence((1 << 12) + 100, 1 << 22)
        .iter()
        .map(|&x| x as u64)
        .collect::<Vec<_>>();
    type TY<'a> = OptPartitionedSequence<IndexedSequence, IndexedSequenceIter<'a>>;
    // type TY<'a> = UniformPartitionedSequence<EliasFano, EliasFanoIter<'a>>;
    // type TY<'a> = UniformPartitionedSequence<IndexedSequence, IndexedSequenceIter<'a>>;
    // type TY<'a> = EliasFano;
    // type TY<'a> = RankedBv;

    let binding = v.clone();
    let x = TY::write_bitvector(
        binding.as_slice(),
        binding.len(),
        *binding.last().unwrap() + 1,
    );

    // println!("{:?}", x);

    let mut bv = BitVectorCollection::with_capacity(0, 0);
    bv.push(x);

    let mut it =
        TY::iter_from_slice_with_data(bv.get(0), binding.len(), *binding.last().unwrap() + 1);

    println!("{:?}", &v[0..15]);

    println!("{:?}", it.next_geq(4));

    let i = 0;
    println!("{:?}", it.move_to_position(i));
    println!("{:?}", v[i]);

    let i = 3;
    println!("{:?}", it.move_to_position(i));
    println!("{:?}", v[i]);

    let i = 230;
    println!("{:?}", it.move_to_position(i));
    println!("{:?}", v[i]);

    let i = 256;
    println!("{:?}", it.move_to_position(i));
    println!("{:?}", v[i]);

    let i = 220;
    println!("{:?}", it.move_to_position(i));
    println!("{:?}", v[i]);

    let i = 222;
    println!("{:?}", it.move_to_position(i));
    println!("{:?}", v[i]);

    let i = 1050;
    println!("{:?}", it.move_to_position(i));
    println!("{:?}", v[i]);

    let i = 1700;
    println!("{:?}", it.move_to_position(i));
    println!("{:?}", v[i]);

    println!("back to zero");
    let i = 0;
    println!("{:?}", it.move_to_position(i));
    println!("{:?}", v[i]);

    let res = it.next_geq(7000).unwrap();
    println!("ngeq {:?}", res);
    println!("check: [{} {} {}]", v[res.1 - 1], v[res.1], v[res.1 + 1]);

    let res = it.next_geq(7000).unwrap();
    println!("ngeq {:?}", res);
    println!("check: [{} {} {}]", v[res.1 - 1], v[res.1], v[res.1 + 1]);

    let res = it.next_geq(300000).unwrap();
    println!("ngeq {:?}", res);
    println!("check: [{} {} {}]", v[res.1 - 1], v[res.1], v[res.1 + 1]);

    let res = it.next_geq(7000).unwrap();
    println!("ngeq {:?}", res);
    println!("check: [{} {} {}]", v[res.1 - 1], v[res.1], v[res.1 + 1]);
}

#[test]
fn pg5() {
    let v = vec![7, 8, 9, 12, 17, 500, 530, 10000];
    type TY<'a> = UniformPartitionedSequence<IndexedSequence, IndexedSequenceIter<'a>>;

    let binding = v.clone();
    let x = TY::write_bitvector(
        binding.as_slice(),
        binding.len(),
        *binding.last().unwrap() + 1,
    );

    // println!("{:?}", x);

    let mut bv = BitVectorCollection::with_capacity(0, 0);
    bv.push(x);

    let mut it =
        TY::iter_from_slice_with_data(bv.get(0), binding.len(), *binding.last().unwrap() + 1);

    println!("{:?}", it.next_geq(10000));
    println!("{:?}", it.next_geq(30));
}

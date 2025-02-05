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
    BitSliceWithOffset, BitVec, CostWindow, EliasFanoIter, EnumeratorFromBitSlice,
    IncreasingSequenceEnumerator, PartitionableSequence, ToBitvector, WriteBitvector,
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
    type TY<'a> = OptPartitionedSequence<IndexedSequence>;
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
    type TY<'a> = OptPartitionedSequence<IndexedSequence>;
    // type TY<'a> = UniformPartitionedSequence<EliasFano, EliasFanoIter<'a>>;
    // type TY<'a> = UniformPartitionedSequence<IndexedSequence>;
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
    // type TY<'a> = OptPartitionedSequence<IndexedSequence>;
    // type TY<'a> = UniformPartitionedSequence<EliasFano, EliasFanoIter<'a>>;
    // type TY<'a> = UniformPartitionedSequence<IndexedSequence, IndexedSequenceIter<'a>>;
    type TY<'a> = EliasFano;
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

    println!("{:?}", it.next_val());

    // let i = 0;
    // println!("{:?}", it.move_to_position(i));
    // println!("{:?}", v[i]);

    // let i = 3;
    // println!("{:?}", it.move_to_position(i));
    // println!("{:?}", v[i]);

    // let i = 230;
    // println!("{:?}", it.move_to_position(i));
    // println!("{:?}", v[i]);

    // let i = 256;
    // println!("{:?}", it.move_to_position(i));
    // println!("{:?}", v[i]);

    // let i = 220;
    // println!("{:?}", it.move_to_position(i));
    // println!("{:?}", v[i]);

    // let i = 222;
    // println!("{:?}", it.move_to_position(i));
    // println!("{:?}", v[i]);

    // let i = 1050;
    // println!("{:?}", it.move_to_position(i));
    // println!("{:?}", v[i]);

    // let i = 1700;
    // println!("{:?}", it.move_to_position(i));
    // println!("{:?}", v[i]);

    // println!("back to zero");
    // let i = 0;
    // println!("{:?}", it.move_to_position(i));
    // println!("{:?}", v[i]);

    let res = it.next_geq(0).unwrap();
    println!("ngeq 0 = {:?}", res);
    println!("check: [{} {}]", v[res.1], v[res.1 + 1]);

    let res = it.next_geq(7000).unwrap();
    println!("ngeq 7000 = {:?}", res);
    println!("check: [{} {} {}]", v[res.1 - 1], v[res.1], v[res.1 + 1]);

    let res = it.next_geq(7000).unwrap();
    println!("ngeq 7000 = {:?}", res);
    println!("check: [{} {} {}]", v[res.1 - 1], v[res.1], v[res.1 + 1]);

    let res = it.next_geq(300000).unwrap();
    println!("ngeq 300000 = {:?}", res);
    println!("check: [{} {} {}]", v[res.1 - 1], v[res.1], v[res.1 + 1]);

    let res = it.next_geq(7000).unwrap();
    println!("ngeq 7000 = {:?}", res);
    println!("check: [{} {} {}]", v[res.1 - 1], v[res.1], v[res.1 + 1]);

    let res = it.next_geq(v[v.len() - 1]).unwrap();
    println!("ngeq {} = {:?}", v[v.len() - 1], res);
    println!("check: [{} {}]", v[res.1 - 1], v[res.1]);
}

#[test]
fn pg5() {
    let v = vec![7, 8, 9, 12, 17, 500, 530, 10000];
    type TY<'a> = UniformPartitionedSequence<IndexedSequence>;

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

#[test]
fn testz() {
    let mut it = EliasFanoIter {
        slice_samples: BitSliceWithOffset {
            data: [
                5030082385371494333,
                6700311067498670348,
                3335167206170424279,
                16493540068612607561,
                10634017460287256807,
                15334724927230796795,
                8785910813499821854,
                4869397838282855209,
                10634374889026174354,
                15053995478851942708,
                5448762807452644178,
                5639183160298065486,
                16443608682209365518,
                10644144715072256286,
                10056200672135059603,
                5642226808733142267,
                14577723518974053966,
                10634010628803669258,
                15245691425624658835,
                11062052705420261689,
                10181485176690910611,
                15250565876976166680,
                712816104089742932,
                11390510799850899769,
                9876421995902321211,
                10612526734923590537,
                2613375746323354515,
                8640121175229811784,
                16039931873647191282,
                16470308396878111605,
                4123348804767328861,
                17070073999288514873,
                7085975112781740344,
                14829220134901834235,
                4117291609571670296,
                1374,
                0,
                13221349936692985856,
                17211481206134110075,
                12694561901081941437,
                4684889932200335962,
                5167881102189489872,
                7647264788450877636,
                12999282836810682095,
                8570481682431565098,
                17775443139632756669,
                12509281326254480758,
                12224261341067310302,
                12311621656517422427,
                6091943880752560061,
                17256631550089063847,
                7411688842969675709,
                599556446212524966,
                15007337673822071914,
                8580877740135922909,
                17846377346165818313,
                14602199029457319734,
                10026905719389130908,
                6608562184026307309,
                3639424563859000677,
                7503365031554880050,
                9898613208182303512,
                8889751232142217301,
                7647693345211792879,
                15732680326041287479,
                579493849330342230,
                423019184324853840,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                3813320747074453504,
            ]
            .as_ref(),
            n_bits: 36,
            offset: 62,
        },
        slice_samples1: BitSliceWithOffset {
            data: [
                6700311067498670348,
                3335167206170424279,
                16493540068612607561,
                10634017460287256807,
                15334724927230796795,
                8785910813499821854,
                4869397838282855209,
                10634374889026174354,
                15053995478851942708,
                5448762807452644178,
                5639183160298065486,
                16443608682209365518,
                10644144715072256286,
                10056200672135059603,
                5642226808733142267,
                14577723518974053966,
                10634010628803669258,
                15245691425624658835,
                11062052705420261689,
                10181485176690910611,
                15250565876976166680,
                712816104089742932,
                11390510799850899769,
                9876421995902321211,
                10612526734923590537,
                2613375746323354515,
                8640121175229811784,
                16039931873647191282,
                16470308396878111605,
                4123348804767328861,
                17070073999288514873,
                7085975112781740344,
                14829220134901834235,
                4117291609571670296,
                1374,
                0,
                13221349936692985856,
                17211481206134110075,
                12694561901081941437,
                4684889932200335962,
                5167881102189489872,
                7647264788450877636,
                12999282836810682095,
                8570481682431565098,
                17775443139632756669,
                12509281326254480758,
                12224261341067310302,
                12311621656517422427,
                6091943880752560061,
                17256631550089063847,
                7411688842969675709,
                599556446212524966,
                15007337673822071914,
                8580877740135922909,
                17846377346165818313,
                14602199029457319734,
                10026905719389130908,
                6608562184026307309,
                3639424563859000677,
                7503365031554880050,
                9898613208182303512,
                8889751232142217301,
                7647693345211792879,
                15732680326041287479,
                579493849330342230,
                423019184324853840,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                3813320747074453504,
            ]
            .as_ref(),
            n_bits: 48,
            offset: 34,
        },
        slice_lo: BitSliceWithOffset {
            data: [
                3335167206170424279,
                16493540068612607561,
                10634017460287256807,
                15334724927230796795,
                8785910813499821854,
                4869397838282855209,
                10634374889026174354,
                15053995478851942708,
                5448762807452644178,
                5639183160298065486,
                16443608682209365518,
                10644144715072256286,
                10056200672135059603,
                5642226808733142267,
                14577723518974053966,
                10634010628803669258,
                15245691425624658835,
                11062052705420261689,
                10181485176690910611,
                15250565876976166680,
                712816104089742932,
                11390510799850899769,
                9876421995902321211,
                10612526734923590537,
                2613375746323354515,
                8640121175229811784,
                16039931873647191282,
                16470308396878111605,
                4123348804767328861,
                17070073999288514873,
                7085975112781740344,
                14829220134901834235,
                4117291609571670296,
                1374,
                0,
                13221349936692985856,
                17211481206134110075,
                12694561901081941437,
                4684889932200335962,
                5167881102189489872,
                7647264788450877636,
                12999282836810682095,
                8570481682431565098,
                17775443139632756669,
                12509281326254480758,
                12224261341067310302,
                12311621656517422427,
                6091943880752560061,
                17256631550089063847,
                7411688842969675709,
                599556446212524966,
                15007337673822071914,
                8580877740135922909,
                17846377346165818313,
                14602199029457319734,
                10026905719389130908,
                6608562184026307309,
                3639424563859000677,
                7503365031554880050,
                9898613208182303512,
                8889751232142217301,
                7647693345211792879,
                15732680326041287479,
                579493849330342230,
                423019184324853840,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                3813320747074453504,
            ]
            .as_ref(),
            n_bits: 2102,
            offset: 18,
        },
        slice_hi: BitSliceWithOffset {
            data: [
                1374,
                0,
                13221349936692985856,
                17211481206134110075,
                12694561901081941437,
                4684889932200335962,
                5167881102189489872,
                7647264788450877636,
                12999282836810682095,
                8570481682431565098,
                17775443139632756669,
                12509281326254480758,
                12224261341067310302,
                12311621656517422427,
                6091943880752560061,
                17256631550089063847,
                7411688842969675709,
                599556446212524966,
                15007337673822071914,
                8580877740135922909,
                17846377346165818313,
                14602199029457319734,
                10026905719389130908,
                6608562184026307309,
                3639424563859000677,
                7503365031554880050,
                9898613208182303512,
                8889751232142217301,
                7647693345211792879,
                15732680326041287479,
                579493849330342230,
                423019184324853840,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                3813320747074453504,
            ]
            .as_ref(),
            n_bits: 2589,
            offset: 8,
        },
        n_bits_lo: 2,
        pointer_size: 12,
        position: 0,
        i_hi: 0,
        len: 1051,
        u: 6146,
        cur_value: 0,
    };

    //actual

    //     n_bits_lo: 2,
    //     pointer_size: 12,
    //     position: 1051,
    //     i_hi: 2587,
    //     len: 1051,
    //     u: 6146,
    //     cur_value: 3928,
    // serching lb in seq: 6145

    // println!("{:?}", it.clone().collect::<Vec<_>>());
    // println!("{:?}", it.next_geq(3928));
    // // println!("{:?}", it);

    // // println!("{:?}", it.collect::<Vec<_>>())

    // println!("{:?}", it.next_geq(128));
    println!("{:?}", it.next_geq(6145));

    //something wrong with nextgeq ????
}

#[test]
fn testz2() {
    // somethign wrong with this
    let v = [
        0, 5, 549, 550, 551, 552, 553, 555, 557, 558, 559, 560, 561, 562, 563, 564, 566, 568, 569,
        570, 575, 578, 580, 581, 582, 584, 585, 586, 587, 588, 590, 591, 592, 593, 597, 598, 599,
        600, 601, 602, 603, 604, 605, 607, 608, 609, 611, 612, 613, 614, 615, 616, 617, 618, 619,
        623, 625, 626, 627, 628, 630, 631, 632, 633, 634, 635, 638, 639, 641, 642, 643, 645, 646,
        648, 650, 654, 655, 657, 658, 659, 660, 661, 662, 663, 664, 665, 666, 667, 670, 671, 675,
        679, 680, 690, 697, 699, 706, 710, 715, 719, 727, 728, 736, 739, 744, 753, 768, 771, 772,
        798, 799, 803, 807, 808, 811, 812, 821, 825, 827, 830, 831, 833, 836, 839, 847, 852, 854,
        855, 858, 866, 875, 882, 886, 892, 915, 933, 953, 973, 977, 979, 980, 981, 983, 985, 987,
        990, 995, 997, 998, 999, 1001, 1009, 1010, 1014, 1016, 1019, 1020, 1021, 1022, 1023, 1069,
        1070, 1071, 1072, 1073, 1074, 1075, 1084, 1098, 1108, 1110, 1131, 1132, 1133, 1134, 1135,
        1136, 1137, 1138, 1139, 1140, 1152, 1153, 1154, 1156, 1157, 1158, 1165, 1167, 1171, 1172,
        1185, 1207, 1219, 1222, 1224, 1225, 1228, 1229, 1230, 1231, 1233, 1234, 1235, 1236, 1238,
        1239, 1244, 1245, 1249, 1250, 1251, 1252, 1253, 1254, 1256, 1257, 1259, 1260, 1261, 1262,
        1263, 1265, 1266, 1267, 1268, 1269, 1275, 1277, 1278, 1279, 1280, 1284, 1286, 1292, 1293,
        1306, 1308, 1310, 1314, 1318, 1322, 1327, 1332, 1336, 1337, 1339, 1342, 1344, 1345, 1353,
        1359, 1360, 1363, 1364, 1379, 1380, 1381, 1382, 1387, 1388, 1389, 1390, 1391, 1393, 1394,
        1395, 1412, 1413, 1414, 1415, 1417, 1419, 1421, 1422, 1423, 1426, 1428, 1429, 1430, 1431,
        1432, 1433, 1434, 1435, 1436, 1437, 1438, 1439, 1442, 1443, 1444, 1445, 1446, 1447, 1448,
        1449, 1451, 1452, 1455, 1457, 1458, 1461, 1462, 1464, 1468, 1469, 1470, 1471, 1484, 1485,
        1486, 1487, 1488, 1492, 1498, 1499, 1500, 1501, 1502, 1503, 1505, 1506, 1508, 1510, 1511,
        1513, 1517, 1518, 1519, 1523, 1526, 1527, 1534, 1537, 1539, 1540, 1543, 1544, 1548, 1549,
        1554, 1555, 1558, 1571, 1573, 1580, 1581, 1582, 1583, 1590, 1591, 1598, 1599, 1602, 1603,
        1605, 1610, 1612, 1613, 1614, 1615, 1616, 1617, 1626, 1629, 1630, 1631, 1633, 1635, 1638,
        1639, 1640, 1641, 1642, 1643, 1644, 1645, 1646, 1660, 1662, 1665, 1666, 1667, 1673, 1674,
        1682, 1684, 1689, 1699, 1700, 1703, 1710, 1715, 1716, 1717, 1719, 1720, 1723, 1726, 1731,
        1732, 1734, 1739, 1750, 1752, 1754, 1755, 1758, 1759, 1763, 1764, 1767, 1768, 1774, 1777,
        1782, 1787, 1790, 1792, 1797, 1800, 1802, 1803, 1804, 1806, 1809, 1811, 1813, 1819, 1820,
        1825, 1827, 1828, 1829, 1830, 1831, 1832, 1833, 1834, 1835, 1836, 1837, 1838, 1839, 1840,
        1841, 1842, 1843, 1844, 1845, 1846, 1847, 1850, 1851, 1853, 1855, 1856, 1857, 1858, 1862,
        1867, 1869, 1870, 1871, 1872, 1873, 1874, 1878, 1880, 1895, 1902, 1906, 1908, 1912, 1913,
        1915, 1922, 1924, 1926, 1928, 1929, 1934, 1935, 1936, 1937, 1941, 1943, 1944, 1945, 1949,
        1952, 1954, 1965, 1966, 1967, 1968, 1969, 1970, 1971, 1972, 1973, 1974, 1975, 1976, 1977,
        1978, 1979, 1980, 1981, 1982, 1983, 1984, 1985, 1986, 1987, 1988, 1989, 1990, 1991, 1992,
        1993, 1994, 1995, 1996, 1997, 1998, 1999, 2000, 2001, 2002, 2003, 2004, 2005, 2007, 2009,
        2010, 2011, 2012, 2013, 2014, 2015, 2016, 2017, 2018, 2019, 2020, 2021, 2022, 2024, 2026,
        2027, 2028, 2029, 2037, 2038, 2039, 2040, 2041, 2044, 2045, 2048, 2050, 2057, 2058, 2065,
        2066, 2075, 2076, 2077, 2078, 2081, 2097, 2098, 2100, 2109, 2112, 2113, 2114, 2126, 2130,
        2136, 2140, 2143, 2144, 2149, 2151, 2170, 2177, 2182, 2199, 2219, 2222, 2224, 2225, 2237,
        2239, 2240, 2242, 2272, 2294, 2305, 2308, 2311, 2325, 2326, 2338, 2339, 2346, 2357, 2379,
        2380, 2382, 2383, 2384, 2385, 2387, 2389, 2391, 2396, 2397, 2398, 2399, 2400, 2401, 2402,
        2406, 2407, 2408, 2409, 2411, 2416, 2417, 2421, 2425, 2426, 2427, 2430, 2433, 2434, 2436,
        2439, 2445, 2446, 2448, 2453, 2457, 2468, 2470, 2471, 2473, 2474, 2475, 2478, 2485, 2492,
        2493, 2494, 2495, 2509, 2510, 2512, 2516, 2525, 2526, 2527, 2528, 2537, 2538, 2548, 2556,
        2561, 2563, 2573, 2585, 2587, 2588, 2593, 2596, 2597, 2598, 2599, 2600, 2601, 2602, 2603,
        2604, 2607, 2608, 2609, 2616, 2618, 2619, 2621, 2622, 2623, 2626, 2627, 2628, 2630, 2632,
        2640, 2647, 2652, 2655, 2657, 2658, 2665, 2671, 2672, 2673, 2674, 2678, 2679, 2681, 2685,
        2694, 2697, 2700, 2704, 2712, 2713, 2720, 2721, 2722, 2728, 2746, 2753, 2754, 2756, 2774,
        2803, 2822, 2826, 2827, 2832, 2833, 2835, 2837, 2840, 2843, 2851, 2857, 2858, 2861, 2873,
        2875, 2876, 2877, 2880, 2881, 2883, 2885, 2888, 2889, 2891, 2892, 2893, 2894, 2895, 2896,
        2897, 2898, 2899, 2900, 2901, 2902, 2905, 2906, 2907, 2908, 2912, 2920, 2934, 2936, 2941,
        2949, 2950, 2953, 2955, 2956, 2957, 2958, 2960, 2961, 2964, 2969, 2972, 2981, 2982, 2984,
        2988, 2990, 3000, 3001, 3004, 3015, 3016, 3017, 3019, 3021, 3022, 3023, 3025, 3026, 3027,
        3029, 3030, 3035, 3038, 3041, 3044, 3048, 3050, 3051, 3075, 3077, 3086, 3087, 3099, 3105,
        3107, 3117, 3126, 3132, 3139, 3143, 3144, 3145, 3146, 3147, 3148, 3149, 3151, 3152, 3154,
        3155, 3156, 3157, 3158, 3159, 3160, 3164, 3165, 3166, 3172, 3177, 3193, 3214, 3217, 3218,
        3234, 3235, 3245, 3247, 3249, 3253, 3267, 3273, 3275, 3276, 3282, 3286, 3297, 3307, 3316,
        3336, 3337, 3338, 3339, 3340, 3341, 3342, 3343, 3344, 3350, 3358, 3368, 3369, 3374, 3376,
        3381, 3402, 3408, 3409, 3414, 3415, 3417, 3419, 3421, 3425, 3426, 3434, 3438, 3440, 3448,
        3452, 3455, 3456, 3457, 3458, 3459, 3460, 3464, 3465, 3466, 3467, 3471, 3472, 3473, 3476,
        3477, 3478, 3479, 3480, 3481, 3482, 3483, 3484, 3485, 3486, 3487, 3488, 3489, 3490, 3491,
        3492, 3493, 3494, 3495, 3496, 3497, 3498, 3515, 3537, 3540, 3551, 3565, 3583, 3593, 3605,
        3609, 3612, 3613, 3617, 3618, 3619, 3620, 3623, 3630, 3631, 3632, 3634, 3635, 3636, 3637,
        3638, 3639, 3640, 3641, 3643, 3649, 3650, 3652, 3655, 3658, 3661, 3662, 3668, 3669, 3675,
        3676, 3680, 3683, 3685, 3686, 3689, 3693, 3697, 3706, 3708, 3710, 3713, 3715, 3718, 3719,
        3723, 3725, 3728, 3738, 3739, 3740, 3741, 3742, 3743, 3744, 3745, 3750, 3751, 3752, 3753,
        3754, 3756, 3757, 3758, 3759, 3763, 3767, 3774, 3776, 3779, 3789, 3791, 3792, 3799, 3824,
        3858, 3861, 3888, 3889, 3890, 3891, 3892, 3895, 3897, 3899, 3900, 3901, 3904, 3905, 3906,
        3907, 3913, 3918, 3919, 3922, 3923, 3924, 3926, 3927, 3928, 3930, 3932, 3933, 3934, 3935,
        3936, 3938, 3939, 3941, 6145,
    ];

    let binding = EliasFano::from(v.as_slice());
    let mut it = binding.iter();
    //actual

    //     n_bits_lo: 2,
    //     pointer_size: 12,
    //     position: 1051,
    //     i_hi: 2587,
    //     len: 1051,
    //     u: 6146,
    //     cur_value: 3928,
    // serching lb in seq: 6145

    // println!("{:?}", it.clone().collect::<Vec<_>>());
    println!("{:?}", it.next_geq(3928));
    // // // println!("{:?}", it);

    // // // println!("{:?}", it.collect::<Vec<_>>())

    // println!("{:?}", it.next_geq(128));
    println!("{:?}", it.next_geq(6145));

    //something wrong with nextgeq ????
}

#[test]
fn test_nextgeq() {
    let v = gen_strictly_increasing_sequence((1 << 13) + 100, 1 << 32)
        .iter()
        .map(|&x| x as u64)
        .collect::<Vec<_>>();

    let queries = gen_strictly_increasing_sequence(1 << 10, 1 << 32)
        .iter()
        .map(|&x| x as u64)
        .collect::<Vec<_>>();

    // type TY<'a> = OptPartitionedSequence<IndexedSequence>;
    // type TY<'a> = UniformPartitionedSequence<EliasFano>;
    // type TY<'a> = UniformPartitionedSequence<IndexedSequence>;
    type TY<'a> = EliasFano;

    let binding = v.clone();
    let x = TY::write_bitvector(
        binding.as_slice(),
        binding.len(),
        *binding.last().unwrap() + 1,
    );

    let u = *binding.last().unwrap() + 1;
    let n = binding.len();
    println!("n {} | u {}", n, u);
    let n_lo_bits = msb(u / n as u64) + 1;

    println!("{:?}", &v[0..25]);

    let v_it = v.into_iter();
    let mut it =
        TY::iter_from_slice_with_data(x.as_bitslice(), binding.len(), binding.last().unwrap() + 1);

    for (i, &q) in queries.iter().enumerate() {
        let a = v_it.clone().skip_while(|&x| x < q).next().unwrap();
        let b = it.next_geq(q).map(|(x, _)| x).unwrap();

        assert_eq!(
            b,
            a,
            "q is {:?}\n hi {}\n lo {}",
            (i, q),
            if a >> n_lo_bits == b >> n_lo_bits {
                "OK"
            } else {
                "FAIL"
            },
            if a % (1 << n_lo_bits) == b % (1 << n_lo_bits) {
                "OK"
            } else {
                "FAIL"
            }
        );
    }
}

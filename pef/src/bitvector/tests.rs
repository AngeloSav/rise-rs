use super::*;
use crate::gen_sequences::{gen_strictly_increasing_sequence, negate_vector};

#[test]
fn test_is_empty() {
    let bv = BitVec::default();
    assert!(bv.is_empty());
}

// Build a bit vector of size n with even positions set to one
// and odd ones to zero
fn build_alternate(n: usize) -> BitVec {
    let mut bv = BitVec::with_capacity(n);
    for i in 0..n {
        bv.push(i % 2 == 0);
    }
    bv
}

#[test]
fn test_get() {
    let n = 1024 + 13;
    let bv = build_alternate(n);

    for i in 0..n {
        assert_eq!(bv.get(i).unwrap(), i % 2 == 0);
    }
}

#[test]
fn test_iter() {
    let n = 1024 + 13;
    let bv: BitVec = build_alternate(n).into();

    for (i, bit) in bv.into_iter().enumerate() {
        assert_eq!(bit, i % 2 == 0);
    }
}

#[test]
fn test_get_set_bits() {
    let n = 1024 + 13;
    let mut bv = BitVec::new();
    bv.extend_with_zeros(n);

    assert_eq!(bv.get_bits(61, 35).unwrap(), 0);
    assert_eq!(bv.get_bits(0, 42).unwrap(), 0);
    assert_eq!(bv.get_bits(n - 42 - 1, 42).unwrap(), 0);
    assert_eq!(bv.get_bits(n - 42, 42).unwrap(), 0);
    assert_eq!(bv.get_bits(n - 1, 1).unwrap(), 0);
    assert_eq!(bv.get_bits(n - 42, 43), None);
    bv.set_bits(0, 6, 42);
    assert_eq!(bv.get_bits(0, 6).unwrap(), 42);
    bv.set_bits(n - 61 - 1, 61, 42);
    assert_eq!(bv.get_bits(n - 61 - 1, 61).unwrap(), 42);
    bv.set_bits(n - 67 - 1, 33, 42);
    assert_eq!(bv.get_bits(n - 67 - 1, 33).unwrap(), 42);
}

#[test]
fn test_from_iter() {
    let n = 1024 + 13;
    let bv = build_alternate(n);

    let bv2: BitVec = (0..n).map(|x| x % 2 == 0).collect();

    assert_eq!(bv, bv2);

    /* Note: if last bits are zero, the bit vector may differ
    because we are inserting only position of ones */
    let bv2: BitVec = (0..n).filter(|x| x % 2 == 0).collect();

    assert_eq!(bv, bv2);
}

#[test]
fn test_next_one_and_zero() {
    let n = 1024 + 13;
    let bv = BitVec::with_ones(n);

    for i in 0..n {
        assert_eq!(bv.next_one(i).unwrap(), i);
    }
    assert_eq!(bv.next_one(n), None);

    let v = vec![
        1, 129, 193, 257, 321, 385, 449, 513, 577, 641, 705, 769, 833, 897, 961,
    ];

    let bv = BitVec::from_iter(v.iter().copied());

    let mut prev_pos = 0;
    for &p in v.iter() {
        assert_eq!(bv.next_one(prev_pos).unwrap(), p);
        prev_pos = p + 1;
    }
    assert_eq!(bv.next_one(*v.last().unwrap() + 1), None);

    for offset in [10, 64, 123, 961] {
        let bswo = BitSliceWithOffset::new(&bv, offset);

        assert_eq!(bswo.len(), bv.len() - offset);

        let mut prev_pos = 0;
        for p in v.iter().filter(|&&x| x >= offset).map(|&x| x - offset) {
            assert_eq!(bswo.next_one(prev_pos).unwrap(), p);
            prev_pos = p + 1;
        }
        assert_eq!(bswo.next_one(*v.last().unwrap() - offset + 1), None);
    }

    let bv = BitVec::from_iter(v.iter().copied());

    let v = negate_vector(&v);

    let mut prev_pos = 0;
    for &p in v.iter() {
        assert_eq!(bv.next_zero(prev_pos).unwrap(), p);
        prev_pos = p + 1;
    }
    assert_eq!(bv.next_zero(*v.last().unwrap() + 1), None);
}

#[test]
fn test_get_bits_iter() {
    for n_bits in 3..4 {
        let mut bv = BitVec::new();
        let max = 1 << n_bits;

        for i in 0..1024 {
            bv.append_bits(i % max, n_bits);
        }

        let mut iter = bv.ones();
        for i in 0..1024 {
            assert_eq!(iter.get_bits(n_bits), Some(i % max));
        }
    }
}

#[test]
fn test_get_bits_iter_2() {
    let bv = BitVec::from_iter(vec![0, 63, 128, 129, 254, 1026]);

    for n_bits in 1..64 {
        let mut iter = bv.ones();
        for position in (0..bv.len() - n_bits).step_by(n_bits) {
            assert_eq!(iter.get_bits(n_bits), bv.get_bits(position, n_bits));
        }
    }
}

#[test]
#[ignore]
fn test_gamma() {
    let mut bv = BitVec::new();
    for i in 0..3042 {
        bv.append_gamma(i);
    }

    for (i, dec) in bv.iter_gamma().enumerate() {
        assert_eq!(dec, i as u64);
    }
}

#[test]
fn test_iter_zeros() {
    let bv = BitVec::default();
    let v: Vec<usize> = bv.zeros().collect();
    assert!(v.is_empty());

    let vv: Vec<usize> = vec![0, 63, 128, 129, 254, 1026];
    let bv: BitVec = vv.iter().copied().collect();

    let v: Vec<usize> = bv.zeros().collect();
    assert_eq!(v, negate_vector(&vv));

    let v: Vec<usize> = bv.zeros_with_pos(63).collect();
    assert_eq!(v[0], 64);
    assert_eq!(*v.last().unwrap(), 1025);
}

#[test]
fn test_iter_ones() {
    let bv = BitVec::default();
    let v: Vec<usize> = bv.ones().collect();
    assert!(v.is_empty());

    let vv: Vec<usize> = vec![0, 63, 128, 129, 254, 1026];
    let bv: BitVec = vv.iter().copied().collect();

    let v: Vec<usize> = bv.ones().collect();
    assert_eq!(v, vv);

    let v: Vec<usize> = bv.ones_with_pos(127).collect();
    assert_eq!(v, vec![128, 129, 254, 1026]);

    let v: Vec<usize> = bv.ones_with_pos(129).collect();
    assert_eq!(v, vec![129, 254, 1026]);

    let v: Vec<usize> = bv.ones_with_pos(130).collect();
    assert_eq!(v, vec![254, 1026]);

    let v: Vec<usize> = bv.ones_with_pos(1027).collect();
    assert_eq!(v, vec![]);

    let vv: Vec<usize> = (0..1024).collect();
    let bv: BitVec = vv.iter().copied().collect();
    let v: Vec<usize> = bv.ones().collect();
    assert_eq!(v, vv);

    let vv = gen_strictly_increasing_sequence(1024 * 4, 1 << 20);

    let bv: BitVec = vv.iter().copied().collect();
    let v: Vec<usize> = bv.ones().collect();
    assert_eq!(v, vv);
}

#[test]
fn test_concat() {
    let mut bv1 = BitVec::new();
    bv1.push(true);
    bv1.push(false);

    let mut bv2 = BitVec::new();
    bv2.push(true);
    bv2.push(true);

    bv1.concat(&bv2);

    assert_eq!(bv1.len(), 4);
    assert_eq!(bv1.get(2), Some(true));

    let vv: Vec<usize> = vec![0, 63, 128, 129, 254, 1026];
    let mut bv1: BitVec = vv.iter().copied().collect();
    let bv2: BitVec = vv.iter().copied().collect();
    bv1.concat(bv2);
    assert_eq!(bv1.len(), 2054);
    assert_eq!(bv1.get(1026), Some(true));
    assert_eq!(bv1.get(2053), Some(true));
    assert_eq!(bv1.get(2054), None);
}

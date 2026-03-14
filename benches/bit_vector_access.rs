use divan::{Bencher, black_box};
use pef::{AccessBin, BitVec, bitvector::bitvector_collection::BitVectorCollectionBuilder};

fn main() {
    divan::main();
}

// Number of bits accessed in get_bits and get_bits_unchecked
const LENS: &[usize] = &[1, 2, 4, 8, 13, 16, 17, 32];

#[divan::bench]
fn get_bit(bencher: Bencher) {
    let bv = BitVec::from_iter(vec![0, 63, 128, 129, 254, 1026]);
    bencher.bench_local(move || {
        for i in 0..bv.len() {
            let _ = black_box(bv.get(i));
        }
    });
}

#[divan::bench]
fn get_bit_unchecked(bencher: Bencher) {
    let bv = BitVec::from_iter(vec![0, 63, 128, 129, 254, 1026]);
    bencher.bench_local(move || {
        for i in 0..bv.len() {
            unsafe {
                let _ = black_box(bv.get_unchecked(i));
            }
        }
    });
}

#[divan::bench(args = LENS)]
fn get_bits(bencher: Bencher, len: usize) {
    let bv = BitVec::from_iter(vec![0, 63, 128, 129, 254, 1026]);
    bencher.bench_local(move || {
        for i in 0..(bv.len() - len) {
            let _ = black_box(bv.get_bits(i, len));
        }
    });
}

#[divan::bench(args = LENS)]
fn get_bits_unchecked(bencher: Bencher, len: usize) {
    let bv = BitVec::from_iter(vec![0, 63, 128, 129, 254, 1026]);
    bencher.bench_local(move || {
        for i in 0..(bv.len() - len) {
            unsafe {
                let _ = black_box(bv.get_bits_unchecked(i, len));
            }
        }
    });
}

#[divan::bench(args = LENS)]
fn get_bits_unchecked_iter(bencher: Bencher, len: usize) {
    let bv = BitVec::from_iter(vec![0, 63, 128, 129, 254, 1026]);
    let n_bits = bv.len();
    bencher.bench_local(move || {
        for _ in 0..len {
            let mut iter = bv.ones();
            for _ in (0..n_bits - len).step_by(len) {
                let _ = black_box(unsafe { iter.get_bits_unchecked(len) });
            }
        }
    });
}

#[divan::bench(args = LENS)]
fn get_bits_unchecked_collection(bencher: Bencher, len: usize) {
    let mut bc = BitVectorCollectionBuilder::default();
    for _ in 0..3 {
        let bv = BitVec::from_iter(vec![0, 63, 128, 129, 254, 341]);
        bc.push(&bv);
    }
    let bc = bc.build();

    bencher.bench_local(move || {
        for i in 0..(341 - 32) {
            unsafe {
                let bv = bc.get(0);
                let _ = black_box(bv.get_bits_unchecked(i, len));
            }
            unsafe {
                let bv = bc.get(1);
                let _ = black_box(bv.get_bits_unchecked(i, len));
            }
            unsafe {
                let bv = bc.get(2);
                let _ = black_box(bv.get_bits_unchecked(i, len));
            }
        }
    });
}

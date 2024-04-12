use divan::{black_box, Bencher};
use pef::{AccessBin, BitVec};

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
        for i in 0..(bv.len() - 32) {
            let _ = black_box(bv.get_bits(i, len));
        }
    });
}

#[divan::bench(args = LENS)]
fn get_bits_unchecked(bencher: Bencher, len: usize) {
    let bv = BitVec::from_iter(vec![0, 63, 128, 129, 254, 1026]);
    bencher.bench_local(move || {
        for i in 0..(bv.len() - 32) {
            unsafe {
                let _ = black_box(bv.get_bits_unchecked(i, len));
            }
        }
    });
}

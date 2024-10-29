use divan::{black_box, Bencher};
use pef::{
    elias_fano::{ef_bv::EliasFano2, EliasFano},
    gen_sequences::gen_strictly_increasing_sequence,
};

fn main() {
    divan::main();
}

const LENS_1: &[usize] = &[1 << 10, 1 << 12, 1 << 14, 1 << 16, 1 << 20, 1 << 25];

#[divan::bench(args = LENS_1)]
fn ef_iter_bench(bencher: Bencher, len: usize) {
    let vec: Vec<_> = gen_strictly_increasing_sequence(len, 1 << 32)
        .into_iter()
        .map(|x| x as u64)
        .collect();
    let binding = EliasFano::from(vec);
    let mut ef = binding.iter();

    bencher.bench_local(move || {
        for _ in 0..len {
            let _ = black_box(ef.next());
        }
    });
}

#[divan::bench(args = LENS_1)]
fn ef2_iter_bench(bencher: Bencher, len: usize) {
    let vec: Vec<_> = gen_strictly_increasing_sequence(len, 1 << 32)
        .into_iter()
        .map(|x| x as u64)
        .collect();
    let binding = EliasFano2::from(vec);
    let mut ef = binding.iter();

    bencher.bench_local(move || {
        for _ in 0..len {
            let _ = black_box(ef.next());
        }
    });
}

#[divan::bench(args = LENS_1)]
fn vec_iter_bench(bencher: Bencher, len: usize) {
    let vec: Vec<_> = gen_strictly_increasing_sequence(len, len << 1)
        .into_iter()
        .map(|x| x as u64)
        .collect();
    let mut it = vec.into_iter();

    bencher.bench_local(move || {
        for _ in 0..len {
            let _ = black_box(it.next());
        }
    });
}

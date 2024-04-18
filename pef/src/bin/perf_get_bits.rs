use divan::black_box;
use pef::gen_sequences::gen_strictly_increasing_sequence;
use pef::AccessBin;

const N_RUNS: usize = 500;

fn main() {
    let n_bits = 1026;
    let v = gen_strictly_increasing_sequence(n_bits / 2, n_bits);

    let bv = pef::bitvector::BitVec::from_iter(v);

    let mut timings = pef::utils::TimingQueries::new(N_RUNS, n_bits);

    for _ in 0..N_RUNS {
        timings.start();
        for i in 0..bv.len() {
            let _ = unsafe { black_box(bv.get_unchecked(i)) };
        }

        timings.stop();
    }
    let (_, _, avg) = timings.get_float();
    println!("{:>2} bits: avg: {:.2} ns", 1, avg);

    for bit_len in 1..=64 {
        let mut timings = pef::utils::TimingQueries::new(N_RUNS, n_bits - bit_len);

        for _ in 0..N_RUNS {
            timings.start();
            for _ in 0..bit_len {
                for pos in (0..bv.len() - bit_len).step_by(bit_len) {
                    let _ = black_box(unsafe { bv.get_bits_unchecked(pos, bit_len) });
                }
            }
            timings.stop();
        }
        let (_, _, avg) = timings.get_float();
        println!("{bit_len:>2} bits: avg: {:.2} ns", avg);
    }

    for bit_len in 1..=64 {
        let mut timings = pef::utils::TimingQueries::new(N_RUNS, n_bits - bit_len);

        for _ in 0..N_RUNS {
            timings.start();
            for _ in 0..bit_len {
                let mut iter = bv.ones();
                for _ in (0..bv.len() - bit_len).step_by(bit_len) {
                    let _ = black_box(unsafe { iter.get_bits_unchecked(bit_len) });
                }
            }
            timings.stop();
        }
        let (_, _, avg) = timings.get_float();
        println!("{bit_len:>2} bits: avg: {:.2} ns", avg);
    }
}

use divan::black_box;
use pef::{
    gen_sequences::{gen_strictly_increasing_sequence, DGaps},
    BitVec,
};

const N_RUNS: usize = 500;

fn main() {
    let n = 1024;   

    // for (i, p) in GAMMA_TABLE.iter().enumerate() {
    //     println!("{i}: {}, {}", p.0, p.1);
    // }

    // let mut bv: BitVec = BitVec::new();
    // for gap in 0..10000 {
    //     bv.append_gamma(gap);
    // }

    // let mut pos = 0;
    // for gap in 0..10000 {
    //     let bits = unsafe{ bv.get_bits_unchecked(pos, GAMMA_BITS) };

    //     let (v, d) = GAMMA_TABLE[bits as usize];

    //     if v != 0 {

    //         pos += d as usize;
    //         let v = v -1;
    //         assert_eq!(v as u64, gap);
    //         continue;
    //     } 

    //     let (v, d) = unsafe{ bv.get_gamma_unchecked(pos)};

    //     pos = d;
    //     assert_eq!(v, gap);
    // }

    let mut timings = pef::utils::TimingQueries::new(N_RUNS, n);
    for avg_gap in (2..1000).step_by(100) {
        let u = avg_gap * n;
        let v = gen_strictly_increasing_sequence(n, u);

        let dgaps = DGaps::new(v.into_iter().map(|x| x as u64));
        let mut bv: BitVec = BitVec::new();

        for gap in dgaps {
            bv.append_gamma(gap);
        }

        for _ in 0..N_RUNS {
            timings.start();
            let _ = black_box(bv.iter_gamma().map(|gap| gap + 1).sum::<u64>());
            timings.stop();
        }

        let (_, _, avg) = timings.get_float();
        println!("Avg gap {avg_gap}: avg: {:.2} ns", avg);
    }
}

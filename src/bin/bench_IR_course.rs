/// GodBolt  flags:
use divan::black_box;
use num::{Bounded, PrimInt};
use rand::{distributions::uniform::SampleUniform, Rng};

use rand::seq::SliceRandom;
use rand::thread_rng;

#[allow(dead_code)]
fn random_permutation(n: usize) -> Vec<usize> {
    let mut v: Vec<usize> = (0..n).collect(); // Create a vector with numbers from 0 to n-1
    let mut rng = thread_rng(); // Random number generator
    v.shuffle(&mut rng); // Shuffle the vector
    v
}

fn random_permutation_with_one_cycle(n: usize) -> Vec<usize> {
    let mut v: Vec<usize> = (0..n).collect();
    v.shuffle(&mut thread_rng());

    // This permutation v may contain multiple cycles,
    // so instead we use it to construct another permutation q with a single cycle
    // see https://en.algorithmica.org/hpc/cpu-cache/latency/
    let mut q = vec![0; n];
    let mut k = v[n - 1];
    for i in 0..n {
        q[k] = v[i];
        k = v[i];
    }

    q
}

fn generate_random_vector<T>(size: usize, max_v: T) -> Vec<T>
where
    T: PrimInt + Bounded + SampleUniform,
{
    let mut rng = rand::thread_rng();
    (0..size)
        .map(|_| rng.gen_range(T::min_value()..max_v))
        .collect()
}

const N_RUNS: usize = 5;
const SEQUENCE_SIZE: usize = 256;

/// https://godbolt.org/z/9dbcPTs91
/// https://uica.uops.info/?code=vpaddq%20%20ymm5%2C%20ymm4%2C%20ymmword%20ptr%20%5Brdi%20%2B%208*rax%5D%0D%0Avpaddq%20%20ymm6%2C%20ymm4%2C%20ymmword%20ptr%20%5Brdi%20%2B%208*rax%20%2B%2032%5D%0D%0Avpaddq%20%20ymm7%2C%20ymm4%2C%20ymmword%20ptr%20%5Brdi%20%2B%208*rax%20%2B%2064%5D%0D%0Avpaddq%20%20ymm8%2C%20ymm4%2C%20ymmword%20ptr%20%5Brdi%20%2B%208*rax%20%2B%2096%5D%0D%0Avpsrlq%20%20ymm9%2C%20ymm2%2C%2032%0D%0Avpmuludq%20%20%20%20%20%20%20%20ymm9%2C%20ymm9%2C%20ymm5%0D%0Avpsrlq%20%20ymm10%2C%20ymm5%2C%2032%0D%0Avpmuludq%20%20%20%20%20%20%20%20ymm10%2C%20ymm10%2C%20ymm2%0D%0Avpaddq%20%20ymm9%2C%20ymm9%2C%20ymm10%0D%0Avpsllq%20%20ymm9%2C%20ymm9%2C%2032%0D%0Avpmuludq%20%20%20%20%20%20%20%20ymm2%2C%20ymm5%2C%20ymm2%0D%0Avpaddq%20%20ymm2%2C%20ymm9%2C%20ymm2%0D%0Avpsrlq%20%20ymm5%2C%20ymm3%2C%2032%0D%0Avpmuludq%20%20%20%20%20%20%20%20ymm5%2C%20ymm6%2C%20ymm5%0D%0Avpsrlq%20%20ymm9%2C%20ymm6%2C%2032%0D%0Avpmuludq%20%20%20%20%20%20%20%20ymm9%2C%20ymm9%2C%20ymm3%0D%0Avpaddq%20%20ymm5%2C%20ymm9%2C%20ymm5%0D%0Avpsllq%20%20ymm5%2C%20ymm5%2C%2032%0D%0Avpmuludq%20%20%20%20%20%20%20%20ymm3%2C%20ymm6%2C%20ymm3%0D%0Avpaddq%20%20ymm3%2C%20ymm3%2C%20ymm5%0D%0Avpsrlq%20%20ymm5%2C%20ymm1%2C%2032%0D%0Avpmuludq%20%20%20%20%20%20%20%20ymm5%2C%20ymm7%2C%20ymm5%0D%0Avpsrlq%20%20ymm6%2C%20ymm7%2C%2032%0D%0Avpmuludq%20%20%20%20%20%20%20%20ymm6%2C%20ymm6%2C%20ymm1%0D%0Avpaddq%20%20ymm5%2C%20ymm5%2C%20ymm6%0D%0Avpsllq%20%20ymm5%2C%20ymm5%2C%2032%0D%0Avpmuludq%20%20%20%20%20%20%20%20ymm1%2C%20ymm7%2C%20ymm1%0D%0Avpaddq%20%20ymm1%2C%20ymm1%2C%20ymm5%0D%0Avpsrlq%20%20ymm5%2C%20ymm0%2C%2032%0D%0Avpmuludq%20%20%20%20%20%20%20%20ymm5%2C%20ymm8%2C%20ymm5%0D%0Avpsrlq%20%20ymm6%2C%20ymm8%2C%2032%0D%0Avpmuludq%20%20%20%20%20%20%20%20ymm6%2C%20ymm6%2C%20ymm0%0D%0Avpaddq%20%20ymm5%2C%20ymm5%2C%20ymm6%0D%0Avpsllq%20%20ymm5%2C%20ymm5%2C%2032%0D%0Avpmuludq%20%20%20%20%20%20%20%20ymm0%2C%20ymm8%2C%20ymm0%0D%0Avpaddq%20%20ymm0%2C%20ymm0%2C%20ymm5&syntax=asIntel&uArchs=SKL&tools=uiCA&alignment=0

#[inline]
pub fn do_stuff(v: &[u64]) -> u64 {
    let mut prod = 1;

    for &value in v.iter() {
        prod *= value;
    }

    prod
}

/// https://godbolt.org/z/TjdYq6Pe8
#[inline]
pub fn do_more_stuff(v: &[u64]) -> (u64, u64) {
    let mut prod = 1;
    let mut sum = 0;

    for &value in v.iter() {
        prod *= value;
        sum += value;
    }

    (prod, sum)
}

#[inline]
pub fn do_more_stuff_2(v: &[u64]) -> (u64, u64, u64) {
    let mut prod = 1;
    let mut sum = 0;
    let mut xor = 0;

    for &value in v.iter() {
        prod *= value;
        sum += value;
        xor ^= value;
    }

    (prod, sum, xor)
}

#[inline]
pub fn do_more_stuff_3(v: &[u64]) -> (u64, u64, u64, u64) {
    let mut prod = 1;
    let mut sum = 0;
    let mut xor = 0;
    let mut or = 0;

    for &value in v.iter() {
        prod *= value;
        sum += value;
        xor ^= value;
        or |= value;
    }

    (prod, sum, xor, or)
}

pub fn do_more_stuff_4(v: &[u64]) -> (u64, u64, u64) {
    let mut prod = 1;
    let mut sum = 0;
    let mut xor = 0;

    for &value in v.iter() {
        sum += value;
        prod *= sum;
        xor ^= prod;
        sum += xor;
    }

    (prod, sum, xor)
}

pub fn do_more_stuff_5(v: &[u64]) -> (u64, u64, u64) {
    let mut prod = 1;
    let mut sum = 0;
    let mut xor = 0;

    for &value in v.iter() {
        // values are random numbers in [0, 99]
        if value > 50 {
            sum += value;
            prod *= sum;
            xor ^= prod;
            sum += xor;
        }
    }

    (prod, sum, xor)
}

#[inline]
pub fn do_stuff_half_wow(v: &[u64]) -> u64 {
    let mut prod = 1;

    for &value in v.iter() {
        if prod >= u64::MAX / 2 {
            prod *= value + 1;
        }
    }
    prod
}

#[inline]
pub fn sum(v: &[usize]) -> usize {
    let mut sum = 0;

    for &value in v.iter() {
        sum += value;
    }

    sum
}

#[inline]
pub fn sum_by_jumping(v: &[usize]) -> usize {
    let mut sum = 0;

    let mut k = 0;
    for _ in 0..v.len() {
        sum += v[k];
        k = v[k];
    }

    sum
}

fn main() {
    let v = generate_random_vector::<u64>(SEQUENCE_SIZE, 100);

    let mut count = 0;

    for &value in v.iter() {
        if value > 50 {
            count += 1;
        }
    }

    println!("count: {} tot: {}", count, v.len());

    let mut timings = pef::utils::TimingQueries::new(N_RUNS, SEQUENCE_SIZE);

    for _ in 0..N_RUNS {
        timings.start();

        let _ = black_box(do_stuff(&v));

        timings.stop();
    }

    let (_min, _max, avg) = timings.get_float();
    println!(
        "do_stuff():\t\tTime per iteration: {:.1} ns avg {_min} {_max}",
        avg
    );

    let mut timings = pef::utils::TimingQueries::new(N_RUNS, SEQUENCE_SIZE);

    for _ in 0..N_RUNS {
        timings.start();

        let (_, _) = black_box(do_more_stuff(&v));

        timings.stop();
    }

    let (_min, _max, avg) = timings.get_float();
    println!(
        "do_more_stuff():\tTime per iteration: {:.1} ns avg {_min} {_max}",
        avg
    );

    let mut timings = pef::utils::TimingQueries::new(N_RUNS, SEQUENCE_SIZE);

    for _ in 0..N_RUNS {
        timings.start();

        let _ = black_box(do_more_stuff_2(&v));

        timings.stop();
    }

    let (_min, _max, avg) = timings.get_float();
    println!(
        "do_more_stuff_2():\tTime per iteration: {:.1} ns avg {_min} {_max}",
        avg
    );

    let mut timings = pef::utils::TimingQueries::new(N_RUNS, SEQUENCE_SIZE);

    for _ in 0..N_RUNS {
        timings.start();

        let _ = black_box(do_more_stuff_3(&v));

        timings.stop();
    }

    let (_min, _max, avg) = timings.get_float();
    println!(
        "do_more_stuff_3():\tTime per iteration: {:.1} ns avg {_min} {_max}",
        avg
    );

    let mut timings = pef::utils::TimingQueries::new(N_RUNS, SEQUENCE_SIZE);

    for _ in 0..N_RUNS {
        timings.start();

        let _ = black_box(do_more_stuff_4(&v));

        timings.stop();
    }

    let (_min, _max, avg) = timings.get_float();
    println!(
        "do_more_stuff_4():\tTime per iteration: {:.1} ns avg {_min} {_max}",
        avg
    );

    let mut timings = pef::utils::TimingQueries::new(N_RUNS, SEQUENCE_SIZE);

    for _ in 0..N_RUNS {
        timings.start();

        let _ = black_box(do_more_stuff_5(&v));

        timings.stop();
    }

    let (_min, _max, avg) = timings.get_float();
    println!(
        "do_more_stuff_5():\tTime per iteration: {:.1} ns avg {_min} {_max}",
        avg
    );

    let w: Vec<u64> = (0..256).map(|i| if i % 2 == 0 { 0 } else { 99 }).collect();
    let mut timings = pef::utils::TimingQueries::new(N_RUNS, SEQUENCE_SIZE);

    for _ in 0..N_RUNS {
        timings.start();

        let _ = black_box(do_more_stuff_5(&w));

        timings.stop();
    }

    let (_min, _max, avg) = timings.get_float();
    println!(
        "do_more_stuff_5():\tTime per iteration: {:.1} ns avg {_min} {_max}",
        avg
    );

    let pi: Vec<usize> = random_permutation_with_one_cycle(16777216);

    let mut timings = pef::utils::TimingQueries::new(N_RUNS, pi.len());
    for _ in 0..N_RUNS {
        timings.start();

        let s = black_box(sum(&pi));

        timings.stop();
        println!("sum: {}", s);
    }

    let (_min, _max, avg) = timings.get_float();
    println!(
        "sum():\t\t  \t\tTime per value: {:.1} ns avg {_min} {_max}",
        avg
    );

    let mut timings = pef::utils::TimingQueries::new(N_RUNS, pi.len());
    for _ in 0..N_RUNS {
        timings.start();

        let s = black_box(sum_by_jumping(&pi));

        timings.stop();
        println!("sum: {}", s);
    }

    let (_min, _max, avg) = timings.get_float();
    println!(
        "sum_by_jumping():\t\t Time per value: {:.1} ns avg {_min} {_max}",
        avg
    );
}

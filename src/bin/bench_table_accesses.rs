//! We simulate several table accesses in a loop and measure the time it takes to access the table.
//! We use an array of 1000000 u16 elements for the sequences of accesses and a table of u32 elements.
//! The sizes of the table vary as a power of 2 from 2^8 to 2^16. The cache capacity usually is 32Kb, so
//! the table of 2^13 elments perfectly fits in the cache.

use divan::black_box;
use num::{Bounded, PrimInt};
use rand::{distributions::uniform::SampleUniform, Rng};
use std::fmt::Debug;
use std::mem;

const N_RUNS: usize = 5;
const SEQUENCE_SIZE: usize = 1000000;
const LOG_MIN: usize = 8;
const LOG_MAX: usize = 24;

fn generate_random_array<T>(size: usize, max_v: T) -> Vec<T>
where
    T: PrimInt + Bounded + SampleUniform,
{
    let mut rng = rand::thread_rng();
    (0..size)
        .map(|_| rng.gen_range(T::min_value()..max_v))
        .collect()
}

fn measure_table_access<T>(log_min: usize, log_max: usize)
where
    T: PrimInt + Bounded + SampleUniform,
{
    for log in log_min..=log_max {
        let size = 1 << log - 1;
        let table = generate_random_array::<T>(size, T::max_value());
        let table_size = mem::size_of_val(&table[0]) * table.len();

        let sequence = generate_random_array::<u32>(SEQUENCE_SIZE, size as u32);
        let mut timings = pef::utils::TimingQueries::new(N_RUNS, SEQUENCE_SIZE);

        for _ in 0..N_RUNS {
            timings.start();

            for &pos in sequence.iter() {
                let _ = black_box(table[pos as usize]);
            }

            timings.stop();
        }
        let (min, max, avg) = timings.get_float();
        println!(
            "Access Table size: 2^{log} ({:.0} Kb) in {:.2} ns avg (min: {:.2} ns, max: {:.2} ns)",
            table_size / 1024,
            avg,
            min,
            max
        );
    }
}

fn measure_table_depend_access<T>(log_min: usize, log_max: usize)
where
    T: PrimInt + Bounded + SampleUniform + TryInto<usize> + TryFrom<usize>,
    <T as TryInto<usize>>::Error: Debug,
    <T as TryFrom<usize>>::Error: Debug,
{
    for log in log_min..=log_max {
        let size = 1 << log - 1;
        let table = generate_random_array::<T>(size, T::max_value());
        let table_size = mem::size_of_val(&table[0]) * table.len();

        let mut timings = pef::utils::TimingQueries::new(N_RUNS, SEQUENCE_SIZE);

        for _ in 0..N_RUNS {
            let mut pos = 0;
            timings.start();

            for _ in 0..SEQUENCE_SIZE {
                pos = black_box((table[pos as usize] % T::max_value()).try_into().unwrap());
            }

            timings.stop();
        }
        let (min, max, avg) = timings.get_float();
        println!(
            "Access Table size: 2^{log} ({:.0} Kb) in {:.2} ns avg (min: {:.2} ns, max: {:.2} ns)",
            table_size / 1024,
            avg,
            min,
            max
        );
    }
}

fn main() {
    println!("Table with u16 elements");
    measure_table_access::<u16>(LOG_MIN, LOG_MAX);

    println!("Table with u16 elements (dependent access)");
    measure_table_depend_access::<u16>(LOG_MIN, LOG_MAX);

    println!("\n\nTable with u32 elements");
    measure_table_access::<u32>(LOG_MIN, LOG_MAX);

    println!("Table with u16 elements (dependent access)");
    measure_table_depend_access::<u32>(LOG_MIN, LOG_MAX);

    println!("\n\nTable with u64 elements");
    measure_table_access::<u64>(LOG_MIN, LOG_MAX);

    println!("Table with u64 elements (dependent access)");
    measure_table_depend_access::<u64>(LOG_MIN, LOG_MAX);

    println!("\n\nTable with u128 elements");
    measure_table_access::<u128>(LOG_MIN, LOG_MAX);
}

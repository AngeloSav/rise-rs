use std::time::Instant;

pub struct TimingQueries {
    timings: Vec<u128>,
    time: Instant,
    n_queries: usize,
}

impl TimingQueries {
    pub fn new(n_runs: usize, n_queries: usize) -> Self {
        Self {
            timings: Vec::with_capacity(n_runs),
            time: Instant::now(),
            n_queries,
        }
    }

    #[inline(always)]
    pub fn start(&mut self) {
        self.time = Instant::now();
    }

    #[inline(always)]
    pub fn stop(&mut self) {
        self.timings.push(self.time.elapsed().as_nanos());
    }

    /// Returns minimum, maximum, average query time per query in nanosecs.
    pub fn get(&self) -> (u128, u128, u128) {
        let min = *self.timings.iter().min().unwrap() / (self.n_queries as u128);
        let max = *self.timings.iter().max().unwrap() / (self.n_queries as u128);
        let avg =
            self.timings.iter().sum::<u128>() / ((self.timings.len() * self.n_queries) as u128);
        (min, max, avg)
    }

    /// Returns minimum, maximum, average query time per query in nanosecs.
    pub fn get_float(&self) -> (f64, f64, f64) {
        let min = *self.timings.iter().min().unwrap() as f64 / (self.n_queries as f64);
        let max = *self.timings.iter().max().unwrap() as f64 / (self.n_queries as f64);
        let avg = self.timings.iter().sum::<u128>() as f64
            / ((self.timings.len() * self.n_queries) as f64);
        (min, max, avg)
    }
}

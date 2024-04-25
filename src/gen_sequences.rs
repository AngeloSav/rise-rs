use num::Integer;
use rand::Rng;

/// Generates a random strictly increasing sequence of `n` values up to `u`.
pub fn gen_strictly_increasing_sequence(n: usize, u: usize) -> Vec<usize> {
    let mut rng = rand::thread_rng();
    let mut v: Vec<usize> = (0..n).map(|_x| rng.gen_range(0..(u - n))).collect();
    v.sort_unstable();
    for (i, value) in v.iter_mut().enumerate() {
        // remove duplicates to make a strictly increasing sequence
        *value += i;
    }
    v
}

/// An iterator that returns the difference between consecutive elements minus one!
pub struct DGaps<T: Integer, I: Iterator<Item = T>> {
    iter: I,
    prev: Option<T>,
}

impl<T: Integer, I: Iterator<Item = T>> DGaps<T, I> {
    pub fn new(iter: I) -> Self {
        Self { iter, prev: None }
    }
}

impl<T: Integer + Copy, I: Iterator<Item = T>> Iterator for DGaps<T, I> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let cur = self.iter.next()?;

        let gap = if let Some(prev) = self.prev {
            assert!(cur > prev, "Sequence is not strictly increasing");
            cur - prev - T::one()
        } else {
            self.prev = Some(cur);
            cur
        };

        self.prev = Some(cur);

        Some(gap)
    }
}

/// Given a strictly increasing vector v, it returns a vector with all
/// the values not in v.
pub fn negate_vector(v: &[usize]) -> Vec<usize> {
    let max = *v.last().unwrap();
    let mut vv = Vec::with_capacity(max - v.len() + 1);
    let mut j = 0;
    for i in 0..max {
        if i == v[j] {
            j += 1;
        } else {
            vv.push(i);
        }
    }
    assert_eq!(max - v.len() + 1, vv.len());
    vv
}

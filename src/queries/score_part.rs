use std::collections::VecDeque;

struct ScoreWindow<'a> {
    start_it: std::iter::Peekable<std::slice::Iter<'a, (u64, f32)>>,
    end_it: std::iter::Peekable<std::slice::Iter<'a, (u64, f32)>>,
    start: usize,
    end: usize,
    sum: f32,
    cost_upper_bound: usize,
    estimated_idf: f32,
    fixed_cost: f32,
    max_queue: VecDeque<f32>,
}

impl<'a> ScoreWindow<'a> {
    fn new(
        sequence: &'a [(u64, f32)],
        cost_upper_bound: usize,
        estimated_idf: f32,
        fixed_cost: f32,
    ) -> Self {
        let start_it = sequence.iter().peekable();
        let end_it = sequence.iter().peekable();

        ScoreWindow {
            start_it,
            end_it,
            start: 0,
            end: 0,
            sum: 0.0,
            estimated_idf,
            fixed_cost,
            max_queue: VecDeque::new(),
            cost_upper_bound,
        }
    }

    #[inline(always)]
    fn start(&self) -> usize {
        self.start
    }

    #[inline(always)]
    fn end(&self) -> usize {
        self.end
    }

    #[inline(always)]
    fn cost_upper_bound(&self) -> f32 {
        self.cost_upper_bound as f32
    }

    #[inline(always)]
    fn size(&self) -> usize {
        self.end - self.start
    }

    #[inline(always)]
    fn advance_start(&mut self) {
        if let Some(&&x) = self.start_it.peek() {
            let v = x.1 * self.estimated_idf;
            if x.1 == *self.max_queue.front().unwrap() {
                self.max_queue.pop_front();
            }

            self.sum -= v;
            self.start += 1;
            self.start_it.next();
        } else {
            panic!("window advanced too far!")
        }

        // todo!()
    }

    #[inline(always)]
    fn advance_end(&mut self) {
        if let Some(&&x) = self.end_it.peek() {
            let v = x.1 * self.estimated_idf;
            self.sum += v;
            while !self.max_queue.is_empty() && *self.max_queue.back().unwrap() < x.1 {
                self.max_queue.pop_back();
            }

            self.max_queue.push_back(x.1);

            self.end += 1;
            self.end_it.next();
        } else {
            panic!("window advanced too far!")
        }
        // todo!()
    }

    fn cost(&self) -> f32 {
        if self.size() < 2 {
            self.fixed_cost
        } else {
            self.size() as f32 * self.max_queue.front().unwrap() * self.estimated_idf - self.sum
                + self.fixed_cost
        }
    }

    fn max(&self) -> f32 {
        *self.max_queue.front().unwrap()
    }
}

pub fn score_opt_partition(
    seq: &[(u64, f32)],
    estimated_idf: f32,
    fixed_cost: f32,
    eps1: f32,
    eps2: f32,
) -> (Vec<u32>, Vec<u32>, Vec<f32>) {
    let mut max: f32 = 0.0;
    let mut sum = 0.0;

    let size = seq.len();

    for x in seq {
        max = max.max(x.1);
        sum += x.1 * estimated_idf;
    }

    let single_block_cost = size as f32 * max * estimated_idf - sum;

    let mut min_cost = vec![single_block_cost; size + 1];
    min_cost[0] = 0.0;

    let mut windows = Vec::new();
    let cost_lb = fixed_cost;
    let mut cost_bound = cost_lb;

    while eps1 == 0.0 || cost_bound < cost_lb / eps1 {
        let w = ScoreWindow::new(seq, cost_bound as usize, estimated_idf, fixed_cost);
        windows.push(w);
        if cost_bound >= single_block_cost {
            break;
        }

        cost_bound *= 1.0 + eps2;
    }

    let mut path = vec![0; size + 1];
    let mut maxs = vec![0.0; size + 1];

    let max_first = seq.iter().map(|x| x.1).fold(0.0, f32::max);
    maxs[size] = max_first;

    for i in 0..size {
        let mut last_end = i + 1;

        for window in windows.iter_mut() {
            debug_assert!(window.start() == i);

            while window.end() < last_end {
                window.advance_end();
            }

            let mut window_cost;
            loop {
                window_cost = window.cost();
                if min_cost[i] + window_cost < min_cost[window.end()] {
                    min_cost[window.end()] = min_cost[i] + window_cost;
                    path[window.end()] = window.start() as u64;
                    maxs[window.end()] = window.max();
                }

                last_end = window.end();
                if window.end() == seq.len() {
                    break;
                }
                if window_cost >= window.cost_upper_bound() {
                    break;
                }
                window.advance_end();
            }
            window.advance_start();
        }
    }

    let mut cur_pos = size;
    let mut max_vals_tmp = Vec::new();
    let mut partition = Vec::new();

    while cur_pos != 0 {
        partition.push(cur_pos);
        max_vals_tmp.push(maxs[cur_pos]);
        cur_pos = path[cur_pos as usize] as usize;
    }

    partition.reverse();
    max_vals_tmp.reverse();

    let mut docids = Vec::new();
    let mut max_values = Vec::new();
    let mut sizes = Vec::new();

    let mut cur = 0;
    for i in 0..(partition.len() - 1) {
        docids.push(seq[partition[i]].0 as u32 - 1);
        max_values.push(max_vals_tmp[i]);
        sizes.push((partition[i] - cur) as u32);
        cur = partition[i];
    }

    sizes.push((partition[partition.len() - 1] - cur) as u32);
    max_values.push(max_vals_tmp[partition.len() - 1]);
    docids.push(seq[size - 1].0 as u32);

    (sizes, docids, max_values)
}

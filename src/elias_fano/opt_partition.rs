use crate::CostWindow;

/// returns a pair (optimal cost, vector of positions) that are the optimal starting point for each block
pub fn optimal_partition<'a, T: CostWindow<'a>>(
    sequence: &'a [u64],
    eps1: f64,
    eps2: f64,
) -> (usize, Vec<usize>) {
    assert!(!sequence.is_empty(), "sequence is empty");
    let single_block_cost = T::single_block_cost(sequence);

    let mut min_cost = vec![single_block_cost; sequence.len() + 1];
    min_cost[0] = 0;

    let mut windows = Vec::new();
    let cost_lb = T::minimum_cost(sequence); // minimum cost
    let mut cost_bound = cost_lb;

    //initialize windows
    while eps1 == 0.0 || cost_bound < (cost_lb as f64 / eps1) as usize {
        windows.push(T::new(sequence, cost_bound));
        if cost_bound >= single_block_cost {
            break;
        }
        cost_bound = ((cost_bound as f64) * (1.0 + eps2)) as usize;
    }

    let mut path = vec![0usize; sequence.len() + 1];
    for i in 0..sequence.len() {
        let mut last_end = i + 1;
        for window in windows.iter_mut() {
            assert_eq!(window.start(), i);

            while window.end() < last_end {
                window.advance_end();
            }

            let mut window_cost;
            loop {
                window_cost = window.window_cost();
                if min_cost[i] + window_cost < min_cost[window.end()] {
                    min_cost[window.end()] = min_cost[i] + window_cost;
                    path[window.end()] = i;
                }

                last_end = window.end();
                if window.end() == sequence.len() {
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

    let mut partition = Vec::new();
    let mut partition_costs = Vec::new();

    let mut cur_pos = sequence.len();
    while cur_pos != 0 {
        partition.push(cur_pos);
        partition_costs.push(min_cost[cur_pos]);
        cur_pos = path[cur_pos];
    }

    partition.reverse();
    partition_costs.reverse();
    println!("{:?}", partition_costs);
    (min_cost[sequence.len()], partition)
}

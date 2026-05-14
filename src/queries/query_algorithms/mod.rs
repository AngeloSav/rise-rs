pub mod and;
pub mod bm_maxscore;
pub mod bm_wand;
pub mod maxscore;
pub mod or;
pub mod ranked_and;
pub mod ranked_or;
pub mod wand;

use std::collections::HashMap;

pub use and::And;
pub use bm_maxscore::BMMaxScore;
pub use bm_wand::BMWand;
pub use maxscore::MaxScore;
pub use or::Or;
pub use ranked_and::RankedAnd;
pub use ranked_or::RankedOr;
pub use wand::Wand;

#[inline]
// given a vector of terms, returns a vector of pairs (term, frequency in query)
fn query_freqs(terms: &[usize]) -> Vec<(usize, usize)> {
    let mut count: HashMap<usize, usize> = HashMap::new();

    for term in terms {
        *count.entry(*term).or_insert(0) += 1;
    }

    count.into_iter().collect::<Vec<_>>()
}

// // no weight
// fn query_freqs(terms: &[usize]) -> Vec<(usize, usize)> {
//     terms.iter().map(|&t| (t, 1)).collect()
// }

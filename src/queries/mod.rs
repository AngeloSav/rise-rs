#![allow(unused_variables)]
use std::collections::HashMap;

use crate::{
    indexes::freq_index::{DocList, FreqIndex, FreqList},
    queries::topk_heap::TopKHeap,
    DocScorer,
};

pub mod bm25;
pub mod posting_metadata;
pub mod topk_heap;

pub use posting_metadata::PostingMetadata;

pub trait QueryOperator {
    // this function takes an index `idx`, a number of terms `terms`,
    fn query<'a, T, S>(&mut self, idx: &'a FreqIndex<T, S>, terms: &[usize]) -> usize
    where
        T: DocList<'a>,
        S: FreqList<'a>;
}

pub struct Or;
pub struct And;
pub struct RankedAnd<Scorer: DocScorer> {
    p_data: PostingMetadata<Scorer>,
    topk_heap: TopKHeap,
}

pub struct Wand<Scorer: DocScorer> {
    p_data: PostingMetadata<Scorer>,
    topk_heap: TopKHeap,
}

pub struct MaxScore<Scorer: DocScorer> {
    p_data: PostingMetadata<Scorer>,
    topk_heap: TopKHeap,
}

impl QueryOperator for Or {
    fn query<'a, T, S>(&mut self, idx: &'a FreqIndex<T, S>, terms: &[usize]) -> usize
    where
        T: DocList<'a>,
        S: FreqList<'a>,
    {
        if terms.is_empty() {
            return 0;
        }

        let mut enums = Vec::with_capacity(terms.len());
        for &term in terms {
            let it = idx.get_plist_iter(term);
            enums.push(it);
        }

        let mut cur_doc = enums
            .iter()
            .map(|x| x.current_doc())
            .min()
            .unwrap()
            .unwrap_or(idx.n_docs as u64);
        let mut size = 0;

        while cur_doc < idx.n_docs as u64 {
            // println!("new round ---------------------");
            // println!("pushing {:?}", cur_doc);
            // unsafe { *v.get_unchecked_mut(size) = cur_doc };
            size += 1;

            let mut next_doc = idx.n_docs as u64;

            for it in enums.iter_mut() {
                let mut cur_term_docid = it.current_doc().unwrap_or(idx.n_docs as u64);
                // println!("new term ---");
                // println!("cur_docid = {:?}", cur_term_docid);
                if core::intrinsics::likely(cur_term_docid == cur_doc) {
                    // println!("update cur!");
                    it.next_doc();
                    cur_term_docid = it.current_doc().unwrap_or(idx.n_docs as u64);
                }

                // println!("check less ---");
                // println!("cur_doc = {:?}", cur_doc);
                // println!("cur_term_docid = {:?}", cur_term_docid);
                if core::intrinsics::likely(cur_term_docid < next_doc) {
                    next_doc = cur_term_docid
                }
            }
            cur_doc = next_doc;
            // println!("nextdoc is {:?}", cur_doc);
        }
        size
    }
}

impl QueryOperator for And {
    fn query<'a, T, S>(&mut self, idx: &'a FreqIndex<T, S>, terms: &[usize]) -> usize
    where
        T: DocList<'a>,
        S: FreqList<'a>,
    {
        if terms.is_empty() {
            return 0;
        }

        let mut enums = Vec::with_capacity(terms.len());

        for &term in terms {
            //lets try boxing
            enums.push(Box::new(idx.get_plist_iter(term)));
        }

        // sort by non-decreasing size
        enums.sort_by_key(|it| it.len());

        let max = idx.n_docs as u64;

        let mut candidate = enums[0].current_doc().unwrap_or(max);

        let mut i = 1;
        let mut size = 0;

        while candidate < max {
            for it in enums.iter_mut().skip(i) {
                it.next_geq(candidate);
                let current = it.current_doc().unwrap_or(max);
                if core::intrinsics::likely(current != candidate) {
                    candidate = current;
                    i = 0;
                    break;
                }
                i += 1;
            }

            if i == enums.len() {
                // unsafe { *v.get_unchecked_mut(size) = candidate };
                size += 1;
                enums[0].next_doc();
                candidate = enums[0].current_doc().unwrap_or(max);
                i = 1;
            }
        }
        size
    }
}

impl<Scorer: DocScorer> RankedAnd<Scorer> {
    pub fn new<'a>(p_data: PostingMetadata<Scorer>, k: usize) -> Self {
        let topk_heap = TopKHeap::new(k);

        Self { p_data, topk_heap }
    }
}

impl<Scorer: DocScorer> Wand<Scorer> {
    pub fn new<'a>(p_data: PostingMetadata<Scorer>, k: usize) -> Self {
        let topk_heap: TopKHeap = TopKHeap::new(k);
        Self { p_data, topk_heap }
    }
}

impl<Scorer: DocScorer> MaxScore<Scorer> {
    pub fn new<'a>(p_data: PostingMetadata<Scorer>, k: usize) -> Self {
        let topk_heap: TopKHeap = TopKHeap::new(k);
        Self { p_data, topk_heap }
    }
}

/// given a vector of terms, returns a vector of pairs (term, frequency in query)
fn query_freqs(terms: &[usize]) -> Vec<(usize, usize)> {
    let mut count: HashMap<usize, usize> = HashMap::new();

    for term in terms {
        *count.entry(*term).or_insert(0) += 1;
    }

    count.into_iter().collect::<Vec<_>>()
}

impl<Scorer: DocScorer> QueryOperator for RankedAnd<Scorer> {
    fn query<'a, T, S>(&mut self, idx: &'a FreqIndex<T, S>, terms: &[usize]) -> usize
    where
        T: DocList<'a>,
        S: FreqList<'a>,
    {
        if terms.is_empty() {
            return 0;
        }

        let query_freqs = query_freqs(terms);

        // contains pair (enum, weight)
        let mut enums = Vec::with_capacity(query_freqs.len());

        self.topk_heap.clear();

        for (term, freq) in query_freqs {
            let it = idx.get_plist_iter(term);
            let q_weight =
                Scorer::query_term_weight(freq as u64, it.len() as u64, idx.n_docs as u64);
            enums.push((it, q_weight));
        }

        // sort by increasing frequency
        enums.sort_by_key(|(it, _)| it.len());

        let max = idx.n_docs as u64;

        let mut candidate = enums[0].0.current_doc().unwrap_or(max);

        let mut i = 1;

        while candidate < max {
            for (it, q_weight) in enums.iter_mut().skip(i) {
                it.next_geq(candidate);
                let current = it.current_doc().unwrap_or(max);
                if core::intrinsics::likely(current != candidate) {
                    candidate = current;
                    i = 0;
                    break;
                }
                i += 1;
            }

            if i == enums.len() {
                let norm_len = self.p_data.get_norm_len(candidate as usize);
                let mut score = 0.0;

                for (it, q_weight) in enums.iter_mut() {
                    score += *q_weight * Scorer::doc_term_weight(it.freq().unwrap(), norm_len);
                }

                self.topk_heap.push(score);

                enums[0].0.next_doc();
                candidate = enums[0].0.current_doc().unwrap_or(max);
                i = 1;
            }
        }

        self.topk_heap.len()
    }
}

impl<Scorer: DocScorer> QueryOperator for Wand<Scorer> {
    fn query<'a, T, S>(&mut self, idx: &'a FreqIndex<T, S>, terms: &[usize]) -> usize
    where
        T: DocList<'a>,
        S: FreqList<'a>,
    {
        if terms.is_empty() {
            return 0;
        }

        let query_freqs = query_freqs(terms);

        // contains pair (enum, query_weight, max_score)
        let mut enums = Vec::with_capacity(query_freqs.len());

        self.topk_heap.clear();

        for (term, freq) in query_freqs {
            let it = idx.get_plist_iter(term);
            let q_weight =
                Scorer::query_term_weight(freq as u64, it.len() as u64, idx.n_docs as u64);

            let max_weight = q_weight * self.p_data.get_max_term_weigth(term);

            enums.push((it, q_weight, max_weight));
        }

        let mut ordered_enums = enums.iter_mut().collect::<Vec<_>>();

        ordered_enums.sort_by_key(|x| x.0.current_doc());
        loop {
            let mut upper_bound = 0.0;
            let mut found_pivot = false;
            let mut pivot = 0;

            while pivot < ordered_enums.len() {
                if ordered_enums[pivot].0.current_doc().is_none() {
                    break;
                }

                upper_bound += ordered_enums[pivot].2;

                if self.topk_heap.can_enter(upper_bound) {
                    found_pivot = true;
                    break;
                }

                pivot += 1;
            }

            // no pivot found, stop
            if !found_pivot {
                break;
            }

            let pivot_id = ordered_enums[pivot].0.current_doc();

            if pivot_id.is_some() && pivot_id == ordered_enums[0].0.current_doc() {
                //match, score pivot
                let mut score = 0.0;
                let norm_len = self.p_data.get_norm_len(pivot_id.unwrap() as usize);

                for scored_enum in ordered_enums.iter_mut() {
                    if scored_enum.0.current_doc() != pivot_id {
                        break;
                    }

                    score += scored_enum.1
                        * Scorer::doc_term_weight(scored_enum.0.freq().unwrap(), norm_len);
                    scored_enum.0.next_doc();
                }

                // insert in topk heap if possible
                self.topk_heap.push(score);

                ordered_enums.sort_by_key(|x| x.0.current_doc());
            } else {
                //no match
                let mut next_list = pivot;
                while ordered_enums[next_list].0.current_doc() == pivot_id {
                    next_list -= 1;
                }

                ordered_enums[next_list].0.next_geq(pivot_id.unwrap());

                for i in (next_list + 1)..ordered_enums.len() {
                    if ordered_enums[i].0.current_doc() < ordered_enums[i - 1].0.current_doc() {
                        ordered_enums.swap(i, i - 1);
                    } else {
                        break;
                    }
                }
            }
        }

        self.topk_heap.len()
    }
}

impl<Scorer: DocScorer> QueryOperator for MaxScore<Scorer> {
    fn query<'a, T, S>(&mut self, idx: &'a FreqIndex<T, S>, terms: &[usize]) -> usize
    where
        T: DocList<'a>,
        S: FreqList<'a>,
    {
        if terms.is_empty() {
            return 0;
        }
        let n_docs = idx.n_docs as u64;
        let query_freqs = query_freqs(terms);

        // contains pair (enum, weight)
        let mut enums = Vec::with_capacity(query_freqs.len());

        self.topk_heap.clear();

        for (term, freq) in query_freqs {
            let it = idx.get_plist_iter(term);
            let q_weight =
                Scorer::query_term_weight(freq as u64, it.len() as u64, idx.n_docs as u64);

            let max_weight = q_weight * self.p_data.get_max_term_weigth(term);

            enums.push((it, q_weight, max_weight));
        }

        let mut ordered_enums = enums.iter_mut().collect::<Vec<_>>();

        ordered_enums.sort_by(|x, y| x.2.partial_cmp(&y.2).unwrap());

        let upper_bounds = ordered_enums
            .iter()
            .map(|x| x.2)
            .scan(0f32, |s, x| {
                *s += x;
                Some(*s)
            })
            .collect::<Vec<_>>();

        let mut non_essential_lists = 0;
        let mut cur_doc = ordered_enums
            .iter()
            .map(|x| x.0.current_doc())
            .min()
            .unwrap()
            .unwrap_or(n_docs);

        while non_essential_lists < ordered_enums.len() && cur_doc < n_docs {
            let mut score = 0.0;
            let mut next_doc = n_docs;
            let norm_len = self.p_data.get_norm_len(cur_doc as usize);

            for i in non_essential_lists..ordered_enums.len() {
                if ordered_enums[i].0.current_doc() == Some(cur_doc) {
                    score += ordered_enums[i].1
                        * Scorer::doc_term_weight(ordered_enums[i].0.freq().unwrap(), norm_len);
                    ordered_enums[i].0.next_doc();
                }
                if ordered_enums[i].0.current_doc().is_some()
                    && ordered_enums[i].0.current_doc().unwrap() < next_doc
                {
                    next_doc = ordered_enums[i].0.current_doc().unwrap();
                }
            }

            for i in (0..non_essential_lists).rev() {
                if !self.topk_heap.can_enter(score + upper_bounds[i]) {
                    break;
                }
                ordered_enums[i].0.next_geq(cur_doc);
                if ordered_enums[i].0.current_doc() == Some(cur_doc) {
                    score += ordered_enums[i].1
                        * Scorer::doc_term_weight(ordered_enums[i].0.freq().unwrap(), norm_len);
                }
            }

            if self.topk_heap.can_enter(score) {
                self.topk_heap.push(score);

                while non_essential_lists < ordered_enums.len()
                    && !self.topk_heap.can_enter(upper_bounds[non_essential_lists])
                {
                    non_essential_lists += 1;
                }
            }

            cur_doc = next_doc;
        }

        self.topk_heap.len()
    }
}

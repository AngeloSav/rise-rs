use std::{
    cmp::{Ordering, Reverse},
    collections::BinaryHeap,
};

/// Implements a min heap with a limited capacity of `k` elements.
pub struct TopKHeap {
    heap: BinaryHeap<Reverse<PostingInfo>>,
    threshold: f32,
    k: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PostingInfo {
    pub docid: u64,
    pub frequency: f32,
}

impl Eq for PostingInfo {}

impl Ord for PostingInfo {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.frequency.total_cmp(&other.frequency)
    }
}

impl PartialOrd for PostingInfo {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl TopKHeap {
    // returns docids of retrieved elements, ordered by descending score
    // NOTE: this implementation may be inefficient as it clones the whole heap before iterating over it
    pub fn into_sorted_vec(&self) -> Vec<PostingInfo> {
        self.heap
            .clone()
            .into_sorted_vec()
            .into_iter()
            .map(|x| x.0)
            .collect()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.heap.len()
    }

    #[inline]
    pub fn can_enter(&self, v: f32) -> bool {
        self.heap.len() < self.k || self.threshold < v
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    #[inline]
    pub fn new(k: usize) -> Self {
        TopKHeap {
            heap: BinaryHeap::with_capacity(k),
            threshold: 0.0,
            k,
        }
    }

    pub fn clear(&mut self) {
        self.heap.clear();
        self.threshold = 0.0;
    }

    #[inline]
    pub fn push(&mut self, score: f32) -> bool {
        self.push_with_id(0, score)
    }

    #[inline]
    pub fn push_with_id(&mut self, id: u64, score: f32) -> bool {
        if self.heap.len() < self.k {
            self.heap.push(Reverse(PostingInfo {
                docid: id,
                frequency: score,
            }));
            self.threshold = self.heap.peek().unwrap().0.frequency;
            return true;
        } else if score > self.threshold {
            self.heap.pop();
            self.heap.push(Reverse(PostingInfo {
                docid: id,
                frequency: score,
            }));
            self.threshold = self.heap.peek().unwrap().0.frequency;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use crate::gen_sequences::gen_positive_sequence;

    use super::*;

    #[test]
    fn test_topk_heap() {
        let mut heap = TopKHeap::new(3);
        heap.push(1.0);
        heap.push(2.0);
        assert_eq!(heap.len(), 2);
        heap.push(3.0);
        heap.push(4.0);
        assert_eq!(heap.threshold, 2.0);
        assert_eq!(heap.len(), 3);

        println!("{:?}", heap.heap);
        heap.push(5.0);
        assert!(heap.can_enter(5.0));
        assert_eq!(heap.threshold, 3.0);

        assert!(!heap.can_enter(0.5));
        heap.push(0.5);
        println!("{:?}", heap.heap);

        heap.push(100.2);
        println!("{:?}", heap.heap);

        heap.push(4.1);
        println!("{:?}", heap.heap);

        heap.clear();
        assert_eq!(heap.len(), 0);
        assert_eq!(heap.threshold, 0.0);
    }

    #[test]
    fn test_random_topk_heap() {
        let mut heap = TopKHeap::new(10);
        let v: Vec<f32> = gen_positive_sequence(1000, 10_000)
            .into_iter()
            .map(|x| x as f32 / 1000.0)
            .collect();

        for &x in &v {
            heap.push(x);
        }

        let mut sorted_v = v.clone();
        sorted_v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let check = sorted_v.iter().cloned().rev().take(10).collect::<Vec<_>>();

        let in_heap = heap
            .into_sorted_vec()
            .into_iter()
            .map(|x| x.frequency)
            .collect::<Vec<_>>();

        assert_eq!(in_heap, check);
    }
}

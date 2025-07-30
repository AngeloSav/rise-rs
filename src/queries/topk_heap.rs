use std::{
    cmp::{Ordering, Reverse},
    collections::BinaryHeap,
};

/// Implements a min heap with a limited capacity of `k` elements.
pub struct TopKHeap {
    heap: BinaryHeap<Reverse<OrderedF32>>,
    k: usize,
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
/// A wrapper around f32 to implement Ord and Eq traits
/// Panics if we add NaN or Inf values
struct OrderedF32(f32);

impl Eq for OrderedF32 {}

impl Ord for OrderedF32 {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.partial_cmp(&other.0).unwrap()
    }
}

impl TopKHeap {
    #[inline]
    pub fn top(&self) -> Option<f32> {
        self.heap.peek().map(|x| x.0 .0)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.heap.len()
    }

    #[inline]
    pub fn can_enter(&self, v: f32) -> bool {
        self.heap.len() < self.k || self.top().unwrap() < v
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    #[inline]
    pub fn new(k: usize) -> Self {
        TopKHeap {
            heap: BinaryHeap::with_capacity(k),
            k,
        }
    }

    pub fn clear(&mut self) {
        self.heap.clear();
    }

    #[inline]
    pub fn push(&mut self, score: f32) {
        if self.heap.len() < self.k {
            // fits in heap
            self.heap.push(Reverse(OrderedF32(score)));
        } else if self.heap.peek().unwrap().0 < OrderedF32(score) {
            //better score
            self.heap.pop();
            self.heap.push(Reverse(OrderedF32(score)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topk_heap() {
        let mut heap = TopKHeap::new(3);
        heap.push(1.0);
        heap.push(2.0);
        assert_eq!(heap.len(), 2);
        heap.push(3.0);
        heap.push(4.0);
        assert_eq!(heap.top(), Some(2.0));
        assert_eq!(heap.len(), 3);

        println!("{:?}", heap.heap);
        heap.push(5.0);
        assert!(heap.can_enter(5.0));
        assert_eq!(heap.top(), Some(3.0));

        assert!(!heap.can_enter(0.5));
        heap.push(0.5);
        println!("{:?}", heap.heap);

        heap.push(100.2);
        println!("{:?}", heap.heap);

        heap.push(4.1);
        println!("{:?}", heap.heap);

        heap.clear();
        assert_eq!(heap.len(), 0);
    }

    #[test]
    fn t() {
        let mut heap = TopKHeap::new(5);
        for i in 0..10 {
            heap.push(i as f32 / 5 as f32);
        }

        println!("{:?}", heap.heap);
    }
}

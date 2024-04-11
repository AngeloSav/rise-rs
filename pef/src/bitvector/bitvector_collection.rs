//! Implements a immutable indexed collection of bitvectors. The bitvectors are stored in a
//! immutable bitvector and the endpoint (bitwise!) of each bit vector is stored.
//! It it possible to get the [`BitSlice`] of the i-th indexed bitvector.

use crate::bitvector::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct BitVecCollection {
    bv: BitVec,
    endpoints: Vec<usize>,
    n_vecs: usize,
}

impl Default for BitVecCollection {
    fn default() -> Self {
        Self::with_capacity(0, 0)
    }
}

impl BitVecCollection {
    pub fn with_capacity(n_bits: usize, n_vecs: usize) -> Self {
        let mut endpoints = Vec::<usize>::with_capacity(n_vecs + 1);
        endpoints.push(0); // First zero is always there

        Self {
            bv: BitVec::with_capacity(n_bits),
            endpoints,
            n_vecs: 0,
        }
    }

    pub fn push<W: AsRef<[u64]>>(&mut self, bv: impl AsRef<BitVector<W>>) {
        self.bv.concat(bv);
        self.endpoints.push(self.bv.len());
        self.n_vecs += 1;
    }

    pub fn get(&self, i: usize) -> BitSliceWithOffset {
        assert!(i < self.n_vecs, "Index out of bounds");

        let start_bit = self.endpoints[i];
        let end_bit = self.endpoints[i + 1];
        let n_bits = end_bit - start_bit;

        let start_word = start_bit / 64;
        let end_word = (end_bit + 63) / 64;
        let offset = start_bit % 64;

        dbg!(start_word, end_word, offset, n_bits, start_bit, end_bit);

        unsafe {
            BitSliceWithOffset::from_raw_parts(&self.bv.data[start_word..end_word], n_bits, offset)
        }
    }

    pub fn len(&self) -> usize {
        self.n_vecs
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// The bit vector may start at an offset (in bits) in the first word (i.e., the first word may contain some bits that are not part of the bit vector). This is useful for the implementation of the [`BitVecCollection`] where we concatenate several binary vectors and we want to avoid padding.
#[derive(Default, Clone, Eq, PartialEq)]
pub struct BitSliceWithOffset<'a> {
    data: &'a [u64],
    n_bits: usize,
    offset: usize,
}

impl<'a> BitSliceWithOffset<'a> {
    pub unsafe fn from_raw_parts(data: &'a [u64], n_bits: usize, offset: usize) -> Self {
        Self {
            data,
            n_bits,
            offset,
        }
    }

    /// Accesses `len` bits, with 1 <= `len` <= 64, starting at position `index`.
    ///
    /// Returns [`None`] if `index`+`len` is out of bounds,
    /// if `len` is 0, or if `len` is greater than 64.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::BitVec;
    /// use pef::bitvector::bitvector_collection::BitSliceWithOffset;
    ///
    /// let v = vec![0b000001010, 0b01010111000000, u64::MAX];
    ///
    /// // Bitslice with offset that excludes the first 64 + 5 bits
    /// let offset = 5;
    /// let bswo = unsafe{ BitSliceWithOffset::from_raw_parts(&v[1..], 54+64, offset)};
    ///
    /// assert_eq!(bswo.get_bits(0, 4), Some(0b1110));
    ///
    /// ```
    #[must_use]
    #[inline]
    pub fn get_bits(&self, index: usize, len: usize) -> Option<u64> {
        if (len == 0) | (len > 64) | (index + len >= self.n_bits) {
            return None;
        }
        // SAFETY: safe access due to the above checks
        Some(unsafe { self.get_bits_unchecked(index, len) })
    }

    /// Accesses `len` bits, starting at position `index`, without performing bounds checking.
    ///
    /// # Safety
    ///
    /// This method is unsafe because it does not perform bounds checking.
    /// It is the caller's responsibility to ensure that the provided `index` and `len`
    /// are within the bounds of the bit vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::BitVec;
    /// use pef::bitvector::bitvector_collection::BitSliceWithOffset;
    ///
    /// let v = vec![0b000001010, 0b01010111000000, u64::MAX];
    ///
    /// // Bitslice with offset that excludes the first 64 + 5 bits
    /// let offset = 5;
    /// let bswo = unsafe{ BitSliceWithOffset::from_raw_parts(&v[1..], 54+64, offset)};
    ///
    /// // This is unsafe because it does not perform bounds checking
    /// unsafe {
    ///     assert_eq!(bswo.get_bits_unchecked(0, 4), 0b1110);
    /// }
    /// ```
    #[must_use]
    #[inline]
    pub unsafe fn get_bits_unchecked(&self, index: usize, len: usize) -> u64 {
        debug_assert!(index + len < self.n_bits, "Index out of bounds");
        BitVector::<&[u64]>::get_bits_slice(self.data.as_ref(), index + self.offset, len)
    }

    /// Returns a non-consuming iterator over positions of bits set to 1 in the bit vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::BitVec;
    ///
    /// let vv: Vec<usize> = vec![0, 63, 128, 129, 254, 1026];
    /// let bv: BitVec = vv.iter().copied().collect();
    ///
    /// let v: Vec<usize> = bv.ones().collect();
    /// assert_eq!(v, vv);
    /// ```
    #[must_use]
    pub fn ones(&self) -> BitVectorBitPositionsIter<true> {
        BitVectorBitPositionsIter::with_pos(
            self.data.as_ref(),
            self.n_bits + self.offset,
            self.offset,
        )
    }

    /// Returns a non-consuming iterator over positions of bits set to 1 in the bit vector, starting at a specified bit position.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::BitVec;
    ///
    /// let vv: Vec<usize> = vec![0, 63, 128, 129, 254, 1026];
    /// let bv: BitVec = vv.iter().copied().collect();
    ///
    /// let v: Vec<usize> = bv.ones_with_pos(2).collect();
    /// assert_eq!(v, vec![63, 128, 129, 254, 1026]);
    /// ```
    #[must_use]
    pub fn ones_with_pos(&self, pos: usize) -> BitVectorBitPositionsIter<true> {
        BitVectorBitPositionsIter::with_pos(
            self.data.as_ref(),
            self.n_bits + self.offset,
            self.offset + pos,
        )
    }

    /// Returns a non-consuming iterator over positions of bits set to 0 in the bit vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::BitVec;
    /// use pef::gen_sequence::negate_vector;
    ///
    /// let vv: Vec<usize> = vec![0, 63, 128, 129, 254, 1026];
    /// let bv: BitVec = vv.iter().copied().collect();
    ///
    /// let v: Vec<usize> = bv.zeros().collect();
    /// assert_eq!(v, negate_vector(&vv));
    /// ```
    #[must_use]
    pub fn zeros(&self) -> BitVectorBitPositionsIter<false> {
        BitVectorBitPositionsIter::with_pos(self.data.as_ref(), self.n_bits, self.offset)
    }

    /// Returns a non-consuming iterator over positions of bits set to 0 in the bit vector, starting at a specified bit position.
    #[must_use]
    pub fn zeros_with_pos(&self, pos: usize) -> BitVectorBitPositionsIter<false> {
        BitVectorBitPositionsIter::with_pos(self.data.as_ref(), self.n_bits, self.offset + pos)
    }

    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.n_bits
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl AccessBin for BitSliceWithOffset<'_> {
    #[inline]
    #[must_use]
    fn get(&self, index: usize) -> Option<bool> {
        dbg!(index, self.n_bits, self.offset);

        if index >= self.n_bits {
            return None;
        }
        Some(unsafe { self.get_unchecked(index) })
    }

    unsafe fn get_unchecked(&self, index: usize) -> bool {
        debug_assert!(index < self.n_bits, "Index out of bounds");
        BitVector::<&[u64]>::get_bit_slice(self.data, index + self.offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    //use crate::gen_sequence::gen_strictly_increasing_sequence;

    #[test]
    fn test_bitvec_collection() {
        let mut bvc = BitVecCollection::default();
        assert!(bvc.is_empty());

        let vv1: Vec<usize> = vec![0, 63, 128, 129, 254, 1026];
        let bv: BitVec = vv1.iter().copied().collect();
        bvc.push(&bv);

        assert_eq!(bvc.len(), 1);
        assert!(!bvc.is_empty());

        let bv = BitVec::default();
        bvc.push(&bv);
        assert_eq!(bvc.len(), 2);

        let vv2: Vec<usize> = vec![0, 61, 127, 130, 242, 365];
        let bv: BitVec = vv2.iter().copied().collect();
        bvc.push(&bv);
        assert_eq!(bvc.len(), 3);

        let bswo = bvc.get(0);
        assert_eq!(bswo.len(), 1027);
        assert_eq!(bswo.get(0), Some(true));
        assert_eq!(bswo.get(63), Some(true));
        assert_eq!(bswo.get(64), Some(false));
        assert_eq!(bswo.get(1026), Some(true));

        assert_eq!(bswo.ones().collect::<Vec<usize>>(), vv1);

        let bswo = bvc.get(1);
        assert_eq!(bswo.len(), 0);
        assert_eq!(bswo.get(0), None);
        assert_eq!(bswo.ones().collect::<Vec<usize>>(), vec![]);

        let bswo = bvc.get(2);
        assert_eq!(bswo.len(), 366);

        assert_eq!(bswo.ones().collect::<Vec<usize>>(), vv2);
    }
}

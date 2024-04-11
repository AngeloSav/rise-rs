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

        let start = self.endpoints[i];
        let end = self.endpoints[i + 1];

        let n_bits = end - start;
        unsafe { BitSliceWithOffset::from_raw_parts(&self.bv.data[start..end], n_bits, start % 64) }
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

    pub fn len(&self) -> usize {
        self.n_bits
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl AccessBin for BitSliceWithOffset<'_> {
    #[inline]
    #[must_use]
    fn get(&self, index: usize) -> Option<bool> {
        if index < self.n_bits {
            return None;
        }
        Some(unsafe { self.get_unchecked(index) })
    }

    unsafe fn get_unchecked(&self, index: usize) -> bool {
        debug_assert!(index < self.n_bits, "Index out of bounds");
        BitVector::<&[u64]>::get_bit_slice(self.data, index + self.offset)
    }
}

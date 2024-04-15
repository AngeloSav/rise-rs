//! This module provides implementations for both mutable and immutable bit vectors.
//!
//! The mutable bit vector offers operations to [`AccessBin`], append, and modify bits at arbitrary positions.
//!
//! The immutable bit vector allows access to bits and can be extended to support [`RankBin`] and [`SelectBin`] queries.
//!
//! For both data structures, it is possible to iterate over bits or positions of bits set either to zero or one.
//!

pub mod bitvector_collection;

// TODO:
// - add CacheLine-based bit vectors
// - create a BitBoxed with fixed size (with_zeros() or with_ones())
// - add a function to get a BitSlice from a starting word of a given bitlength

use crate::AccessBin;

use bincode::de;
use serde::{Deserialize, Serialize};

/// A resizable, growable, and mutable bit vector.
pub type BitVec = BitVector<Vec<u64>>;
/// Bit operations on a slice of u64, immutable or mutable but not growable bit vector.
pub type BitSlice<'a> = BitVector<&'a [u64]>;
/// Bit operations on a boxed slice of u64, immutable or mutable but not growable bit vector.
pub type BitBoxed = BitVector<Box<[u64]>>;

/// Implementation of an immutable bit vector.
#[derive(Default, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct BitVector<V: AsRef<[u64]>> {
    data: V,
    n_bits: usize,
}

impl<V: AsRef<[u64]>> BitVector<V> {
    /// Creates a `BitVector` from raw parts.
    ///
    /// # Safety
    ///
    /// This method is unsafe because it does not perform bounds checking.
    /// It is the caller's responsibility to ensure that the provided `data` and `n_bits`
    /// are valid and consistent.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::{BitSlice, BitVec};
    ///
    /// let data = vec![0, 2, 3, 4, 5];
    /// let n_bits = data.len() * 64;
    /// let bv = unsafe { BitVec::from_raw_parts(data, n_bits) };
    ///
    /// assert_eq!(bv.get_bits(64, 64), Some(2));
    ///
    /// let data = vec![0, 2, 3, 4, 5];
    /// let n_bits = data.len() * 64;
    /// let bs = unsafe { BitSlice::from_raw_parts(&data[1..], n_bits-64) };
    ///
    /// assert_eq!(bs.get_bits(0, 64), Some(2));
    ///
    /// ```
    pub unsafe fn from_raw_parts(data: V, n_bits: usize) -> Self {
        Self { data, n_bits }
    }

    /// Accesses `len` bits, with 0 <= `len` <= 64, starting at position `index`.
    ///
    /// Returns [`None`] if `index`+`len` is out of bounds or if `len` is greater than 64.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::{BitVec, BitSlice, AccessBin};
    ///
    /// let v = vec![0,2,3,4,5];
    /// let bv: BitVec = v.into_iter().collect();
    /// assert_eq!(bv.get(1), Some(false));
    ///
    /// assert_eq!(bv.get_bits(1, 3), Some(0b110)); // Accesses bits from index 1 to 3
    ///
    /// // Accessing bits from index 1 to 8, which is out of bounds
    /// assert_eq!(bv.get_bits(1, 8), None);
    ///
    /// // Accessing more than 64 bits
    /// assert_eq!(bv.get_bits(0, 65), None);
    ///
    /// // Accessing 0 bits
    /// assert_eq!(bv.get_bits(2, 0), Some(0));
    ///
    /// // Accessing last bit
    /// assert_eq!(bv.get_bits(bv.len()-1, 1), Some(1));
    /// ```
    #[must_use]
    #[inline]
    pub fn get_bits(&self, index: usize, len: usize) -> Option<u64> {
        if (len > 64) | (index + len > self.n_bits) {
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
    /// use pef::{BitVec};
    ///
    /// let v = vec![0,2,3,4,5];
    /// let bv: BitVec = v.into_iter().collect();
    ///
    /// assert_eq!(unsafe{bv.get_bits_unchecked(0, 4)}, 0b1101);
    /// assert_eq!(unsafe{bv.get_bits_unchecked(0, 0)}, 0);
    /// ```
    #[must_use]
    #[inline]
    pub unsafe fn get_bits_unchecked(&self, index: usize, len: usize) -> u64 {
        Self::get_bits_slice(self.data.as_ref(), index, len)
    }

    // TODO: make the to functions a trait and implement for &[u64] together with set_bit and set_bits for &mut [T]. This way we can have a generic type T which implements those traits for &[T] and &mut [T].

    // Private function to decode bits at a given index on a slice.
    // The function does not check bounds while accessing data.
    #[inline]
    unsafe fn get_bits_slice(data: &[u64], index: usize, len: usize) -> u64 {
        let block = index >> 6;
        let shift = index & 63;

        let mask = if len == 64 {
            std::u64::MAX
        } else {
            (1_u64 << len) - 1
        };

        if shift + len <= 64 {
            return *data.get_unchecked(block) >> shift & mask;
        }

        (*data.get_unchecked(block) >> shift)
            | (*data.get_unchecked(block + 1) << (64 - shift) & mask)
    }

    // Private function to decode a bit at a given index on a slice. The function does not
    // check bounds.
    #[inline]
    #[must_use]
    unsafe fn get_bit_slice(data: &[u64], index: usize) -> bool {
        let word = index >> 6;
        let pos_in_word = index & 63;

        *data.get_unchecked(word) >> pos_in_word & 1 != 0
    }

    /// Gets a whole 64-bit word from the bit vector at index `i` in the underlying vector of u64.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::BitVec;
    ///
    /// let v = vec![0,2,3,4,5];
    /// let bv: BitVec = v.into_iter().collect();
    ///
    /// // Get the 64-bit word at index 0
    /// let word = bv.get_word(0);
    /// assert_eq!(word, 0b111101);
    /// ```
    #[must_use]
    #[inline]
    pub fn get_word(&self, i: usize) -> u64 {
        self.data.as_ref()[i]
    }

    #[must_use]
    #[inline]
    pub unsafe fn get_word_unchecked(&self, i: usize) -> u64 {
        *self.data.as_ref().get_unchecked(i)
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
        BitVectorBitPositionsIter::new(self.data.as_ref(), self.n_bits)
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
        BitVectorBitPositionsIter::with_pos(self.data.as_ref(), self.n_bits, pos)
    }

    /// Returns a non-consuming iterator over positions of bits set to 0 in the bit vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::BitVec;
    /// use pef::gen_sequences::negate_vector;
    ///
    /// let vv: Vec<usize> = vec![0, 63, 128, 129, 254, 1026];
    /// let bv: BitVec = vv.iter().copied().collect();
    ///
    /// let v: Vec<usize> = bv.zeros().collect();
    /// assert_eq!(v, negate_vector(&vv));
    /// ```
    #[must_use]
    pub fn zeros(&self) -> BitVectorBitPositionsIter<false> {
        BitVectorBitPositionsIter::new(self.data.as_ref(), self.n_bits)
    }

    /// Returns a non-consuming iterator over positions of bits set to 0 in the bit vector, starting at a specified bit position.
    #[must_use]
    pub fn zeros_with_pos(&self, pos: usize) -> BitVectorBitPositionsIter<false> {
        BitVectorBitPositionsIter::with_pos(self.data.as_ref(), self.n_bits, pos)
    }

    /// Returns a non-consuming iterator over bits of the bit vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::BitVec;
    ///
    /// let v = vec![0,2,3,5];
    /// let bv: BitVec = v.into_iter().collect();
    ///
    /// let mut iter = bv.iter();
    /// assert_eq!(iter.next(), Some(true)); // First bit is true
    /// assert_eq!(iter.next(), Some(false)); // Second bit is false
    /// assert_eq!(iter.next(), Some(true)); // Third bit is true
    /// assert_eq!(iter.next(), Some(true)); // Fourth bit is true
    /// assert_eq!(iter.next(), Some(false)); // Fifth bit is false
    /// assert_eq!(iter.next(), Some(true)); // Sixth bit is true
    /// assert_eq!(iter.next(), None); // End of the iterator
    /// ```
    pub fn iter(&self) -> BitVectorIter<V, &Self> {
        BitVectorIter {
            bv: self,
            i: 0,
            n_bits: self.n_bits,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Checks if the bit vector is empty.
    ///
    /// # Returns
    ///
    /// Returns `true` if the bit vector is empty, `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::BitVec;
    ///
    /// let v = vec![0,2,3,4,5];
    /// let bv: BitVec = v.into_iter().collect();
    ///
    /// assert!(!bv.is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.n_bits == 0
    }

    /// Returns the number of bits in the bit vector.
    ///
    /// # Returns
    ///
    /// The number of bits in the bit vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::BitVec;
    ///
    /// let v = vec![0,2,3,4,5];
    /// let bv: BitVec = v.into_iter().collect();
    ///
    /// assert_eq!(bv.len(), 6);
    /// ```
    pub fn len(&self) -> usize {
        self.n_bits
    }

    /// Counts the number of ones (bits set to 1) in the bit vector.
    /// This is an expensive operation, as it requires iterating over the entire bit vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::BitVec;
    ///
    /// let v = vec![0,2,3,4,5];
    /// let bv: BitVec = v.into_iter().collect();
    ///
    /// assert_eq!(bv.count_ones(), 5);
    /// ```
    pub fn count_ones(&self) -> usize {
        self.data
            .as_ref()
            .iter()
            .map(|word| word.count_ones() as usize)
            .sum()
    }

    /// Counts the number of zeros (bits set to 0) in the bit vector.
    /// This is an expensive operation, as it requires iterating over the entire bit vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::BitVec;
    ///
    /// let v = vec![0,2,3,4,5];
    /// let bv: BitVec = v.into_iter().collect();
    ///
    /// assert_eq!(bv.count_zeros(), 1);
    /// ```
    #[inline]
    #[must_use]
    pub fn count_zeros(&self) -> usize {
        self.len() - self.count_ones()
    }
}

impl<V: AsRef<[u64]>> AccessBin for BitVector<V> {
    /// Returns the bit at the given position `index`,
    /// or [`None`] if `index` is out of bounds.
    ///
    /// # Examples
    /// ```
    /// use pef::{BitVec, AccessBin};
    ///
    /// let v = vec![0,2,3,4,5];
    /// let bv: BitVec = v.into_iter().collect();
    ///
    /// assert_eq!(bv.get(5), Some(true));
    /// assert_eq!(bv.get(1), Some(false));
    /// assert_eq!(bv.get(10), None);
    /// ```
    #[must_use]
    #[inline]
    fn get(&self, index: usize) -> Option<bool> {
        if index >= self.len() {
            return None;
        }
        Some(unsafe { self.get_unchecked(index) })
    }

    /// Returns the bit at position `index`.
    ///
    /// # Safety
    /// Calling this method with an out-of-bounds index is undefined behavior.
    ///
    /// # Examples
    /// ```
    /// use pef::{BitVec, AccessBin};
    ///
    /// let v = vec![0,2,3,4,5];
    /// let bv: BitVec = v.into_iter().collect();
    ///
    /// assert_eq!(unsafe{bv.get_unchecked(5)}, true);
    /// ```
    #[must_use]
    #[inline]
    unsafe fn get_unchecked(&self, index: usize) -> bool {
        Self::get_bit_slice(self.data.as_ref(), index)
    }
}

impl<V: AsRef<[u64]> + AsMut<[u64]>> BitVector<V> {
    /// Sets the bit at the given position `index` to `bit`.
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::{BitVec, BitBoxed, AccessBin};
    ///
    /// let mut bv = BitVec::with_capacity(2);
    /// bv.push(true);
    /// bv.push(false);
    ///
    /// bv.set(1, true);
    /// assert_eq!(bv.get(1), Some(true));
    ///
    /// // This will panic because index is out of bounds
    /// // bv.set(10, false);
    ///
    /// let mut bb = BitBoxed::from(bv);
    /// bb.set(0, false);
    /// assert_eq!(bb.get(0), Some(false));
    ///
    /// ```
    #[inline]
    pub fn set(&mut self, index: usize, bit: bool) {
        assert!(index < self.n_bits);

        let word = index >> 6;
        let pos_in_word = index & 63;
        self.data.as_mut()[word] &= !(1_u64 << pos_in_word);
        self.data.as_mut()[word] |= (bit as u64) << pos_in_word;
    }

    /// Sets `len` bits, with 1 <= `len` <= 64,
    /// starting at position `index` to the `len` least
    /// significant bits in `bits`.
    ///
    /// # Panics
    ///
    /// Panics if `index`+`len` is out of bounds,
    /// `len` is greater than 64, or if the most significant bit in `bits`
    /// is at a position larger than or equal to `len`.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::{BitVec, BitBoxed};
    ///
    /// let mut bv = BitVec::with_zeros(5);
    /// bv.set_bits(0, 3, 0b101); // Sets bits 0 to 2 to 101
    /// assert_eq!(bv.get_bits(0, 3), Some(0b101));
    ///
    /// let mut bb = BitBoxed::from(bv);
    /// bb.set_bits(0, 3, 0b100); // Sets bits 0 to 2 to 100
    /// assert_eq!(bb.get_bits(0, 3), Some(0b100))
    /// ```
    #[inline]
    pub fn set_bits(&mut self, index: usize, len: usize, bits: u64) {
        assert!(index + len <= self.n_bits);
        // check there are no spurious bits
        assert!(len == 64 || (bits >> len) == 0);
        assert!(len <= 64);

        if len == 0 {
            return;
        }

        let mask = if len == 64 {
            std::u64::MAX
        } else {
            (1_u64 << len) - 1
        };
        let word = index >> 6;
        let pos_in_word = index & 63;

        self.data.as_mut()[word] &= !(mask << pos_in_word);
        self.data.as_mut()[word] |= bits << pos_in_word;

        let stored = 64 - pos_in_word;
        if stored < len {
            self.data.as_mut()[word + 1] &= !(mask >> stored);
            self.data.as_mut()[word + 1] |= bits >> stored;
        }
    }
}

impl BitVector<Vec<u64>> {
    /// Creates a new empty growable bit vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::BitVec;
    ///
    /// let bv = BitVec::new();
    /// assert_eq!(bv.len(), 0);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an empty bit vector with at least a capacity of `n_bits`.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::BitVec;
    ///
    /// let bv = BitVec::new();
    /// assert_eq!(bv.len(), 0);
    /// ```
    #[must_use]
    pub fn with_capacity(n_bits: usize) -> Self {
        let capacity = (n_bits + 63) / 64;
        Self {
            data: Vec::with_capacity(capacity),
            ..Self::default()
        }
    }

    /// Pushes a `bit` at the end of the bit vector.
    ///
    /// # Panics
    ///
    /// Panics if the size of the bit vector exceeds `usize::MAX` bits.
    ///
    /// # Example
    ///
    /// ```
    /// use pef::{BitVec, AccessBin};
    ///
    /// let mut bv = BitVec::new();
    /// bv.push(true);
    /// bv.push(false);
    /// bv.push(true);
    ///
    /// assert_eq!(bv.len(), 3);
    /// assert_eq!(bv.get(0), Some(true));
    /// assert_eq!(bv.count_ones(), 2);
    /// ```
    #[inline]
    pub fn push(&mut self, bit: bool) {
        let pos_in_word = self.n_bits % 64;
        if pos_in_word == 0 {
            self.data.push(0);
        }

        // push a 1
        if let Some(last) = self.data.last_mut() {
            *last |= (bit as u64) << pos_in_word;
        };

        self.n_bits += 1;
    }

    /// Appends `len` bits at the end of the bit vector by taking
    /// the least significant `len` bits in the u64 value `bits`.
    ///
    /// # Panics
    ///
    /// Panics if `len` is larger than 64 or if a bit of position
    /// larger than `len` is set in `bits`.
    ///
    /// Panics if the size of the bit vector exceeds `usize::MAX` bits.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::BitVec;
    ///
    /// let mut bv = BitVec::with_capacity(7);
    /// bv.append_bits(0b101, 3);  // appends 101
    /// bv.append_bits(0b0110, 4); // appends 0110  
    ///
    ///         
    /// assert_eq!(bv.len(), 7);
    /// assert_eq!(bv.get_bits(0, 3), Some(5));
    /// ```
    #[inline]
    pub fn append_bits(&mut self, bits: u64, len: usize) {
        assert!(len == 64 || (bits >> len) == 0);
        assert!(len <= 64);

        if len == 0 {
            return;
        }

        let pos_in_word: usize = self.n_bits & 63;
        self.n_bits += len;

        if pos_in_word == 0 {
            self.data.push(bits);
        } else if let Some(last) = self.data.last_mut() {
            *last |= bits << pos_in_word;
            if len > 64 - pos_in_word {
                self.data.push(bits >> (64 - pos_in_word));
            }
        }
    }

    /// Appends the bits of a given bit vector at the end of the current bit vector.
    pub fn concat<W: AsRef<[u64]>>(&mut self, rhs: impl AsRef<BitVector<W>>) {
        let rhs = rhs.as_ref();

        if rhs.is_empty() {
            return;
        }

        let shift = self.n_bits % 64;
        let n_bits = self.n_bits + rhs.n_bits;
        let n_words = (n_bits + 63) / 64;

        if shift == 0 {
            // word-aligned, easy case
            self.data.extend(rhs.data.as_ref().iter());
        } else {
            for w in rhs.data.as_ref().iter().take(self.data.len() - 1) {
                let cur_word = self.data.last_mut().unwrap();
                *cur_word |= w << shift;
                self.data.push(w >> (64 - shift));
            }
            let cur_word = self.data.last_mut().unwrap();
            *cur_word |= *rhs.data.as_ref().last().unwrap() << shift;
            if self.data.len() < n_words {
                self.data
                    .push(*rhs.data.as_ref().last().unwrap() >> (64 - shift));
            }
        }

        self.n_bits = n_bits;
    }

    /// Extends the bit vector by adding `n` bits set to 0.
    ///
    /// # Panics
    ///
    /// Panics if the size of the bit vector exceeds `usize::MAX` bits.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::{BitVec, AccessBin};
    ///
    /// let mut bv = BitVec::with_capacity(10);
    /// bv.extend_with_zeros(10);
    /// assert_eq!(bv.len(), 10);
    /// assert_eq!(bv.get(8), Some(false));
    /// ```
    pub fn extend_with_zeros(&mut self, n: usize) {
        let new_size = (self.n_bits + n + 63) / 64;
        self.data.resize_with(new_size, Default::default);
        self.n_bits += n;
    }

    /// Extends the bit vector by adding `n` bits set to 1.
    ///
    /// # Panics
    ///
    /// Panics if the size of the bit vector exceeds `usize::MAX` bits.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::{BitVec, AccessBin};
    ///
    /// let mut bv = BitVec::with_capacity(100);
    /// bv.extend_with_ones(100);
    /// assert_eq!(bv.len(), 100);
    /// assert_eq!(bv.get(8), Some(true));
    /// assert_eq!(bv.get(99), Some(true));
    /// ```
    pub fn extend_with_ones(&mut self, n: usize) {
        let new_size = (self.n_bits + n + 63) / 64;
        self.data.resize_with(new_size, || u64::MAX);

        let last = n % 64;
        if last > 0 {
            *self.data.last_mut().unwrap() = u64::MAX >> (64 - last);
        }
        self.n_bits += n;
    }

    /// Encode `v` with Elias Gamma encoding. We assume that `v` is a non-negative integer (i.e., `v` can be zero).
    /// The largest possible value for `v` is `u64::MAX - 1`.
    pub fn write_gamma(&mut self, v: u64) {
        let v = v + 1;

        let n_bits = (64 - v.leading_zeros()) as usize;

        if n_bits > 1 {
            self.extend_with_zeros(n_bits - 1);
        }
        self.append_bits(v, n_bits);
    }

    /// Shrinks the underlying vector of 64-bit words to fit the actual size of the bit vector.
    pub fn shrink_to_fit(&mut self) {
        self.data.shrink_to_fit();
    }
}

impl<V: AsRef<[u64]> + From<Vec<u64>>> BitVector<V> {
    /// Creates a bit vector with `n_bits` set to 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::BitBoxed;
    ///
    /// let bb = BitBoxed::with_zeros(5);
    /// assert_eq!(bb.len(), 5);
    /// assert_eq!(bb.count_ones(), 0);
    /// ```
    #[must_use]
    pub fn with_zeros(n_bits: usize) -> Self {
        let n_words = (n_bits + 63) / 64;
        let data = vec![0_u64; n_words];

        BitVector {
            data: data.into(),
            n_bits,
        }
    }

    /// Creates a bit vector with `n_bits` set to 1.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::BitBoxed;
    ///
    /// let bb = BitBoxed::with_ones(5);
    /// assert_eq!(bb.len(), 5);
    /// assert_eq!(bb.count_ones(), 5);
    ///
    /// let bb = BitBoxed::with_ones(123);
    /// assert_eq!(bb.len(), 123);
    /// assert_eq!(bb.count_ones(), 123);
    ///
    /// let bb = BitBoxed::with_ones(128);
    /// assert_eq!(bb.len(), 128);
    /// assert_eq!(bb.count_ones(), 128);
    /// ```
    #[must_use]
    pub fn with_ones(n_bits: usize) -> Self {
        let n_words = (n_bits + 63) / 64;
        let last_word = n_bits & 63;
        let mut data = vec![std::u64::MAX; n_words - 1];
        data.push(if last_word == 0 {
            std::u64::MAX
        } else {
            (1_u64 << last_word) - 1
        });

        BitVector {
            data: data.into(),
            n_bits,
        }
    }
}

impl Extend<bool> for BitVector<Vec<u64>> {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = bool>,
    {
        for bit in iter {
            self.push(bit);
        }
    }

    /* Nigthly
        fn extend_one(&mut self, item: bool) {
            self.push(item);
        }
        fn extend_reserve(&mut self, additional: usize) {
            self.data.reserve
        }
    */
}

/// Extends a `BitVector` with an iterator over `usize` values.
///
/// # Examples
///
/// ```
/// use pef::{BitVec, AccessBin};
///
/// let mut bv = BitVec::new();
///
/// // Extending the bit vector with a range of positions
/// bv.extend(0..5);
/// assert_eq!(bv.len(), 5);
/// assert_eq!(bv.get(3), Some(true));
/// ```
impl Extend<usize> for BitVector<Vec<u64>> {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = usize>,
    {
        for pos in iter {
            if pos >= self.n_bits {
                self.extend_with_zeros(pos + 1 - self.n_bits);
            }
            self.set(pos, true);
        }
    }
}

// impl SpaceUsage for BitVector {
//     /// Returns the space usage in bytes.
//     #[must_use]
//     fn space_usage_byte(&self) -> usize {
//         self.data.space_usage_byte() + 8
//     }
// }

/// Creates a `BitVector` from an iterator over `bool` values.
///
/// # Examples
///
/// ```
/// use pef::{AccessBin, BitVec};
///
/// // Create a bit vector from an iterator over bool values
/// let bv: BitVec = vec![true, false, true].into_iter().collect();
///
/// assert_eq!(bv.len(), 3);
/// assert_eq!(bv.get(1), Some(false));
/// ```
impl FromIterator<bool> for BitVector<Vec<u64>> {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = bool>,
    {
        let mut bv = BitVec::default();
        bv.extend(iter);

        bv
    }
}

impl FromIterator<bool> for BitVector<Box<[u64]>> {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = bool>,
    {
        BitVector::<Vec<u64>>::from_iter(iter).into()
    }
}

// it contains all the type of num_traits::int::PrimInt without bool
pub trait MyPrimInt: TryInto<usize> {}

macro_rules! impl_my_prim_int {
    ($($t:ty),*) => {
        $(impl MyPrimInt for $t {
        })*
    }
}

impl_my_prim_int![i8, u8, i16, u16, i32, u32, i64, u64, isize, usize, u128, i128];

/// Creates a `BitVector` from an iterator over non-negative integer values.
///
/// # Panics
/// Panics if any value of the sequence cannot be converted to usize.
///
/// # Examples
///
/// ```
/// use pef::{AccessBin, BitVec};
///
/// // Create a bit vector from an iterator over usize values
/// let bv: BitVec = vec![0, 1, 3, 5].into_iter().collect();
///
/// assert_eq!(bv.len(), 6);
/// assert_eq!(bv.get(3), Some(true));
/// ```
impl<V> FromIterator<V> for BitVector<Vec<u64>>
where
    V: MyPrimInt,
    <V as TryInto<usize>>::Error: std::fmt::Debug,
{
    #[must_use]
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = V>,
        <V as TryInto<usize>>::Error: std::fmt::Debug,
    {
        let mut bv = BitVector::<Vec<u64>>::default();
        bv.extend(
            iter.into_iter()
                .map(|x| x.try_into().expect("Cannot a value convert to usize")),
        );

        bv.into()
    }
}

impl<V> FromIterator<V> for BitVector<Box<[u64]>>
where
    V: MyPrimInt,
    <V as TryInto<usize>>::Error: std::fmt::Debug,
{
    #[must_use]
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = V>,
        <V as TryInto<usize>>::Error: std::fmt::Debug,
    {
        BitVector::<Vec<u64>>::from_iter(iter).into()
    }
}

/// Implements conversion from mutable `BitVector` to an immutable one.
///
/// This conversion consumes the original mutable `BitVector` and creates an
/// immutable version.
///
/// # Examples
///
/// ```
/// use pef::{BitVec,BitBoxed, AccessBin};
///
/// let mut bvm = BitVec::new();
/// bvm.push(true);
/// bvm.push(false);
///
/// // Convert mutable BitVector to immutable BitVector
/// let bv: BitBoxed = bvm.into();
///
/// assert_eq!(bv.get(0), Some(true));
/// ```
impl From<BitVector<Vec<u64>>> for BitVector<Box<[u64]>> {
    fn from(bvm: BitVector<Vec<u64>>) -> Self {
        Self {
            data: bvm.data.into_boxed_slice(),
            n_bits: bvm.n_bits,
        }
    }
}

/// Implements conversion from an immutable `BitVector` to a mutable one.
///
/// This conversion takes ownership of the original `BitVector` and creates a mutable version.
///
/// # Examples
///
/// ```
/// use pef::{BitVec, BitBoxed, AccessBin};
///
/// let v = vec![0,2,3,4,5];
/// let mut bv: BitBoxed = v.into_iter().collect();
///
/// let mut bvm: BitVec = bv.into();
///
/// assert_eq!(bvm.get(0), Some(true));
/// assert_eq!(bvm.len(), 6);
/// bvm.push(true);
/// assert_eq!(bvm.len(), 7);
/// ```
impl From<BitVector<Box<[u64]>>> for BitVector<Vec<u64>> {
    fn from(bv: BitVector<Box<[u64]>>) -> Self {
        Self {
            data: bv.data.into(),
            n_bits: bv.n_bits,
        }
    }
}

impl From<BitVector<&[u64]>> for BitVector<Vec<u64>> {
    fn from(bv: BitVector<&[u64]>) -> Self {
        Self {
            data: bv.data.into(),
            n_bits: bv.n_bits,
        }
    }
}

impl<V: AsRef<[u64]>> AsRef<BitVector<V>> for BitVector<V> {
    fn as_ref(&self) -> &BitVector<V> {
        self
    }
}

pub struct BitVectorGammaIter<'a> {
    inner_iter: BitVectorBitPositionsIter<'a, true>,
}

impl<'a> BitVectorGammaIter<'a> {
    /// Offset is needed by BitSliceWithOffset. It is the number of bits to skip in the first word before starting to read the bit vector.
    #[must_use]
    #[inline]
    pub fn new(data: &'a [u64], n_bits: usize, offset: usize) -> Self {
        assert!(
            offset < 64,
            "The offset must be within the first word (i.e., < 64)"
        );

        BitVectorGammaIter {
            inner_iter: BitVectorBitPositionsIter::with_pos_and_offset(data, n_bits, 0, offset),
        }
    }
}

// impl<'a> Iterator for BitVectorGammaIter<'a> {
//     type Item = u64;
//     fn next(&mut self) -> Option<Self::Item> {
//         let prev_pos = self.inner_iter.cur_position;
//         let pos = self.inner_iter.next()?;
//         let l = pos - prev_pos;
//         (1_u64 << l) | self.inner_iter.get_bits(l)?;
//     }
// }

#[derive(Debug)]
pub struct BitVectorBitPositionsIter<'a, const BIT: bool> {
    data: &'a [u64],
    n_bits: usize,         // Number of bits in the bit vector
    cur_position: usize,   // Current position in the bit vector
    cur_word_pos: usize,   // Position of the next word to read in data
    buffer: u128,          // Last word we read
    bits_in_buffer: usize, // How many bits can we read from cur_word?
    offset: usize, // Offset needed by BitSliceWithOffset. Rescale the position by this ammount
}

impl<'a, const BIT: bool> BitVectorBitPositionsIter<'a, BIT> {
    #[must_use]
    #[inline]
    pub fn new(data: &'a [u64], n_bits: usize) -> Self {
        Self::with_pos_and_offset(data, n_bits, 0, 0)
    }

    #[must_use]
    #[inline]
    pub fn with_pos(data: &'a [u64], n_bits: usize, pos: usize) -> Self {
        Self::with_pos_and_offset(data, n_bits, pos, 0)
    }
    // The function is unsafe because it assumes that the current word is in bounds.
    // The buffer may contain up to 128 bits, but we only read 64 bits at a time.
    // As we want to access up to 64 bits with get_bits(), we aim at having at least 64 bits in the buffer.

    #[inline]
    unsafe fn fill_buffer(&mut self) {
        self.buffer |= (if BIT {
            *self.data.get_unchecked(self.cur_word_pos)
        } else {
            // for zeros, just negate the word and report the positions of bit set to one!
            !*self.data.get_unchecked(self.cur_word_pos)
        } as u128)
            << self.bits_in_buffer;
        self.cur_word_pos += 1;
        self.bits_in_buffer += 64;
    }

    #[must_use]
    #[inline]
    pub fn with_pos_and_offset(data: &'a [u64], n_bits: usize, pos: usize, offset: usize) -> Self {
        let pos = pos + offset;
        let cur_word_pos = pos >> 6;
        let mut me = Self {
            data,
            n_bits,
            cur_position: pos,
            cur_word_pos: cur_word_pos,
            buffer: 0,
            bits_in_buffer: 0,
            offset: offset,
        };

        if cur_word_pos >= data.len() {
            return me;
        }

        // SAFETY: we are sure that cur_word_pos is in bounds
        unsafe {
            me.fill_buffer();
        }

        // We are sure there are 64 bits in the buffer
        let l = pos % 64; // How many bits to skip in the first word

        me.buffer >>= l; // remove bits to skip
        me.bits_in_buffer -= l;

        me
    }
}

impl<'a, const BIT: bool> BitVectorBitPositionsIter<'a, BIT> {
    /// If bits == 0, return 0
    pub fn get_bits(&mut self, bits: usize) -> Option<u64> {
        if bits > 64 || self.cur_position + bits > self.n_bits {
            return None;
        }

        // SAFETY: the check self.cur_position + bits <= self.n_bits guarntees
        // that cur_word_pos is in bounds while filling the buffer in unsafe get_bits_unchecked

        Some(unsafe { self.get_bits_unchecked(bits) })
    }

    pub unsafe fn get_bits_unchecked(&mut self, bits: usize) -> u64 {
        // UB if cur_word_pos is out of bounds
        if self.bits_in_buffer < bits {
            self.fill_buffer();
        }

        let res = (self.buffer & ((1 << bits) - 1)) as u64; // incorrect if bits > 64
        self.buffer >>= bits;
        self.bits_in_buffer -= bits;

        res
    }
}

/// Iterator over the positions of bits set to BIT (false for zeros,
/// true for ones) in the bit vector.
impl<'a, const BIT: bool> Iterator for BitVectorBitPositionsIter<'a, BIT> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur_position >= self.n_bits {
            return None;
        }

        dbg!(self.cur_position);

        while self.buffer == 0 {
            if self.cur_word_pos < self.data.len() {
                // SAFETY: check above guarantees that cur_word_pos is in bounds
                self.cur_position += self.bits_in_buffer;
                self.bits_in_buffer = 0;
                unsafe { self.fill_buffer() };
            } else {
                return None;
            }
        }

        // +1 is because we are looking for the position of the first bit set to 1
        let l = self.buffer.trailing_zeros() as usize + 1;
        self.cur_position += l;
        self.bits_in_buffer -= l;
        self.buffer >>= l;

        // self.cur_position points to the next bit to read
        if self.cur_position - 1 >= self.n_bits {
            // We may have read more bits than possible
            self.bits_in_buffer = 0;
            self.buffer = 0;
            None
        } else {
            Some(self.cur_position - self.offset - 1)
        }
    }
}

pub struct BitVectorIter<V: AsRef<[u64]>, T: AsRef<BitVector<V>>> {
    bv: T,
    n_bits: usize,
    i: usize,
    _phantom: std::marker::PhantomData<V>,
}

impl<V: AsRef<[u64]>, T: AsRef<BitVector<V>>> ExactSizeIterator for BitVectorIter<V, T> {
    fn len(&self) -> usize {
        self.bv.as_ref().n_bits - self.i
    }
}

impl<V: AsRef<[u64]>, T: AsRef<BitVector<V>>> Iterator for BitVectorIter<V, T> {
    type Item = bool;
    fn next(&mut self) -> Option<Self::Item> {
        if self.i < self.n_bits {
            self.i += 1;
            Some(unsafe { self.bv.as_ref().get_unchecked(self.i - 1) })
        } else {
            None
        }
    }
}

impl<V: AsRef<[u64]>> IntoIterator for BitVector<V> {
    type IntoIter = BitVectorIter<V, BitVector<V>>;
    type Item = bool;

    fn into_iter(self) -> Self::IntoIter {
        let n_bits = self.as_ref().n_bits;
        BitVectorIter {
            bv: self,
            i: 0,
            n_bits,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<'a, V: AsRef<[u64]>> IntoIterator for &'a BitVector<V> {
    type IntoIter = BitVectorIter<V, &'a BitVector<V>>;
    type Item = bool;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<V: AsRef<[u64]>> std::fmt::Debug for BitVector<V> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let data_str: Vec<String> = self
            .data
            .as_ref()
            .iter()
            .map(|x| format!("{:b}", x))
            .collect();
        write!(
            fmt,
            "BitVector {{ n_bits:{:?}, data:{:?}}}",
            self.n_bits, data_str
        )
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
    #[inline]
    pub unsafe fn from_raw_parts(data: &'a [u64], n_bits: usize, offset: usize) -> Self {
        Self {
            data,
            n_bits,
            offset,
        }
    }

    /// Accesses `len` bits, with 0 <= `len` <= 64, starting at position `index`.
    ///
    /// Returns [`None`] if `index`+`len` is out of bounds or if `len` is greater than 64.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::BitVec;
    /// use pef::BitSliceWithOffset;
    ///
    /// let v = vec![0b000001010, 0b01010111000000, u64::MAX];
    ///
    /// // Bitslice with offset that excludes the first 64 + 5 bits
    /// let offset = 5;
    /// let bswo = unsafe{ BitSliceWithOffset::from_raw_parts(&v[1..], 59+64, offset)};
    ///
    /// assert_eq!(bswo.len(), 59+64);
    /// assert_eq!(bswo.get_bits(0, 4), Some(0b1110));
    /// assert_eq!(bswo.get_bits(bswo.len()-2, 1), Some(1));
    /// assert_eq!(bswo.get_bits(bswo.len()-2, 0), Some(0));
    ///
    /// ```
    #[must_use]
    #[inline]
    pub fn get_bits(&self, index: usize, len: usize) -> Option<u64> {
        if (len > 64) | (index + len > self.n_bits) {
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
    /// use pef::BitSliceWithOffset;
    ///
    /// let v = vec![0b000001010, 0b01010111000000, u64::MAX];
    ///
    /// // Bitslice with offset that excludes the first 64 + 5 bits
    /// let offset = 5;
    /// let bswo = unsafe{ BitSliceWithOffset::from_raw_parts(&v[1..], 59+64, offset)};
    ///
    /// assert_eq!(unsafe{bswo.get_bits_unchecked(0, 4)}, 0b1110);
    /// assert_eq!(unsafe{bswo.get_bits_unchecked(0, 0)}, 0);
    ///
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
    /// use pef::BitSliceWithOffset;
    ///
    /// let v = vec![0b000001010, 0b01010111000000, u64::MAX];
    ///
    /// // Bitslice with offset that excludes the first 64 + 5 bits
    /// let offset = 5;
    /// let bswo = unsafe{ BitSliceWithOffset::from_raw_parts(&v[1..], 59+64, offset)};
    /// let mut v = vec![1, 2, 3, 5, 7];
    /// v.extend(59..(59+64));
    /// assert_eq!(bswo.ones().collect::<Vec<_>>(), v);
    /// ```
    #[must_use]
    pub fn ones(&self) -> BitVectorBitPositionsIter<true> {
        BitVectorBitPositionsIter::with_pos_and_offset(
            self.data.as_ref(),
            self.n_bits + self.offset,
            0,
            self.offset,
        )
    }

    /// Returns a non-consuming iterator over positions of bits set to 1 in the bit vector, starting at a specified bit position.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::BitVec;
    /// use pef::BitSliceWithOffset;
    ///
    /// let v = vec![0b000001010, 0b01010111000000, u64::MAX];
    ///
    /// // Bit slice with offset that excludes the first 64 + 5 bits
    /// let offset = 5;
    /// let bswo = unsafe{ BitSliceWithOffset::from_raw_parts(&v[1..], 59+64, offset)};
    /// let mut v = vec![5, 7];
    /// v.extend(59..(59+64));
    /// assert_eq!(bswo.ones_with_pos(5).collect::<Vec<_>>(), v);
    /// ```
    #[must_use]
    pub fn ones_with_pos(&self, pos: usize) -> BitVectorBitPositionsIter<true> {
        BitVectorBitPositionsIter::with_pos_and_offset(
            self.data.as_ref(),
            self.n_bits + self.offset,
            pos,
            self.offset,
        )
    }

    /// Returns a non-consuming iterator over positions of bits set to 0 in the bit vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use pef::BitVec;
    /// use pef::gen_sequences::negate_vector;
    ///
    /// let vv: Vec<usize> = vec![0, 63, 128, 129, 254, 1026];
    /// let bv: BitVec = vv.iter().copied().collect();
    ///
    /// let v: Vec<usize> = bv.zeros().collect();
    /// assert_eq!(v, negate_vector(&vv));
    /// ```
    #[must_use]
    pub fn zeros(&self) -> BitVectorBitPositionsIter<false> {
        BitVectorBitPositionsIter::with_pos_and_offset(
            self.data.as_ref(),
            self.n_bits + self.offset,
            0,
            self.offset,
        )
    }

    /// Returns a non-consuming iterator over positions of bits set to 0 in the bit vector, starting at a specified bit position.
    #[must_use]
    pub fn zeros_with_pos(&self, pos: usize) -> BitVectorBitPositionsIter<false> {
        BitVectorBitPositionsIter::with_pos_and_offset(
            self.data.as_ref(),
            self.n_bits + self.offset,
            pos,
            self.offset,
        )
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
mod tests;

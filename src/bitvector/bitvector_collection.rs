//! Implements mutable and immutable indexed collections of bitvectors.
//! The bitvectors are concatenated either in a [`BitVec`] or [`BitBoxed`] and
//! the endpoints (bitwise!) of the bit vectors are stored. This way, it is possible to
//! get the i-th indexed bitvector. However, as we save space by storing the bit vectors
//! without padding, we get a [`BitSliceWithOffset`]. This is a wrapper around a slice on
//! the bit vector and the offset of the first bit in the first word.
//!
//! The code is similar to the C++ implementation [here](https://github.com/ot/ds2i/blob/master/bitvector_collection.hpp).

// TODO: remake all doctests: NOW we use a BVBuilder!!!!

use crate::{bitvector::*, EliasFano, EnumeratorFromBitSlice, SequenceEnumerator, WriteBitvector};
use mem_dbg::{MemDbg, MemSize};
use serde::{Deserialize, Serialize};

pub type BitVecCollection = BitVectorCollection<Vec<u64>>;
pub type BitBoxedCollection = BitVectorCollection<Box<[u64]>>;

pub type BitVecCollectionBuilder = BitVectorCollectionBuilder<Vec<u64>>;
//pub type BitSliceCollection<'a> = BitVectorCollection<&'a [u64]>;

/// Represents a mutable or immutable indexed collection of bitvectors.
/// The bitvectors are concatenated either in a `BitVec` or `BitBoxed` and
/// the endpoints (bitwise!) of the bit vectors are stored. This way, it is possible to
/// get the i-th indexed bitvector. However, as we save space by storing the bit vectors
/// without padding, we get a `BitSliceWithOffset`. This is a wrapper around a slice on
/// the bit vector and the offset of the first bit in the first word.
///
/// # Examples
///
/// ```
/// use pef::{BitVecCollection, BitVecCollectionBuilder, BitVec, AccessBin};
///
/// let mut bvcb = BitVecCollectionBuilder::default();
///
/// let vv1: Vec<usize> = vec![0, 63, 128, 129, 254, 1026];
/// let bv: BitVec = vv1.iter().copied().collect();
/// bvcb.push(&bv);
///
/// let bvc = bvcb.clone().build();
/// assert_eq!(bvc.len(), 1);
/// assert!(!bvc.is_empty());
///
/// let bv = BitVec::default();
/// bvcb.push(&bv);
/// let bvc = bvcb.clone().build();
/// assert_eq!(bvc.len(), 2);
///
/// let vv2: Vec<usize> = vec![0, 61, 127, 130, 242, 365];
/// let bv: BitVec = vv2.iter().copied().collect();
/// bvcb.push(&bv);
/// let bvc = bvcb.clone().build();
/// assert_eq!(bvc.len(), 3);
///
/// let bswo = bvc.get(0);
/// assert_eq!(bswo.len(), 1027);
/// assert_eq!(bswo.get(0), Some(true));
/// assert_eq!(bswo.get(63), Some(true));
/// assert_eq!(bswo.get(64), Some(false));
/// assert_eq!(bswo.get(1026), Some(true));
///
/// assert_eq!(bswo.ones().collect::<Vec<usize>>(), vv1);
///
/// let bswo = bvc.get(1);
/// assert_eq!(bswo.len(), 0);
/// assert_eq!(bswo.get(0), None);
/// assert_eq!(bswo.ones().collect::<Vec<usize>>(), vec![]);
///
/// let bswo = bvc.get(2);
/// assert_eq!(bswo.len(), 366);
///
/// assert_eq!(bswo.ones().collect::<Vec<usize>>(), vv2);
/// ```
#[derive(Default, Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct BitVectorCollectionBuilder<V: AsRef<[u64]>> {
    pub(crate) bv: BitVector<V>,
    endpoints: Vec<usize>,
    n_vecs: usize,
}

impl BitVectorCollectionBuilder<Vec<u64>> {
    #[must_use]
    pub fn with_capacity(n_bits: usize, n_vecs: usize) -> Self {
        Self {
            bv: BitVec::with_capacity(n_bits),
            endpoints: Vec::<usize>::with_capacity(n_vecs + 1),
            n_vecs: 0,
        }
    }

    /// Appends a bitvector to the collection.
    ///
    /// # Arguments
    ///
    /// * `bv` - The bitvector to append.
    pub fn push<W: AsRef<[u64]>>(&mut self, bv: impl AsRef<BitVector<W>>) {
        if self.endpoints.is_empty() {
            // First zero is always there
            // We use this check here to avoid allocation while creating an empty collection.
            self.endpoints.push(0);
        }

        self.bv.concat(bv);
        self.endpoints.push(self.bv.len());
        self.n_vecs += 1;
    }

    pub fn build(self) -> BitVecCollection {
        let u = self.bv.len() as u64 + 1;
        let n = self.endpoints.len();
        let v = self
            .endpoints
            .into_iter()
            .map(|x| x as u64)
            .collect::<Vec<_>>();
        BitVecCollection {
            bv: self.bv,
            endpoints: EliasFano::write_bitvector(&v, n, u),
            n_vecs: self.n_vecs,
        }
    }
}

// impl BitVectorCollection<Vec<u64>> {
//     /// Creates a new `BitVectorCollection` with the specified capacity.
//     ///
//     /// # Arguments
//     ///
//     /// * `n_bits` - The initial capacity in bits.
//     /// * `n_vecs` - The initial capacity for the number of bitvectors.
//     ///
//     /// # Returns
//     ///
//     /// A new `BitVectorCollection` with the specified capacity.
//     #[must_use]
//     pub fn with_capacity(n_bits: usize, n_vecs: usize) -> Self {
//         Self {
//             bv: BitVec::with_capacity(n_bits),
//             endpoints: Vec::<usize>::with_capacity(n_vecs + 1),
//             n_vecs: 0,
//         }
//     }

//     /// Appends a bitvector to the collection.
//     ///
//     /// # Arguments
//     ///
//     /// * `bv` - The bitvector to append.
//     pub fn push<W: AsRef<[u64]>>(&mut self, bv: impl AsRef<BitVector<W>>) {
//         if self.endpoints.is_empty() {
//             // First zero is always there
//             // We use this check here to avoid allocation while creating an empty collection.
//             self.endpoints.push(0);
//         }

//         self.bv.concat(bv);
//         self.endpoints.push(self.bv.len());
//         self.n_vecs += 1;
//     }
// }

impl From<BitBoxedCollection> for BitVecCollection {
    fn from(bbc: BitBoxedCollection) -> Self {
        let BitVectorCollection {
            bv,
            endpoints,
            n_vecs,
        } = bbc;
        Self {
            bv: bv.into(),
            endpoints: endpoints.into(),
            n_vecs,
        }
    }
}

impl From<BitVecCollection> for BitBoxedCollection {
    fn from(bvc: BitVecCollection) -> Self {
        let BitVectorCollection {
            bv,
            endpoints,
            n_vecs,
        } = bvc;
        Self {
            bv: bv.into(),
            endpoints: endpoints.into(), // TODO: Elias-Fano encoding
            n_vecs,
        }
    }
}

/// Immutable Bitvector collection, also the endpoints are compressed using elias fano
#[derive(Default, Clone, Debug, Serialize, Deserialize, Eq, PartialEq, MemSize, MemDbg)]
pub struct BitVectorCollection<V: AsRef<[u64]>> {
    pub(crate) bv: BitVector<V>,
    endpoints: BitVector<V>,
    n_vecs: usize,
}

impl<V: AsRef<[u64]>> BitVectorCollection<V> {
    /// Returns the i-th bitvector as a `BitSliceWithOffset`.
    ///
    /// # Panics
    ///
    /// Panics if the index is out of bounds.
    ///
    /// # Arguments
    ///
    /// * `i` - The index of the bitvector to retrieve.
    ///
    /// # Returns
    ///
    /// The i-th bitvector as a `BitSliceWithOffset`.
    #[must_use]
    #[inline]
    pub fn get(&self, i: usize) -> BitSliceWithOffset<'_> {
        assert!(i < self.n_vecs, "Index out of bounds");

        let mut ef_it = EliasFano::iter_from_slice(
            self.endpoints.as_bitslice(),
            self.n_vecs + 1,
            self.bv.len() as u64 + 1,
        );

        let start = ef_it.move_to_position(i).0 as usize;
        let end = ef_it.next().unwrap() as usize;

        self.bv.as_bitslice().slice(start, end)
    }

    /// Returns the number of bitvectors in the collection.
    ///
    /// # Returns
    ///
    /// The number of bitvectors in the collection.
    #[must_use]
    pub fn len(&self) -> usize {
        self.n_vecs
    }

    /// Returns the total number of bits in the collection.
    ///
    /// # Returns
    ///
    /// The total number of bits in the collection.
    #[must_use]
    pub fn n_bits(&self) -> usize {
        self.bv.len()
    }

    /// Returns `true` if the collection is empty, `false` otherwise.
    ///
    /// # Returns
    ///
    /// `true` if the collection is empty, `false` otherwise.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.n_vecs == 0
    }
}

impl<W: AsRef<[u64]>> SpaceUsage for BitVectorCollection<W> {
    fn space_usage_byte(&self) -> usize {
        // println!("size data: {}", self.bv.space_usage_byte());
        // println!("size endpoints: {}", self.endpoints.space_usage_byte());

        self.bv.space_usage_byte()
            + self.endpoints.space_usage_byte()
            + std::mem::size_of::<usize>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    //use crate::gen_sequence::gen_strictly_increasing_sequence;

    #[test]
    fn test_bitvec_collection() {
        let mut bvc = BitVectorCollectionBuilder::default();
        // assert!(bvc.is_empty());

        let vv1: Vec<usize> = vec![0, 63, 128, 129, 254, 1026];
        let bv: BitVec = vv1.iter().copied().collect();
        bvc.push(&bv);

        assert_eq!(bvc.clone().build().len(), 1);
        assert!(!bvc.clone().build().is_empty());

        let bv = BitVec::default();
        bvc.push(&bv);
        assert_eq!(bvc.clone().build().len(), 2);

        let vv2: Vec<usize> = vec![0, 61, 127, 130, 242, 365];
        let bv: BitVec = vv2.iter().copied().collect();
        bvc.push(&bv);

        println!("{:?}", bvc.endpoints);
        let bvc = bvc.build();
        assert_eq!(bvc.len(), 3);

        // let bswo = bvc.get(0);
        // assert_eq!(bswo.len(), 1027);
        // assert_eq!(bswo.get(0), Some(true));
        // assert_eq!(bswo.get(63), Some(true));
        // assert_eq!(bswo.get(64), Some(false));
        // assert_eq!(bswo.get(1026), Some(true));

        // assert_eq!(bswo.ones().collect::<Vec<usize>>(), vv1);

        // let bswo = bvc.get(1);
        // assert_eq!(bswo.len(), 0);
        // assert_eq!(bswo.get(0), None);
        // assert_eq!(bswo.ones().collect::<Vec<usize>>(), vec![]);

        let bswo = bvc.get(2);
        assert_eq!(bswo.len(), 366);

        assert_eq!(bswo.ones().collect::<Vec<usize>>(), vv2);
    }

    #[test]
    fn test_from() {
        let mut bvc = BitVectorCollectionBuilder::default();
        let vv1: Vec<usize> = vec![0, 63, 128, 129, 254, 1026];
        let bv: BitVec = vv1.iter().copied().collect();
        bvc.push(&bv);
        let bvc = bvc.build();

        let bbc: BitBoxedCollection = bvc.clone().into();
        let bvc2: BitVecCollection = bbc.into();
        assert_eq!(bvc, bvc2);
    }
}

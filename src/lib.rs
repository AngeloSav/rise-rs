#![feature(unchecked_shifts)]

pub mod bitvector;
pub use bitvector::bitvector_collection::{BitBoxedCollection, BitVecCollection};
pub use bitvector::BitVector;
pub use bitvector::{BitBoxed, BitSlice, BitSliceWithOffset, BitVec};

pub mod darray;
pub use darray::DArray;

pub mod elias_fano;

pub mod space_usage;

pub mod gen_sequences;
pub mod utils;

/// A trait for the support of `get` query over the binary alphabet.
pub trait AccessBin {
    /// Returns the bit at the given position `i`,
    /// or [`None`] if ```i``` is out of bounds.
    fn get(&self, i: usize) -> Option<bool>;

    /// Returns the bit at the given position `i`.
    ///
    /// # Safety
    /// Calling this method with an out-of-bounds index is undefined behavior.
    unsafe fn get_unchecked(&self, i: usize) -> bool;
}

/// A trait for the support of `select` query over the binary alphabet.
pub trait SelectBin {
    /// Returns the position of the `i+1`-th occurrence of a bit set to `1`.
    /// Returns `None` if there is no such position.
    fn select1(&self, i: usize) -> Option<usize>;

    /// Returns the position of the `i+1`-th occurrence of a bit set to `1`.
    ///
    /// # Safety
    /// This method doesn't check that such element exists
    /// Calling this method with an i >= maximum rank1 is undefined behaviour.
    unsafe fn select1_unchecked(&self, i: usize) -> usize;

    /// Returns the position of the `i+1`-th occurrence of a bit set to `0`.
    /// Returns `None` if there is no such position.
    fn select0(&self, i: usize) -> Option<usize>;

    /// Returns the position of the `i+1`-th occurrence of a bit set to  `0`.
    ///
    /// # Safety
    /// This method doesnt check that such element exists
    /// Calling this method with an `i >= maximum rank0` is undefined behaviour.
    unsafe fn select0_unchecked(&self, i: usize) -> usize;
}

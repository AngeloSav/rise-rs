pub mod bitvector;
pub use bitvector::bitvector_collection::BitVecCollection;
pub use bitvector::{BitBoxed, BitSlice, BitVec};

pub mod gen_sequence;

/// A trait for the support of `get` query over the binary alphabet.
pub trait AccessBin {
    /// Returns the bit at the given position `i`,
    /// or [`None`] if ```i``` is out of bounds.
    fn get(&self, i: usize) -> Option<bool>;

    /// Returns the symbol at the given position `i`.
    ///
    /// # Safety
    /// Calling this method with an out-of-bounds index is undefined behavior.
    unsafe fn get_unchecked(&self, i: usize) -> bool;
}

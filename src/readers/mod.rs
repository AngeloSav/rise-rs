//! Readers for binary collection files in the DS2I format.
//!
//! A binary collection consists of two parallel files:
//! - `.docs` — concatenated posting lists of document IDs (each list preceded
//!   by its length).
//! - `.freqs` — the corresponding term frequencies, in the same order.
//!
//! [`BinaryCollectionIterator`] reads both files and yields one posting list
//! at a time, making it the primary entry point for index construction.

pub mod ds2i_reader;

pub use ds2i_reader::*;

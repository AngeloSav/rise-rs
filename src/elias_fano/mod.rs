pub mod all_ones_seq;
pub mod complement_ef;
pub mod elias_fano;
pub mod indexed_seq;
pub mod indexed_seq_complement;
pub mod opt_partition;
pub mod ranked_bv;
pub mod strict_ef;
pub mod uniform_partitioned_seq;

pub use elias_fano::{EliasFano, EliasFanoIter};

#[cfg(test)]
mod tests;

// Configuration constants for constructing the index and metadata.
pub const LENGTH_THRESHOLD: usize = 0;
pub const MDATA_LENGTH_THRESHOLD: usize = LENGTH_THRESHOLD;

// Parameters for block partitioning and scoring for the metadata.
pub const MDATA_EPS1: f32 = 0.01;
pub const MDATA_EPS2: f32 = 0.4;
pub const MDATA_FIXED_COST: f32 = 12.0;
pub const MDATA_BLOCK_SIZE: usize = 128;

// Parameters for RankedBv iterator
pub const RBV_LOG_RANK_SAMPLING: usize = 9; // length of buckets
pub const RBV_LOG_SAMPLING1: usize = 8;
pub const RBV_LINEAR_SCAN_THRESHOLD: usize = 8;

pub const EF_LOG_SAMPLING0: usize = 9;
pub const EF_LOG_SAMPLING1: usize = 8;
pub const EF_LINEAR_SCAN_THRESHOLD: usize = 8;

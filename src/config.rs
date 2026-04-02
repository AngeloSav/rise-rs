//! Global tuning constants for index construction and query processing.
//!
//! These values control the trade-off between space and query speed.  In
//! general, larger sampling rates reduce space but increase the cost of random
//! access; smaller values do the opposite.  Change them and rebuild the index
//! to experiment.

// Configuration constants for constructing the index and metadata.
pub const LENGTH_THRESHOLD: usize = 0;
pub const MDATA_LENGTH_THRESHOLD: usize = LENGTH_THRESHOLD;

// Parameters for block partitioning and scoring for the metadata.
pub const MDATA_EPS1: f32 = 0.01;
pub const MDATA_EPS2: f32 = 0.4;
pub const MDATA_FIXED_COST: f32 = 12.0;
pub const MDATA_BLOCK_SIZE: usize = 128;

// Parameters for the OptEFIdx index.
pub const OPT_EPS_1: f64 = 0.0;
pub const OPT_EPS_2: f64 = 0.3;

// Parameters for RankedBv iterator
pub const RBV_LOG_RANK_SAMPLING: usize = 9; // length of buckets
pub const RBV_LOG_SAMPLING1: usize = 8;
pub const RBV_LINEAR_SCAN_THRESHOLD: usize = 8;

// Parameters for Elias-Fano and Complement Elias-Fano
pub const EF_LOG_SAMPLING0: usize = 9;
pub const EF_LOG_SAMPLING1: usize = 8;
pub const EF_LINEAR_SCAN_THRESHOLD: usize = 8;

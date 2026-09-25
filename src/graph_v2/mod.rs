//! Additive, reproducible vocabulary graph pipeline. Legacy runners are untouched.
pub mod calibration;
pub mod embeddings;
pub mod experiment;
pub mod graph;
pub mod leiden;
pub mod metrics;
pub mod selection;

pub const VERSION: &str = "hsk-graph-v2/1";

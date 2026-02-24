//! Benchmark module
//! Precision latency measurements for all hot paths

pub mod latency;

pub use latency::{
    run_all_benchmarks,
    run_bench,
    BenchResult,
};

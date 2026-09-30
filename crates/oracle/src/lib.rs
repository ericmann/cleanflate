//! Dev-only test oracle: reference C zlib bindings, corpus generation, and
//! benchmarking support for `cleanflate`. Never used by `cleanflate` itself.

pub mod bench;
pub mod bench_compare;
pub mod bench_env;
pub mod bench_json;
pub mod conformance;
pub mod ffi;
pub mod generate;
pub mod inputs;
pub mod malformed;
pub mod manifest;
pub mod matrix;
pub mod perf_prep;
pub mod prng;
pub mod regress;

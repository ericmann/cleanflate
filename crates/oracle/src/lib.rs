//! Dev-only test oracle: reference C zlib bindings, corpus generation, and
//! benchmarking support for `cleanflate`. Never used by `cleanflate` itself.

pub mod ffi;
pub mod inputs;
pub mod malformed;
pub mod matrix;
pub mod prng;

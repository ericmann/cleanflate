#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod bitreader;
#[allow(dead_code)] // write_code is reused by later Huffman/inflate tests
pub(crate) mod bitwriter;
mod checksum;
mod dynbuilder;
mod error;
mod huffman;
mod huffman_table;
mod huffman_validation;
mod inflate_dynamic;
mod inflate_fixed;
mod inflate_stored;
mod invariants;
mod stream_raw;
mod tables;
mod window;

#[test]
fn crate_builds() {
    assert_eq!(1 + 1, 2);
}

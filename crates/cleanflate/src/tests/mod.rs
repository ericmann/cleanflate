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
mod error;
mod huffman;
mod tables;
mod window;

#[test]
fn crate_builds() {
    assert_eq!(1 + 1, 2);
}

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
mod decode_path;
mod dynbuilder;
mod error;
mod fastpath;
mod gzip;
mod gzip_header;
mod huffman;
mod huffman_table;
mod huffman_validation;
mod inflate_dynamic;
mod inflate_fixed;
mod inflate_stored;
mod invariants;
mod stream_contract;
mod stream_raw;
mod tables;
mod window;
mod zlib;

#[test]
fn crate_builds() {
    assert_eq!(1 + 1, 2);
}

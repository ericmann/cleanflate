#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod bitreader;
mod checksum;
mod error;
mod tables;
mod window;

#[test]
fn crate_builds() {
    assert_eq!(1 + 1, 2);
}

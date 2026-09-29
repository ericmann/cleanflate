#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod checksum;
mod error;
mod tables;

#[test]
fn crate_builds() {
    assert_eq!(1 + 1, 2);
}

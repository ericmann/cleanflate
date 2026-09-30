use crate::config::{FAST_LOOP_MIN_INPUT, FAST_LOOP_MIN_OUTPUT};
use crate::tables::MAX_MATCH;

#[test]
fn fast_loop_margins_valid() {
    // A word refill needs at least 8 input bytes.
    assert!(std::hint::black_box(FAST_LOOP_MIN_INPUT) >= 8);
    // A whole match must fit without stopping halfway.
    assert!(std::hint::black_box(FAST_LOOP_MIN_OUTPUT) >= MAX_MATCH);
    assert!(std::hint::black_box(FAST_LOOP_MIN_OUTPUT) >= 258);
}

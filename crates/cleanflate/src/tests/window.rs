use crate::decode_path::DecodePath;
use crate::window::Window;

fn window_with(data: &[u8]) -> Window {
    let mut w = Window::new();
    w.append(data);
    w
}

#[test]
fn push_and_get_back() {
    let w = window_with(&[1, 2, 3]);
    assert_eq!(w.get_back(1), Some(3));
    assert_eq!(w.get_back(2), Some(2));
    assert_eq!(w.get_back(3), Some(1));
    assert_eq!(w.get_back(4), None);
}

#[test]
fn wraps_after_32768() {
    let data: Vec<u8> = (0..32768u32).map(|i| (i % 256) as u8).collect();
    let mut w = window_with(&data);
    assert_eq!(w.available(), 32768);
    w.append(&[0xAB]);
    assert_eq!(w.available(), 32768);
    assert_eq!(w.get_back(1), Some(0xAB));
    // Exactly WINDOW_SIZE back is still the oldest byte still resident
    // (the second byte ever appended, value 1, since the first byte is
    // the one just overwritten by 0xAB).
    assert_eq!(w.get_back(32768), Some(1));
    // One more than the window size is never available.
    assert_eq!(w.get_back(32769), None);
}

#[test]
fn distance_beyond_available_is_none() {
    let w = window_with(&[1]);
    assert_eq!(w.get_back(0), None);
    assert_eq!(w.get_back(2), None);
}

#[test]
fn overlap_distance_1_repeats() {
    let w = window_with(b"x");
    for path in [DecodePath::Fast, DecodePath::Reference] {
        let mut out = [0u8; 5];
        let n = w.copy_match(path, &mut out, 0, 1, 5);
        assert_eq!(n, 5);
        assert_eq!(&out, b"xxxxx");
    }
}

#[test]
fn overlap_distance_3() {
    let w = window_with(b"abc");
    for path in [DecodePath::Fast, DecodePath::Reference] {
        let mut out = [0u8; 7];
        let n = w.copy_match(path, &mut out, 0, 3, 7);
        assert_eq!(n, 7);
        assert_eq!(&out, b"abcabca");
    }
}

#[test]
fn copy_respects_out_len() {
    let w = window_with(b"z");
    for path in [DecodePath::Fast, DecodePath::Reference] {
        let mut out = [0u8; 3];
        let n = w.copy_match(path, &mut out, 0, 1, 100);
        assert_eq!(n, 3);
        assert_eq!(&out, b"zzz");
    }
}

#[test]
fn prefill_keeps_last_32k() {
    let mut w = Window::new();
    let data: Vec<u8> = (0..40_000u32).map(|i| (i % 256) as u8).collect();
    w.prefill(&data);
    assert_eq!(w.available(), 32768);
    // The last byte prefilled is the most recent.
    let last = *data.last().expect("non-empty");
    assert_eq!(w.get_back(1), Some(last));
}

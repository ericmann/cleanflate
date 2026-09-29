//! M1/M2 conformance tests (SPEC S5 M1, M2): every raw-DEFLATE manifest
//! entry decodes byte-identically to its recorded input, both one-shot and
//! streamed through odd input/output chunk sizes.

use cleanflate::{Decompressor, Format, Options};
use oracle::conformance::{check_entries, decode_streaming, selected};

/// M1: the stored (level 0) and fixed-Huffman subset, one-shot.
#[test]
fn conformance_raw_stored_and_fixed() {
    let entries = selected(|e| {
        e.framing == "raw" && e.dictionary.is_none() && (e.level == 0 || e.strategy == "fixed")
    });

    let report = check_entries(&entries, |bytes| {
        cleanflate::inflate(bytes).map_err(|e| format!("{e:?}"))
    });

    println!(
        "conformance_raw_stored_and_fixed: {} entries checked",
        report.checked
    );
    report.assert_ok();
}

/// M2: every raw entry (dynamic Huffman included), one-shot.
#[test]
fn conformance_raw_all() {
    let entries = selected(|e| e.framing == "raw" && e.dictionary.is_none());

    let report = check_entries(&entries, |bytes| {
        cleanflate::inflate(bytes).map_err(|e| format!("{e:?}"))
    });

    println!("conformance_raw_all: {} entries checked", report.checked);
    report.assert_ok();
}

/// M2: every raw entry, streamed through a `Decompressor` with
/// 1,021-byte input chunks and a 997-byte output buffer, neither of
/// which divides evenly into typical block/window sizes.
#[test]
fn conformance_raw_all_streaming_odd_chunks() {
    let entries = selected(|e| e.framing == "raw" && e.dictionary.is_none());

    let report = check_entries(&entries, |bytes| {
        let mut d = Decompressor::new(Format::Raw, Options::default());
        decode_streaming(&mut d, bytes, 1021, 997)
    });

    println!(
        "conformance_raw_all_streaming_odd_chunks: {} entries checked",
        report.checked
    );
    report.assert_ok();
}

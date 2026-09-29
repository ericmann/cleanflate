//! M1 conformance test (SPEC S5 M1): every raw, dictionary-free, stored
//! (level 0) or fixed-Huffman entry in the manifest decodes with
//! `cleanflate::inflate` byte-identically to its recorded input.

use oracle::conformance::{check_entries, selected};

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

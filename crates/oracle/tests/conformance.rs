//! M1/M2/M3 conformance tests (SPEC S5 M1-M3): every manifest entry
//! decodes byte-identically to its recorded input -- raw, zlib and gzip,
//! with and without a preset dictionary, one-shot and streamed through
//! odd input/output chunk sizes.

use cleanflate::{decompress_with, Decompressor, Format, Options};
use oracle::conformance::{check_entries, decode_streaming, dictionary_bytes, selected};

/// M1: the stored (level 0) and fixed-Huffman subset, one-shot.
#[test]
fn conformance_raw_stored_and_fixed() {
    let entries = selected(|e| {
        e.framing == "raw" && e.dictionary.is_none() && (e.level == 0 || e.strategy == "fixed")
    });

    let report = check_entries(&entries, |_entry, bytes| {
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

    let report = check_entries(&entries, |_entry, bytes| {
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

    let report = check_entries(&entries, |_entry, bytes| {
        let mut d = Decompressor::new(Format::Raw, Options::default());
        decode_streaming(&mut d, bytes, 1021, 997)
    });

    println!(
        "conformance_raw_all_streaming_odd_chunks: {} entries checked",
        report.checked
    );
    report.assert_ok();
}

/// M3: every zlib entry without a preset dictionary, one-shot.
#[test]
fn conformance_zlib_all() {
    let entries = selected(|e| e.framing == "zlib" && e.dictionary.is_none());

    let report = check_entries(&entries, |_entry, bytes| {
        cleanflate::zlib_decompress(bytes).map_err(|e| format!("{e:?}"))
    });

    println!("conformance_zlib_all: {} entries checked", report.checked);
    report.assert_ok();
}

/// M3: every zlib entry with a preset dictionary, decoded with the first
/// `dictionary.len` bytes of the manifest's recorded dictionary input.
#[test]
fn conformance_zlib_dictionary() {
    let entries = selected(|e| e.framing == "zlib" && e.dictionary.is_some());

    let report = check_entries(&entries, |entry, bytes| {
        let dictionary = entry
            .dictionary
            .as_ref()
            .ok_or_else(|| "entry has no dictionary despite the filter".to_string())?;
        let dict_bytes = dictionary_bytes(dictionary)?;
        let options = Options {
            dictionary: Some(dict_bytes),
            ..Options::default()
        };
        decompress_with(Format::Zlib, bytes, &options)
            .map(|d| d.data)
            .map_err(|e| format!("{e:?}"))
    });

    println!(
        "conformance_zlib_dictionary: {} entries checked",
        report.checked
    );
    report.assert_ok();
}

/// M3: every gzip entry, one-shot.
#[test]
fn conformance_gzip_all() {
    let entries = selected(|e| e.framing == "gzip");

    let report = check_entries(&entries, |_entry, bytes| {
        cleanflate::gzip_decompress(bytes).map_err(|e| format!("{e:?}"))
    });

    println!("conformance_gzip_all: {} entries checked", report.checked);
    report.assert_ok();
}

/// M3: every dictionary-free zlib or gzip entry, streamed through a
/// `Decompressor` with 1,021-byte input chunks and a 997-byte output
/// buffer.
#[test]
fn conformance_wrappers_streaming_odd_chunks() {
    let entries =
        selected(|e| (e.framing == "zlib" || e.framing == "gzip") && e.dictionary.is_none());

    let report = check_entries(&entries, |entry, bytes| {
        let format = match entry.framing.as_str() {
            "zlib" => Format::Zlib,
            "gzip" => Format::Gzip,
            other => return Err(format!("unexpected framing {other}")),
        };
        let mut d = Decompressor::new(format, Options::default());
        decode_streaming(&mut d, bytes, 1021, 997)
    });

    println!(
        "conformance_wrappers_streaming_odd_chunks: {} entries checked",
        report.checked
    );
    report.assert_ok();
}

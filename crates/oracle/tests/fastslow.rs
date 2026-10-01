//! Fast-vs-reference differential test (SPEC S5): the production decode
//! path and the simple reference path must agree on output, errors,
//! per-call consumed/produced/status and finish results, on the
//! conformance corpus, the malformed corpus and generated streams.

use proptest::prelude::*;

use cleanflate::test_hooks::DecodePath;
use cleanflate::{Format, Options};
use oracle::conformance::{check_entries, dictionary_bytes, repo_root, selected};
use oracle::ffi::{deflate_bytes, DeflateParams};

mod common;
use common::{
    chunk_schedule_strategy, compare_sinks, data_strategy, describe_diff, dump_candidate, oneshot,
    params_strategy, to_cleanflate_format, trace,
};

fn format_of(framing: &str) -> Format {
    match framing {
        "zlib" => Format::Zlib,
        "gzip" => Format::Gzip,
        _ => Format::Raw,
    }
}

fn options_for(entry: &oracle::manifest::ManifestEntry) -> Result<Options, String> {
    let dictionary = match &entry.dictionary {
        Some(d) => Some(dictionary_bytes(d)?),
        None => None,
    };
    Ok(Options {
        dictionary,
        ..Options::default()
    })
}

/// Compare both paths one-shot. `Err` means the paths disagree; on
/// agreement the shared decode result is returned inside `Ok`.
fn compare_agree(
    format: Format,
    options: &Options,
    input: &[u8],
) -> Result<Result<Vec<u8>, String>, String> {
    let fast = oneshot(format, options, DecodePath::Fast, input);
    let reference = oneshot(format, options, DecodePath::Reference, input);
    if fast != reference {
        let short = |r: &Result<(Vec<u8>, usize, String), String>| match r {
            Ok((d, c, _)) => format!("Ok(len={}, consumed={c})", d.len()),
            Err(e) => format!("Err({e})"),
        };
        return Err(format!(
            "one-shot mismatch: fast={} reference={}",
            short(&fast),
            short(&reference)
        ));
    }
    Ok(fast.map(|(d, _, _)| d))
}

/// Like [`compare_agree`] but a shared decode error is also an `Err`.
fn compare_oneshot(format: Format, options: &Options, input: &[u8]) -> Result<Vec<u8>, String> {
    compare_agree(format, options, input)?
}

fn compare_traces(
    format: Format,
    options: &Options,
    input: &[u8],
    in_sched: &[usize],
    out_sched: &[usize],
) -> Result<(), String> {
    let fast = trace(
        format,
        options,
        DecodePath::Fast,
        input,
        in_sched,
        out_sched,
    );
    let reference = trace(
        format,
        options,
        DecodePath::Reference,
        input,
        in_sched,
        out_sched,
    );
    if fast.stalled || reference.stalled {
        return Err(format!(
            "trace stalled (fast={} reference={})",
            fast.stalled, reference.stalled
        ));
    }
    match describe_diff(&fast, &reference) {
        None => Ok(()),
        Some(d) => Err(d),
    }
}

#[test]
fn fastslow_conformance_oneshot() {
    let entries = selected(|_| true);
    let report = check_entries(&entries, |entry, bytes| {
        let options = options_for(entry)?;
        compare_oneshot(format_of(&entry.framing), &options, bytes)
    });
    println!("fastslow_conformance_oneshot: {} entries", report.checked);
    report.assert_ok();
}

#[test]
fn fastslow_conformance_streaming() {
    let entries = selected(|_| true);
    let report = check_entries(&entries, |entry, bytes| {
        let options = options_for(entry)?;
        let format = format_of(&entry.framing);
        compare_traces(format, &options, bytes, &[1021], &[997])?;
        // The trace equality above is the assertion; return the one-shot
        // output so check_entries also verifies it against the manifest.
        compare_oneshot(format, &options, bytes)
    });
    println!("fastslow_conformance_streaming: {} entries", report.checked);
    report.assert_ok();
}

#[test]
fn sinks_conformance_oneshot() {
    let entries = selected(|_| true);
    let report = check_entries(&entries, |entry, bytes| {
        let options = options_for(entry)?;
        let shared = compare_sinks(format_of(&entry.framing), &options, bytes)?;
        shared.map(|(data, _, _)| data)
    });
    println!("sinks_conformance_oneshot: {} entries", report.checked);
    report.assert_ok();
}

#[test]
fn sinks_malformed() {
    let root = repo_root();
    let tsv = root.join("testdata/malformed/EXPECTED.tsv");
    let text = std::fs::read_to_string(&tsv).unwrap_or_else(|e| {
        panic!(
            "failed to read {}: {e} -- run `make malformed` first",
            tsv.display()
        )
    });
    let mut rows = 0usize;
    let mut failures = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let raw = line.split('\t').next().unwrap_or("");
        if raw.is_empty() {
            continue;
        }
        let path = raw.trim_start_matches("sweep:");
        rows += 1;
        let file = root.join("testdata/malformed").join(path);
        let bytes = std::fs::read(&file).unwrap_or_else(|e| {
            panic!(
                "failed to read {}: {e} -- run `make malformed` first",
                file.display()
            )
        });
        let format = if path.starts_with("rfc1950/") {
            Format::Zlib
        } else if path.starts_with("rfc1952/") {
            Format::Gzip
        } else {
            Format::Raw
        };
        if let Err(e) = compare_sinks(format, &Options::default(), &bytes) {
            failures.push(format!("{path}: {e}"));
        }
    }
    assert!(
        rows > 0,
        "{} has zero rows -- run `make malformed` first",
        tsv.display()
    );
    assert!(
        failures.is_empty(),
        "sink mismatches ({} of {rows} rows):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn fastslow_malformed() {
    let root = repo_root();
    let tsv = root.join("testdata/malformed/EXPECTED.tsv");
    let text = std::fs::read_to_string(&tsv).unwrap_or_else(|e| {
        panic!(
            "failed to read {}: {e} -- run `make malformed` first",
            tsv.display()
        )
    });
    let mut rows = 0usize;
    let mut failures = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let raw = line.split('\t').next().unwrap_or("");
        if raw.is_empty() {
            continue;
        }
        let path = raw.trim_start_matches("sweep:");
        rows += 1;
        let file = root.join("testdata/malformed").join(path);
        let bytes = std::fs::read(&file).unwrap_or_else(|e| {
            panic!(
                "failed to read {}: {e} -- run `make malformed` first",
                file.display()
            )
        });
        let format = if path.starts_with("rfc1950/") {
            Format::Zlib
        } else if path.starts_with("rfc1952/") {
            Format::Gzip
        } else {
            Format::Raw
        };
        let options = Options::default();
        if let Err(e) = compare_agree(format, &options, &bytes) {
            failures.push(format!("{path}: {e}"));
        }
        for (i, o) in [
            (&[1usize][..], &[1usize][..]),
            (&[3, 5, 7][..], &[2, 11][..]),
        ] {
            if let Err(e) = compare_traces(format, &options, &bytes, i, o) {
                failures.push(format!("{path} sched {i:?}/{o:?}: {e}"));
            }
        }
    }
    assert!(
        rows > 0,
        "{} has zero rows -- run `make malformed` first",
        tsv.display()
    );
    assert!(
        failures.is_empty(),
        "fast/reference mismatches ({} of {rows} rows):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn case_count() -> u32 {
    std::env::var("CLEANFLATE_FASTSLOW_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(256)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: case_count(), ..ProptestConfig::default() })]

    #[test]
    fn fastslow_proptest(
        data in data_strategy(),
        (framing, level, strategy, window_bits, mem_level) in params_strategy(),
        in_sched in chunk_schedule_strategy(),
        out_sched in chunk_schedule_strategy(),
        flip_ratio in 0.0f64..1.0,
        flip_xor in 1u8..=255,
    ) {
        let params = DeflateParams { level, strategy, window_bits, mem_level, framing };
        let compressed = match deflate_bytes(&data, &params, None) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let format = to_cleanflate_format(framing);
        let options = Options::default();

        let mut flipped = compressed.clone();
        if !flipped.is_empty() {
            let idx = ((flip_ratio * flipped.len() as f64) as usize).min(flipped.len() - 1);
            if let Some(b) = flipped.get_mut(idx) {
                *b ^= flip_xor;
            }
        }

        for (tag, input) in [("clean", &compressed), ("flipped", &flipped)] {
            if let Err(e) = compare_traces(format, &options, input, &in_sched, &out_sched) {
                let path = dump_candidate(
                    &format!("fastslow-{tag}"),
                    format,
                    input,
                    &in_sched,
                    &out_sched,
                    &e,
                );
                return Err(TestCaseError::fail(format!(
                    "{tag}: {e} (saved {})",
                    path.display()
                )));
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: case_count(), ..ProptestConfig::default() })]

    #[test]
    fn sinks_proptest(
        data in data_strategy(),
        (framing, level, strategy, window_bits, mem_level) in params_strategy(),
        flip_ratio in 0.0f64..1.0,
        flip_xor in 1u8..=255,
        limit_pick in 0usize..6,
    ) {
        let params = DeflateParams { level, strategy, window_bits, mem_level, framing };
        let compressed = match deflate_bytes(&data, &params, None) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let format = to_cleanflate_format(framing);
        let len = data.len() as u64;
        let max_output = match limit_pick {
            0 => 0,
            1 => len / 2,
            2 => len.saturating_sub(1),
            3 => len,
            4 => len + 1,
            _ => Options::default().max_output,
        };
        let options = Options { max_output, ..Options::default() };

        let mut flipped = compressed.clone();
        if !flipped.is_empty() {
            let idx = ((flip_ratio * flipped.len() as f64) as usize).min(flipped.len() - 1);
            if let Some(b) = flipped.get_mut(idx) {
                *b ^= flip_xor;
            }
        }

        for (tag, input) in [("clean", &compressed), ("flipped", &flipped)] {
            if let Err(e) = compare_sinks(format, &options, input) {
                let path = dump_candidate(&format!("sinks-{tag}"), format, input, &[input.len().max(1)], &[65_536], &e);
                return Err(TestCaseError::fail(format!(
                    "{tag} max_output={max_output}: {e} (saved {})",
                    path.display()
                )));
            }
        }
    }
}

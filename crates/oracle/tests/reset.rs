//! Reset differential (SPEC S5): a decompressor that was driven partway
//! (or to the end, or to an error), then `reset()`, must behave exactly
//! like a fresh one on any later stream: same calls, same output, same
//! `finish`, same counters.

use proptest::prelude::*;

use cleanflate::test_hooks::{decompressor_with_path, DecodePath};
use cleanflate::{Decompressor, Format, Options, Status, TrailingData};
use oracle::conformance::{check_entries, dictionary_bytes, repo_root, selected};
use oracle::ffi::{deflate_bytes, DeflateParams, Framing, Strategy};
use oracle::manifest::ManifestEntry;

mod common;
use common::{
    chunk_schedule_strategy, data_strategy, describe_diff, oneshot, params_strategy,
    to_cleanflate_format, trace, trace_with,
};

fn format_of(framing: &str) -> Format {
    match framing {
        "zlib" => Format::Zlib,
        "gzip" => Format::Gzip,
        _ => Format::Raw,
    }
}

fn options_for(entry: &ManifestEntry) -> Result<Options, String> {
    let dictionary = match &entry.dictionary {
        Some(d) => Some(dictionary_bytes(d)?),
        None => None,
    };
    Ok(Options {
        dictionary,
        ..Options::default()
    })
}

/// The ways a decompressor can have been used before `reset()`.
const SCENARIOS: usize = 6;

/// Drive `d` on `a` into the state `scenario` names: 0 untouched, 1 one
/// small call, 2 to the end, 3 truncated input then `finish` (an error),
/// 4 one flipped byte to the end, 5 to the end (the caller passes a small
/// `max_output`, so this ends at the output limit).
fn drive_prefix(d: &mut Decompressor, a: &[u8], scenario: usize) {
    let run_to_end = |d: &mut Decompressor, input: &[u8]| {
        let mut out = vec![0u8; 1 << 16];
        let mut pos = 0usize;
        for _ in 0..4096 {
            match d.decompress(input.get(pos..).unwrap_or(&[]), &mut out) {
                Err(_) => break,
                Ok(p) => {
                    pos += p.consumed;
                    if p.status == Status::Done || (p.consumed == 0 && p.produced == 0) {
                        break;
                    }
                }
            }
        }
    };
    match scenario {
        0 => {}
        1 => {
            let mut out = [0u8; 50];
            let _ = d.decompress(a.get(..a.len().min(100)).unwrap_or(&[]), &mut out);
        }
        2 | 5 => run_to_end(d, a),
        3 => {
            run_to_end(d, a.get(..a.len() / 2).unwrap_or(&[]));
            let _ = d.finish();
        }
        _ => {
            let mut flipped = a.to_vec();
            if let Some(b) = flipped.get_mut(a.len() / 2) {
                *b ^= 0x5A;
            }
            run_to_end(d, &flipped);
        }
    }
}

fn scenario_options(base: &Options, scenario: usize) -> Options {
    if scenario == 5 {
        Options {
            max_output: 100,
            ..base.clone()
        }
    } else {
        base.clone()
    }
}

/// Trace `target` on a fresh decompressor and on one that ran `prefix` in
/// `scenario` and was then reset; `Err` describes any difference.
fn compare_reset(
    format: Format,
    options: &Options,
    path: DecodePath,
    prefix: &[u8],
    target: &[u8],
    scenario: usize,
    (in_sched, out_sched): (&[usize], &[usize]),
) -> Result<(), String> {
    let options = scenario_options(options, scenario);
    let fresh = trace(format, &options, path, target, in_sched, out_sched);
    let mut d = decompressor_with_path(format, options.clone(), path);
    drive_prefix(&mut d, prefix, scenario);
    d.reset();
    let after = trace_with(&mut d, target, in_sched, out_sched);
    if fresh.stalled || after.stalled {
        return Err("trace stalled".to_string());
    }
    match describe_diff(&fresh, &after) {
        None => Ok(()),
        Some(e) => Err(format!("scenario {scenario} {path:?}: {e}")),
    }
}

fn generated_bytes(entry: &ManifestEntry) -> Result<Vec<u8>, String> {
    let path = repo_root().join("testdata/generated").join(&entry.path);
    std::fs::read(&path)
        .map_err(|e| format!("reading {}: {e} -- run `make corpus` first", path.display()))
}

#[test]
fn reset_equals_fresh_conformance() {
    let entries = selected(|_| true);
    let report = check_entries(&entries, |entry, bytes| {
        let options = options_for(entry)?;
        let format = format_of(&entry.framing);
        // A different entry of the same framing drives the decompressor
        // before the reset.
        let same: Vec<&ManifestEntry> = entries
            .iter()
            .filter(|e| e.framing == entry.framing)
            .collect();
        let pos = same.iter().position(|e| e.id == entry.id).unwrap_or(0);
        let other = same
            .get((pos + 1) % same.len())
            .ok_or("no other entry of the same framing")?;
        let prefix = generated_bytes(other)?;
        let scenario = entry.id.bytes().map(usize::from).sum::<usize>() % SCENARIOS;
        compare_reset(
            format,
            &options,
            DecodePath::Fast,
            &prefix,
            bytes,
            scenario,
            (&[1021], &[997]),
        )?;
        // One-byte chunks for every fourth entry of up to 64 KiB (all of them
        // would dominate the quick tier's run time).
        let id_sum = entry.id.bytes().map(usize::from).sum::<usize>();
        if bytes.len() <= 64 * 1024 && id_sum / SCENARIOS % 4 == 0 {
            compare_reset(
                format,
                &options,
                DecodePath::Fast,
                &prefix,
                bytes,
                (scenario + 1) % SCENARIOS,
                (&[1], &[1]),
            )?;
        }
        oneshot(format, &options, DecodePath::Fast, bytes).map(|(d, _, _)| d)
    });
    println!("reset_equals_fresh_conformance: {} entries", report.checked);
    report.assert_ok();
}

#[test]
fn reset_equals_fresh_options() {
    let all = selected(|_| true);
    let pick = |framing: &str, with_dict: bool| -> ManifestEntry {
        all.iter()
            .filter(|e| e.framing == framing && e.dictionary.is_some() == with_dict)
            .min_by_key(|e| e.input_len)
            .unwrap_or_else(|| panic!("no {framing} entry (dictionary: {with_dict}) in the tier"))
            .clone()
    };
    let mut failures = Vec::new();
    for framing in ["raw", "zlib", "gzip"] {
        let plain = pick(framing, false);
        let bytes = generated_bytes(&plain).expect("plain entry bytes");
        let format = format_of(framing);
        let variants: Vec<(&str, Options)> = vec![
            ("default", Options::default()),
            (
                "trailing allow",
                Options {
                    trailing: TrailingData::Allow,
                    ..Options::default()
                },
            ),
            (
                "small max_output",
                Options {
                    max_output: 10,
                    ..Options::default()
                },
            ),
            (
                "max_gzip_headers 1",
                Options {
                    max_gzip_headers: 1,
                    ..Options::default()
                },
            ),
        ];
        // A stream with trailing bytes exercises the trailing policy too.
        let mut with_trailing = bytes.clone();
        with_trailing.extend_from_slice(b"\x1fXYZ");
        for (name, options) in &variants {
            for scenario in 0..SCENARIOS {
                for (tag, target) in [("clean", &bytes), ("trailing", &with_trailing)] {
                    for path in [DecodePath::Fast, DecodePath::Reference] {
                        if let Err(e) = compare_reset(
                            format,
                            options,
                            path,
                            &bytes,
                            target,
                            scenario,
                            (&[777], &[333]),
                        ) {
                            failures.push(format!("{framing} {name} {tag}: {e}"));
                        }
                    }
                }
            }
        }
    }
    // Dictionaries. Raw: built here with the oracle so the dictionary is
    // always exercised; zlib: the smallest manifest entry that needs one.
    let dict = b"the quick brown fox jumps over the lazy dog, again and again";
    let raw_data: Vec<u8> = dict.iter().cycle().take(4000).copied().collect();
    let raw_params = DeflateParams {
        level: 6,
        strategy: Strategy::Default,
        window_bits: 15,
        mem_level: 8,
        framing: Framing::Raw,
    };
    let raw_bytes = deflate_bytes(&raw_data, &raw_params, Some(dict))
        .expect("oracle raw deflate with dictionary");
    let raw_options = Options {
        dictionary: Some(dict.to_vec()),
        ..Options::default()
    };
    let zlib_entry = all
        .iter()
        .filter(|e| e.framing == "zlib" && e.dictionary.is_some())
        .min_by_key(|e| e.input_len)
        .unwrap_or_else(|| {
            panic!("no zlib entry with a dictionary in the tier -- run `make corpus`")
        });
    let cases: Vec<(&str, Format, Vec<u8>, Options)> = vec![
        ("raw", Format::Raw, raw_bytes, raw_options),
        (
            "zlib",
            Format::Zlib,
            generated_bytes(zlib_entry).expect("dictionary entry bytes"),
            options_for(zlib_entry).expect("dictionary bytes"),
        ),
    ];
    for (framing, format, bytes, options) in &cases {
        for scenario in 0..SCENARIOS {
            for path in [DecodePath::Fast, DecodePath::Reference] {
                if let Err(e) = compare_reset(
                    *format,
                    options,
                    path,
                    bytes,
                    bytes,
                    scenario,
                    (&[777], &[333]),
                ) {
                    failures.push(format!("{framing} dictionary: {e}"));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} reset mismatches:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn reset_repeated_same_result() {
    let entries = selected(|_| true);
    for framing in ["raw", "zlib", "gzip"] {
        let entry = entries
            .iter()
            .filter(|e| e.framing == framing && e.dictionary.is_none())
            .min_by_key(|e| e.input_len)
            .unwrap_or_else(|| panic!("no {framing} entry in the tier"));
        let bytes = generated_bytes(entry).expect("entry bytes");
        let format = format_of(framing);
        let options = Options::default();
        let expected = trace(format, &options, DecodePath::Fast, &bytes, &[500], &[400]);
        let mut d = decompressor_with_path(format, options.clone(), DecodePath::Fast);
        for round in 0..100 {
            let got = trace_with(&mut d, &bytes, &[500], &[400]);
            assert_eq!(got, expected, "{framing} round {round}");
            d.reset();
        }
    }
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
    fn reset_proptest(
        first in data_strategy(),
        second in data_strategy(),
        (framing, level, strategy, window_bits, mem_level) in params_strategy(),
        in_sched in chunk_schedule_strategy(),
        out_sched in chunk_schedule_strategy(),
        scenario in 0usize..SCENARIOS,
        prefix_calls in 0usize..=6,
        flip in proptest::option::of((0.0f64..1.0, 1u8..=255)),
    ) {
        let params = DeflateParams { level, strategy, window_bits, mem_level, framing };
        let (Ok(a), Ok(mut b)) = (deflate_bytes(&first, &params, None), deflate_bytes(&second, &params, None)) else {
            return Ok(());
        };
        if let Some((ratio, xor)) = flip {
            if !b.is_empty() {
                let idx = ((ratio * b.len() as f64) as usize).min(b.len() - 1);
                if let Some(x) = b.get_mut(idx) {
                    *x ^= xor;
                }
            }
        }
        let format = to_cleanflate_format(framing);
        for path in [DecodePath::Fast, DecodePath::Reference] {
            // Drive `prefix_calls` calls with this case's own schedules, then
            // reset and compare against a fresh decompressor.
            let mut d = decompressor_with_path(format, Options::default(), path);
            let mut pos = 0usize;
            for call in 0..prefix_calls {
                let in_n = in_sched.get(call % in_sched.len().max(1)).copied().unwrap_or(1);
                let out_n = out_sched.get(call % out_sched.len().max(1)).copied().unwrap_or(1).max(1);
                let end = pos.saturating_add(in_n).min(a.len());
                let mut out = vec![0u8; out_n];
                if let Ok(p) = d.decompress(a.get(pos..end).unwrap_or(&[]), &mut out) {
                    pos += p.consumed;
                }
            }
            d.reset();
            let fresh = trace(format, &Options::default(), path, &b, &in_sched, &out_sched);
            let after = trace_with(&mut d, &b, &in_sched, &out_sched);
            if fresh.stalled || after.stalled {
                return Err(TestCaseError::fail("prefix trace stalled"));
            }
            if let Some(e) = describe_diff(&fresh, &after) {
                return Err(TestCaseError::fail(format!("prefix_calls {prefix_calls} {path:?}: {e}")));
            }
            if let Err(e) = compare_reset(
                format, &Options::default(), path, &a, &b, scenario, (&in_sched, &out_sched),
            ) {
                return Err(TestCaseError::fail(e));
            }
        }
    }
}

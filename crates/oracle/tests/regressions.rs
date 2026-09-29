//! Every committed regression fixture decodes correctly, identically on
//! the Fast and Reference paths, one-shot and streamed (SPEC S5). A
//! missing manifest, zero rows or a missing file is a failure (C13).

use std::collections::BTreeSet;

use cleanflate::test_hooks::{decompress_with_path, DecodePath};
use cleanflate::{Format, Options};
use oracle::manifest::sha256_hex;
use oracle::regress::{load_rows, regressions_dir, Row};

mod common;
use common::{describe_diff, trace, CallOutcome};

fn format_of(row: &Row) -> Format {
    match row.format.as_str() {
        "zlib" => Format::Zlib,
        "gzip" => Format::Gzip,
        "raw" => Format::Raw,
        other => panic!("{}: unknown format {other:?}", row.path),
    }
}

fn check_schedule(
    row: &Row,
    bytes: &[u8],
    in_s: &[usize],
    out_s: &[usize],
    errors: &mut Vec<String>,
) {
    let format = format_of(row);
    let options = Options::default();
    let fast = trace(format, &options, DecodePath::Fast, bytes, in_s, out_s);
    let reference = trace(format, &options, DecodePath::Reference, bytes, in_s, out_s);
    if let Some(d) = describe_diff(&fast, &reference) {
        errors.push(format!(
            "{} sched {in_s:?}/{out_s:?}: fast != reference: {d}",
            row.path
        ));
        return;
    }
    if fast.stalled {
        errors.push(format!("{} sched {in_s:?}/{out_s:?}: stalled", row.path));
        return;
    }
    if row.expect == "ok" {
        if fast.finish.as_deref() != Some("Ok(())") {
            errors.push(format!(
                "{} sched {in_s:?}/{out_s:?}: finish {:?}",
                row.path, fast.finish
            ));
        }
        if Some(fast.output.len()) != row.out_len
            || Some(sha256_hex(&fast.output)) != row.out_sha256
        {
            errors.push(format!(
                "{} sched {in_s:?}/{out_s:?}: streamed output len {} sha mismatch",
                row.path,
                fast.output.len()
            ));
        }
    } else {
        let kind = row.expect.trim_start_matches("err:");
        match fast.calls.last() {
            Some(CallOutcome::Err(e)) if e.starts_with(kind) => {}
            other => errors.push(format!(
                "{} sched {in_s:?}/{out_s:?}: expected error {kind}, last call {other:?}",
                row.path
            )),
        }
    }
}

#[test]
fn regressions_all_paths() {
    let rows = load_rows().unwrap_or_else(|e| panic!("{e}"));
    let dir = regressions_dir();
    let mut errors = Vec::new();
    for row in &rows {
        let file = dir.join(&row.path);
        let bytes = std::fs::read(&file).unwrap_or_else(|e| {
            panic!(
                "{}: cannot read {}: {e} -- run `make regress-seed`",
                row.path,
                file.display()
            )
        });
        let format = format_of(row);
        let options = Options::default();
        let mut results = Vec::new();
        for path in [DecodePath::Fast, DecodePath::Reference] {
            results.push(decompress_with_path(format, &bytes, &options, path));
        }
        for (path, result) in [DecodePath::Fast, DecodePath::Reference]
            .iter()
            .zip(&results)
        {
            if row.expect == "ok" {
                match result {
                    Ok(d) => {
                        if Some(d.data.len()) != row.out_len
                            || Some(sha256_hex(&d.data)) != row.out_sha256
                        {
                            errors.push(format!("{} {path:?}: one-shot output mismatch", row.path));
                        }
                    }
                    Err(e) => errors.push(format!("{} {path:?}: one-shot error {e:?}", row.path)),
                }
            } else {
                let kind = row.expect.trim_start_matches("err:");
                match result {
                    Err(e) if e.kind().as_str() == kind => {}
                    other => errors.push(format!(
                        "{} {path:?}: expected error kind {kind}, got {:?}",
                        row.path,
                        other.as_ref().map(|d| d.data.len())
                    )),
                }
            }
        }
        check_schedule(row, &bytes, &[1], &[1], &mut errors);
        if let (Some(i), Some(o)) = (&row.in_chunks, &row.out_chunks) {
            check_schedule(row, &bytes, i, o, &mut errors);
        } else if row.in_chunks.is_some() || row.out_chunks.is_some() {
            let i = row.in_chunks.clone().unwrap_or_else(|| vec![1021]);
            let o = row.out_chunks.clone().unwrap_or_else(|| vec![997]);
            check_schedule(row, &bytes, &i, &o, &mut errors);
        }
    }
    assert!(
        errors.is_empty(),
        "regression failures:\n{}",
        errors.join("\n")
    );
}

#[test]
fn regressions_manifest_matches_files() {
    let rows = load_rows().unwrap_or_else(|e| panic!("{e}"));
    let dir = regressions_dir();
    let mut row_paths = BTreeSet::new();
    for row in &rows {
        assert!(
            row_paths.insert(row.path.clone()),
            "duplicate manifest row {}",
            row.path
        );
        assert!(
            dir.join(&row.path).is_file(),
            "row {} names a missing file",
            row.path
        );
    }
    let mut on_disk = BTreeSet::new();
    for sub in ["rfc1951", "rfc1950", "rfc1952"] {
        let Ok(entries) = std::fs::read_dir(dir.join(sub)) else {
            continue;
        };
        for e in entries {
            let e = e.expect("dir entry");
            let name = e.file_name().to_string_lossy().to_string();
            if name.ends_with(".bin") {
                on_disk.insert(format!("{sub}/{name}"));
            }
        }
    }
    assert_eq!(on_disk, row_paths, "fixture files and manifest rows differ");
}

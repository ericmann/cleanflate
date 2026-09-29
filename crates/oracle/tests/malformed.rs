use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

struct Row {
    path: String,
    expected: String,
    is_sweep: bool,
}

fn parse_tsv(text: &str) -> Vec<Row> {
    let mut rows = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let mut parts = line.splitn(3, '\t');
        let path = parts.next().unwrap_or("").to_string();
        let expected = parts.next().unwrap_or("").to_string();
        if path.is_empty() {
            continue;
        }
        let is_sweep = path.starts_with("sweep:");
        rows.push(Row {
            path: if is_sweep {
                path.trim_start_matches("sweep:").to_string()
            } else {
                path
            },
            expected,
            is_sweep,
        });
    }
    rows
}

/// Decode `bytes` with the decoder matching `row.path`'s corpus
/// subdirectory, projecting the result the same way as the TSV's
/// `expected` column (`"OK"` or an `ErrorKind` name).
fn decode_for_row(path: &str, bytes: &[u8]) -> Result<Vec<u8>, cleanflate::Error> {
    if let Some(rest) = path.strip_prefix("rfc1950/") {
        let _ = rest;
        cleanflate::decompress_with(
            cleanflate::Format::Zlib,
            bytes,
            &cleanflate::Options::default(),
        )
        .map(|d| d.data)
    } else if let Some(rest) = path.strip_prefix("rfc1952/") {
        let _ = rest;
        cleanflate::decompress_with(
            cleanflate::Format::Gzip,
            bytes,
            &cleanflate::Options::default(),
        )
        .map(|d| d.data)
    } else {
        cleanflate::inflate(bytes)
    }
}

#[test]
fn malformed_raw_cases() {
    let root = repo_root();
    let tsv_path = root.join("testdata/malformed/EXPECTED.tsv");
    let text = fs::read_to_string(&tsv_path).unwrap_or_else(|e| {
        panic!(
            "failed to read {}: {e} -- run `make malformed` first",
            tsv_path.display()
        )
    });
    let rows = parse_tsv(&text);
    assert!(!rows.is_empty(), "EXPECTED.tsv has no data rows");

    let mut failures: Vec<String> = Vec::new();

    for row in &rows {
        if !row.path.starts_with("rfc1951/") {
            continue;
        }
        let file_path = root.join("testdata/malformed").join(&row.path);
        let bytes = fs::read(&file_path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", file_path.display()));

        if row.is_sweep {
            for len in 0..bytes.len() {
                let prefix = &bytes[..len];
                match cleanflate::inflate(prefix) {
                    Ok(_) => failures.push(format!(
                        "{} prefix len {len}: expected Truncated, got Ok",
                        row.path
                    )),
                    Err(e) => {
                        let kind = e.kind().as_str();
                        if kind != "Truncated" {
                            failures.push(format!(
                                "{} prefix len {len}: expected Truncated, got {kind}",
                                row.path
                            ));
                        }
                    }
                }
            }
        } else {
            let result = cleanflate::inflate(&bytes);
            let actual = match &result {
                Ok(_) => "OK".to_string(),
                Err(e) => e.kind().as_str().to_string(),
            };
            if actual != row.expected {
                failures.push(format!(
                    "{}: expected {}, got {actual}",
                    row.path, row.expected
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "malformed corpus mismatches ({} of {} rows):\n{}",
        failures.len(),
        rows.len(),
        failures.join("\n")
    );
}

/// rfc1950 rows are decoded with `zlib_decompress`, rfc1952 rows with
/// `gzip_decompress`; sweep rows assert every strict prefix is
/// `Truncated`.
#[test]
fn malformed_wrapper_cases() {
    let root = repo_root();
    let tsv_path = root.join("testdata/malformed/EXPECTED.tsv");
    let text = fs::read_to_string(&tsv_path).unwrap_or_else(|e| {
        panic!(
            "failed to read {}: {e} -- run `make malformed` first",
            tsv_path.display()
        )
    });
    let rows = parse_tsv(&text);

    let mut failures: Vec<String> = Vec::new();

    for row in &rows {
        if !(row.path.starts_with("rfc1950/") || row.path.starts_with("rfc1952/")) {
            continue;
        }
        let file_path = root.join("testdata/malformed").join(&row.path);
        let bytes = fs::read(&file_path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", file_path.display()));

        if row.is_sweep {
            for len in 0..bytes.len() {
                let prefix = &bytes[..len];
                match decode_for_row(&row.path, prefix) {
                    Ok(_) => failures.push(format!(
                        "{} prefix len {len}: expected Truncated, got Ok",
                        row.path
                    )),
                    Err(e) => {
                        let kind = e.kind().as_str();
                        if kind != "Truncated" {
                            failures.push(format!(
                                "{} prefix len {len}: expected Truncated, got {kind}",
                                row.path
                            ));
                        }
                    }
                }
            }
        } else {
            let result = decode_for_row(&row.path, &bytes);
            let actual = match &result {
                Ok(_) => "OK".to_string(),
                Err(e) => e.kind().as_str().to_string(),
            };
            if actual != row.expected {
                failures.push(format!(
                    "{}: expected {}, got {actual}",
                    row.path, row.expected
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "malformed wrapper corpus mismatches ({} of {} rows):\n{}",
        failures.len(),
        rows.len(),
        failures.join("\n")
    );
}

#[test]
fn malformed_corpus_is_up_to_date() {
    let root = repo_root();
    let committed_dir = root.join("testdata/malformed");
    let committed_tsv = fs::read(committed_dir.join("EXPECTED.tsv"))
        .expect("testdata/malformed/EXPECTED.tsv missing -- run `make malformed`");

    let temp = std::env::temp_dir().join(format!(
        "cleanflate-malformed-regen-{}-{}",
        std::process::id(),
        rand_suffix()
    ));
    for dir in ["rfc1951", "rfc1950", "rfc1952"] {
        fs::create_dir_all(temp.join(dir)).expect("create temp dir");
    }

    let mut cases = oracle::malformed::cases();
    cases.extend(oracle::malformed::rfc1950_cases());
    cases.extend(oracle::malformed::rfc1952_cases());

    let mut tsv = String::new();
    tsv.push_str("# Hand-built malformed corpus (SPEC S4): RFC 1951/1950/1952.\n");
    tsv.push_str("# path\texpected\trule\n");
    for case in &cases {
        let rel_path = format!("{}/{}.bin", case.family.dir_name(), case.name);
        fs::write(temp.join(&rel_path), &case.bytes).expect("write fixture");
        tsv.push_str(&format!("{rel_path}\t{}\t{}\n", case.expected, case.rule));
        if case.sweep {
            tsv.push_str(&format!(
                "sweep:{rel_path}\tTruncated\tevery strict prefix of a valid stream is truncated\n"
            ));
        }
    }

    assert_eq!(
        tsv.as_bytes(),
        committed_tsv.as_slice(),
        "testdata/malformed/EXPECTED.tsv is stale -- run `make malformed` and commit the result"
    );

    for case in &cases {
        let rel_path = format!("{}/{}.bin", case.family.dir_name(), case.name);
        let committed_bytes = fs::read(committed_dir.join(&rel_path))
            .unwrap_or_else(|e| panic!("missing committed fixture {rel_path}: {e}"));
        assert_eq!(
            case.bytes, committed_bytes,
            "testdata/malformed/{rel_path} is stale -- run `make malformed` and commit the result"
        );
    }

    let _ = fs::remove_dir_all(&temp);
}

fn rand_suffix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

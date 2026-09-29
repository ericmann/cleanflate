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
    fs::create_dir_all(temp.join("rfc1951")).expect("create temp dir");

    let cases = oracle::malformed::cases();
    let mut tsv = String::new();
    tsv.push_str("# Hand-built raw-DEFLATE malformed corpus (SPEC S4).\n");
    tsv.push_str("# path\texpected\trule\n");
    for case in &cases {
        let rel_path = format!("rfc1951/{}.bin", case.name);
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
        let rel_path = format!("rfc1951/{}.bin", case.name);
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

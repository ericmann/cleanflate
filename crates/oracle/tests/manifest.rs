use std::fs;
use std::path::{Path, PathBuf};

use oracle::ffi::{Framing, Strategy};
use oracle::generate::{check, write_manifest};
use oracle::manifest::{DictionaryRecord, Manifest, ManifestEntry};
use oracle::matrix::CorpusTier;

/// A temp corpora root with no `canterbury/`/`silesia/` subdirectories, so
/// `inputs_for_tier(Quick, ...)` reduces to the 16 synthetic inputs only --
/// small enough to run this file's matrix twice per test quickly.
fn empty_corpora_root(tmp: &Path) -> PathBuf {
    let root = tmp.join("corpora");
    fs::create_dir_all(&root).unwrap();
    root
}

fn sample_entry() -> ManifestEntry {
    ManifestEntry {
        id: "empty__raw_l6_default_w15_m8".to_string(),
        tier: "synthetic".to_string(),
        input: "synthetic:empty".to_string(),
        input_sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
            .to_string(),
        input_len: 0,
        framing: "raw".to_string(),
        level: 6,
        strategy: "default".to_string(),
        window_bits: 15,
        mem_level: 8,
        dictionary: None,
        path: "synthetic/empty/rfc1951-l6-default-w15-m8.bin".to_string(),
        sha256: "0000000000000000000000000000000000000000000000000000000000000".to_string(),
        len: 2,
    }
}

#[test]
fn manifest_serde_roundtrip() {
    let mut with_dict = sample_entry();
    with_dict.id = "canterbury-alice29.txt__zlib_l6_default_w15_m8_dict".to_string();
    with_dict.dictionary = Some(DictionaryRecord {
        input: "testdata/corpora/canterbury/lcet10.txt".to_string(),
        len: 426754,
        sha256: "abc123".to_string(),
    });

    let manifest = Manifest::new("1.3.1".to_string(), vec![sample_entry(), with_dict]);

    let json = serde_json::to_string_pretty(&manifest).unwrap();
    let back: Manifest = serde_json::from_str(&json).unwrap();

    assert_eq!(manifest.format, back.format);
    assert_eq!(manifest.zlib_version, back.zlib_version);
    assert_eq!(manifest.seed, back.seed);
    assert_eq!(manifest.entries.len(), back.entries.len());
    for (a, b) in manifest.entries.iter().zip(back.entries.iter()) {
        assert_eq!(a, b);
    }
    // Explicit `Framing`/`Strategy` tokens used by generate.rs must round
    // trip through the string fields (sanity, not the serde mechanism).
    assert_eq!(oracle::manifest::framing_token(Framing::Zlib), "zlib");
    assert_eq!(oracle::manifest::strategy_token(Strategy::Rle), "rle");
}

#[test]
fn generate_small_matrix_twice_is_identical() {
    let tmp_a = tempdir();
    let tmp_b = tempdir();

    let corpora_root = empty_corpora_root(&tmp_a);

    let out_a = tmp_a.join("generated");
    let manifest_a = tmp_a.join("MANIFEST.json");
    let out_b = tmp_b.join("generated");
    let manifest_b = tmp_b.join("MANIFEST.json");

    let report_a = write_manifest(CorpusTier::Quick, &out_a, &manifest_a, &corpora_root).unwrap();
    let report_b = write_manifest(CorpusTier::Quick, &out_b, &manifest_b, &corpora_root).unwrap();

    assert_eq!(report_a.generated, report_b.generated);
    assert!(report_a.generated > 0);

    let man_a = Manifest::load(&manifest_a).unwrap();
    let man_b = Manifest::load(&manifest_b).unwrap();
    assert_eq!(man_a.entries.len(), man_b.entries.len());

    for (ea, eb) in man_a.entries.iter().zip(man_b.entries.iter()) {
        assert_eq!(ea, eb);
        let bytes_a = fs::read(out_a.join(&ea.path)).unwrap();
        let bytes_b = fs::read(out_b.join(&eb.path)).unwrap();
        assert_eq!(bytes_a, bytes_b, "entry {} differs between runs", ea.id);
        assert_eq!(oracle::manifest::sha256_hex(&bytes_a), ea.sha256);
    }
}

#[test]
fn check_mode_detects_tampered_file() {
    let tmp = tempdir();
    let corpora_root = empty_corpora_root(&tmp);
    let out = tmp.join("generated");
    let manifest_path = tmp.join("MANIFEST.json");

    write_manifest(CorpusTier::Quick, &out, &manifest_path, &corpora_root).unwrap();

    let manifest = Manifest::load(&manifest_path).unwrap();
    let entry = manifest.entries.first().expect("at least one entry");
    let path = out.join(&entry.path);
    fs::write(&path, b"tampered bytes, not what deflate produced").unwrap();

    let report = check(CorpusTier::Quick, &out, &manifest_path, &corpora_root).unwrap();
    assert!(
        report
            .problems
            .iter()
            .any(|p| p.contains(&entry.id) && p.contains("sha256 mismatch")),
        "expected a sha256 mismatch problem for {}, got {:?}",
        entry.id,
        report.problems
    );
}

#[test]
fn check_mode_detects_missing_manifest_entry() {
    let tmp = tempdir();
    let corpora_root = empty_corpora_root(&tmp);
    let out = tmp.join("generated");
    let manifest_path = tmp.join("MANIFEST.json");

    write_manifest(CorpusTier::Quick, &out, &manifest_path, &corpora_root).unwrap();

    let mut manifest = Manifest::load(&manifest_path).unwrap();
    let removed_id = manifest.entries.remove(0).id;
    manifest.save(&manifest_path).unwrap();

    let report = check(CorpusTier::Quick, &out, &manifest_path, &corpora_root).unwrap();
    assert!(
        report
            .problems
            .iter()
            .any(|p| p.contains(&removed_id) && p.contains("missing manifest entry")),
        "expected a missing manifest entry problem for {removed_id}, got {:?}",
        report.problems
    );
}

// Minimal temp-dir helper (avoids adding a `tempfile` dev-dependency for
// four tests): a process-unique directory under the target dir, removed on
// drop.
struct TempDir(PathBuf);

impl std::ops::Deref for TempDir {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn tempdir() -> TempDir {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("oracle-manifest-test-{pid}-{n}"));
    fs::create_dir_all(&dir).unwrap();
    TempDir(dir)
}

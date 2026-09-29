//! Manifest-driven conformance harness (SPEC S4, S5 M1+). Reads generated
//! corpus files, verifies them against `testdata/MANIFEST.json`, decodes
//! them with a caller-supplied decode function, and compares the decoded
//! output's SHA-256 against the manifest's recorded `input_sha256`.
//!
//! C13 applies here: a missing corpus file, a missing manifest, or a
//! filter that selects nothing are failures, never silent skips.

use std::path::{Path, PathBuf};

use cleanflate::{Decompressor, Status};

use crate::inputs::generate_synthetic;
use crate::manifest::{sha256_hex, Manifest, ManifestEntry};

/// Feed `bytes` into `decompressor` in `input_chunk`-sized pieces, reading
/// into an `output_chunk`-sized buffer, looping `decompress` calls
/// (including empty-input calls, since a chunk boundary can land inside
/// already-buffered bits that still need a further call to flush) until
/// [`Status::Done`], or until all input has been fed and a call makes no
/// further progress -- gzip in particular can sit in a "boundary" state
/// after its last member's trailer, correctly reporting `NeedsInput`
/// rather than `Done` since another member could still follow; only
/// `finish()` (called unconditionally afterwards) resolves that. Used to
/// prove chunking invariance against odd chunk sizes that don't divide
/// the corpus (SPEC S3.3, S5 M2/M3).
pub fn decode_streaming(
    decompressor: &mut Decompressor,
    bytes: &[u8],
    input_chunk: usize,
    output_chunk: usize,
) -> Result<Vec<u8>, String> {
    let mut out = vec![0u8; output_chunk.max(1)];
    let mut result = Vec::new();
    let mut pos = 0usize;

    loop {
        let end = pos.saturating_add(input_chunk).min(bytes.len());
        let chunk = bytes.get(pos..end).unwrap_or(&[]);
        let progress = decompressor
            .decompress(chunk, &mut out)
            .map_err(|e| format!("{e:?}"))?;
        pos += progress.consumed;
        result.extend_from_slice(out.get(..progress.produced).unwrap_or(&[]));

        if progress.status == Status::Done {
            break;
        }
        if progress.consumed == 0 && progress.produced == 0 && pos >= bytes.len() {
            // No more input, and this call made no progress: let finish()
            // decide whether this is a valid stopping point.
            break;
        }
    }

    decompressor.finish().map_err(|e| format!("{e:?}"))?;
    Ok(result)
}

/// `testdata/`, resolved from `CARGO_MANIFEST_DIR`, never the process cwd
/// (so `cargo test` works regardless of the directory it's invoked from).
pub fn testdata_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("testdata")
}

/// The repository root (`testdata/`'s parent), for resolving a manifest
/// entry's `input`/`dictionary.input` fields, which are recorded relative
/// to it.
pub fn repo_root() -> PathBuf {
    testdata_root().join("..")
}

/// Load `testdata/MANIFEST.json`. Panics (fails the test, not a silent
/// skip) if it is missing or unparsable -- run `make corpus` first.
pub fn load_manifest() -> Manifest {
    let path = testdata_root().join("MANIFEST.json");
    Manifest::load(&path).unwrap_or_else(|e| {
        panic!(
            "oracle conformance: loading {}: {e} -- run `make corpus`",
            path.display()
        )
    })
}

/// Which tiers `CLEANFLATE_TIER` (`quick` default, or `full`) admits.
fn tier_allowlist() -> (&'static str, &'static [&'static str]) {
    match std::env::var("CLEANFLATE_TIER").ok().as_deref() {
        Some("full") => ("full", &["synthetic", "canterbury", "silesia"]),
        _ => ("quick", &["synthetic", "canterbury"]),
    }
}

/// Manifest entries matching `filter`, limited to the tier named by the
/// `CLEANFLATE_TIER` env var (`quick` default, `full` also admits
/// Silesia), sorted by id. Panics if this selects zero entries -- an
/// empty selection means the filter or the manifest is wrong, not that
/// there is nothing to test.
pub fn selected<F>(filter: F) -> Vec<ManifestEntry>
where
    F: Fn(&ManifestEntry) -> bool,
{
    let manifest = load_manifest();
    let (tier_name, allowed) = tier_allowlist();
    let mut out: Vec<ManifestEntry> = manifest
        .entries
        .into_iter()
        .filter(|e| allowed.contains(&e.tier.as_str()) && filter(e))
        .collect();
    if out.is_empty() {
        panic!(
            "oracle conformance: filter selected zero entries (tier={tier_name}) -- \
             this is a failure, not an empty pass"
        );
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// One failed entry: which id, and either the error `decode_fn` returned
/// or the first differing byte offset against the manifest's recorded
/// input.
#[derive(Debug, Clone)]
pub struct Failure {
    /// The failing entry's manifest id.
    pub id: String,
    /// Human-readable detail.
    pub detail: String,
}

/// The outcome of one `check_entries` run.
#[derive(Debug, Clone, Default)]
pub struct Report {
    /// Total entries checked (pass + fail).
    pub checked: usize,
    /// Every failure, collected rather than stopping at the first.
    pub failures: Vec<Failure>,
}

impl Report {
    /// Panics with every failure listed if any entry failed.
    pub fn assert_ok(&self) {
        if self.failures.is_empty() {
            return;
        }
        let mut msg = format!(
            "{} of {} conformance entries failed:\n",
            self.failures.len(),
            self.checked
        );
        for f in &self.failures {
            msg.push_str(&format!("  {}: {}\n", f.id, f.detail));
        }
        panic!("{msg}");
    }
}

fn original_bytes(entry: &ManifestEntry, root: &Path) -> Result<Vec<u8>, String> {
    if let Some(id) = entry.input.strip_prefix("synthetic:") {
        Ok(generate_synthetic(id))
    } else {
        std::fs::read(root.join(&entry.input))
            .map_err(|e| format!("reading input {}: {e}", entry.input))
    }
}

/// Read the first `dictionary.len` bytes of the file named by a manifest
/// entry's `dictionary.input` field (relative to the repo root). Used to
/// reconstruct the exact preset dictionary a zlib entry was generated
/// with.
pub fn dictionary_bytes(dictionary: &crate::manifest::DictionaryRecord) -> Result<Vec<u8>, String> {
    let root = repo_root();
    let bytes = std::fs::read(root.join(&dictionary.input))
        .map_err(|e| format!("reading dictionary {}: {e}", dictionary.input))?;
    let len = dictionary.len.min(bytes.len());
    Ok(bytes.get(..len).unwrap_or(&bytes).to_vec())
}

fn first_diff_offset(a: &[u8], b: &[u8]) -> usize {
    a.iter()
        .zip(b.iter())
        .position(|(x, y)| x != y)
        .unwrap_or_else(|| a.len().min(b.len()))
}

/// For each of `entries`: read its generated file, verify its sha256
/// against the manifest (a mismatch or missing file fails with a message
/// telling the operator to run `make corpus`), decode it with
/// `decode_fn` (given the entry itself, so the caller can pick a format
/// or supply a dictionary per entry), and compare the decoded output's
/// sha256 against the manifest's `input_sha256`. Collects every failure
/// instead of stopping at the first.
pub fn check_entries<F>(entries: &[ManifestEntry], decode_fn: F) -> Report
where
    F: Fn(&ManifestEntry, &[u8]) -> Result<Vec<u8>, String>,
{
    let testdata = testdata_root();
    let root = repo_root();
    let mut report = Report::default();

    for entry in entries {
        report.checked += 1;
        let path = testdata.join("generated").join(&entry.path);

        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                report.failures.push(Failure {
                    id: entry.id.clone(),
                    detail: format!("missing generated file ({e}): run make corpus"),
                });
                continue;
            }
        };

        let actual_sha = sha256_hex(&bytes);
        if actual_sha != entry.sha256 {
            report.failures.push(Failure {
                id: entry.id.clone(),
                detail: format!(
                    "generated file sha256 mismatch (manifest {}, disk {actual_sha}): run make corpus",
                    entry.sha256
                ),
            });
            continue;
        }

        let decoded = match decode_fn(entry, &bytes) {
            Ok(d) => d,
            Err(e) => {
                report.failures.push(Failure {
                    id: entry.id.clone(),
                    detail: format!("decode error: {e}"),
                });
                continue;
            }
        };

        let decoded_sha = sha256_hex(&decoded);
        if decoded_sha != entry.input_sha256 {
            let detail = match original_bytes(entry, &root) {
                Ok(original) => {
                    let offset = first_diff_offset(&decoded, &original);
                    format!(
                        "output sha256 mismatch (decoded {} bytes, expected {} bytes), first differing offset {offset}",
                        decoded.len(),
                        original.len()
                    )
                }
                Err(e) => format!(
                    "output sha256 mismatch, and could not re-read the original input to \
                     locate the offset: {e}"
                ),
            };
            report.failures.push(Failure {
                id: entry.id.clone(),
                detail,
            });
        }
    }

    report
}

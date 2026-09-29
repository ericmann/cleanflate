//! `testdata/MANIFEST.json` format: parameters, input source and SHA-256
//! for every generated conformance corpus entry (SPEC S4).

use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ffi::{Framing, Strategy};
use crate::matrix::CORPUS_SEED;

/// A preset dictionary source recorded on a manifest entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DictionaryRecord {
    /// The dictionary's own source, same convention as `input`.
    pub input: String,
    /// Dictionary length in bytes.
    pub len: usize,
    /// SHA-256 of the dictionary bytes, hex-encoded.
    pub sha256: String,
}

/// One generated corpus entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    /// Unique id (matches `EntrySpec::id`).
    pub id: String,
    /// `synthetic`, `canterbury` or `silesia`.
    pub tier: String,
    /// `synthetic:<id>` or `testdata/corpora/<dir>/<name>`.
    pub input: String,
    /// SHA-256 of the input bytes, hex-encoded.
    pub input_sha256: String,
    /// Input length in bytes.
    pub input_len: usize,
    /// `raw`, `zlib` or `gzip`.
    pub framing: String,
    /// Compression level, 0..=9.
    pub level: u8,
    /// Compression strategy name.
    pub strategy: String,
    /// zlib windowBits magnitude, 9..=15.
    pub window_bits: u8,
    /// zlib memLevel, 1..=9.
    pub mem_level: u8,
    /// Preset dictionary, if any.
    pub dictionary: Option<DictionaryRecord>,
    /// Path relative to `testdata/generated/`.
    pub path: String,
    /// SHA-256 of the generated output bytes, hex-encoded.
    pub sha256: String,
    /// Generated output length in bytes.
    pub len: usize,
}

/// The full manifest: `testdata/MANIFEST.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Manifest format version.
    pub format: u32,
    /// The C zlib version string the entries were generated against.
    pub zlib_version: String,
    /// Deterministic seed, hex-encoded (`CORPUS_SEED`).
    pub seed: String,
    /// Entries, sorted by `id`.
    pub entries: Vec<ManifestEntry>,
}

impl Manifest {
    /// A freshly built manifest for `zlib_version`, with `entries` already
    /// sorted by id.
    pub fn new(zlib_version: String, mut entries: Vec<ManifestEntry>) -> Self {
        entries.sort_by(|a, b| a.id.cmp(&b.id));
        Manifest {
            format: 1,
            zlib_version,
            seed: format!("{CORPUS_SEED:#018x}"),
            entries,
        }
    }

    /// Load a manifest from `path`.
    pub fn load(path: &Path) -> Result<Manifest, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("reading {}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("parsing {}: {e}", path.display()))
    }

    /// Write this manifest to `path` as pretty JSON with a trailing
    /// newline.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let mut text =
            serde_json::to_string_pretty(self).map_err(|e| format!("serializing manifest: {e}"))?;
        text.push('\n');
        std::fs::write(path, text).map_err(|e| format!("writing {}: {e}", path.display()))
    }

    /// Find an entry by id.
    pub fn find(&self, id: &str) -> Option<&ManifestEntry> {
        self.entries.iter().find(|e| e.id == id)
    }
}

/// Hex-encoded SHA-256 of `data`.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// `raw`/`zlib`/`gzip` token for a manifest entry's `framing` field.
pub fn framing_token(framing: Framing) -> &'static str {
    match framing {
        Framing::Raw => "raw",
        Framing::Zlib => "zlib",
        Framing::Gzip => "gzip",
    }
}

/// Strategy token for a manifest entry's `strategy` field.
pub fn strategy_token(strategy: Strategy) -> &'static str {
    match strategy {
        Strategy::Default => "default",
        Strategy::Filtered => "filtered",
        Strategy::HuffmanOnly => "huffman",
        Strategy::Rle => "rle",
        Strategy::Fixed => "fixed",
    }
}

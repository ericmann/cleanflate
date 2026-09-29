//! Deterministic synthetic inputs and corpus-file enumeration for the
//! conformance corpus generator (SPEC S4).

use std::fs;
use std::path::{Path, PathBuf};

use crate::matrix::CORPUS_SEED;
use crate::prng::{fnv1a64, SplitMix64};

/// Which family an input belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// Deterministically generated synthetic input.
    Synthetic,
    /// A file from the Canterbury corpus.
    Canterbury,
    /// A file from the Silesia corpus.
    Silesia,
}

enum Source {
    Synthetic,
    File(PathBuf),
}

/// One input the conformance corpus is generated from.
pub struct Input {
    /// Stable identifier, used to build entry ids and output paths.
    pub id: String,
    /// Which family this input belongs to.
    pub tier: Tier,
    /// Byte length, known without reading synthetic content or without
    /// reading (only stat-ing) corpus files.
    pub len: usize,
    source: Source,
}

impl Input {
    /// Materialize this input's bytes.
    pub fn bytes(&self) -> Vec<u8> {
        match &self.source {
            Source::Synthetic => generate_synthetic(&self.id),
            Source::File(path) => fs::read(path).unwrap_or_else(|e| {
                panic!("oracle: failed to read corpus file {}: {e}", path.display())
            }),
        }
    }

    /// The manifest `input` field: `synthetic:<id>` for a synthetic input,
    /// `testdata/corpora/<dir>/<name>` for a corpus file.
    pub fn manifest_source(&self) -> String {
        match self.tier {
            Tier::Synthetic => format!("synthetic:{}", self.id),
            Tier::Canterbury => format!(
                "testdata/corpora/canterbury/{}",
                self.id.strip_prefix("canterbury-").unwrap_or(&self.id)
            ),
            Tier::Silesia => format!(
                "testdata/corpora/silesia/{}",
                self.id.strip_prefix("silesia-").unwrap_or(&self.id)
            ),
        }
    }
}

fn seed_for(id: &str) -> u64 {
    CORPUS_SEED ^ fnv1a64(id.as_bytes())
}

const WORDS: [&str; 64] = [
    "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel", "india", "juliet",
    "kilo", "lima", "mike", "november", "oscar", "papa", "quebec", "romeo", "sierra", "tango",
    "uniform", "victor", "whiskey", "xray", "yankee", "zulu", "ash", "birch", "cedar", "dogwood",
    "elm", "fir", "gum", "holly", "ivy", "juniper", "kapok", "larch", "maple", "nutmeg", "oak",
    "pine", "quince", "redwood", "spruce", "teak", "umbra", "vine", "willow", "yew", "aster",
    "basil", "clover", "daisy", "elder", "fern", "gorse", "heather", "iris", "jasmine", "kudzu",
    "lotus", "myrtle", "nettle",
];

fn window_input(d: usize, rng: &mut SplitMix64) -> Vec<u8> {
    let r = rng.bytes(512);
    let filler = rng.bytes(d - 512);
    let tail = rng.bytes(1024);
    let mut out = Vec::with_capacity(d + 512 + 1024);
    out.extend_from_slice(&r);
    out.extend_from_slice(&filler);
    out.extend_from_slice(&r);
    out.extend_from_slice(&tail);
    out
}

fn text_256kib(rng: &mut SplitMix64) -> Vec<u8> {
    let target = 256 * 1024;
    let mut out = Vec::with_capacity(target + 32);
    while out.len() < target {
        let idx = (rng.next_u64() as usize) % WORDS.len();
        out.extend_from_slice(WORDS.get(idx).copied().unwrap_or("alpha").as_bytes());
        let sep = if rng.next_u64() % 8 == 0 { b'\n' } else { b' ' };
        out.push(sep);
    }
    out.truncate(target);
    out
}

/// Generate the deterministic content for a synthetic input id. Panics on
/// an unknown id -- this is oracle-internal test scaffolding, not the
/// cleanflate library, so a panic here (a programmer error, not hostile
/// input) is acceptable.
pub fn generate_synthetic(id: &str) -> Vec<u8> {
    let mut rng = SplitMix64::new(seed_for(id));
    match id {
        "empty" => Vec::new(),
        "one-byte" => vec![0x61],
        "zeros-1KiB" => vec![0u8; 1024],
        "zeros-64KiB" => vec![0u8; 64 * 1024],
        "zeros-1MiB" => vec![0u8; 1024 * 1024],
        "zeros-16MiB" => vec![0u8; 16 * 1024 * 1024],
        "random-64KiB" => rng.bytes(64 * 1024),
        "random-1MiB" => rng.bytes(1024 * 1024),
        "text-256KiB" => text_256kib(&mut rng),
        "repeat-abc-1MiB" => {
            let pattern = b"abc";
            let target = 1024 * 1024;
            let mut out = Vec::with_capacity(target);
            while out.len() < target {
                out.extend_from_slice(pattern);
            }
            out.truncate(target);
            out
        }
        "repeat-period17-1MiB" => {
            let pattern = rng.bytes(17);
            let target = 1024 * 1024;
            let mut out = Vec::with_capacity(target + 17);
            while out.len() < target {
                out.extend_from_slice(&pattern);
            }
            out.truncate(target);
            out
        }
        "window-d32767" => window_input(32767, &mut rng),
        "window-d32768" => window_input(32768, &mut rng),
        "window-d32769" => window_input(32769, &mut rng),
        "run-64KiB" => {
            let mut out = Vec::with_capacity(1 + 65536);
            out.extend_from_slice(&rng.bytes(1));
            out.extend(std::iter::repeat_n(0x42u8, 65536));
            out
        }
        "period258-64KiB" => {
            let block = rng.bytes(258);
            let mut out = Vec::with_capacity(258 * 254);
            for _ in 0..254 {
                out.extend_from_slice(&block);
            }
            out
        }
        other => panic!("oracle: unknown synthetic input id: {other}"),
    }
}

/// The ordered list of synthetic input ids (SPEC S4).
pub const SYNTHETIC_IDS: [&str; 16] = [
    "empty",
    "one-byte",
    "zeros-1KiB",
    "zeros-64KiB",
    "zeros-1MiB",
    "zeros-16MiB",
    "random-64KiB",
    "random-1MiB",
    "text-256KiB",
    "repeat-abc-1MiB",
    "repeat-period17-1MiB",
    "window-d32767",
    "window-d32768",
    "window-d32769",
    "run-64KiB",
    "period258-64KiB",
    // NOTE: array length below must match; see debug_assert in tests if
    // this list is edited.
];

/// Build the full list of synthetic inputs with their lengths precomputed.
pub fn synthetic_inputs() -> Vec<Input> {
    SYNTHETIC_IDS
        .iter()
        .map(|&id| {
            let bytes = generate_synthetic(id);
            Input {
                id: id.to_string(),
                tier: Tier::Synthetic,
                len: bytes.len(),
                source: Source::Synthetic,
            }
        })
        .collect()
}

fn corpus_inputs(dir: &Path, tier: Tier, prefix: &str) -> Vec<Input> {
    let mut names: Vec<String> = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
            .filter_map(|e| e.file_name().into_string().ok())
            .collect(),
        Err(_) => return Vec::new(),
    };
    names.sort();

    names
        .into_iter()
        .filter_map(|name| {
            let path = dir.join(&name);
            let len = fs::metadata(&path).ok()?.len() as usize;
            Some(Input {
                id: format!("{prefix}-{name}"),
                tier,
                len,
                source: Source::File(path),
            })
        })
        .collect()
}

/// Enumerate every regular file in `testdata/corpora/canterbury/` under
/// `corpora_root`, sorted by name.
pub fn canterbury_inputs(corpora_root: &Path) -> Vec<Input> {
    corpus_inputs(
        &corpora_root.join("canterbury"),
        Tier::Canterbury,
        "canterbury",
    )
}

/// Enumerate every regular file in `testdata/corpora/silesia/` under
/// `corpora_root`, sorted by name.
pub fn silesia_inputs(corpora_root: &Path) -> Vec<Input> {
    corpus_inputs(&corpora_root.join("silesia"), Tier::Silesia, "silesia")
}

/// Find a specific Canterbury file by name (e.g. `lcet10.txt`), used for
/// the dictionary matrix entries. Returns `None` if the corpus or file is
/// absent.
pub fn find_canterbury_file(corpora_root: &Path, name: &str) -> Option<PathBuf> {
    let path = corpora_root.join("canterbury").join(name);
    if path.is_file() {
        Some(path)
    } else {
        None
    }
}

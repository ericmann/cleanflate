//! `oracle perf-prep`: prepare compressed inputs for cleanflate-only
//! profiling (SPEC S4.3). This step runs C zlib and is never profiled;
//! the profiled example only reads the files it writes.

use std::path::{Path, PathBuf};

use crate::ffi::{crc32, deflate_bytes, DeflateParams, Framing, Strategy};
use crate::inputs::{canterbury_inputs, silesia_inputs};

/// One row of `index.tsv`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexRow {
    /// File name (`<name>.bin` under the input directory).
    pub file: String,
    /// Compressed length in bytes.
    pub compressed_len: usize,
    /// Original length in bytes.
    pub original_len: usize,
    /// CRC-32 of the original bytes (from C zlib).
    pub original_crc32: u32,
}

const INDEX_HEADER: &str = "file\tcompressed_len\toriginal_len\toriginal_crc32\n";

/// Render `index.tsv`.
pub fn render_index(rows: &[IndexRow]) -> String {
    let mut s = INDEX_HEADER.to_string();
    for r in rows {
        s.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            r.file, r.compressed_len, r.original_len, r.original_crc32
        ));
    }
    s
}

/// Parse `index.tsv`.
pub fn parse_index(text: &str) -> Result<Vec<IndexRow>, String> {
    let mut rows = Vec::new();
    for (n, line) in text.lines().enumerate().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        let [file, clen, olen, crc] = cols.as_slice() else {
            return Err(format!("index line {}: expected 4 columns", n + 1));
        };
        let num = |s: &str| {
            s.parse::<u64>()
                .map_err(|e| format!("index line {}: {e}", n + 1))
        };
        rows.push(IndexRow {
            file: (*file).to_string(),
            compressed_len: num(clen)? as usize,
            original_len: num(olen)? as usize,
            original_crc32: num(crc)? as u32,
        });
    }
    Ok(rows)
}

/// Framing name (`raw|zlib|gzip|rfc1951|rfc1950|rfc1952`) to the
/// framing and its directory prefix.
pub fn framing_alias(name: &str) -> Result<(Framing, &'static str), String> {
    match name {
        "raw" | "rfc1951" => Ok((Framing::Raw, "rfc1951")),
        "zlib" | "rfc1950" => Ok((Framing::Zlib, "rfc1950")),
        "gzip" | "rfc1952" => Ok((Framing::Gzip, "rfc1952")),
        other => Err(format!(
            "unknown framing {other:?} (rfc1951|rfc1950|rfc1952 or raw|zlib|gzip)"
        )),
    }
}

/// `perf-prep` arguments.
#[derive(Debug, Clone)]
pub struct PrepArgs {
    /// `silesia` or `canterbury`.
    pub corpus: String,
    /// Framing.
    pub framing: Framing,
    /// Directory prefix of the framing.
    pub framing_dir: &'static str,
    /// Compression level.
    pub level: u8,
    /// Output root.
    pub out: PathBuf,
}

/// Parse `perf-prep` arguments.
pub fn parse_args(args: &[String]) -> Result<PrepArgs, String> {
    let mut corpus = None;
    let mut framing = None;
    let mut level = 6u8;
    let mut out = PathBuf::from("testdata/perf-inputs");
    let mut i = 0;
    while i < args.len() {
        let flag = args.get(i).map(String::as_str).unwrap_or("");
        let value = args
            .get(i + 1)
            .ok_or_else(|| format!("{flag} requires a value"))?;
        match flag {
            "--corpus" => corpus = Some(value.clone()),
            "--framing" => framing = Some(framing_alias(value)?),
            "--level" => {
                level = value
                    .parse::<u8>()
                    .ok()
                    .filter(|l| *l <= 9)
                    .ok_or("--level must be 0..=9")?
            }
            "--out" => out = PathBuf::from(value),
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 2;
    }
    let corpus = corpus.ok_or("--corpus silesia|canterbury is required")?;
    if corpus != "silesia" && corpus != "canterbury" {
        return Err(format!("unknown corpus {corpus:?}"));
    }
    let (framing, framing_dir) = framing.ok_or("--framing is required")?;
    Ok(PrepArgs {
        corpus,
        framing,
        framing_dir,
        level,
        out,
    })
}

/// True if `dir/index.tsv` exists and every listed file has its length.
fn up_to_date(dir: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(dir.join("index.tsv")) else {
        return false;
    };
    let Ok(rows) = parse_index(&text) else {
        return false;
    };
    !rows.is_empty()
        && rows.iter().all(|r| {
            std::fs::metadata(dir.join(&r.file))
                .map(|m| m.len() as usize == r.compressed_len)
                .unwrap_or(false)
        })
}

/// Run `perf-prep`; returns the output directory.
pub fn run(args: &PrepArgs, corpora_root: &Path) -> Result<PathBuf, String> {
    let dir = args
        .out
        .join(&args.corpus)
        .join(format!("{}-l{}", args.framing_dir, args.level));
    if up_to_date(&dir) {
        return Ok(dir);
    }
    let inputs = if args.corpus == "silesia" {
        silesia_inputs(corpora_root)
    } else {
        canterbury_inputs(corpora_root)
    };
    if inputs.is_empty() {
        return Err(format!(
            "no {} files under {} -- see testdata/corpora/README.md to fetch them",
            args.corpus,
            corpora_root.display()
        ));
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    let mut rows = Vec::new();
    for input in &inputs {
        let original = input.bytes();
        let params = DeflateParams {
            level: args.level,
            strategy: Strategy::Default,
            window_bits: 15,
            mem_level: 8,
            framing: args.framing,
        };
        let compressed = deflate_bytes(&original, &params, None)
            .map_err(|e| format!("{}: oracle deflate failed: {e:?}", input.id))?;
        let name = input
            .id
            .split_once('-')
            .map_or(input.id.as_str(), |(_, f)| f);
        let file = format!("{name}.bin");
        std::fs::write(dir.join(&file), &compressed).map_err(|e| format!("writing {file}: {e}"))?;
        rows.push(IndexRow {
            file,
            compressed_len: compressed.len(),
            original_len: original.len(),
            original_crc32: crc32(&original),
        });
    }
    std::fs::write(dir.join("index.tsv"), render_index(&rows))
        .map_err(|e| format!("writing index.tsv: {e}"))?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_tsv_roundtrip() {
        let rows = vec![
            IndexRow {
                file: "a.bin".into(),
                compressed_len: 10,
                original_len: 100,
                original_crc32: 0xDEAD_BEEF,
            },
            IndexRow {
                file: "b.bin".into(),
                compressed_len: 0,
                original_len: 0,
                original_crc32: 0,
            },
        ];
        assert_eq!(parse_index(&render_index(&rows)).expect("parses"), rows);
        assert!(parse_index("h\nonly\ttwo\n").is_err());
    }

    #[test]
    fn framing_aliases() {
        for (name, dir) in [
            ("raw", "rfc1951"),
            ("rfc1951", "rfc1951"),
            ("zlib", "rfc1950"),
            ("rfc1950", "rfc1950"),
            ("gzip", "rfc1952"),
            ("rfc1952", "rfc1952"),
        ] {
            assert_eq!(framing_alias(name).expect("known").1, dir);
        }
        assert!(framing_alias("deflate").is_err());
    }
}

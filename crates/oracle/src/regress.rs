//! Committed regression fixtures (`testdata/regressions/`): a manifest of
//! repro inputs with their expected result and chunk schedules, plus the
//! `regress-add` and `regress-seed` commands that maintain it. Expected
//! output hashes come from C zlib through `crate::ffi`, never from
//! cleanflate (CLAUDE.md C16).

use std::path::{Path, PathBuf};

use crate::ffi::{
    deflate_bytes, inflate_bytes, inflate_gzip_members, DeflateParams, Framing, OracleStatus,
    Strategy,
};
use crate::manifest::sha256_hex;
use crate::prng::SplitMix64;

/// Output limit used when computing expected output (far above any
/// fixture).
const EXPECT_MAX_OUTPUT: usize = 256 << 20;

const MANIFEST_HEADER: &str =
    "# Regression fixtures. One row per committed repro; see README.md.\n\
# path\tformat\texpect\tout_len\tout_sha256\tin_chunks\tout_chunks\torigin\n";

/// One manifest row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Path relative to `testdata/regressions/`.
    pub path: String,
    /// `raw`, `zlib` or `gzip`.
    pub format: String,
    /// `ok` or `err:<ErrorKind name>`.
    pub expect: String,
    /// Expected output length (`ok` rows).
    pub out_len: Option<usize>,
    /// Expected output SHA-256 (`ok` rows).
    pub out_sha256: Option<String>,
    /// Input chunk schedule, cycled.
    pub in_chunks: Option<Vec<usize>>,
    /// Output chunk schedule, cycled.
    pub out_chunks: Option<Vec<usize>>,
    /// Where the fixture came from.
    pub origin: String,
}

/// `testdata/regressions/`.
pub fn regressions_dir() -> PathBuf {
    crate::conformance::testdata_root().join("regressions")
}

/// The corpus subdirectory used for a format name.
pub fn dir_for_format(format: &str) -> Result<&'static str, String> {
    match format {
        "raw" => Ok("rfc1951"),
        "zlib" => Ok("rfc1950"),
        "gzip" => Ok("rfc1952"),
        other => Err(format!("unknown format {other:?} (raw|zlib|gzip)")),
    }
}

/// Parse a comma-separated list of positive integers, or `-` for none.
pub fn parse_chunks(text: &str) -> Result<Option<Vec<usize>>, String> {
    if text == "-" {
        return Ok(None);
    }
    let mut out = Vec::new();
    for part in text.split(',') {
        let n: usize = part
            .trim()
            .parse()
            .map_err(|e| format!("bad chunk size {part:?}: {e}"))?;
        if n == 0 {
            return Err("chunk sizes must be positive".to_string());
        }
        out.push(n);
    }
    Ok(Some(out))
}

fn render_chunks(c: &Option<Vec<usize>>) -> String {
    match c {
        None => "-".to_string(),
        Some(v) => v.iter().map(usize::to_string).collect::<Vec<_>>().join(","),
    }
}

/// Parse `MANIFEST.tsv` text.
pub fn parse_manifest(text: &str) -> Result<Vec<Row>, String> {
    let mut rows = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 8 {
            return Err(format!(
                "line {}: expected 8 columns, got {}",
                n + 1,
                cols.len()
            ));
        }
        let col = |i: usize| cols.get(i).copied().unwrap_or("");
        let opt = |s: &str| if s == "-" { None } else { Some(s.to_string()) };
        let out_len = match col(3) {
            "-" => None,
            s => Some(
                s.parse::<usize>()
                    .map_err(|e| format!("line {}: bad out_len: {e}", n + 1))?,
            ),
        };
        rows.push(Row {
            path: col(0).to_string(),
            format: col(1).to_string(),
            expect: col(2).to_string(),
            out_len,
            out_sha256: opt(col(4)),
            in_chunks: parse_chunks(col(5)).map_err(|e| format!("line {}: {e}", n + 1))?,
            out_chunks: parse_chunks(col(6)).map_err(|e| format!("line {}: {e}", n + 1))?,
            origin: col(7).to_string(),
        });
    }
    Ok(rows)
}

/// Render rows as `MANIFEST.tsv` text.
pub fn render_manifest(rows: &[Row]) -> String {
    let mut s = MANIFEST_HEADER.to_string();
    for r in rows {
        s.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            r.path,
            r.format,
            r.expect,
            r.out_len.map_or("-".to_string(), |n| n.to_string()),
            r.out_sha256.clone().unwrap_or_else(|| "-".to_string()),
            render_chunks(&r.in_chunks),
            render_chunks(&r.out_chunks),
            r.origin
        ));
    }
    s
}

/// Load the committed manifest. Fails (never skips) if it is missing or
/// has no rows.
pub fn load_rows() -> Result<Vec<Row>, String> {
    let path = regressions_dir().join("MANIFEST.tsv");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("reading {}: {e} -- run `make regress-seed`", path.display()))?;
    let rows = parse_manifest(&text)?;
    if rows.is_empty() {
        return Err(format!(
            "{} has zero rows -- run `make regress-seed`",
            path.display()
        ));
    }
    Ok(rows)
}

fn framing_of(format: &str) -> Result<Framing, String> {
    match format {
        "raw" => Ok(Framing::Raw),
        "zlib" => Ok(Framing::Zlib),
        "gzip" => Ok(Framing::Gzip),
        other => Err(format!("unknown format {other:?}")),
    }
}

/// Decode `input` with C zlib; requires a clean, complete decode that
/// consumes all of `input`.
fn oracle_expected(format: &str, input: &[u8]) -> Result<Vec<u8>, String> {
    let framing = framing_of(format)?;
    let outcome = if framing == Framing::Gzip {
        inflate_gzip_members(input, EXPECT_MAX_OUTPUT)
    } else {
        inflate_bytes(input, framing, None, EXPECT_MAX_OUTPUT)
    };
    if outcome.status != OracleStatus::StreamEnd || outcome.consumed != input.len() {
        return Err(format!(
            "the oracle does not decode this input cleanly (status {:?}, consumed {} of {}); \
             use --expect err:<Kind> for an input that must be rejected",
            outcome.status,
            outcome.consumed,
            input.len()
        ));
    }
    Ok(outcome.output)
}

/// Arguments of `oracle regress-add`.
#[derive(Debug, Clone)]
pub struct AddArgs {
    /// Input file to copy in.
    pub input: PathBuf,
    /// `raw`, `zlib` or `gzip`.
    pub format: String,
    /// Fixture name (file stem).
    pub name: String,
    /// Input chunk schedule.
    pub in_chunks: Option<Vec<usize>>,
    /// Output chunk schedule.
    pub out_chunks: Option<Vec<usize>>,
    /// `ok` or `err:<Kind>`.
    pub expect: String,
    /// Origin note.
    pub origin: String,
}

/// Parse `regress-add` arguments.
pub fn parse_add_args(args: &[String]) -> Result<AddArgs, String> {
    let mut input = None;
    let mut format = None;
    let mut name = None;
    let mut in_chunks = None;
    let mut out_chunks = None;
    let mut expect = "ok".to_string();
    let mut origin = None;
    let mut i = 0;
    while i < args.len() {
        let flag = args.get(i).map(String::as_str).unwrap_or("");
        let value = args
            .get(i + 1)
            .ok_or_else(|| format!("{flag} requires a value"))?
            .clone();
        match flag {
            "--input" => input = Some(PathBuf::from(value)),
            "--format" => format = Some(value),
            "--name" => name = Some(value),
            "--in-chunks" => in_chunks = parse_chunks(&value)?,
            "--out-chunks" => out_chunks = parse_chunks(&value)?,
            "--expect" => expect = value,
            "--origin" => origin = Some(value),
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 2;
    }
    if expect != "ok" && !expect.starts_with("err:") {
        return Err("--expect must be ok or err:<Kind>".to_string());
    }
    Ok(AddArgs {
        input: input.ok_or("--input is required")?,
        format: format.ok_or("--format is required")?,
        name: name.ok_or("--name is required")?,
        in_chunks,
        out_chunks,
        expect,
        origin: origin.ok_or("--origin is required")?,
    })
}

fn read_rows_if_any(root: &Path) -> Result<Vec<Row>, String> {
    let path = root.join("MANIFEST.tsv");
    match std::fs::read_to_string(&path) {
        Ok(t) => parse_manifest(&t),
        Err(_) => Ok(Vec::new()),
    }
}

/// Add one fixture to `root` (normally `testdata/regressions/`).
pub fn add_to(root: &Path, args: &AddArgs, bytes: &[u8]) -> Result<(), String> {
    if args.name.is_empty() || args.name.contains(['/', '\\', '\t']) {
        return Err("--name must be a plain file stem".to_string());
    }
    let dir = dir_for_format(&args.format)?;
    let rel = format!("{dir}/{}.bin", args.name);
    let mut rows = read_rows_if_any(root)?;
    if rows.iter().any(|r| r.path == rel) {
        return Err(format!("fixture {rel} already exists"));
    }
    let (out_len, out_sha256) = if args.expect == "ok" {
        let out = oracle_expected(&args.format, bytes)?;
        (Some(out.len()), Some(sha256_hex(&out)))
    } else {
        (None, None)
    };
    let full = root.join(&rel);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("creating {}: {e}", parent.display()))?;
    }
    std::fs::write(&full, bytes).map_err(|e| format!("writing {}: {e}", full.display()))?;
    rows.push(Row {
        path: rel,
        format: args.format.clone(),
        expect: args.expect.clone(),
        out_len,
        out_sha256,
        in_chunks: args.in_chunks.clone(),
        out_chunks: args.out_chunks.clone(),
        origin: args.origin.clone(),
    });
    std::fs::write(root.join("MANIFEST.tsv"), render_manifest(&rows))
        .map_err(|e| format!("writing manifest: {e}"))
}

/// `oracle regress-add`.
pub fn run_add(args: &[String]) -> Result<(), String> {
    let parsed = parse_add_args(args)?;
    let bytes = std::fs::read(&parsed.input)
        .map_err(|e| format!("reading {}: {e}", parsed.input.display()))?;
    add_to(&regressions_dir(), &parsed, &bytes)?;
    println!("oracle regress-add: added {}", parsed.name);
    Ok(())
}

/// One deterministic seed fixture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedFixture {
    /// Fixture name.
    pub name: &'static str,
    /// `raw`, `zlib` or `gzip`.
    pub format: &'static str,
    /// The compressed input bytes.
    pub bytes: Vec<u8>,
    /// Input chunks.
    pub in_chunks: Vec<usize>,
    /// Output chunks.
    pub out_chunks: Vec<usize>,
    /// Origin note.
    pub origin: &'static str,
}

const SEED: u64 = 0x005E_ED0F_0A11_C0DE;

/// Short repeated patterns with random literals between them.
fn match_heavy(rng: &mut SplitMix64, len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len + 512);
    while out.len() < len {
        let plen = 1 + (rng.next_u64() % 8) as usize;
        let mut pat = vec![0u8; plen];
        rng.fill_bytes(&mut pat);
        let reps = 2 + (rng.next_u64() % 39) as usize;
        for _ in 0..reps {
            out.extend_from_slice(&pat);
        }
        let lits = 1 + (rng.next_u64() % 6) as usize;
        let mut buf = vec![0u8; lits];
        rng.fill_bytes(&mut buf);
        out.extend_from_slice(&buf);
    }
    out.truncate(len);
    out
}

const WORDS: [&str; 24] = [
    "the", "of", "and", "stream", "window", "block", "literal", "length", "distance", "code",
    "huffman", "table", "symbol", "match", "copy", "byte", "bit", "header", "trailer", "checksum",
    "dynamic", "fixed", "stored", "final",
];

/// Text-like data: random words separated by spaces and newlines.
fn text_like(rng: &mut SplitMix64, len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len + 16);
    while out.len() < len {
        let w = WORDS
            .get((rng.next_u64() % WORDS.len() as u64) as usize)
            .copied()
            .unwrap_or("x");
        out.extend_from_slice(w.as_bytes());
        out.push(if rng.next_u64() % 12 == 0 {
            b'\n'
        } else {
            b' '
        });
    }
    out.truncate(len);
    out
}

fn compress(
    data: &[u8],
    level: u8,
    strategy: Strategy,
    framing: Framing,
) -> Result<Vec<u8>, String> {
    let params = DeflateParams {
        level,
        strategy,
        window_bits: 15,
        mem_level: 8,
        framing,
    };
    deflate_bytes(data, &params, None).map_err(|e| format!("deflate failed: {e:?}"))
}

/// The four seed fixtures, built deterministically.
pub fn seed_fixtures() -> Result<Vec<SeedFixture>, String> {
    let mut rng = SplitMix64::new(SEED);
    let heavy = match_heavy(&mut rng, 11_264);
    let mut fixtures = vec![
        SeedFixture {
            name: "fixed-heavy-match-11k",
            format: "raw",
            bytes: compress(&heavy, 6, Strategy::Fixed, Framing::Raw)?,
            in_chunks: vec![1],
            out_chunks: vec![1],
            origin: "seed: shape of the lost phase-1 P4-08 repro (Fixed, ~11 KiB, match-heavy), not its bytes",
        },
        SeedFixture {
            name: "fixed-heavy-match-11k",
            format: "zlib",
            bytes: compress(&heavy, 6, Strategy::Fixed, Framing::Zlib)?,
            in_chunks: vec![7, 1, 13],
            out_chunks: vec![1, 258, 3],
            origin: "seed: shape of the lost phase-1 P4-08 repro (Fixed, ~11 KiB, match-heavy), not its bytes",
        },
    ];

    let a = text_like(&mut rng, 2_000);
    let b = match_heavy(&mut rng, 3_000);
    let mut two = compress(&a, 6, Strategy::Default, Framing::Gzip)?;
    two.extend_from_slice(&compress(&b, 9, Strategy::Default, Framing::Gzip)?);
    fixtures.push(SeedFixture {
        name: "two-members",
        format: "gzip",
        bytes: two,
        in_chunks: vec![1],
        out_chunks: vec![1],
        origin: "seed: two concatenated gzip members with different data",
    });

    let big = text_like(&mut rng, 65_536);
    fixtures.push(SeedFixture {
        name: "dynamic-l9-64k",
        format: "raw",
        bytes: compress(&big, 9, Strategy::Default, Framing::Raw)?,
        in_chunks: vec![8, 9, 15, 16, 17],
        out_chunks: vec![257, 258, 259],
        origin: "seed: 64 KiB text-like data at level 9 (dynamic blocks)",
    });
    Ok(fixtures)
}

/// Create any missing seed fixtures in `root`; existing names are kept.
pub fn seed_into(root: &Path) -> Result<usize, String> {
    let mut added = 0;
    for f in seed_fixtures()? {
        let rel = format!("{}/{}.bin", dir_for_format(f.format)?, f.name);
        if read_rows_if_any(root)?.iter().any(|r| r.path == rel) {
            continue;
        }
        let args = AddArgs {
            input: PathBuf::new(),
            format: f.format.to_string(),
            name: f.name.to_string(),
            in_chunks: Some(f.in_chunks.clone()),
            out_chunks: Some(f.out_chunks.clone()),
            expect: "ok".to_string(),
            origin: f.origin.to_string(),
        };
        add_to(root, &args, &f.bytes)?;
        added += 1;
    }
    Ok(added)
}

/// `oracle regress-seed`.
pub fn run_seed() -> Result<(), String> {
    let added = seed_into(&regressions_dir())?;
    println!("oracle regress-seed: {added} fixture(s) added");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_roundtrip() {
        let rows = vec![
            Row {
                path: "rfc1951/a.bin".into(),
                format: "raw".into(),
                expect: "ok".into(),
                out_len: Some(5),
                out_sha256: Some("ab".into()),
                in_chunks: Some(vec![1, 2]),
                out_chunks: None,
                origin: "test".into(),
            },
            Row {
                path: "rfc1952/b.bin".into(),
                format: "gzip".into(),
                expect: "err:BadHeader".into(),
                out_len: None,
                out_sha256: None,
                in_chunks: None,
                out_chunks: Some(vec![3]),
                origin: "test two".into(),
            },
        ];
        let text = render_manifest(&rows);
        assert_eq!(parse_manifest(&text).expect("parses"), rows);
    }

    #[test]
    fn chunk_list_parse_rejects_zero() {
        assert!(parse_chunks("1,0,3").is_err());
        assert!(parse_chunks("x").is_err());
        assert_eq!(parse_chunks("-").expect("dash"), None);
        assert_eq!(parse_chunks("4,5").expect("list"), Some(vec![4, 5]));
    }

    #[test]
    fn seed_is_deterministic() {
        let a = seed_fixtures().expect("seeds");
        let b = seed_fixtures().expect("seeds");
        assert_eq!(a, b);
        assert_eq!(a.len(), 4);
    }
}

//! Machine-readable bench results (`docs/perf/<task-id>.json`, schema 2;
//! documented in `docs/perf/README.md`).

use serde::{Deserialize, Serialize};

/// The schema number writers emit.
pub const SCHEMA: u32 = 2;

/// The machine a bench ran on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Machine {
    /// `uname -srm`.
    pub uname: String,
    /// CPU model name from `/proc/cpuinfo`.
    pub cpu_model: String,
    /// CPU frequency governor, or `unknown`.
    pub governor: String,
    /// `/proc/loadavg` at the start of the run.
    pub loadavg: String,
    /// `rustc --version`.
    pub rustc: String,
}

/// One (file, framing, level, mode, implementation) measurement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultRow {
    /// `silesia` or `canterbury`.
    pub corpus: String,
    /// File name within the corpus.
    pub file: String,
    /// `raw`, `zlib` or `gzip`.
    pub framing: String,
    /// Compression level of the fixture.
    pub level: u8,
    /// `oneshot` or `streaming`.
    pub mode: String,
    /// `cleanflate` or `c_zlib`.
    #[serde(rename = "impl")]
    pub implementation: String,
    /// Uncompressed length in bytes.
    pub bytes: usize,
    /// Decodes per timed sample.
    pub reps: u32,
    /// Median nanoseconds per decode.
    pub median_ns: f64,
    /// Best (fastest sample) nanoseconds per decode.
    #[serde(alias = "min_ns")]
    pub best_ns: f64,
}

fn default_c_zlib_mode() -> String {
    "oneshot".to_string()
}

/// One aggregate over the files of a (corpus, framing, level, mode).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Aggregate {
    /// `<corpus>/<framing>/l<level>/<mode>`.
    pub key: String,
    /// Total uncompressed bytes.
    pub total_bytes: u64,
    /// cleanflate MiB/s: total bytes / sum of per-file medians.
    pub cleanflate_mib_s_median: f64,
    /// cleanflate MiB/s: total bytes / sum of per-file best times.
    #[serde(alias = "cleanflate_mib_s_min")]
    pub cleanflate_mib_s_best: f64,
    /// C zlib MiB/s from medians.
    pub c_zlib_mib_s_median: f64,
    /// C zlib MiB/s from best times.
    #[serde(alias = "c_zlib_mib_s_min")]
    pub c_zlib_mib_s_best: f64,
    /// The mode of the C zlib rows used as the denominator (`oneshot` or
    /// `streaming`); schema-1 files lack it and default to `oneshot`.
    #[serde(default = "default_c_zlib_mode")]
    pub c_zlib_mode: String,
    /// cleanflate median MiB/s / C zlib median MiB/s of the same mode.
    pub ratio: f64,
}

/// Raw `stats` counters of one (file, framing, level, mode), or the
/// per-framing total when `file` is `*total*`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatsEntry {
    /// `silesia` or `canterbury`.
    pub corpus: String,
    /// File name, or `*total*`.
    pub file: String,
    /// `raw`, `zlib` or `gzip`.
    pub framing: String,
    /// Compression level of the fixture.
    pub level: u8,
    /// `oneshot` or `streaming`.
    pub mode: String,
    /// Counter name to value; histogram buckets are `<name>_<index>`.
    pub counters: std::collections::BTreeMap<String, u64>,
}

/// A complete bench result file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchJson {
    /// 2 for files written now; 1 (read-only) is also accepted on load.
    pub schema: u32,
    /// `full`, `quick` or `custom`.
    pub kind: String,
    /// The task id the run belongs to.
    pub task: String,
    /// `git rev-parse HEAD`.
    pub git_head: String,
    /// Whether the working tree had uncommitted changes.
    pub git_dirty: bool,
    /// UTC, `YYYY-MM-DDTHH:MM:SSZ`.
    pub timestamp: String,
    /// The machine.
    pub machine: Machine,
    /// The linked C zlib version string.
    pub c_zlib_version: String,
    /// Timed samples per measurement.
    pub iters: u32,
    /// Minimum duration of one timed sample.
    pub min_sample_ms: u64,
    /// Whether cleanflate was built with the `stats` feature.
    pub stats_build: bool,
    /// Per-file measurements.
    pub results: Vec<ResultRow>,
    /// Aggregates.
    pub aggregates: Vec<Aggregate>,
    /// Raw counters (only for `oracle bench --stats`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stats: Vec<StatsEntry>,
}

impl BenchJson {
    /// Parse a bench JSON, accepting schema 1 (old field names) and 2.
    pub fn parse(text: &str) -> Result<BenchJson, String> {
        let j: BenchJson = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if j.schema != 1 && j.schema != SCHEMA {
            return Err(format!(
                "unsupported bench schema {} (this build reads 1 and {SCHEMA}); regenerate the file with `make bench` or use a matching oracle build",
                j.schema
            ));
        }
        Ok(j)
    }
}

/// Format seconds since the Unix epoch as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn format_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil-from-days (proleptic Gregorian), days since 1970-01-01.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    if month <= 2 {
        year += 1;
    }
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> BenchJson {
        BenchJson {
            schema: SCHEMA,
            kind: "quick".into(),
            task: "T".into(),
            git_head: "abc".into(),
            git_dirty: false,
            timestamp: format_utc(0),
            machine: Machine {
                uname: "u".into(),
                cpu_model: "c".into(),
                governor: "unknown".into(),
                loadavg: "0.1".into(),
                rustc: "r".into(),
            },
            c_zlib_version: "1.3".into(),
            iters: 5,
            min_sample_ms: 20,
            stats_build: false,
            results: vec![ResultRow {
                corpus: "silesia".into(),
                file: "dickens".into(),
                framing: "raw".into(),
                level: 6,
                mode: "oneshot".into(),
                implementation: "cleanflate".into(),
                bytes: 10,
                reps: 4,
                median_ns: 12.5,
                best_ns: 11.0,
            }],
            aggregates: vec![Aggregate {
                key: "silesia/raw/l6/oneshot".into(),
                total_bytes: 10,
                cleanflate_mib_s_median: 1.0,
                cleanflate_mib_s_best: 2.0,
                c_zlib_mib_s_median: 3.0,
                c_zlib_mib_s_best: 4.0,
                c_zlib_mode: "oneshot".into(),
                ratio: 0.5,
            }],
            stats: Vec::new(),
        }
    }

    #[test]
    fn json_roundtrip() {
        let j = sample();
        let text = serde_json::to_string_pretty(&j).expect("serializes");
        assert!(text.contains("\"impl\""));
        let back: BenchJson = serde_json::from_str(&text).expect("parses");
        assert_eq!(back, j);
    }

    #[test]
    fn schema1_file_loads_with_best_fields() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/perf/baselines/phase2.json"
        );
        let text = std::fs::read_to_string(path).expect("phase2 baseline is committed");
        let j = BenchJson::parse(&text).expect("schema-1 loads");
        assert_eq!(j.schema, 1);
        assert!(j.results.iter().all(|r| r.best_ns > 0.0));
        assert!(j.aggregates.iter().all(|a| a.cleanflate_mib_s_best > 0.0));
    }

    #[test]
    fn schema2_roundtrip() {
        let j = sample();
        assert_eq!(j.schema, 2);
        let text = serde_json::to_string(&j).expect("serializes");
        assert!(text.contains("best_ns") && !text.contains("min_ns"));
        assert!(text.contains("cleanflate_mib_s_best"));
        assert_eq!(BenchJson::parse(&text).expect("parses"), j);
    }

    #[test]
    fn unknown_schema_rejected() {
        let mut j = sample();
        j.schema = 3;
        let text = serde_json::to_string(&j).expect("serializes");
        let err = BenchJson::parse(&text).expect_err("schema 3 rejected");
        assert!(err.contains("unsupported bench schema 3"), "{err}");
    }

    #[test]
    fn utc_formatting() {
        assert_eq!(format_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(format_utc(1_700_000_000), "2023-11-14T22:13:20Z");
    }
}

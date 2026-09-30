//! Bench comparison tools: `bench-noise`, `bench-compare` and `bench-md`
//! (SPEC S4.1, S4.2). A change counts as a win or a loss only beyond
//! twice the measured noise; a per-file regression is a drop larger than
//! the noise of that file and of its aggregate.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::bench::mib_per_sec;
use crate::bench_json::{BenchJson, ResultRow};

/// Noise between two runs of the same code, as percentages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Noise {
    /// Always 1.
    pub schema: u32,
    /// Kind of the two runs.
    pub kind: String,
    /// The two input paths.
    pub sources: Vec<String>,
    /// Noise % per aggregate key.
    pub aggregate: BTreeMap<String, f64>,
    /// Largest aggregate noise %.
    pub aggregate_max_pct: f64,
    /// Noise % per `<corpus>/<file>/<framing>/l<level>/<mode>` key.
    pub per_file: BTreeMap<String, f64>,
    /// Noise % of the like-for-like ratio per aggregate key (absent in
    /// phase-2 noise files).
    #[serde(default)]
    pub ratio_aggregate: BTreeMap<String, f64>,
    /// Largest ratio noise %.
    #[serde(default)]
    pub ratio_aggregate_max_pct: f64,
}

/// Rounding tolerance (percentage points) for threshold comparisons: a
/// pair compared against its own noise sits exactly on the boundary.
const EPS: f64 = 1e-6;

/// The verdict on one aggregate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Better by more than twice the noise.
    Win,
    /// Worse by more than twice the noise.
    Loss,
    /// Within twice the noise.
    Neutral,
    /// No noise data.
    Unknown,
}

impl Verdict {
    fn name(self) -> &'static str {
        match self {
            Verdict::Win => "WIN",
            Verdict::Loss => "LOSS",
            Verdict::Neutral => "NEUTRAL",
            Verdict::Unknown => "UNKNOWN",
        }
    }
}

/// One aggregate line of a comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct AggLine {
    /// Aggregate key.
    pub key: String,
    /// MiB/s in A.
    pub a: f64,
    /// MiB/s in B.
    pub b: f64,
    /// (B - A) / A in percent.
    pub delta_pct: f64,
    /// Twice the key's noise in percent, if known.
    pub two_noise_pct: Option<f64>,
    /// The verdict.
    pub verdict: Verdict,
    /// B's median against the baseline's median of the same key, percent.
    pub vs_baseline_pct: Option<f64>,
    /// B's like-for-like ratio.
    pub b_ratio: f64,
    /// (B ratio / A ratio - 1) x 100, or the denominator changed.
    pub ratio_delta: RatioDelta,
    /// The ratio verdict by the same 2x rule against the ratio noise.
    pub ratio_verdict: Verdict,
}

/// The change of the like-for-like ratio between A and B.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RatioDelta {
    /// Percent change of the ratio.
    Pct(f64),
    /// A and B divided by different C zlib modes; no comparison.
    DenominatorChanged,
}

/// The result of comparing two bench files.
#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    /// Aggregate lines, in key order.
    pub aggregates: Vec<AggLine>,
    /// Per-file regression descriptions.
    pub regressions: Vec<String>,
    /// Keys only in A.
    pub only_a: Vec<String>,
    /// Keys only in B.
    pub only_b: Vec<String>,
    /// Whether noise data was available.
    pub noise_known: bool,
}

/// The per-file key.
pub fn file_key(r: &ResultRow) -> String {
    format!(
        "{}/{}/{}/l{}/{}",
        r.corpus, r.file, r.framing, r.level, r.mode
    )
}

fn per_file_throughput(j: &BenchJson) -> BTreeMap<String, f64> {
    j.results
        .iter()
        .filter(|r| r.implementation == "cleanflate")
        .map(|r| (file_key(r), mib_per_sec(r.bytes, r.median_ns / 1e9)))
        .collect()
}

fn aggregate_throughput(j: &BenchJson) -> BTreeMap<String, f64> {
    j.aggregates
        .iter()
        .map(|a| (a.key.clone(), a.cleanflate_mib_s_median))
        .collect()
}

fn aggregate_ratios(j: &BenchJson) -> BTreeMap<String, f64> {
    j.aggregates
        .iter()
        .map(|a| (a.key.clone(), a.ratio))
        .collect()
}

fn pct_diff(a: f64, b: f64) -> f64 {
    if a <= 0.0 {
        0.0
    } else {
        (a - b).abs() / a * 100.0
    }
}

/// Noise between two runs: |a - b| / a in percent, cleanflate median
/// throughput only.
pub fn noise_from(a: &BenchJson, b: &BenchJson, sources: [&str; 2]) -> Noise {
    let (aa, ab) = (aggregate_throughput(a), aggregate_throughput(b));
    let aggregate: BTreeMap<String, f64> = aa
        .iter()
        .filter_map(|(k, va)| ab.get(k).map(|vb| (k.clone(), pct_diff(*va, *vb))))
        .collect();
    let (fa, fb) = (per_file_throughput(a), per_file_throughput(b));
    let per_file: BTreeMap<String, f64> = fa
        .iter()
        .filter_map(|(k, va)| fb.get(k).map(|vb| (k.clone(), pct_diff(*va, *vb))))
        .collect();
    let aggregate_max_pct = aggregate.values().copied().fold(0.0, f64::max);
    let (ra, rb) = (aggregate_ratios(a), aggregate_ratios(b));
    let ratio_aggregate: BTreeMap<String, f64> = ra
        .iter()
        .filter_map(|(k, va)| rb.get(k).map(|vb| (k.clone(), pct_diff(*va, *vb))))
        .collect();
    let ratio_aggregate_max_pct = ratio_aggregate.values().copied().fold(0.0, f64::max);
    Noise {
        schema: 1,
        kind: a.kind.clone(),
        sources: sources.iter().map(|s| s.to_string()).collect(),
        aggregate,
        aggregate_max_pct,
        per_file,
        ratio_aggregate,
        ratio_aggregate_max_pct,
    }
}

/// Compare A (before) with B (after). Refuses stats builds.
pub fn compare(a: &BenchJson, b: &BenchJson, noise: Option<&Noise>) -> Result<Comparison, String> {
    compare_with_baseline(a, b, noise, None)
}

/// [`compare`], also measuring B against a baseline file (the phase-2 numbers).
pub fn compare_with_baseline(
    a: &BenchJson,
    b: &BenchJson,
    noise: Option<&Noise>,
    baseline: Option<&BenchJson>,
) -> Result<Comparison, String> {
    if a.stats_build || b.stats_build {
        return Err("refusing to compare a stats build: its counters distort timing".to_string());
    }
    let (aa, ab) = (aggregate_throughput(a), aggregate_throughput(b));
    let base = baseline.map(aggregate_throughput);
    let agg_a: BTreeMap<&str, &crate::bench_json::Aggregate> =
        a.aggregates.iter().map(|g| (g.key.as_str(), g)).collect();
    let agg_b: BTreeMap<&str, &crate::bench_json::Aggregate> =
        b.aggregates.iter().map(|g| (g.key.as_str(), g)).collect();
    let mut aggregates = Vec::new();
    for (key, va) in &aa {
        let Some(vb) = ab.get(key) else { continue };
        let delta = if *va > 0.0 {
            (vb - va) / va * 100.0
        } else {
            0.0
        };
        let two = noise.map(|n| 2.0 * n.aggregate.get(key).copied().unwrap_or(n.aggregate_max_pct));
        let verdict = match two {
            None => Verdict::Unknown,
            Some(t) if delta > t + EPS => Verdict::Win,
            Some(t) if delta < -t - EPS => Verdict::Loss,
            Some(_) => Verdict::Neutral,
        };
        let vs_baseline_pct = base
            .as_ref()
            .and_then(|m| m.get(key))
            .filter(|v| **v > 0.0)
            .map(|v| (vb - v) / v * 100.0);
        let (ga, gb) = (agg_a.get(key.as_str()), agg_b.get(key.as_str()));
        let b_ratio = gb.map_or(f64::NAN, |g| g.ratio);
        let (ratio_delta, ratio_verdict) = match (ga, gb) {
            (Some(x), Some(y)) if x.c_zlib_mode != y.c_zlib_mode => {
                (RatioDelta::DenominatorChanged, Verdict::Unknown)
            }
            (Some(x), Some(y)) => {
                let d = if x.ratio > 0.0 {
                    (y.ratio / x.ratio - 1.0) * 100.0
                } else {
                    0.0
                };
                let two_r = noise.filter(|n| !n.ratio_aggregate.is_empty()).map(|n| {
                    2.0 * n
                        .ratio_aggregate
                        .get(key)
                        .copied()
                        .unwrap_or(n.ratio_aggregate_max_pct)
                });
                let v = match two_r {
                    None => Verdict::Unknown,
                    Some(t) if d > t + EPS => Verdict::Win,
                    Some(t) if d < -t - EPS => Verdict::Loss,
                    Some(_) => Verdict::Neutral,
                };
                (RatioDelta::Pct(d), v)
            }
            _ => (RatioDelta::DenominatorChanged, Verdict::Unknown),
        };
        aggregates.push(AggLine {
            key: key.clone(),
            a: *va,
            b: *vb,
            delta_pct: delta,
            two_noise_pct: two,
            verdict,
            vs_baseline_pct,
            b_ratio,
            ratio_delta,
            ratio_verdict,
        });
    }

    let (fa, fb) = (per_file_throughput(a), per_file_throughput(b));
    let mut regressions = Vec::new();
    if let Some(n) = noise {
        for (key, va) in &fa {
            let Some(vb) = fb.get(key) else { continue };
            let delta = if *va > 0.0 {
                (vb - va) / va * 100.0
            } else {
                0.0
            };
            let file_noise = n.per_file.get(key).copied().unwrap_or(0.0);
            let agg_key = agg_key_of(key);
            let agg_noise = n
                .aggregate
                .get(&agg_key)
                .copied()
                .unwrap_or(n.aggregate_max_pct);
            let threshold = file_noise.max(agg_noise);
            if delta < -threshold - EPS {
                regressions.push(format!(
                    "REGRESSION {key}: {va:.1} -> {vb:.1} MiB/s ({delta:+.1}%, threshold -{threshold:.1}%)"
                ));
            }
        }
    }

    let only_a = aa
        .keys()
        .chain(fa.keys())
        .filter(|k| !ab.contains_key(*k) && !fb.contains_key(*k))
        .cloned()
        .collect();
    let only_b = ab
        .keys()
        .chain(fb.keys())
        .filter(|k| !aa.contains_key(*k) && !fa.contains_key(*k))
        .cloned()
        .collect();
    Ok(Comparison {
        aggregates,
        regressions,
        only_a,
        only_b,
        noise_known: noise.is_some(),
    })
}

/// `<corpus>/<framing>/l<level>/<mode>` from a per-file key.
fn agg_key_of(file_key: &str) -> String {
    let parts: Vec<&str> = file_key.split('/').collect();
    match parts.as_slice() {
        [corpus, _file, framing, level, mode] => format!("{corpus}/{framing}/{level}/{mode}"),
        _ => file_key.to_string(),
    }
}

/// Render a comparison for the terminal.
pub fn render_comparison(c: &Comparison) -> String {
    let mut out = String::new();
    if !c.noise_known {
        out.push_str("noise unknown\n");
    }
    out.push_str(
        "| key | A MiB/s | B MiB/s | delta % | 2x noise % | verdict | vs phase-2 % | B ratio | ratio delta % | ratio verdict |\n",
    );
    out.push_str("|---|---:|---:|---:|---:|---|---:|---:|---:|---|\n");
    for l in &c.aggregates {
        let (rd, rv) = match l.ratio_delta {
            RatioDelta::Pct(d) => (format!("{d:+.2}"), l.ratio_verdict.name().to_string()),
            RatioDelta::DenominatorChanged => {
                ("n/a (denominator changed)".to_string(), "n/a".to_string())
            }
        };
        out.push_str(&format!(
            "| {} | {:.1} | {:.1} | {:+.2} | {} | {} | {} | {:.3} | {} | {} |\n",
            l.key,
            l.a,
            l.b,
            l.delta_pct,
            l.two_noise_pct
                .map_or("-".to_string(), |t| format!("{t:.2}")),
            l.verdict.name(),
            l.vs_baseline_pct
                .map_or("n/a".to_string(), |v| format!("{v:+.2}")),
            l.b_ratio,
            rd,
            rv
        ));
    }
    if c.noise_known {
        out.push('\n');
        if c.regressions.is_empty() {
            out.push_str("no per-file regressions\n");
        }
        for r in &c.regressions {
            out.push_str(r);
            out.push('\n');
        }
    } else {
        out.push_str("\nper-file regression check skipped: noise unknown\n");
    }
    for k in &c.only_a {
        out.push_str(&format!("only in A: {k}\n"));
    }
    for k in &c.only_b {
        out.push_str(&format!("only in B: {k}\n"));
    }
    out
}

/// Process exit code: 1 only with `fail_on_regression` and a regression.
pub fn exit_code(c: &Comparison, fail_on_regression: bool) -> i32 {
    i32::from(fail_on_regression && !c.regressions.is_empty())
}

/// Markdown tables of a bench file: aggregates, plus per-file rows with
/// `files`. `filter` keeps keys starting with the prefix.
pub fn render_md(j: &BenchJson, files: bool, filter: Option<&str>) -> String {
    render_md_with_baseline(j, files, filter, None)
}

/// [`render_md`] with a `vs phase-2 %` column against `baseline`.
pub fn render_md_with_baseline(
    j: &BenchJson,
    files: bool,
    filter: Option<&str>,
    baseline: Option<&BenchJson>,
) -> String {
    let keep = |k: &str| filter.is_none_or(|f| k.starts_with(f));
    let vs = |now: f64, then: Option<&f64>| -> String {
        match then {
            Some(t) if *t > 0.0 => format!("{:+.1}", (now - t) / t * 100.0),
            _ => "n/a".to_string(),
        }
    };
    let mut out = String::new();
    out.push_str(&format!(
        "Bench {} ({}), git {}{}, C zlib {}, iters {}\n\n",
        j.task,
        j.kind,
        j.git_head.get(..12).unwrap_or(&j.git_head),
        if j.git_dirty { " (dirty)" } else { "" },
        j.c_zlib_version,
        j.iters
    ));
    let base_agg = baseline.map(aggregate_throughput).unwrap_or_default();
    out.push_str("| key | cleanflate MiB/s (median) | cleanflate MiB/s (best) | C zlib MiB/s (median, same mode) | ratio (like for like) | vs phase-2 % |\n");
    out.push_str("|---|---:|---:|---:|---:|---:|\n");
    for a in j.aggregates.iter().filter(|a| keep(&a.key)) {
        out.push_str(&format!(
            "| {} | {:.1} | {:.1} | {:.1} | {:.3} | {} |\n",
            a.key,
            a.cleanflate_mib_s_median,
            a.cleanflate_mib_s_best,
            a.c_zlib_mib_s_median,
            a.ratio,
            vs(a.cleanflate_mib_s_median, base_agg.get(&a.key))
        ));
    }
    if files {
        out.push_str(
            "\n| file key | bytes | cleanflate MiB/s | C zlib MiB/s | ratio (like for like) | vs phase-2 % |\n|---|---:|---:|---:|---:|---:|\n",
        );
        let zlib: BTreeMap<String, f64> = j
            .results
            .iter()
            .filter(|r| r.implementation == "c_zlib")
            .map(|r| (file_key(r), mib_per_sec(r.bytes, r.median_ns / 1e9)))
            .collect();
        let bytes: BTreeMap<String, usize> = j
            .results
            .iter()
            .filter(|r| r.implementation == "cleanflate")
            .map(|r| (file_key(r), r.bytes))
            .collect();
        let base_files = baseline.map(per_file_throughput).unwrap_or_default();
        let pft = per_file_throughput(j);
        let mut rows: Vec<(&String, &f64)> = pft.iter().filter(|(k, _)| keep(k)).collect();
        rows.sort_by(|(ka, _), (kb, _)| {
            let corpus = |k: &str| k.split('/').next().unwrap_or("").to_string();
            let size = |k: &String| bytes.get(k).copied().unwrap_or(0);
            corpus(ka)
                .cmp(&corpus(kb))
                .then(size(ka).cmp(&size(kb)))
                .then(ka.cmp(kb))
        });
        for (key, cf) in rows {
            // Same-mode C zlib row; schema-1 files only have one-shot rows.
            let oneshot = key.rsplit_once('/').map(|(b, _)| format!("{b}/oneshot"));
            let cz = zlib
                .get(key)
                .or_else(|| oneshot.as_ref().and_then(|k| zlib.get(k)))
                .copied()
                .unwrap_or(0.0);
            let ratio = if cz > 0.0 { cf / cz } else { f64::NAN };
            out.push_str(&format!(
                "| {key} | {} | {cf:.1} | {cz:.1} | {ratio:.3} | {} |\n",
                bytes.get(key).copied().unwrap_or(0),
                vs(*cf, base_files.get(key))
            ));
        }
    }
    out
}

/// The default noise file for two runs of the same kind, relative to `root`:
/// the phase-3 file if present, else the phase-2 file (`true` in the second
/// element means "using phase-2 noise"). `None` for other or mixed kinds.
pub fn default_noise_path(root: &Path, a: &BenchJson, b: &BenchJson) -> Option<(String, bool)> {
    let (phase3, phase2) = match (a.kind.as_str(), b.kind.as_str()) {
        ("full", "full") => (
            "docs/perf/baselines/phase3-noise.json",
            "docs/perf/noise.json",
        ),
        ("quick", "quick") => (
            "docs/perf/baselines/phase3-noise-quick.json",
            "docs/perf/noise-quick.json",
        ),
        _ => return None,
    };
    if root.join(phase3).is_file() {
        Some((phase3.to_string(), false))
    } else {
        Some((phase2.to_string(), true))
    }
}

/// Load a bench JSON file.
pub fn load_json(path: &Path) -> Result<BenchJson, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    BenchJson::parse(&text).map_err(|e| format!("parsing {}: {e}", path.display()))
}

/// Load a noise file, or `None` if it does not exist.
pub fn load_noise(path: &Path) -> Result<Option<Noise>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| format!("parsing {}: {e}", path.display())),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench_json::{Aggregate, Machine};

    fn json(kind: &str, agg: f64, files: &[(&str, f64)]) -> BenchJson {
        let mib = 1024.0 * 1024.0;
        BenchJson {
            schema: crate::bench_json::SCHEMA,
            kind: kind.into(),
            task: "t".into(),
            git_head: "0123456789abcdef".into(),
            git_dirty: false,
            timestamp: "x".into(),
            machine: Machine {
                uname: "u".into(),
                cpu_model: "c".into(),
                governor: "g".into(),
                loadavg: "l".into(),
                rustc: "r".into(),
                ..Machine::default()
            },
            c_zlib_version: "1".into(),
            cargo_profile: Default::default(),
            c_zlib: Default::default(),
            iters: 5,
            min_sample_ms: 20,
            stats_build: false,
            results: files
                .iter()
                .map(|(f, mibs)| ResultRow {
                    corpus: "c".into(),
                    file: (*f).into(),
                    framing: "raw".into(),
                    level: 6,
                    mode: "oneshot".into(),
                    implementation: "cleanflate".into(),
                    bytes: mib as usize,
                    reps: 1,
                    median_ns: 1e9 / mibs,
                    best_ns: 1e9 / mibs,
                })
                .collect(),
            aggregates: vec![Aggregate {
                key: "c/raw/l6/oneshot".into(),
                total_bytes: 1,
                cleanflate_mib_s_median: agg,
                cleanflate_mib_s_best: agg,
                c_zlib_mib_s_median: 100.0,
                c_zlib_mib_s_best: 100.0,
                c_zlib_mode: "oneshot".into(),
                ratio: agg / 100.0,
            }],
            stats: Vec::new(),
        }
    }

    fn noise(agg_pct: f64, file_pct: f64) -> Noise {
        Noise {
            schema: 1,
            kind: "quick".into(),
            sources: vec![],
            aggregate: BTreeMap::from([("c/raw/l6/oneshot".to_string(), agg_pct)]),
            aggregate_max_pct: agg_pct,
            per_file: BTreeMap::from([("c/f/raw/l6/oneshot".to_string(), file_pct)]),
            ratio_aggregate: BTreeMap::new(),
            ratio_aggregate_max_pct: 0.0,
        }
    }

    #[test]
    fn noise_from_two_runs() {
        let a = json("quick", 100.0, &[("f", 50.0)]);
        let b = json("quick", 103.0, &[("f", 48.0)]);
        let n = noise_from(&a, &b, ["a.json", "b.json"]);
        assert!((n.aggregate["c/raw/l6/oneshot"] - 3.0).abs() < 1e-9);
        assert!((n.per_file["c/f/raw/l6/oneshot"] - 4.0).abs() < 1e-9);
        assert!((n.aggregate_max_pct - 3.0).abs() < 1e-9);
        assert_eq!(n.sources, vec!["a.json", "b.json"]);
    }

    #[test]
    fn verdicts_win_loss_neutral() {
        let n = noise(2.0, 2.0);
        let a = json("quick", 100.0, &[("f", 50.0)]);
        let verdict = |b: f64| {
            let c = compare(&a, &json("quick", b, &[("f", 50.0)]), Some(&n)).expect("compares");
            c.aggregates[0].verdict
        };
        assert_eq!(verdict(105.0), Verdict::Win);
        assert_eq!(verdict(95.0), Verdict::Loss);
        assert_eq!(verdict(103.0), Verdict::Neutral);
        let c = compare(&a, &json("quick", 105.0, &[("f", 50.0)]), None).expect("compares");
        assert_eq!(c.aggregates[0].verdict, Verdict::Unknown);
        assert!(render_comparison(&c).contains("noise unknown"));
    }

    #[test]
    fn regression_threshold_is_max_of_file_and_aggregate_noise() {
        let a = json("quick", 100.0, &[("f", 100.0)]);
        let regress = |file_pct: f64, agg_pct: f64, new: f64| {
            let b = json("quick", 100.0, &[("f", new)]);
            compare(&a, &b, Some(&noise(agg_pct, file_pct)))
                .expect("compares")
                .regressions
                .len()
        };
        assert_eq!(regress(2.0, 5.0, 96.0), 0, "-4% within aggregate noise 5%");
        assert_eq!(regress(2.0, 5.0, 94.0), 1, "-6% beyond both");
        assert_eq!(regress(8.0, 5.0, 94.0), 0, "-6% within file noise 8%");
        assert_eq!(regress(8.0, 5.0, 91.0), 1);
    }

    #[test]
    fn a_pair_is_neutral_against_its_own_noise() {
        let a = json("quick", 100.0, &[("f", 100.0), ("g", 70.0)]);
        let b = json("quick", 93.0, &[("f", 87.0), ("g", 77.0)]);
        let n = noise_from(&a, &b, ["a", "b"]);
        let c = compare(&a, &b, Some(&n)).expect("compares");
        assert!(c.regressions.is_empty(), "{:?}", c.regressions);
        assert_eq!(c.aggregates[0].verdict, Verdict::Neutral);
    }

    #[test]
    fn fail_on_regression_flag() {
        let a = json("quick", 100.0, &[("f", 100.0)]);
        let b = json("quick", 100.0, &[("f", 80.0)]);
        let c = compare(&a, &b, Some(&noise(1.0, 1.0))).expect("compares");
        assert_eq!(c.regressions.len(), 1);
        assert_eq!(exit_code(&c, true), 1);
        assert_eq!(exit_code(&c, false), 0);
        let ok = compare(&a, &a, Some(&noise(1.0, 1.0))).expect("compares");
        assert_eq!(exit_code(&ok, true), 0);
    }

    #[test]
    fn refuses_stats_build_json() {
        let a = json("quick", 100.0, &[("f", 100.0)]);
        let mut b = a.clone();
        b.stats_build = true;
        assert!(compare(&a, &b, None).is_err());
        assert!(compare(&b, &a, None).is_err());
    }

    #[test]
    fn no_min_in_rendered_headers() {
        let j = json("quick", 100.0, &[("f", 100.0)]);
        let table_lines = |s: &str| -> Vec<String> {
            s.lines()
                .filter(|l| l.starts_with('|'))
                .map(str::to_string)
                .collect()
        };
        let md = render_md(&j, true, None);
        let cmp = render_comparison(&compare(&j, &j, None).expect("compares"));
        let stdout = crate::bench::render_markdown(&j);
        for text in [&md, &cmp, &stdout] {
            for l in table_lines(text) {
                assert!(!l.to_lowercase().contains("min"), "{l}");
            }
        }
        assert!(md.contains("(best)"));
        assert!(stdout.contains("best:"));
    }

    fn with_ratio(mut j: BenchJson, ratio: f64) -> BenchJson {
        for a in &mut j.aggregates {
            a.ratio = ratio;
        }
        j
    }

    #[test]
    fn delta_vs_baseline_column() {
        let a = json("quick", 100.0, &[("f", 100.0)]);
        let b = json("quick", 150.0, &[("f", 150.0)]);
        let base = json("quick", 75.0, &[("f", 75.0)]);
        let c = compare_with_baseline(&a, &b, None, Some(&base)).expect("compares");
        let v = c.aggregates[0]
            .vs_baseline_pct
            .expect("baseline key present");
        assert!((v - 100.0).abs() < 1e-9, "{v}");
        assert!(render_comparison(&c).contains("+100.00"));
        let md = render_md_with_baseline(&b, false, None, Some(&base));
        assert!(md.contains("vs phase-2 %") && md.contains("+100.0"), "{md}");
    }

    #[test]
    fn ratio_delta_and_verdict() {
        let a = with_ratio(json("quick", 100.0, &[("f", 100.0)]), 0.50);
        let b = with_ratio(json("quick", 100.0, &[("f", 100.0)]), 0.60);
        let mut n = noise(2.0, 2.0);
        n.ratio_aggregate = BTreeMap::from([("c/raw/l6/oneshot".to_string(), 3.0)]);
        n.ratio_aggregate_max_pct = 3.0;
        let c = compare(&a, &b, Some(&n)).expect("compares");
        match c.aggregates[0].ratio_delta {
            RatioDelta::Pct(d) => assert!((d - 20.0).abs() < 1e-9),
            RatioDelta::DenominatorChanged => panic!("same denominator"),
        }
        assert_eq!(c.aggregates[0].ratio_verdict, Verdict::Win);
        let small = with_ratio(json("quick", 100.0, &[("f", 100.0)]), 0.52);
        let c = compare(&a, &small, Some(&n)).expect("compares");
        assert_eq!(c.aggregates[0].ratio_verdict, Verdict::Neutral);
        let c = compare(&a, &b, Some(&noise(2.0, 2.0))).expect("compares");
        assert_eq!(
            c.aggregates[0].ratio_verdict,
            Verdict::Unknown,
            "phase-2 noise has no ratio noise"
        );
    }

    #[test]
    fn denominator_change_marked() {
        let a = json("quick", 100.0, &[("f", 100.0)]);
        let mut b = a.clone();
        b.aggregates[0].c_zlib_mode = "streaming".into();
        let c = compare(&a, &b, None).expect("compares");
        assert_eq!(c.aggregates[0].ratio_delta, RatioDelta::DenominatorChanged);
        assert!(render_comparison(&c).contains("n/a (denominator changed)"));
    }

    #[test]
    fn per_file_rows_sorted_by_size_with_bytes() {
        let mut j = json(
            "quick",
            100.0,
            &[("big", 100.0), ("small", 100.0), ("mid", 100.0)],
        );
        for r in &mut j.results {
            r.bytes = match r.file.as_str() {
                "big" => 3000,
                "mid" => 2000,
                _ => 1000,
            };
        }
        let md = render_md(&j, true, None);
        let pos = |f: &str| md.find(&format!("c/{f}/raw")).expect("row present");
        assert!(pos("small") < pos("mid") && pos("mid") < pos("big"), "{md}");
        assert!(md.contains("| bytes |") && md.contains("| 1000 |") && md.contains("| 3000 |"));
    }

    #[test]
    fn noise_includes_ratio() {
        let a = with_ratio(json("quick", 100.0, &[("f", 100.0)]), 0.50);
        let b = with_ratio(json("quick", 100.0, &[("f", 100.0)]), 0.52);
        let n = noise_from(&a, &b, ["a", "b"]);
        assert!((n.ratio_aggregate["c/raw/l6/oneshot"] - 4.0).abs() < 1e-9);
        assert!((n.ratio_aggregate_max_pct - 4.0).abs() < 1e-9);
        let text = serde_json::to_string(&n).expect("serializes");
        assert!(text.contains("ratio_aggregate_max_pct"));
        let old = r#"{"schema":1,"kind":"quick","sources":[],"aggregate":{},"aggregate_max_pct":1.0,"per_file":{}}"#;
        let n: Noise = serde_json::from_str(old).expect("phase-2 noise loads");
        assert!(n.ratio_aggregate.is_empty());
    }

    #[test]
    fn default_noise_prefers_phase3_then_phase2() {
        let root = std::env::temp_dir().join(format!("cleanflate-noise-{}", std::process::id()));
        let base = root.join("docs/perf/baselines");
        std::fs::create_dir_all(&base).expect("temp dir");
        let full = json("full", 1.0, &[]);
        let quick = json("quick", 1.0, &[]);
        assert_eq!(
            default_noise_path(&root, &full, &full),
            Some(("docs/perf/noise.json".to_string(), true))
        );
        std::fs::write(base.join("phase3-noise.json"), "{}").expect("write");
        assert_eq!(
            default_noise_path(&root, &full, &full),
            Some(("docs/perf/baselines/phase3-noise.json".to_string(), false))
        );
        assert_eq!(
            default_noise_path(&root, &quick, &quick),
            Some(("docs/perf/noise-quick.json".to_string(), true))
        );
        std::fs::write(base.join("phase3-noise-quick.json"), "{}").expect("write");
        assert_eq!(
            default_noise_path(&root, &quick, &quick),
            Some((
                "docs/perf/baselines/phase3-noise-quick.json".to_string(),
                false
            ))
        );
        assert_eq!(default_noise_path(&root, &full, &quick), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_baseline_prints_na() {
        let a = json("quick", 100.0, &[("f", 100.0)]);
        let c = compare_with_baseline(&a, &a, None, None).expect("compares");
        assert_eq!(c.aggregates[0].vs_baseline_pct, None);
        let text = render_comparison(&c);
        assert!(text.contains("| n/a |"), "{text}");
        assert!(render_md_with_baseline(&a, true, None, None).contains("n/a"));
    }

    #[test]
    fn markdown_render_has_all_keys() {
        let j = json("quick", 100.0, &[("f", 100.0), ("g", 90.0)]);
        let md = render_md(&j, true, None);
        assert!(md.contains("c/raw/l6/oneshot"));
        assert!(md.contains("c/f/raw/l6/oneshot"));
        assert!(md.contains("c/g/raw/l6/oneshot"));
        let only_agg = render_md(&j, false, None);
        assert!(!only_agg.contains("c/f/raw"));
        assert!(!render_md(&j, false, Some("zzz")).contains("c/raw/l6"));
    }
}

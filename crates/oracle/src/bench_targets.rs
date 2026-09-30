//! Mechanical evaluation of the phase-3 targets T1-T6 (SPEC 4.1) from one
//! bench JSON. Targets are reported, never gates.

use std::collections::BTreeMap;

use crate::bench_compare::{compare, Noise, Verdict};
use crate::bench_json::{Aggregate, BenchJson};

/// The outcome of one target.
#[derive(Debug, Clone, PartialEq)]
pub struct TargetReport {
    /// `T1`..`T6`.
    pub id: &'static str,
    /// What the target requires.
    pub title: &'static str,
    /// `Some(true)` MET, `Some(false)` NOT MET, `None` not evaluable.
    pub met: Option<bool>,
    /// The worst row and its value (or why the target is `n/a`).
    pub worst: String,
    /// Every failing row.
    pub failing: Vec<String>,
    /// Rows excluded because they have no like-for-like denominator, and
    /// other information-only lines.
    pub info: Vec<String>,
}

struct Key<'a> {
    corpus: &'a str,
    framing: &'a str,
    level: &'a str,
    mode: &'a str,
}

fn parse_key(k: &str) -> Option<Key<'_>> {
    let mut it = k.split('/');
    let key = Key {
        corpus: it.next()?,
        framing: it.next()?,
        level: it.next()?,
        mode: it.next()?,
    };
    it.next().is_none().then_some(key)
}

const FRAMINGS: [&str; 3] = ["raw", "zlib", "gzip"];

/// A ratio threshold check over the aggregates selected by `select`.
fn ratio_target(
    id: &'static str,
    title: &'static str,
    threshold: f64,
    j: &BenchJson,
    select: impl Fn(&Key) -> bool,
) -> TargetReport {
    let mut checked: Vec<(&Aggregate, f64)> = Vec::new();
    let mut info = Vec::new();
    for a in &j.aggregates {
        let Some(k) = parse_key(&a.key) else { continue };
        if !select(&k) {
            continue;
        }
        if a.c_zlib_mode != k.mode {
            info.push(format!(
                "{}: n/a (not like for like: denominator is C zlib {})",
                a.key, a.c_zlib_mode
            ));
            continue;
        }
        checked.push((a, a.ratio));
    }
    let failing: Vec<String> = checked
        .iter()
        .filter(|(_, r)| r.is_nan() || *r < threshold)
        .map(|(a, r)| format!("{}: ratio {r:.3} < {threshold:.2}", a.key))
        .collect();
    let worst = checked
        .iter()
        .min_by(|x, y| x.1.total_cmp(&y.1))
        .map_or("n/a (no evaluable rows)".to_string(), |(a, r)| {
            format!("{} ratio {r:.3} (threshold >= {threshold:.2})", a.key)
        });
    TargetReport {
        id,
        title,
        met: (!checked.is_empty()).then_some(failing.is_empty()),
        worst,
        failing,
        info,
    }
}

fn t2(j: &BenchJson) -> TargetReport {
    let mut rows: Vec<(String, f64)> = Vec::new();
    let by_key: BTreeMap<&str, &Aggregate> =
        j.aggregates.iter().map(|a| (a.key.as_str(), a)).collect();
    for a in &j.aggregates {
        let Some(k) = parse_key(&a.key) else { continue };
        if k.mode != "oneshot" {
            continue;
        }
        let sk = format!("{}/{}/{}/streaming", k.corpus, k.framing, k.level);
        if let Some(s) = by_key.get(sk.as_str()) {
            if s.cleanflate_mib_s_median > 0.0 {
                rows.push((
                    format!("{}/{}/{}", k.corpus, k.framing, k.level),
                    a.cleanflate_mib_s_median / s.cleanflate_mib_s_median,
                ));
            }
        }
    }
    let failing: Vec<String> = rows
        .iter()
        .filter(|(_, r)| r.is_nan() || *r < 1.0)
        .map(|(k, r)| format!("{k}: one-shot / streaming = {r:.3} < 1"))
        .collect();
    let worst = rows
        .iter()
        .min_by(|x, y| x.1.total_cmp(&y.1))
        .map_or("n/a (no evaluable rows)".to_string(), |(k, r)| {
            format!("{k} one-shot / streaming {r:.3} (threshold >= 1.00)")
        });
    // Per-file cases where one-shot is slower: information only.
    let mut info = Vec::new();
    let median = |file: &str, r: &crate::bench_json::ResultRow, mode: &str| {
        j.results.iter().find(|x| {
            x.implementation == "cleanflate"
                && x.mode == mode
                && x.corpus == r.corpus
                && x.file == file
                && x.framing == r.framing
                && x.level == r.level
        })
    };
    for r in j
        .results
        .iter()
        .filter(|r| r.implementation == "cleanflate" && r.mode == "oneshot")
    {
        if let Some(s) = median(&r.file, r, "streaming") {
            if r.median_ns > s.median_ns {
                info.push(format!(
                    "info: {}/{}/{}/l{}: one-shot slower than streaming ({:.0} ns vs {:.0} ns)",
                    r.corpus, r.file, r.framing, r.level, r.median_ns, s.median_ns
                ));
            }
        }
    }
    TargetReport {
        id: "T2",
        title: "cleanflate one-shot >= streaming, every row",
        met: (!rows.is_empty()).then_some(failing.is_empty()),
        worst,
        failing,
        info,
    }
}

fn t4(j: &BenchJson) -> TargetReport {
    let by_key: BTreeMap<&str, &Aggregate> =
        j.aggregates.iter().map(|a| (a.key.as_str(), a)).collect();
    let mut rows: Vec<(String, f64)> = Vec::new();
    for a in &j.aggregates {
        let Some(k) = parse_key(&a.key) else { continue };
        if k.framing != "raw" {
            continue;
        }
        for framed in ["zlib", "gzip"] {
            let fk = format!("{}/{framed}/{}/{}", k.corpus, k.level, k.mode);
            if let Some(f) = by_key.get(fk.as_str()) {
                let raw = a.cleanflate_mib_s_median;
                if raw > 0.0 {
                    rows.push((fk, (raw - f.cleanflate_mib_s_median) / raw));
                }
            }
        }
    }
    let failing: Vec<String> = rows
        .iter()
        .filter(|(_, o)| o.is_nan() || *o > 0.03)
        .map(|(k, o)| format!("{k}: overhead {:.2}% > 3%", o * 100.0))
        .collect();
    let worst = rows
        .iter()
        .max_by(|x, y| x.1.total_cmp(&y.1))
        .map_or("n/a (no evaluable rows)".to_string(), |(k, o)| {
            format!("{k} overhead {:.2}% (threshold <= 3%)", o * 100.0)
        });
    TargetReport {
        id: "T4",
        title: "zlib and gzip within 3% of raw throughput",
        met: (!rows.is_empty()).then_some(failing.is_empty()),
        worst,
        failing,
        info: Vec::new(),
    }
}

fn t6(j: &BenchJson, baseline: Option<&BenchJson>, noise: Option<&Noise>) -> TargetReport {
    let title = "no row slower than the phase-2 baseline beyond noise";
    let Some(base) = baseline else {
        return TargetReport {
            id: "T6",
            title,
            met: None,
            worst: "n/a (baseline missing)".to_string(),
            failing: Vec::new(),
            info: Vec::new(),
        };
    };
    match compare(base, j, noise) {
        Err(e) => TargetReport {
            id: "T6",
            title,
            met: None,
            worst: format!("n/a ({e})"),
            failing: Vec::new(),
            info: Vec::new(),
        },
        Ok(c) => {
            let mut failing: Vec<String> = c
                .aggregates
                .iter()
                .filter(|l| l.verdict == Verdict::Loss)
                .map(|l| {
                    format!(
                        "LOSS {}: {:.1} -> {:.1} MiB/s ({:+.2}%)",
                        l.key, l.a, l.b, l.delta_pct
                    )
                })
                .collect();
            failing.extend(c.regressions.iter().cloned());
            let worst = c
                .aggregates
                .iter()
                .min_by(|x, y| x.delta_pct.total_cmp(&y.delta_pct))
                .map_or("n/a (no common rows)".to_string(), |l| {
                    format!("{} {:+.2}% vs baseline", l.key, l.delta_pct)
                });
            let mut info = Vec::new();
            if !c.noise_known {
                info.push(
                    "noise unknown: per-file regressions not checked, verdicts UNKNOWN".to_string(),
                );
            }
            TargetReport {
                id: "T6",
                title,
                met: (!c.aggregates.is_empty()).then_some(failing.is_empty()),
                worst,
                failing,
                info,
            }
        }
    }
}

/// Evaluate T1-T6 over `j`.
pub fn evaluate(
    j: &BenchJson,
    baseline: Option<&BenchJson>,
    noise: Option<&Noise>,
) -> Vec<TargetReport> {
    let levels = ["l1", "l6", "l9"];
    vec![
        ratio_target(
            "T1",
            "Silesia streaming >= 0.80x C zlib streaming",
            0.80,
            j,
            |k| {
                k.corpus == "silesia"
                    && k.mode == "streaming"
                    && FRAMINGS.contains(&k.framing)
                    && levels.contains(&k.level)
            },
        ),
        t2(j),
        ratio_target(
            "T3",
            "Canterbury >= 0.70x C zlib, every mode",
            0.70,
            j,
            |k| k.corpus == "canterbury" && (k.mode == "oneshot" || k.mode == "streaming"),
        ),
        t4(j),
        ratio_target(
            "T5",
            "Silesia level-6 one-shot >= 0.70x C zlib",
            0.70,
            j,
            |k| {
                k.corpus == "silesia"
                    && k.level == "l6"
                    && k.mode == "oneshot"
                    && FRAMINGS.contains(&k.framing)
            },
        ),
        t6(j, baseline, noise),
    ]
}

/// Markdown for PHASE3.md: one line per target, then every failing row.
pub fn render(reports: &[TargetReport]) -> String {
    let mut out = String::new();
    for r in reports {
        let verdict = match r.met {
            Some(true) => "MET",
            Some(false) => "NOT MET",
            None => "n/a",
        };
        out.push_str(&format!(
            "- {} ({}): {verdict}; worst: {}\n",
            r.id, r.title, r.worst
        ));
    }
    for r in reports {
        if r.failing.is_empty() && r.info.is_empty() {
            continue;
        }
        out.push_str(&format!("\n{} rows:\n", r.id));
        for f in &r.failing {
            out.push_str(&format!("- FAIL {f}\n"));
        }
        for i in &r.info {
            out.push_str(&format!("- {i}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench_json::Machine;

    fn agg(key: &str, median: f64, ratio: f64, zmode: &str) -> Aggregate {
        Aggregate {
            key: key.into(),
            total_bytes: 1,
            cleanflate_mib_s_median: median,
            cleanflate_mib_s_best: median,
            c_zlib_mib_s_median: 100.0,
            c_zlib_mib_s_best: 100.0,
            c_zlib_mode: zmode.into(),
            ratio,
        }
    }

    fn json(aggs: Vec<Aggregate>) -> BenchJson {
        BenchJson {
            schema: crate::bench_json::SCHEMA,
            kind: "full".into(),
            task: "t".into(),
            git_head: "h".into(),
            git_dirty: false,
            timestamp: "x".into(),
            machine: Machine::default(),
            c_zlib_version: "1".into(),
            cargo_profile: Default::default(),
            c_zlib: Default::default(),
            iters: 7,
            min_sample_ms: 20,
            stats_build: false,
            results: Vec::new(),
            aggregates: aggs,
            stats: Vec::new(),
        }
    }

    fn get<'a>(r: &'a [TargetReport], id: &str) -> &'a TargetReport {
        r.iter().find(|t| t.id == id).expect("target present")
    }

    #[test]
    fn t1_streaming_threshold() {
        let ok = json(vec![
            agg("silesia/raw/l1/streaming", 1.0, 0.85, "streaming"),
            agg("silesia/zlib/l6/streaming", 1.0, 0.80, "streaming"),
        ]);
        assert_eq!(get(&evaluate(&ok, None, None), "T1").met, Some(true));
        let bad = json(vec![
            agg("silesia/raw/l1/streaming", 1.0, 0.85, "streaming"),
            agg("silesia/gzip/l9/streaming", 1.0, 0.79, "streaming"),
        ]);
        let r = evaluate(&bad, None, None);
        assert_eq!(get(&r, "T1").met, Some(false));
        assert!(get(&r, "T1").worst.contains("silesia/gzip/l9/streaming"));
        assert!(render(&r).contains("NOT MET"));
    }

    #[test]
    fn t2_oneshot_vs_streaming() {
        let ok = json(vec![
            agg("silesia/raw/l6/oneshot", 110.0, 0.7, "oneshot"),
            agg("silesia/raw/l6/streaming", 100.0, 0.7, "streaming"),
        ]);
        assert_eq!(get(&evaluate(&ok, None, None), "T2").met, Some(true));
        let bad = json(vec![
            agg("canterbury/raw/l6/oneshot", 90.0, 0.7, "oneshot"),
            agg("canterbury/raw/l6/streaming", 100.0, 0.7, "streaming"),
        ]);
        let r = evaluate(&bad, None, None);
        assert_eq!(get(&r, "T2").met, Some(false));
        assert_eq!(get(&r, "T2").failing.len(), 1);
    }

    #[test]
    fn t3_canterbury_all_modes() {
        let ok = json(vec![
            agg("canterbury/raw/l1/oneshot", 1.0, 0.70, "oneshot"),
            agg("canterbury/gzip/l9/streaming", 1.0, 0.9, "streaming"),
        ]);
        assert_eq!(get(&evaluate(&ok, None, None), "T3").met, Some(true));
        let bad = json(vec![
            agg("canterbury/raw/l1/oneshot", 1.0, 0.9, "oneshot"),
            agg("canterbury/zlib/l6/streaming", 1.0, 0.69, "streaming"),
        ]);
        assert_eq!(get(&evaluate(&bad, None, None), "T3").met, Some(false));
    }

    #[test]
    fn t4_framing_overhead() {
        let ok = json(vec![
            agg("silesia/raw/l6/oneshot", 100.0, 0.7, "oneshot"),
            agg("silesia/zlib/l6/oneshot", 97.0, 0.7, "oneshot"),
            agg("silesia/gzip/l6/oneshot", 98.0, 0.7, "oneshot"),
        ]);
        assert_eq!(get(&evaluate(&ok, None, None), "T4").met, Some(true));
        let bad = json(vec![
            agg("silesia/raw/l6/oneshot", 100.0, 0.7, "oneshot"),
            agg("silesia/zlib/l6/oneshot", 96.0, 0.7, "oneshot"),
        ]);
        let r = evaluate(&bad, None, None);
        assert_eq!(get(&r, "T4").met, Some(false));
        assert!(get(&r, "T4").worst.contains("4.00%"));
    }

    #[test]
    fn t5_level6_oneshot() {
        let ok = json(vec![
            agg("silesia/raw/l6/oneshot", 1.0, 0.70, "oneshot"),
            agg("silesia/raw/l1/oneshot", 1.0, 0.10, "oneshot"),
        ]);
        assert_eq!(get(&evaluate(&ok, None, None), "T5").met, Some(true));
        let bad = json(vec![agg("silesia/gzip/l6/oneshot", 1.0, 0.65, "oneshot")]);
        assert_eq!(get(&evaluate(&bad, None, None), "T5").met, Some(false));
    }

    #[test]
    fn t6_uses_compare() {
        let base = json(vec![agg("silesia/raw/l6/oneshot", 100.0, 0.7, "oneshot")]);
        let noise = Noise {
            schema: 1,
            kind: "full".into(),
            sources: vec![],
            aggregate: BTreeMap::from([("silesia/raw/l6/oneshot".to_string(), 2.0)]),
            aggregate_max_pct: 2.0,
            per_file: BTreeMap::new(),
            ratio_aggregate: BTreeMap::new(),
            ratio_aggregate_max_pct: 0.0,
        };
        let same = json(vec![agg("silesia/raw/l6/oneshot", 101.0, 0.7, "oneshot")]);
        assert_eq!(
            get(&evaluate(&same, Some(&base), Some(&noise)), "T6").met,
            Some(true)
        );
        let slow = json(vec![agg("silesia/raw/l6/oneshot", 90.0, 0.7, "oneshot")]);
        let r = evaluate(&slow, Some(&base), Some(&noise));
        assert_eq!(get(&r, "T6").met, Some(false));
        assert!(get(&r, "T6").failing[0].starts_with("LOSS"));
        assert_eq!(get(&evaluate(&slow, None, None), "T6").met, None);
    }

    #[test]
    fn schema1_streaming_is_na() {
        let old = json(vec![agg("silesia/raw/l6/streaming", 1.0, 0.1, "oneshot")]);
        let r = evaluate(&old, None, None);
        assert_eq!(get(&r, "T1").met, None);
        assert!(get(&r, "T1").info[0].contains("n/a (not like for like"));
        assert!(render(&r).contains("n/a"));
    }

    #[test]
    fn reuse_rows_ignored() {
        let j = json(vec![
            agg("canterbury/raw/l6/reuse", 1.0, 0.01, "reuse"),
            agg("canterbury/raw/l6/oneshot", 1.0, 0.9, "oneshot"),
        ]);
        let r = evaluate(&j, None, None);
        assert_eq!(get(&r, "T3").met, Some(true));
        assert!(get(&r, "T3").failing.is_empty());
    }
}

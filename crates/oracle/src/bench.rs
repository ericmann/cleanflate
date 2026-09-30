//! `oracle bench`: single-thread decompression throughput, cleanflate vs
//! C zlib (SPEC S4.1, S4.2). Release-only: refuses to run in a debug
//! build. C zlib is a black-box stopwatch: only `ffi::inflate_bytes` is
//! timed, and fixtures come from `ffi::deflate_bytes`.

use std::path::{Path, PathBuf};
use std::time::Instant;

use cleanflate::{decompress_with, Decompressor, Format, Options, Status};

use crate::bench_json::{format_utc, Aggregate, BenchJson, Machine, ResultRow, StatsEntry};
use crate::ffi::{
    deflate_bytes, inflate_bytes, inflate_streaming_into, zlib_version, DeflateParams, Framing,
    Strategy,
};
use crate::inputs::{canterbury_inputs, silesia_inputs, Input};

/// Minimum duration of one timed sample. A bench-local constant, not a
/// cleanflate tunable.
pub const MIN_SAMPLE_MS: u64 = 20;

/// Silesia files in the quick subset (all Canterbury files are always
/// included).
pub const QUICK_SILESIA: [&str; 5] = ["dickens", "mr", "ooffice", "sao", "xml"];

/// Streaming-mode chunk size for both input and output windows.
const STREAM_CHUNK: usize = 64 * 1024;

/// Fewest samples a `--kind full` run may take.
pub const FULL_MIN_ITERS: u32 = 7;

/// Which corpus to benchmark against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchCorpus {
    /// The Silesia corpus.
    Silesia,
    /// The Canterbury corpus.
    Canterbury,
}

impl BenchCorpus {
    fn name(self) -> &'static str {
        match self {
            BenchCorpus::Silesia => "silesia",
            BenchCorpus::Canterbury => "canterbury",
        }
    }
}

/// How cleanflate is driven.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// `decompress_with` on the whole input.
    OneShot,
    /// `Decompressor` fed 64 KiB chunks.
    Streaming,
}

/// `--kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Milestone-end run.
    Full,
    /// Iteration-time run.
    Quick,
    /// Anything else.
    Custom,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Full => "full",
            Kind::Quick => "quick",
            Kind::Custom => "custom",
        }
    }
}

/// `oracle bench` parameters.
#[derive(Debug, Clone)]
pub struct BenchArgs {
    /// Run kind.
    pub kind: Kind,
    /// Corpora to read.
    pub corpora: Vec<BenchCorpus>,
    /// Restrict to the quick subset.
    pub subset_quick: bool,
    /// Framings to measure.
    pub framings: Vec<Framing>,
    /// Compression levels of the fixtures.
    pub levels: Vec<u8>,
    /// cleanflate modes.
    pub modes: Vec<Mode>,
    /// Timed samples per measurement.
    pub iters: u32,
    /// Task id recorded in the JSON.
    pub task: String,
    /// JSON output path.
    pub json: Option<PathBuf>,
    /// Collect cleanflate's `stats` counters (needs the oracle `stats`
    /// feature).
    pub stats: bool,
}

fn framing_name(f: Framing) -> &'static str {
    match f {
        Framing::Raw => "raw",
        Framing::Zlib => "zlib",
        Framing::Gzip => "gzip",
    }
}

fn parse_list<T>(
    flag: &str,
    value: &str,
    one: impl Fn(&str) -> Option<T>,
) -> Result<Vec<T>, String> {
    let mut out = Vec::new();
    for part in value.split(',') {
        out.push(one(part.trim()).ok_or_else(|| format!("{flag}: unknown value {part:?}"))?);
    }
    Ok(out)
}

/// Parse `oracle bench` arguments.
pub fn parse_args(args: &[String]) -> Result<BenchArgs, String> {
    let mut kind = Kind::Custom;
    let mut corpora = vec![BenchCorpus::Silesia];
    let mut subset_quick = false;
    let mut framings = vec![Framing::Raw, Framing::Zlib, Framing::Gzip];
    let mut levels = vec![6u8];
    let mut modes = vec![Mode::OneShot, Mode::Streaming];
    let mut iters: Option<u32> = None;
    let mut task = "adhoc".to_string();
    let mut json = None;
    let mut stats = false;
    let mut i = 0;
    while i < args.len() {
        let flag = args.get(i).map(String::as_str).unwrap_or("");
        if flag == "--stats" {
            if !cfg!(feature = "stats") {
                return Err(
                    "--stats needs the stats feature: rebuild with --features stats".to_string(),
                );
            }
            stats = true;
            i += 1;
            continue;
        }
        let value = args
            .get(i + 1)
            .ok_or_else(|| format!("{flag} requires a value"))?;
        match flag {
            "--kind" => {
                kind = match value.as_str() {
                    "full" => Kind::Full,
                    "quick" => Kind::Quick,
                    "custom" => Kind::Custom,
                    other => return Err(format!("--kind: unknown value {other:?}")),
                }
            }
            "--corpus" => {
                corpora = parse_list(flag, value, |s| match s {
                    "silesia" => Some(BenchCorpus::Silesia),
                    "canterbury" => Some(BenchCorpus::Canterbury),
                    _ => None,
                })?
            }
            "--subset" => match value.as_str() {
                "quick" => subset_quick = true,
                other => return Err(format!("--subset: unknown value {other:?}")),
            },
            "--framings" => {
                framings = parse_list(flag, value, |s| match s {
                    "raw" | "rfc1951" => Some(Framing::Raw),
                    "zlib" | "rfc1950" => Some(Framing::Zlib),
                    "gzip" | "rfc1952" => Some(Framing::Gzip),
                    _ => None,
                })?
            }
            "--levels" => {
                levels = parse_list(flag, value, |s| s.parse::<u8>().ok().filter(|l| *l <= 9))?
            }
            "--modes" => {
                modes = parse_list(flag, value, |s| match s {
                    "oneshot" => Some(Mode::OneShot),
                    "streaming" => Some(Mode::Streaming),
                    _ => None,
                })?
            }
            "--iters" => iters = Some(value.parse().map_err(|e| format!("--iters: {e}"))?),
            "--task" => task = value.clone(),
            "--json" => json = Some(PathBuf::from(value)),
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 2;
    }
    let iters = iters.unwrap_or(match kind {
        Kind::Full => FULL_MIN_ITERS,
        _ => 5,
    });
    if kind == Kind::Full && iters < FULL_MIN_ITERS {
        return Err(format!(
            "--kind full requires --iters >= {FULL_MIN_ITERS} (got {iters})"
        ));
    }
    if iters == 0 && !stats {
        return Err("--iters must be at least 1 (0 is allowed only with --stats)".to_string());
    }
    Ok(BenchArgs {
        kind,
        corpora,
        subset_quick,
        framings,
        levels,
        modes,
        iters,
        task,
        json,
        stats,
    })
}

/// `bytes` per `secs` in MiB/s; a non-positive time reports 0.
pub fn mib_per_sec(bytes: usize, elapsed_secs: f64) -> f64 {
    if elapsed_secs <= 0.0 {
        return 0.0;
    }
    (bytes as f64 / (1024.0 * 1024.0)) / elapsed_secs
}

/// Median of `values` (mean of the two middle values when even); 0 for
/// an empty slice.
pub fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    let mid = v.len() / 2;
    if v.len() % 2 == 1 {
        v.get(mid).copied().unwrap_or(0.0)
    } else {
        (v.get(mid - 1).copied().unwrap_or(0.0) + v.get(mid).copied().unwrap_or(0.0)) / 2.0
    }
}

/// Minimum of `values`; 0 for an empty slice.
pub fn minimum(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().copied().fold(f64::INFINITY, f64::min)
    }
}

/// Aggregate MiB/s: total bytes over the sum of per-file times.
pub fn aggregate_mib_s(total_bytes: usize, per_file_secs: &[f64]) -> f64 {
    mib_per_sec(total_bytes, per_file_secs.iter().sum())
}

/// The smallest power of two `reps` with `reps * one_secs >= min_ms`.
pub fn reps_for(one_secs: f64, min_ms: u64) -> u32 {
    let target = min_ms as f64 / 1000.0;
    let mut reps = 1u32;
    while (reps as f64) * one_secs < target && reps < (1 << 24) {
        reps <<= 1;
    }
    reps
}

fn inputs_for(corpus: BenchCorpus, root: &Path, subset_quick: bool) -> Result<Vec<Input>, String> {
    let all = match corpus {
        BenchCorpus::Silesia => silesia_inputs(root),
        BenchCorpus::Canterbury => canterbury_inputs(root),
    };
    if all.is_empty() {
        return Err(format!(
            "no {} files under {} -- see testdata/corpora/README.md to fetch them",
            corpus.name(),
            root.display()
        ));
    }
    let picked: Vec<Input> = all
        .into_iter()
        .filter(|i| {
            !subset_quick
                || corpus == BenchCorpus::Canterbury
                || QUICK_SILESIA.iter().any(|n| i.id == format!("silesia-{n}"))
        })
        .collect();
    if picked.is_empty() {
        return Err(format!(
            "the quick subset selected no {} files; see testdata/corpora/README.md",
            corpus.name()
        ));
    }
    Ok(picked)
}

fn format_of(framing: Framing) -> Format {
    match framing {
        Framing::Raw => Format::Raw,
        Framing::Zlib => Format::Zlib,
        Framing::Gzip => Format::Gzip,
    }
}

/// Decode `compressed` through a `Decompressor` in 64 KiB windows into
/// the pre-sized `out`; returns bytes produced.
fn decode_streaming(format: Format, compressed: &[u8], out: &mut [u8]) -> Result<usize, String> {
    decode_streaming_dec(format, compressed, out).map(|(n, _)| n)
}

/// [`decode_streaming`], also returning the finished decompressor.
fn decode_streaming_dec(
    format: Format,
    compressed: &[u8],
    out: &mut [u8],
) -> Result<(usize, Decompressor), String> {
    let mut d = Decompressor::new(format, Options::default());
    let mut in_pos = 0usize;
    let mut out_pos = 0usize;
    loop {
        let in_end = (in_pos + STREAM_CHUNK).min(compressed.len());
        let out_end = (out_pos + STREAM_CHUNK).min(out.len());
        let chunk = compressed.get(in_pos..in_end).unwrap_or(&[]);
        let window = out.get_mut(out_pos..out_end).unwrap_or(&mut []);
        let p = d.decompress(chunk, window).map_err(|e| format!("{e:?}"))?;
        in_pos += p.consumed;
        out_pos += p.produced;
        match p.status {
            Status::Done => break,
            Status::OutputFull => {}
            Status::NeedsInput => {
                if p.consumed == 0 && p.produced == 0 && in_pos >= compressed.len() {
                    break;
                }
            }
        }
    }
    d.finish().map_err(|e| format!("{e:?}"))?;
    Ok((out_pos, d))
}

/// One timed implementation of a decode.
#[derive(Clone, Copy)]
enum Imp {
    CleanflateOneShot,
    CleanflateStreaming,
    CZlibOneShot,
    CZlibStreaming,
}

struct Fixture<'a> {
    framing: Framing,
    original: &'a [u8],
    compressed: &'a [u8],
}

/// Run one decode with `imp`; returns the output length (or 0 on error).
fn run_once(imp: Imp, fx: &Fixture, scratch: &mut [u8]) -> usize {
    match imp {
        Imp::CleanflateOneShot => {
            decompress_with(format_of(fx.framing), fx.compressed, &Options::default())
                .map(|d| d.data.len())
                .unwrap_or(0)
        }
        Imp::CleanflateStreaming => {
            decode_streaming(format_of(fx.framing), fx.compressed, scratch).unwrap_or(0)
        }
        Imp::CZlibOneShot => {
            let max = fx.original.len().saturating_add(1024);
            inflate_bytes(fx.compressed, fx.framing, None, max)
                .output
                .len()
        }
        Imp::CZlibStreaming => inflate_streaming_into(
            fx.compressed,
            fx.framing,
            STREAM_CHUNK,
            scratch,
            STREAM_CHUNK,
        )
        .unwrap_or(0),
    }
}

/// Decode once with `imp` and require output byte-identical to the
/// original.
fn verify(imp: Imp, fx: &Fixture, scratch: &mut [u8]) -> Result<(), String> {
    let ok = match imp {
        Imp::CleanflateOneShot => {
            let d = decompress_with(format_of(fx.framing), fx.compressed, &Options::default())
                .map_err(|e| format!("{e:?}"))?;
            d.data == fx.original
        }
        Imp::CleanflateStreaming => {
            let n = decode_streaming(format_of(fx.framing), fx.compressed, scratch)?;
            scratch.get(..n) == Some(fx.original)
        }
        Imp::CZlibOneShot => {
            let max = fx.original.len().saturating_add(1024);
            inflate_bytes(fx.compressed, fx.framing, None, max).output == fx.original
        }
        Imp::CZlibStreaming => {
            let n = inflate_streaming_into(
                fx.compressed,
                fx.framing,
                STREAM_CHUNK,
                scratch,
                STREAM_CHUNK,
            )
            .map_err(|e| format!("C zlib streaming failed: {e:?}"))?;
            scratch.get(..n) == Some(fx.original)
        }
    };
    if ok {
        Ok(())
    } else {
        Err("decoded output differs from the original".to_string())
    }
}

fn time_reps(imp: Imp, fx: &Fixture, scratch: &mut [u8], reps: u32) -> f64 {
    let start = Instant::now();
    for _ in 0..reps {
        std::hint::black_box(run_once(imp, fx, scratch));
    }
    start.elapsed().as_secs_f64()
}

fn imp_names(imp: Imp) -> (&'static str, &'static str) {
    match imp {
        Imp::CleanflateOneShot => ("cleanflate", "oneshot"),
        Imp::CleanflateStreaming => ("cleanflate", "streaming"),
        Imp::CZlibOneShot => ("c_zlib", "oneshot"),
        Imp::CZlibStreaming => ("c_zlib", "streaming"),
    }
}

/// The per-sample run order for the selected modes: cleanflate one-shot,
/// cleanflate streaming, C zlib one-shot, C zlib streaming. C zlib runs in
/// exactly the modes selected, so every ratio has a like-for-like denominator.
fn sample_order(modes: &[Mode]) -> Vec<Imp> {
    let one = modes.contains(&Mode::OneShot);
    let stream = modes.contains(&Mode::Streaming);
    let mut v = Vec::new();
    if one {
        v.push(Imp::CleanflateOneShot);
    }
    if stream {
        v.push(Imp::CleanflateStreaming);
    }
    if one {
        v.push(Imp::CZlibOneShot);
    }
    if stream {
        v.push(Imp::CZlibStreaming);
    }
    v
}

/// Everything `run` produces.
pub struct BenchOutcome {
    /// The JSON document.
    pub json: BenchJson,
}

/// Run the benchmark. Refuses (`Err`) in a debug build.
pub fn run(args: &BenchArgs, corpora_root: &Path) -> Result<BenchOutcome, String> {
    if cfg!(debug_assertions) {
        return Err(
            "oracle bench refuses to run in a debug build -- use `cargo run --release`".to_string(),
        );
    }
    let mut results: Vec<ResultRow> = Vec::new();
    #[cfg(feature = "stats")]
    let mut stats_entries: Vec<StatsEntry> = Vec::new();
    let read_loadavg = || {
        std::fs::read_to_string("/proc/loadavg")
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string())
    };
    let loadavg = read_loadavg();
    let c_zlib = crate::bench_env::c_zlib_info(zlib_version(), crate::ffi::zlib_compile_flags());
    let cargo_profile = crate::bench_env::cargo_profile();
    let git_head = command_output("git", &["rev-parse", "HEAD"]);
    let git_dirty = !command_output("git", &["status", "--porcelain"]).is_empty();
    let mut machine = Machine {
        uname: command_output("uname", &["-srm"]),
        cpu_model: cpu_model(),
        governor: std::fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string()),
        loadavg,
        rustc: command_output("rustc", &["--version"]),
        cores: crate::bench_env::logical_cpus(),
        os: crate::bench_env::os_pretty_name(),
        kernel: command_output("uname", &["-r"]),
        loadavg_end: String::new(),
    };
    print!(
        "{}",
        crate::bench_env::render_environment(
            &machine,
            &cargo_profile,
            &c_zlib,
            &git_head,
            git_dirty
        )
    );

    for &corpus in &args.corpora {
        let inputs = inputs_for(corpus, corpora_root, args.subset_quick)?;
        for input in &inputs {
            let original = input.bytes();
            let file = input
                .id
                .split_once('-')
                .map_or(input.id.as_str(), |(_, f)| f);
            for &framing in &args.framings {
                for &level in &args.levels {
                    let params = DeflateParams {
                        level,
                        strategy: Strategy::Default,
                        window_bits: 15,
                        mem_level: 8,
                        framing,
                    };
                    let compressed = deflate_bytes(&original, &params, None)
                        .map_err(|e| format!("{}: oracle deflate failed: {e:?}", input.id))?;
                    let fx = Fixture {
                        framing,
                        original: &original,
                        compressed: &compressed,
                    };
                    let mut scratch = vec![0u8; original.len() + STREAM_CHUNK];

                    #[cfg(feature = "stats")]
                    if args.stats {
                        collect_stats(
                            &mut stats_entries,
                            corpus.name(),
                            file,
                            framing,
                            level,
                            &fx,
                            &args.modes,
                            &mut scratch,
                        )?;
                    }
                    if args.iters == 0 {
                        continue;
                    }

                    let imps = sample_order(&args.modes);

                    let mut reps = Vec::new();
                    for &imp in &imps {
                        verify(imp, &fx, &mut scratch).map_err(|e| {
                            format!(
                                "{} {} l{level} {:?}: {e}",
                                input.id,
                                framing_name(framing),
                                imp_names(imp)
                            )
                        })?;
                        let one = time_reps(imp, &fx, &mut scratch, 1);
                        reps.push(reps_for(one, MIN_SAMPLE_MS));
                    }

                    let mut samples: Vec<Vec<f64>> = vec![Vec::new(); imps.len()];
                    for _ in 0..args.iters {
                        for (k, &imp) in imps.iter().enumerate() {
                            let r = reps.get(k).copied().unwrap_or(1);
                            let total = time_reps(imp, &fx, &mut scratch, r);
                            if let Some(s) = samples.get_mut(k) {
                                s.push(total * 1e9 / r as f64);
                            }
                        }
                    }
                    for (k, &imp) in imps.iter().enumerate() {
                        let s = samples.get(k).map(Vec::as_slice).unwrap_or(&[]);
                        let (name, mode) = imp_names(imp);
                        results.push(ResultRow {
                            corpus: corpus.name().to_string(),
                            file: file.to_string(),
                            framing: framing_name(framing).to_string(),
                            level,
                            mode: mode.to_string(),
                            implementation: name.to_string(),
                            bytes: original.len(),
                            reps: reps.get(k).copied().unwrap_or(1),
                            median_ns: median(s),
                            best_ns: minimum(s),
                        });
                    }
                    eprintln!(
                        "oracle bench: {} {} l{level} done",
                        input.id,
                        framing_name(framing)
                    );
                }
            }
        }
    }

    let aggregates = aggregates_of(&results);
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    machine.loadavg_end = read_loadavg();
    let json = BenchJson {
        schema: crate::bench_json::SCHEMA,
        kind: args.kind.name().to_string(),
        task: args.task.clone(),
        git_head,
        git_dirty,
        timestamp: format_utc(secs),
        machine,
        c_zlib_version: c_zlib.version.clone(),
        cargo_profile,
        c_zlib,
        iters: args.iters,
        min_sample_ms: MIN_SAMPLE_MS,
        stats_build: cfg!(feature = "stats"),
        results,
        aggregates,
        #[cfg(feature = "stats")]
        stats: finish_stats(stats_entries),
        #[cfg(not(feature = "stats"))]
        stats: Vec::new(),
    };
    Ok(BenchOutcome { json })
}

/// Flatten a `Stats` into named counters (histograms as `<name>_<i>`).
#[cfg(feature = "stats")]
pub fn stats_counters(s: &cleanflate::Stats) -> std::collections::BTreeMap<String, u64> {
    let mut m = std::collections::BTreeMap::new();
    let mut put = |k: &str, v: u64| {
        m.insert(k.to_string(), v);
    };
    put("step_calls", s.step_calls);
    put("state_dispatches", s.state_dispatches);
    put("output_bytes", s.output_bytes);
    put("blocks_stored", s.blocks_stored);
    put("blocks_fixed", s.blocks_fixed);
    put("blocks_dynamic", s.blocks_dynamic);
    put("stored_bytes", s.stored_bytes);
    put("literals", s.literals);
    put("matches", s.matches);
    put("match_bytes", s.match_bytes);
    put("overlap_matches", s.overlap_matches);
    put("litlen_table_decodes", s.litlen_table_decodes);
    put("litlen_serial_decodes", s.litlen_serial_decodes);
    put("dist_table_decodes", s.dist_table_decodes);
    put("dist_serial_decodes", s.dist_serial_decodes);
    put("codelen_table_decodes", s.codelen_table_decodes);
    put("codelen_serial_decodes", s.codelen_serial_decodes);
    put("refills", s.refills);
    put("refill_bytes", s.refill_bytes);
    put("given_back_bytes", s.given_back_bytes);
    put("table_builds", s.table_builds);
    put("table_entries_built", s.table_entries_built);
    put("fixed_table_clones", s.fixed_table_clones);
    put("window_bytes_written", s.window_bytes_written);
    put("fast_loop_entries", s.fast_loop_entries);
    put("fast_loop_symbols", s.fast_loop_symbols);
    put("oneshot_resizes", s.oneshot_resizes);
    put("oneshot_bytes_allocated", s.oneshot_bytes_allocated);
    for (i, v) in s.match_len_hist.iter().enumerate() {
        put(&format!("match_len_hist_{i}"), *v);
    }
    for (i, v) in s.match_dist_hist.iter().enumerate() {
        put(&format!("match_dist_hist_{i}"), *v);
    }
    m
}

/// One untimed stats decode per selected mode.
#[cfg(feature = "stats")]
#[allow(clippy::too_many_arguments)]
fn collect_stats(
    out: &mut Vec<StatsEntry>,
    corpus: &str,
    file: &str,
    framing: Framing,
    level: u8,
    fx: &Fixture,
    modes: &[Mode],
    scratch: &mut [u8],
) -> Result<(), String> {
    for mode in modes {
        let (mode_name, stats) = match mode {
            Mode::OneShot => {
                let (d, s) = cleanflate::decompress_with_stats(
                    format_of(framing),
                    fx.compressed,
                    &Options::default(),
                )
                .map_err(|e| format!("{corpus}/{file}: {e:?}"))?;
                if d.data != fx.original {
                    return Err(format!("{corpus}/{file}: stats decode output differs"));
                }
                ("oneshot", s)
            }
            Mode::Streaming => {
                let (n, d) = decode_streaming_dec(format_of(framing), fx.compressed, scratch)
                    .map_err(|e| format!("{corpus}/{file}: {e}"))?;
                if scratch.get(..n) != Some(fx.original) {
                    return Err(format!("{corpus}/{file}: streaming stats output differs"));
                }
                ("streaming", d.stats())
            }
        };
        out.push(StatsEntry {
            corpus: corpus.to_string(),
            file: file.to_string(),
            framing: framing_name(framing).to_string(),
            level,
            mode: mode_name.to_string(),
            counters: stats_counters(&stats),
        });
    }
    Ok(())
}

/// Add per-(corpus, framing, level, mode) totals and print the report.
#[cfg(feature = "stats")]
fn finish_stats(mut entries: Vec<StatsEntry>) -> Vec<StatsEntry> {
    let mut groups: Vec<(String, String, u8, String)> = Vec::new();
    for e in &entries {
        let k = (e.corpus.clone(), e.framing.clone(), e.level, e.mode.clone());
        if !groups.contains(&k) {
            groups.push(k);
        }
    }
    let mut totals = Vec::new();
    for (corpus, framing, level, mode) in groups {
        let mut sum: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
        for e in entries.iter().filter(|e| {
            e.corpus == corpus && e.framing == framing && e.level == level && e.mode == mode
        }) {
            for (k, v) in &e.counters {
                let slot = sum.entry(k.clone()).or_insert(0);
                *slot = slot.saturating_add(*v);
            }
        }
        totals.push(StatsEntry {
            corpus,
            file: "*total*".to_string(),
            framing,
            level,
            mode,
            counters: sum,
        });
    }
    for t in &totals {
        print!("{}", render_stats(t));
    }
    entries.extend(totals);
    entries
}

fn ratio_pct(n: u64, d: u64) -> f64 {
    if d == 0 {
        0.0
    } else {
        n as f64 * 100.0 / d as f64
    }
}

fn per(n: u64, d: u64) -> f64 {
    if d == 0 {
        0.0
    } else {
        n as f64 / d as f64
    }
}

/// Human-readable statistics for one totals entry.
pub fn render_stats(t: &StatsEntry) -> String {
    let c = |k: &str| t.counters.get(k).copied().unwrap_or(0);
    let mut out = format!(
        "\n### stats {}/{}/l{}/{}\n\n",
        t.corpus, t.framing, t.level, t.mode
    );
    let blocks = c("blocks_stored") + c("blocks_fixed") + c("blocks_dynamic");
    out.push_str(&format!(
        "- output bytes {}, blocks {} (stored {}, fixed {}, dynamic {}), literals {}, matches {}\n",
        c("output_bytes"),
        blocks,
        c("blocks_stored"),
        c("blocks_fixed"),
        c("blocks_dynamic"),
        c("literals"),
        c("matches")
    ));
    out.push_str(&format!(
        "- litlen table-decode {:.1}%, dist table-decode {:.1}%\n",
        ratio_pct(
            c("litlen_table_decodes"),
            c("litlen_table_decodes") + c("litlen_serial_decodes")
        ),
        ratio_pct(
            c("dist_table_decodes"),
            c("dist_table_decodes") + c("dist_serial_decodes")
        )
    ));
    out.push_str(&format!(
        "- dispatches per output byte {:.3}, bytes per refill {:.2}, overlap matches {:.1}%, stored {:.1}%\n",
        per(c("state_dispatches"), c("output_bytes")),
        per(c("refill_bytes"), c("refills")),
        ratio_pct(c("overlap_matches"), c("matches")),
        ratio_pct(c("stored_bytes"), c("output_bytes"))
    ));
    out.push_str(&format!(
        "- table builds {} ({:.2} per block), one-shot resizes {}\n",
        c("table_builds"),
        per(c("table_builds"), blocks),
        c("oneshot_resizes")
    ));
    let hist = |name: &str, n: usize| -> String {
        (0..n)
            .map(|i| format!("{:.1}", ratio_pct(c(&format!("{name}_{i}")), c("matches"))))
            .collect::<Vec<_>>()
            .join(" / ")
    };
    out.push_str(&format!(
        "- match length % (3-4/5-8/9-16/17-32/33-64/65-128/129-258): {}\n",
        hist("match_len_hist", 7)
    ));
    out.push_str(&format!(
        "- match distance % (1/2-4/5-16/17-64/65-256/257-1K/1K-4K/4K-16K/16K-32K): {}\n",
        hist("match_dist_hist", 9)
    ));
    out
}

/// Aggregates for every (corpus, framing, level, cleanflate mode). The C zlib
/// denominator comes from C zlib rows of the same mode.
pub fn aggregates_of(results: &[ResultRow]) -> Vec<Aggregate> {
    let mut keys: Vec<(String, String, u8, String)> = Vec::new();
    for r in results.iter().filter(|r| r.implementation == "cleanflate") {
        let k = (r.corpus.clone(), r.framing.clone(), r.level, r.mode.clone());
        if !keys.contains(&k) {
            keys.push(k);
        }
    }
    let mut out = Vec::new();
    for (corpus, framing, level, mode) in keys {
        let pick = |imp: &str, m: &str| -> Vec<&ResultRow> {
            results
                .iter()
                .filter(|r| {
                    r.corpus == corpus
                        && r.framing == framing
                        && r.level == level
                        && r.implementation == imp
                        && r.mode == m
                })
                .collect()
        };
        let cf = pick("cleanflate", &mode);
        let cz = pick("c_zlib", &mode);
        let total: usize = cf.iter().map(|r| r.bytes).sum();
        let secs = |rows: &[&ResultRow], f: fn(&ResultRow) -> f64| -> Vec<f64> {
            rows.iter().map(|r| f(r) / 1e9).collect()
        };
        let cf_med = aggregate_mib_s(total, &secs(&cf, |r| r.median_ns));
        let cf_best = aggregate_mib_s(total, &secs(&cf, |r| r.best_ns));
        let cz_med = aggregate_mib_s(total, &secs(&cz, |r| r.median_ns));
        let cz_best = aggregate_mib_s(total, &secs(&cz, |r| r.best_ns));
        out.push(Aggregate {
            key: format!("{corpus}/{framing}/l{level}/{mode}"),
            total_bytes: total as u64,
            cleanflate_mib_s_median: cf_med,
            cleanflate_mib_s_best: cf_best,
            c_zlib_mib_s_median: cz_med,
            c_zlib_mib_s_best: cz_best,
            c_zlib_mode: mode.clone(),
            ratio: if cz_med > 0.0 {
                cf_med / cz_med
            } else {
                f64::NAN
            },
        });
    }
    out
}

/// Two-line definition footer printed under the stdout table.
pub const TABLE_FOOTER: &str = "median: throughput from the median per-decode time of the samples; best: throughput of the fastest sample. Aggregates: total bytes / sum of per-file times.\nratio (like for like): cleanflate median / C zlib median of the same mode.\n";

/// A markdown table of the aggregates.
pub fn render_markdown(json: &BenchJson) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Bench {} ({}), C zlib {}, iters {}\n\n",
        json.task, json.kind, json.c_zlib_version, json.iters
    ));
    out.push_str("| key | cleanflate MiB/s (median) | cleanflate MiB/s (best) | C zlib MiB/s (median) | ratio (like for like) |\n");
    out.push_str("|---|---:|---:|---:|---:|\n");
    for a in &json.aggregates {
        out.push_str(&format!(
            "| {} | {:.1} | {:.1} | {:.1} | {:.3} |\n",
            a.key,
            a.cleanflate_mib_s_median,
            a.cleanflate_mib_s_best,
            a.c_zlib_mib_s_median,
            a.ratio
        ));
    }
    out.push('\n');
    out.push_str(TABLE_FOOTER);
    out
}

/// Run a command and return its trimmed stdout, or `unknown`.
pub fn command_output(command: &str, args: &[&str]) -> String {
    std::process::Command::new(command)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// CPU model name from `/proc/cpuinfo`.
pub fn cpu_model() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|text| {
            text.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split(':').nth(1))
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throughput_math() {
        let ten_mib = 10 * 1024 * 1024;
        assert!((mib_per_sec(ten_mib, 1.0) - 10.0).abs() < 1e-9);
        assert!((mib_per_sec(ten_mib, 0.5) - 20.0).abs() < 1e-9);
        assert_eq!(mib_per_sec(ten_mib, 0.0), 0.0);
        assert_eq!(mib_per_sec(ten_mib, -1.0), 0.0);
        assert_eq!(mib_per_sec(0, 1.0), 0.0);
    }

    #[test]
    fn median_and_best_odd_even() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(&[4.0, 1.0, 3.0, 2.0]), 2.5);
        assert_eq!(median(&[]), 0.0);
        assert_eq!(minimum(&[3.0, 1.0, 2.0]), 1.0);
        assert_eq!(minimum(&[]), 0.0);
    }

    #[test]
    fn aggregate_is_total_bytes_over_total_median_time() {
        let mib = 1024 * 1024;
        // 2 MiB in 1 s and 6 MiB in 1 s: 8 MiB over 2 s, not the mean of rates.
        let agg = aggregate_mib_s(8 * mib, &[1.0, 1.0]);
        assert!((agg - 4.0).abs() < 1e-9);
        let row = |file: &str, imp: &str, mode: &str, bytes: usize, ns: f64| ResultRow {
            corpus: "c".into(),
            file: file.into(),
            framing: "raw".into(),
            level: 6,
            mode: mode.into(),
            implementation: imp.into(),
            bytes,
            reps: 1,
            median_ns: ns,
            best_ns: ns / 2.0,
        };
        let rows = vec![
            row("a", "cleanflate", "oneshot", 2 * mib, 1e9),
            row("b", "cleanflate", "oneshot", 6 * mib, 1e9),
            row("a", "c_zlib", "oneshot", 2 * mib, 5e8),
            row("b", "c_zlib", "oneshot", 6 * mib, 5e8),
        ];
        let aggs = aggregates_of(&rows);
        assert_eq!(aggs.len(), 1);
        assert_eq!(aggs[0].key, "c/raw/l6/oneshot");
        assert!((aggs[0].cleanflate_mib_s_median - 4.0).abs() < 1e-9);
        assert!((aggs[0].cleanflate_mib_s_best - 8.0).abs() < 1e-9);
        assert!((aggs[0].c_zlib_mib_s_median - 8.0).abs() < 1e-9);
        assert!((aggs[0].ratio - 0.5).abs() < 1e-9);
    }

    #[test]
    fn aggregate_ratio_is_like_for_like() {
        let mib = 1024 * 1024;
        let row = |imp: &str, mode: &str, ns: f64| ResultRow {
            corpus: "c".into(),
            file: "a".into(),
            framing: "raw".into(),
            level: 6,
            mode: mode.into(),
            implementation: imp.into(),
            bytes: 4 * mib,
            reps: 1,
            median_ns: ns,
            best_ns: ns,
        };
        let rows = vec![
            row("cleanflate", "oneshot", 1e9),
            row("cleanflate", "streaming", 2e9),
            row("c_zlib", "oneshot", 5e8),
            row("c_zlib", "streaming", 1e9),
        ];
        let aggs = aggregates_of(&rows);
        let get = |k: &str| aggs.iter().find(|a| a.key.ends_with(k)).expect("key");
        assert!((get("oneshot").ratio - 0.5).abs() < 1e-9);
        assert!(
            (get("streaming").ratio - 0.5).abs() < 1e-9,
            "divides by C zlib streaming"
        );
        assert_eq!(get("streaming").c_zlib_mode, "streaming");
        assert!((get("streaming").c_zlib_mib_s_median - 4.0).abs() < 1e-9);
    }

    #[test]
    fn schema1_aggregate_defaults_to_oneshot_denominator() {
        let text = r#"{"key":"c/raw/l6/streaming","total_bytes":1,"cleanflate_mib_s_median":1.0,
            "cleanflate_mib_s_min":2.0,"c_zlib_mib_s_median":3.0,"c_zlib_mib_s_min":4.0,"ratio":0.3}"#;
        let a: Aggregate = serde_json::from_str(text).expect("schema-1 aggregate parses");
        assert_eq!(a.c_zlib_mode, "oneshot");
        assert_eq!(a.cleanflate_mib_s_best, 2.0);
    }

    #[test]
    fn sample_order_includes_c_zlib_streaming() {
        let names = |modes: &[Mode]| -> Vec<(&'static str, &'static str)> {
            sample_order(modes).into_iter().map(imp_names).collect()
        };
        assert_eq!(
            names(&[Mode::Streaming, Mode::OneShot]),
            vec![
                ("cleanflate", "oneshot"),
                ("cleanflate", "streaming"),
                ("c_zlib", "oneshot"),
                ("c_zlib", "streaming"),
            ]
        );
        assert_eq!(
            names(&[Mode::Streaming]),
            vec![("cleanflate", "streaming"), ("c_zlib", "streaming")]
        );
        assert_eq!(
            names(&[Mode::OneShot]),
            vec![("cleanflate", "oneshot"), ("c_zlib", "oneshot")]
        );
    }

    #[test]
    fn reps_reach_min_sample() {
        assert_eq!(reps_for(1.0, 20), 1);
        assert_eq!(reps_for(0.010, 20), 2);
        assert_eq!(reps_for(0.001, 20), 32);
        for one in [0.3e-3, 1e-3, 7e-3, 19e-3, 0.5] {
            let r = reps_for(one, 20);
            assert!(r.is_power_of_two());
            assert!(r as f64 * one >= 0.020);
            assert!(r == 1 || (r / 2) as f64 * one < 0.020);
        }
    }

    #[test]
    fn stats_flag_requires_feature() {
        let args = vec!["--stats".to_string()];
        let parsed = parse_args(&args);
        if cfg!(feature = "stats") {
            assert!(parsed.expect("stats build accepts --stats").stats);
            let zero = vec![
                "--stats".to_string(),
                "--iters".to_string(),
                "0".to_string(),
            ];
            assert_eq!(parse_args(&zero).expect("iters 0 with stats").iters, 0);
        } else {
            assert!(parsed
                .expect_err("needs the feature")
                .contains("--features stats"));
        }
        let zero = vec!["--iters".to_string(), "0".to_string()];
        assert!(parse_args(&zero).is_err(), "iters 0 needs --stats");
    }

    #[test]
    fn quick_subset_names() {
        assert_eq!(QUICK_SILESIA, ["dickens", "mr", "ooffice", "sao", "xml"]);
    }

    #[test]
    fn full_kind_requires_seven_iters() {
        let a = |v: &[&str]| parse_args(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert!(a(&["--kind", "full", "--iters", "6"]).is_err());
        assert_eq!(a(&["--kind", "full"]).expect("default").iters, 7);
        assert!(a(&["--kind", "full", "--iters", "7"]).is_ok());
        assert!(a(&["--kind", "quick", "--iters", "3"]).is_ok());
        let p = a(&["--framings", "rfc1951,rfc1952"]).expect("aliases");
        assert_eq!(p.framings, vec![Framing::Raw, Framing::Gzip]);
    }
}

//! `oracle bench`: single-thread decompression throughput, cleanflate vs
//! C zlib (SPEC S5 M4). Release-only: refuses to run in a debug build,
//! since debug numbers are meaningless for a performance pass.

use std::path::Path;
use std::time::Instant;

use cleanflate::{decompress_with, Format, Options};

use crate::ffi::{deflate_bytes, inflate_bytes, zlib_version, DeflateParams, Framing, Strategy};
use crate::inputs::{canterbury_inputs, silesia_inputs, Input};

/// Which corpus to benchmark against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchCorpus {
    /// The Silesia corpus (SPEC's target for the >= 70% acceptance bar).
    Silesia,
    /// The Canterbury corpus, used as a fallback when Silesia is absent.
    Canterbury,
}

/// `oracle bench` parameters.
#[derive(Debug, Clone, Copy)]
pub struct BenchArgs {
    /// Which corpus to read files from.
    pub corpus: BenchCorpus,
    /// Timed repetitions per file/framing; best-of-N is reported.
    pub iters: u32,
    /// Compression level used to build the fixtures to decompress.
    pub level: u8,
}

impl Default for BenchArgs {
    fn default() -> Self {
        BenchArgs {
            corpus: BenchCorpus::Silesia,
            iters: 5,
            level: 6,
        }
    }
}

/// One file's timing result for one framing.
#[derive(Debug, Clone)]
pub struct FileResult {
    /// The input's id (e.g. `silesia-dickens`).
    pub name: String,
    /// Uncompressed length in bytes.
    pub uncompressed_len: usize,
    /// Best (lowest) cleanflate decode time, in seconds.
    pub cleanflate_secs: f64,
    /// Best (lowest) C zlib decode time, in seconds.
    pub zlib_secs: f64,
}

impl FileResult {
    /// Uncompressed MB/s.
    pub fn cleanflate_mib_s(&self) -> f64 {
        mib_per_sec(self.uncompressed_len, self.cleanflate_secs)
    }

    /// Uncompressed MB/s.
    pub fn zlib_mib_s(&self) -> f64 {
        mib_per_sec(self.uncompressed_len, self.zlib_secs)
    }

    /// cleanflate throughput / zlib throughput.
    pub fn ratio(&self) -> f64 {
        if self.zlib_secs <= 0.0 {
            return f64::NAN;
        }
        self.zlib_secs / self.cleanflate_secs
    }
}

/// `uncompressed_bytes / elapsed_secs`, converted to MiB/s. `elapsed_secs
/// <= 0.0` (a clock that didn't advance) reports `0.0` rather than
/// dividing by zero.
fn mib_per_sec(bytes: usize, elapsed_secs: f64) -> f64 {
    if elapsed_secs <= 0.0 {
        return 0.0;
    }
    (bytes as f64 / (1024.0 * 1024.0)) / elapsed_secs
}

/// Results for one framing (`raw` or `zlib`) across every file.
#[derive(Debug, Clone)]
pub struct FramingReport {
    /// `raw` or `zlib`.
    pub framing_name: &'static str,
    /// Per-file results.
    pub files: Vec<FileResult>,
}

impl FramingReport {
    /// Aggregate MiB/s: total uncompressed bytes / total best-of-N time,
    /// for cleanflate and zlib respectively.
    pub fn aggregate_mib_s(&self) -> (f64, f64) {
        let total_bytes: usize = self.files.iter().map(|f| f.uncompressed_len).sum();
        let total_cleanflate: f64 = self.files.iter().map(|f| f.cleanflate_secs).sum();
        let total_zlib: f64 = self.files.iter().map(|f| f.zlib_secs).sum();
        (
            mib_per_sec(total_bytes, total_cleanflate),
            mib_per_sec(total_bytes, total_zlib),
        )
    }

    /// Aggregate ratio: aggregate cleanflate MiB/s / aggregate zlib MiB/s.
    pub fn aggregate_ratio(&self) -> f64 {
        let (cleanflate, zlib) = self.aggregate_mib_s();
        if zlib <= 0.0 {
            return f64::NAN;
        }
        cleanflate / zlib
    }
}

/// The full `oracle bench` outcome.
#[derive(Debug, Clone)]
pub struct BenchReport {
    /// Which corpus was actually used (may differ from the request if it
    /// fell back).
    pub corpus_name: &'static str,
    /// The C zlib version linked in.
    pub zlib_version: String,
    /// One report per framing (`raw`, `zlib`).
    pub framings: Vec<FramingReport>,
}

fn inputs_for(corpus: BenchCorpus, corpora_root: &Path) -> Vec<Input> {
    match corpus {
        BenchCorpus::Silesia => silesia_inputs(corpora_root),
        BenchCorpus::Canterbury => canterbury_inputs(corpora_root),
    }
}

/// Time `f` `iters` times (each call independent, no warmup discarded)
/// and return the minimum elapsed seconds -- best-of-N, the usual way to
/// suppress scheduler/cache noise in a microbenchmark.
fn best_of<F: FnMut()>(iters: u32, mut f: F) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..iters.max(1) {
        let start = Instant::now();
        f();
        let elapsed = start.elapsed().as_secs_f64();
        if elapsed < best {
            best = elapsed;
        }
    }
    best
}

fn bench_one_framing(
    inputs: &[Input],
    framing: Framing,
    framing_name: &'static str,
    args: &BenchArgs,
) -> Result<FramingReport, String> {
    let mut files = Vec::with_capacity(inputs.len());

    for input in inputs {
        let bytes = input.bytes();
        let params = DeflateParams {
            level: args.level,
            strategy: Strategy::Default,
            window_bits: 15,
            mem_level: 8,
            framing,
        };
        let compressed = deflate_bytes(&bytes, &params, None)
            .map_err(|e| format!("{}: oracle deflate failed: {e:?}", input.id))?;

        let format = match framing {
            Framing::Raw => Format::Raw,
            Framing::Zlib => Format::Zlib,
            Framing::Gzip => Format::Gzip,
        };

        let cleanflate_secs = best_of(args.iters, || {
            let decoded = decompress_with(format, &compressed, &Options::default())
                .unwrap_or_else(|e| panic!("{}: cleanflate decode failed: {e:?}", input.id));
            std::hint::black_box(decoded.data.len());
        });

        let max_output = (bytes.len() as u64).saturating_add(1024);
        let zlib_secs = best_of(args.iters, || {
            let outcome = inflate_bytes(
                &compressed,
                framing,
                None,
                max_output.min(usize::MAX as u64) as usize,
            );
            std::hint::black_box(outcome.output.len());
        });

        files.push(FileResult {
            name: input.id.clone(),
            uncompressed_len: bytes.len(),
            cleanflate_secs,
            zlib_secs,
        });
    }

    Ok(FramingReport {
        framing_name,
        files,
    })
}

/// Run the full benchmark. Refuses (`Err`) in a debug build.
pub fn run(args: &BenchArgs, corpora_root: &Path) -> Result<BenchReport, String> {
    if cfg!(debug_assertions) {
        return Err(
            "oracle bench refuses to run in a debug build -- use `cargo run --release`".to_string(),
        );
    }

    let inputs = inputs_for(args.corpus, corpora_root);
    if inputs.is_empty() {
        return Err(format!(
            "no input files found for {:?} under {}",
            args.corpus,
            corpora_root.display()
        ));
    }

    let raw = bench_one_framing(&inputs, Framing::Raw, "raw", args)?;
    let zlib = bench_one_framing(&inputs, Framing::Zlib, "zlib", args)?;

    Ok(BenchReport {
        corpus_name: match args.corpus {
            BenchCorpus::Silesia => "silesia",
            BenchCorpus::Canterbury => "canterbury",
        },
        zlib_version: zlib_version(),
        framings: vec![raw, zlib],
    })
}

/// Render `report` as a markdown fragment: one table per framing plus an
/// aggregate line, suitable for pasting into `docs/PERF.md`.
pub fn render_markdown(report: &BenchReport) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Corpus: {} | zlib {}\n\n",
        report.corpus_name, report.zlib_version
    ));

    for framing in &report.framings {
        out.push_str(&format!("### {}\n\n", framing.framing_name));
        out.push_str("| file | bytes | cleanflate MiB/s | zlib MiB/s | ratio |\n");
        out.push_str("|---|---:|---:|---:|---:|\n");
        for f in &framing.files {
            out.push_str(&format!(
                "| {} | {} | {:.1} | {:.1} | {:.2} |\n",
                f.name,
                f.uncompressed_len,
                f.cleanflate_mib_s(),
                f.zlib_mib_s(),
                f.ratio()
            ));
        }
        let (cleanflate_agg, zlib_agg) = framing.aggregate_mib_s();
        out.push_str(&format!(
            "| **aggregate** | | **{:.1}** | **{:.1}** | **{:.2}** |\n\n",
            cleanflate_agg,
            zlib_agg,
            framing.aggregate_ratio()
        ));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::mib_per_sec;

    #[test]
    fn throughput_math() {
        // 10 MiB in 1 second is 10 MiB/s.
        let ten_mib = 10 * 1024 * 1024;
        assert!((mib_per_sec(ten_mib, 1.0) - 10.0).abs() < 1e-9);

        // Half the time, double the throughput.
        assert!((mib_per_sec(ten_mib, 0.5) - 20.0).abs() < 1e-9);

        // A non-positive elapsed time (clock didn't advance) must not
        // divide by zero or go negative/infinite.
        assert_eq!(mib_per_sec(ten_mib, 0.0), 0.0);
        assert_eq!(mib_per_sec(ten_mib, -1.0), 0.0);

        // Zero bytes is zero throughput regardless of time.
        assert_eq!(mib_per_sec(0, 1.0), 0.0);
    }
}

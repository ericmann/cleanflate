//! `oracle`: dev-only CLI for corpus and fixture generation. Run from the
//! repository root (as the Makefile does).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use oracle::bench::{self, BenchArgs, BenchCorpus};
use oracle::generate;
use oracle::malformed;
use oracle::matrix::CorpusTier;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("malformed") => match run_malformed() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("oracle malformed: {e}");
                ExitCode::FAILURE
            }
        },
        Some("generate") => match run_generate(&args[2..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("oracle generate: {e}");
                ExitCode::FAILURE
            }
        },
        Some("bench") => match run_bench(&args[2..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("oracle bench: {e}");
                ExitCode::FAILURE
            }
        },
        Some("regress-add") => match oracle::regress::run_add(&args[2..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("oracle regress-add: {e}");
                ExitCode::FAILURE
            }
        },
        Some("regress-seed") => match oracle::regress::run_seed() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("oracle regress-seed: {e}");
                ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!(
                "usage: oracle regress-add --input PATH --format raw|zlib|gzip --name NAME [--in-chunks LIST] [--out-chunks LIST] [--expect ok|err:KIND] --origin TEXT | oracle regress-seed | oracle malformed | oracle generate --tier quick|full [--write-manifest] [--out DIR] | oracle bench [--corpus silesia|canterbury] [--iters N] [--level L]"
            );
            ExitCode::FAILURE
        }
    }
}

struct GenerateArgs {
    tier: CorpusTier,
    write_manifest: bool,
    out: PathBuf,
}

fn parse_generate_args(args: &[String]) -> Result<GenerateArgs, String> {
    let mut tier: Option<CorpusTier> = None;
    let mut write_manifest = false;
    let mut out = generate::default_out_dir();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--tier" => {
                let v = args
                    .get(i + 1)
                    .ok_or_else(|| "--tier requires a value".to_string())?;
                tier = Some(match v.as_str() {
                    "quick" => CorpusTier::Quick,
                    "full" => CorpusTier::Full,
                    other => return Err(format!("unknown --tier value: {other}")),
                });
                i += 2;
            }
            "--write-manifest" => {
                write_manifest = true;
                i += 1;
            }
            "--out" => {
                let v = args
                    .get(i + 1)
                    .ok_or_else(|| "--out requires a value".to_string())?;
                out = PathBuf::from(v);
                i += 2;
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(GenerateArgs {
        tier: tier.ok_or_else(|| "--tier quick|full is required".to_string())?,
        write_manifest,
        out,
    })
}

fn run_generate(args: &[String]) -> Result<(), String> {
    let parsed = parse_generate_args(args)?;
    let corpora_root = generate::default_corpora_root();
    let manifest_path = generate::default_manifest_path();

    fs::create_dir_all(&parsed.out)
        .map_err(|e| format!("creating {}: {e}", parsed.out.display()))?;

    let report = if parsed.write_manifest {
        generate::write_manifest(parsed.tier, &parsed.out, &manifest_path, &corpora_root)?
    } else {
        generate::check(parsed.tier, &parsed.out, &manifest_path, &corpora_root)?
    };

    if !report.problems.is_empty() {
        eprintln!("oracle generate: {} problem(s):", report.problems.len());
        for problem in &report.problems {
            eprintln!("  {problem}");
        }
        return Err(format!("{} problem(s) found", report.problems.len()));
    }

    println!(
        "oracle generate: {} generated, {} skipped (already up to date)",
        report.generated, report.skipped
    );
    Ok(())
}

fn run_malformed() -> Result<(), String> {
    let root = Path::new("testdata/malformed");
    for dir in ["rfc1951", "rfc1950", "rfc1952"] {
        let out_dir = root.join(dir);
        fs::create_dir_all(&out_dir).map_err(|e| format!("creating {}: {e}", out_dir.display()))?;
    }

    let mut cases = malformed::cases();
    cases.extend(malformed::rfc1950_cases());
    cases.extend(malformed::rfc1952_cases());

    let mut tsv = String::new();
    tsv.push_str("# Hand-built malformed corpus (SPEC S4): RFC 1951/1950/1952.\n");
    tsv.push_str("# path\texpected\trule\n");

    for case in &cases {
        let rel_path = format!("{}/{}.bin", case.family.dir_name(), case.name);
        let full_path = root.join(&rel_path);
        fs::write(&full_path, &case.bytes)
            .map_err(|e| format!("writing {}: {e}", full_path.display()))?;
        tsv.push_str(&format!("{rel_path}\t{}\t{}\n", case.expected, case.rule));
        if case.sweep {
            tsv.push_str(&format!(
                "sweep:{rel_path}\tTruncated\tevery strict prefix of a valid stream is truncated\n"
            ));
        }
    }

    let tsv_path = root.join("EXPECTED.tsv");
    fs::write(&tsv_path, tsv).map_err(|e| format!("writing {}: {e}", tsv_path.display()))?;

    println!(
        "oracle malformed: wrote {} cases to {}",
        cases.len(),
        root.display()
    );
    Ok(())
}

fn parse_bench_args(args: &[String]) -> Result<BenchArgs, String> {
    let mut result = BenchArgs::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--corpus" => {
                let v = args
                    .get(i + 1)
                    .ok_or_else(|| "--corpus requires a value".to_string())?;
                result.corpus = match v.as_str() {
                    "silesia" => BenchCorpus::Silesia,
                    "canterbury" => BenchCorpus::Canterbury,
                    other => return Err(format!("unknown --corpus value: {other}")),
                };
                i += 2;
            }
            "--iters" => {
                let v = args
                    .get(i + 1)
                    .ok_or_else(|| "--iters requires a value".to_string())?;
                result.iters = v.parse().map_err(|e| format!("--iters: {e}"))?;
                i += 2;
            }
            "--level" => {
                let v = args
                    .get(i + 1)
                    .ok_or_else(|| "--level requires a value".to_string())?;
                result.level = v.parse().map_err(|e| format!("--level: {e}"))?;
                i += 2;
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(result)
}

fn command_output(command: &str, args: &[&str]) -> String {
    std::process::Command::new(command)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| format!("<{command} unavailable>"))
}

fn cpu_model() -> String {
    fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|text| {
            text.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split(':').nth(1))
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "<unknown CPU>".to_string())
}

fn run_bench(args: &[String]) -> Result<(), String> {
    let mut bench_args = parse_bench_args(args)?;
    let corpora_root = generate::default_corpora_root();

    if bench_args.corpus == BenchCorpus::Silesia
        && oracle::inputs::silesia_inputs(&corpora_root).is_empty()
    {
        eprintln!(
            "oracle bench: Silesia corpus missing, falling back to Canterbury \
             (run tools/cleanroom/fetch-corpora.sh silesia for the SPEC S5 M4 target corpus)"
        );
        bench_args.corpus = BenchCorpus::Canterbury;
    }

    let report = bench::run(&bench_args, &corpora_root)?;
    let table = bench::render_markdown(&report);

    let uname = command_output("uname", &["-srm"]);
    let rustc = command_output("rustc", &["--version"]);
    let cpu = cpu_model();

    let doc = format!(
        "# PERF.md\n\n\
         Single-thread decompression throughput, cleanflate vs C zlib \
         (SPEC S5 M4). Method: `oracle bench` compresses each corpus file \
         with C zlib at the given level (raw and zlib framing), then times \
         best-of-{iters} `cleanflate::decompress_with` against \
         `oracle::ffi::inflate_bytes` with `std::time::Instant`, \
         single-threaded, release build only. Throughput is uncompressed \
         MiB/s; ratio is cleanflate / zlib (>= 1.0 means cleanflate is at \
         least as fast).\n\n\
         Machine: {uname}\n\
         CPU: {cpu}\n\
         Compiler: {rustc}\n\n\
         ## Baseline (P4-07)\n\n\
         {table}",
        iters = bench_args.iters,
    );

    let path = Path::new("docs/PERF.md");
    fs::write(path, doc).map_err(|e| format!("writing {}: {e}", path.display()))?;

    println!("oracle bench: wrote {}", path.display());
    Ok(())
}

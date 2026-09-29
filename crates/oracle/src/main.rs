//! `oracle`: dev-only CLI for corpus and fixture generation. Run from the
//! repository root (as the Makefile does).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use oracle::bench;
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
                "usage: oracle regress-add --input PATH --format raw|zlib|gzip --name NAME [--in-chunks LIST] [--out-chunks LIST] [--expect ok|err:KIND] --origin TEXT | oracle regress-seed | oracle malformed | oracle generate --tier quick|full [--write-manifest] [--out DIR] | oracle bench [--kind full|quick|custom] [--corpus LIST] [--subset quick] [--framings LIST] [--levels LIST] [--modes LIST] [--iters N] [--task ID] [--json PATH]"
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

fn run_bench(args: &[String]) -> Result<(), String> {
    let bench_args = bench::parse_args(args)?;
    let corpora_root = generate::default_corpora_root();
    let outcome = bench::run(&bench_args, &corpora_root)?;
    print!("{}", bench::render_markdown(&outcome.json));

    if let Some(path) = &bench_args.json {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("creating {}: {e}", parent.display()))?;
        }
        let text = serde_json::to_string_pretty(&outcome.json)
            .map_err(|e| format!("serializing results: {e}"))?;
        fs::write(path, text + "\n").map_err(|e| format!("writing {}: {e}", path.display()))?;
        println!("\noracle bench: wrote {}", path.display());
    }
    Ok(())
}

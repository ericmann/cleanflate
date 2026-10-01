//! Cleanflate-only checksum microbenchmark: MiB/s per algorithm, per
//! implementation variant and buffer size. Needs the `test-hooks` feature.
//!
//! Usage: `checksum_bench [--sizes 64,4096,65536,1048576] [--total-mib 256]
//! [--iters 7] [--task ID] [--json PATH]`.

use std::process::{Command, ExitCode};
use std::time::Instant;

use cleanflate::test_hooks::checksum as hooks;

type Variant = (&'static str, &'static str, fn(u32, &[u8]) -> u32, u32);

/// (algorithm, variant, function, initial finished value).
fn variants() -> Vec<Variant> {
    vec![
        ("crc32", "bytewise", hooks::crc32_bytewise, 0),
        ("crc32", "sliced4", hooks::crc32_sliced4, 0),
        ("crc32", "sliced8", hooks::crc32_sliced8, 0),
        ("crc32", "sliced16", hooks::crc32_sliced16, 0),
        ("crc32", "production", hooks::crc32_production, 0),
        ("adler32", "bytewise", hooks::adler32_bytewise, 1),
        ("adler32", "production", hooks::adler32_production, 1),
    ]
}

struct Args {
    sizes: Vec<usize>,
    total_mib: usize,
    iters: usize,
    task: String,
    json: Option<String>,
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        sizes: vec![64, 4096, 65_536, 1_048_576],
        total_mib: 256,
        iters: 7,
        task: "adhoc".to_string(),
        json: None,
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let flag = args.get(i).map(String::as_str).unwrap_or("");
        let value = args
            .get(i + 1)
            .ok_or_else(|| format!("{flag} requires a value"))?;
        match flag {
            "--sizes" => {
                a.sizes = value
                    .split(',')
                    .map(|s| s.parse::<usize>().map_err(|e| format!("--sizes: {e}")))
                    .collect::<Result<_, _>>()?;
            }
            "--total-mib" => {
                a.total_mib = value.parse().map_err(|e| format!("--total-mib: {e}"))?
            }
            "--iters" => a.iters = value.parse().map_err(|e| format!("--iters: {e}"))?,
            "--task" => a.task = value.clone(),
            "--json" => a.json = Some(value.clone()),
            other => return Err(format!("unknown argument {other}")),
        }
        i += 2;
    }
    if a.sizes.is_empty() || a.sizes.contains(&0) || a.iters == 0 || a.total_mib == 0 {
        return Err("--sizes, --iters and --total-mib must be non-zero".to_string());
    }
    Ok(a)
}

/// Fixed-seed xorshift64 bytes.
fn fill(len: usize) -> Vec<u8> {
    let mut x = 0x9E37_79B9_7F4A_7C15u64;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 24) as u8
        })
        .collect()
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    let mid = v.len() / 2;
    if v.len() % 2 == 1 {
        v.get(mid).copied().unwrap_or(0.0)
    } else {
        (v.get(mid.wrapping_sub(1)).copied().unwrap_or(0.0) + v.get(mid).copied().unwrap_or(0.0))
            / 2.0
    }
}

fn run_cmd(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let variants = variants();
    let max = args.sizes.iter().copied().max().unwrap_or(0);
    let buf = fill(max);

    // Every variant of an algorithm must agree on every size first.
    for &size in &args.sizes {
        let data = buf.get(..size).ok_or("buffer shorter than a size")?;
        for algo in ["crc32", "adler32"] {
            let mut expected: Option<u32> = None;
            for (a, name, f, init) in &variants {
                if *a != algo {
                    continue;
                }
                let got = f(*init, data);
                match expected {
                    None => expected = Some(got),
                    Some(e) if e == got => {}
                    Some(e) => {
                        return Err(format!(
                            "{algo} {name} size {size}: {got:#010x} differs from {e:#010x}"
                        ))
                    }
                }
            }
        }
    }

    let total_bytes = args.total_mib * 1024 * 1024;
    let mut rows: Vec<(String, String, usize, f64, f64)> = Vec::new();
    println!("| algo | variant | size | median MiB/s | best MiB/s |");
    println!("|---|---|---:|---:|---:|");
    for (algo, name, f, init) in &variants {
        for &size in &args.sizes {
            let data = buf.get(..size).ok_or("buffer shorter than a size")?;
            let reps = (total_bytes / size).max(1);
            let mut samples = Vec::with_capacity(args.iters);
            for _ in 0..args.iters {
                let start = Instant::now();
                let mut acc = *init;
                for _ in 0..reps {
                    acc = f(acc, std::hint::black_box(data));
                }
                std::hint::black_box(acc);
                let secs = start.elapsed().as_secs_f64().max(1e-9);
                samples.push((reps * size) as f64 / (1024.0 * 1024.0) / secs);
            }
            let best = samples.iter().copied().fold(0.0, f64::max);
            let med = median(&mut samples);
            println!("| {algo} | {name} | {size} | {med:.0} | {best:.0} |");
            rows.push((algo.to_string(), name.to_string(), size, med, best));
        }
    }

    if let Some(path) = &args.json {
        let mut out = String::new();
        out.push_str(&format!(
            "{{\"schema\": 1, \"kind\": \"checksum\", \"task\": \"{}\", \"git_head\": \"{}\", \"git_dirty\": {}, \"timestamp\": \"{}\", \"rows\": [\n",
            args.task,
            run_cmd("git", &["rev-parse", "HEAD"]),
            !run_cmd("git", &["status", "--porcelain"]).is_empty(),
            run_cmd("date", &["-u", "+%Y-%m-%dT%H:%M:%SZ"]),
        ));
        for (k, (algo, variant, size, med, best)) in rows.iter().enumerate() {
            out.push_str(&format!(
                "  {{\"algo\": \"{algo}\", \"variant\": \"{variant}\", \"size\": {size}, \"median_mib_s\": {med:.1}, \"best_mib_s\": {best:.1}, \"iters\": {}}}{}\n",
                args.iters,
                if k + 1 == rows.len() { "" } else { "," }
            ));
        }
        out.push_str("]}\n");
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("creating {parent:?}: {e}"))?;
        }
        std::fs::write(path, out).map_err(|e| format!("writing {path}: {e}"))?;
        println!("\nchecksum_bench: wrote {path}");
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("checksum_bench: {e}");
            ExitCode::FAILURE
        }
    }
}

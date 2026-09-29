//! `oracle`: dev-only CLI for corpus and fixture generation. Run from the
//! repository root (as the Makefile does).

use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use oracle::malformed;

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
        _ => {
            eprintln!("usage: oracle malformed");
            ExitCode::FAILURE
        }
    }
}

fn run_malformed() -> Result<(), String> {
    let out_dir = Path::new("testdata/malformed/rfc1951");
    fs::create_dir_all(out_dir).map_err(|e| format!("creating {}: {e}", out_dir.display()))?;

    let cases = malformed::cases();
    let mut tsv = String::new();
    tsv.push_str("# Hand-built raw-DEFLATE malformed corpus (SPEC S4).\n");
    tsv.push_str("# path\texpected\trule\n");

    for case in &cases {
        let rel_path = format!("rfc1951/{}.bin", case.name);
        let full_path = Path::new("testdata/malformed").join(&rel_path);
        fs::write(&full_path, &case.bytes)
            .map_err(|e| format!("writing {}: {e}", full_path.display()))?;
        tsv.push_str(&format!("{rel_path}\t{}\t{}\n", case.expected, case.rule));
        if case.sweep {
            tsv.push_str(&format!(
                "sweep:{rel_path}\tTruncated\tevery strict prefix of a valid stream is truncated\n"
            ));
        }
    }

    let tsv_path = Path::new("testdata/malformed/EXPECTED.tsv");
    fs::write(tsv_path, tsv).map_err(|e| format!("writing {}: {e}", tsv_path.display()))?;

    println!(
        "oracle malformed: wrote {} cases to {}",
        cases.len(),
        out_dir.display()
    );
    Ok(())
}

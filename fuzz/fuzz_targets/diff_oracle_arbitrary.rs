//! Differential fuzz target (c): feed arbitrary bytes (not necessarily
//! anything C zlib itself produced) to `cleanflate` and to the oracle's
//! own decoder, under every framing, and classify how their
//! accept/reject verdicts compare. Divergences are *logged*, not
//! failures -- an "arbitrary bytes accepted differently" mismatch is
//! expected background noise from two independently-written decoders
//! disagreeing on the edges of an ambiguous or malformed input (SPEC S5
//! M4 (c)); only a panic or timeout is a fuzz failure here.
#![no_main]

use std::fs::OpenOptions;
use std::io::Write;

use libfuzzer_sys::fuzz_target;

use cleanflate::{decompress_with, Error, Format, Options, TrailingData};
use oracle::ffi::{inflate_bytes, Framing, OracleStatus};

const FUZZ_MAX_OUTPUT: u64 = 1 << 24;
const MAX_LOGGED_INPUT_BYTES: usize = 4096;

fn default_log_path() -> String {
    std::env::var("CLEANFLATE_DIVERGENCE_LOG")
        .unwrap_or_else(|_| "fuzz/artifacts/divergences.jsonl".to_string())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn oracle_status_str(status: OracleStatus) -> String {
    match status {
        OracleStatus::StreamEnd => "StreamEnd".to_string(),
        OracleStatus::DataError => "DataError".to_string(),
        OracleStatus::NeedDict => "NeedDict".to_string(),
        OracleStatus::Truncated => "Truncated".to_string(),
        OracleStatus::OutputLimit => "OutputLimit".to_string(),
        OracleStatus::OtherError(code) => format!("OtherError({code})"),
    }
}

fn framing_token(framing: Framing) -> &'static str {
    match framing {
        Framing::Raw => "raw",
        Framing::Zlib => "zlib",
        Framing::Gzip => "gzip",
    }
}

#[allow(clippy::too_many_arguments)]
fn log_divergence(
    framing: Framing,
    class: &str,
    clean_kind: &str,
    oracle_status: &str,
    data: &[u8],
) {
    let path = default_log_path();
    let truncated = data
        .get(..MAX_LOGGED_INPUT_BYTES.min(data.len()))
        .unwrap_or(data);
    let line = format!(
        "{{\"framing\":\"{}\",\"class\":\"{}\",\"clean_kind\":\"{}\",\"oracle_status\":\"{}\",\"input_hex\":\"{}\"}}\n",
        framing_token(framing),
        class,
        clean_kind,
        oracle_status,
        hex_encode(truncated),
    );
    if let Some(parent) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = f.write_all(line.as_bytes());
    }
}

fn to_cleanflate_format(framing: Framing) -> Format {
    match framing {
        Framing::Raw => Format::Raw,
        Framing::Zlib => Format::Zlib,
        Framing::Gzip => Format::Gzip,
    }
}

fuzz_target!(|data: &[u8]| {
    for &framing in &[Framing::Raw, Framing::Zlib, Framing::Gzip] {
        let format = to_cleanflate_format(framing);
        let options = Options {
            trailing: TrailingData::Allow,
            max_output: FUZZ_MAX_OUTPUT,
            ..Options::default()
        };
        let clean_result = decompress_with(format, data, &options);
        let oracle_outcome = inflate_bytes(data, framing, None, FUZZ_MAX_OUTPUT as usize);

        let clean_ok = clean_result.is_ok();
        let oracle_ok = matches!(oracle_outcome.status, OracleStatus::StreamEnd);
        let clean_kind = match &clean_result {
            Ok(_) => "OK".to_string(),
            Err(e) => error_kind_str(e),
        };
        let oracle_status = oracle_status_str(oracle_outcome.status);

        match (clean_ok, oracle_ok) {
            (true, true) => {
                let clean_data = &clean_result.as_ref().expect("checked Ok above").data;
                if *clean_data == oracle_outcome.output {
                    // both_ok_same: not logged.
                } else {
                    log_divergence(framing, "both_ok_differ", &clean_kind, &oracle_status, data);
                }
            }
            (false, false) => {
                // both_err: not logged.
            }
            (true, false) => {
                log_divergence(
                    framing,
                    "clean_ok_oracle_err",
                    &clean_kind,
                    &oracle_status,
                    data,
                );
            }
            (false, true) => {
                log_divergence(
                    framing,
                    "clean_err_oracle_ok",
                    &clean_kind,
                    &oracle_status,
                    data,
                );
            }
        }
    }
});

fn error_kind_str(e: &Error) -> String {
    e.kind().as_str().to_string()
}

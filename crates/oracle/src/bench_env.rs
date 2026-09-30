//! Environment capture for bench runs: OS, toolchain profile and the linked
//! C library's build (SPEC 4.2, "Environment"). Nothing here reads any
//! library source; only the version, build flags and Cargo metadata.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The Cargo build profile the bench was built with.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CargoProfile {
    /// Always `release` (the bench refuses debug builds).
    pub profile: String,
    /// `cfg!(debug_assertions)` of the bench binary.
    pub debug_assertions: bool,
    /// The `[profile.release]` table of the workspace `Cargo.toml`, verbatim.
    pub release_profile_toml: String,
    /// `CARGO_PROFILE_*`, `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS` and
    /// `CARGO_BUILD_RUSTFLAGS` variables present at run time.
    pub env: BTreeMap<String, String>,
}

/// The C library build the bench timed against.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CZlibInfo {
    /// The library's version string.
    pub version: String,
    /// The library's build flags as `0x<hex>`.
    pub compile_flags: String,
    /// Version of the `libz-sys` package, or `unknown`.
    pub libz_sys_version: String,
    /// Resolved features of the `libz-sys` package.
    pub libz_sys_features: Vec<String>,
}

/// `PRETTY_NAME` from os-release text, else `unknown`.
pub fn pretty_name_from(text: &str) -> String {
    text.lines()
        .find_map(|l| l.strip_prefix("PRETTY_NAME="))
        .map(|v| v.trim().trim_matches('"').to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

/// `PRETTY_NAME` of the os-release file at `path`, else `unknown`.
pub fn os_pretty_name_at(path: &str) -> String {
    std::fs::read_to_string(path)
        .map(|t| pretty_name_from(&t))
        .unwrap_or_else(|_| "unknown".to_string())
}

/// `PRETTY_NAME` from `/etc/os-release`, else `unknown`.
pub fn os_pretty_name() -> String {
    os_pretty_name_at("/etc/os-release")
}

/// The `[profile.release]` table of a `Cargo.toml` text, verbatim from its
/// header line to the line before the next table header.
pub fn release_profile_section(toml: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut inside = false;
    for line in toml.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            if inside {
                break;
            }
            inside = t == "[profile.release]";
        }
        if inside {
            out.push(line);
        }
    }
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }
    out.join("\n")
}

/// The variables of the environment that shape the build.
pub fn profile_env_from(vars: impl Iterator<Item = (String, String)>) -> BTreeMap<String, String> {
    vars.filter(|(k, _)| {
        k.starts_with("CARGO_PROFILE_")
            || k == "RUSTFLAGS"
            || k == "CARGO_ENCODED_RUSTFLAGS"
            || k == "CARGO_BUILD_RUSTFLAGS"
    })
    .collect()
}

/// The captured Cargo profile of this run.
pub fn cargo_profile() -> CargoProfile {
    let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml");
    let toml = std::fs::read_to_string(manifest).unwrap_or_default();
    CargoProfile {
        profile: "release".to_string(),
        debug_assertions: cfg!(debug_assertions),
        release_profile_toml: release_profile_section(&toml),
        env: profile_env_from(std::env::vars()),
    }
}

/// `0x<hex>` of the C library's compile flags.
pub fn compile_flags_hex(flags: u64) -> String {
    format!("0x{flags:x}")
}

/// The version and resolved features of package `name` from
/// `cargo metadata --format-version 1` output.
pub fn metadata_package(json: &str, name: &str) -> Option<(String, Vec<String>)> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let pkg = v
        .get("packages")?
        .as_array()?
        .iter()
        .find(|p| p.get("name").and_then(|n| n.as_str()) == Some(name))?;
    let version = pkg.get("version")?.as_str()?.to_string();
    let id = pkg.get("id")?.as_str()?;
    let node = v
        .get("resolve")?
        .get("nodes")?
        .as_array()?
        .iter()
        .find(|n| n.get("id").and_then(|i| i.as_str()) == Some(id))?;
    let features = node
        .get("features")?
        .as_array()?
        .iter()
        .filter_map(|f| f.as_str().map(str::to_string))
        .collect();
    Some((version, features))
}

/// The host target triple from `rustc -vV`.
fn host_triple() -> Option<String> {
    let out = std::process::Command::new("rustc")
        .arg("-vV")
        .output()
        .ok()?;
    String::from_utf8(out.stdout)
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("host: ").map(|h| h.trim().to_string()))
}

/// Run `cargo metadata` once and extract `libz-sys`; `unknown` on any
/// failure, with a warning, never an abort.
pub fn libz_sys_info() -> (String, Vec<String>) {
    // Restricting to the host platform keeps `--offline` working: other
    // platforms' dependencies need not be downloaded.
    let host = host_triple();
    let mut args = vec!["metadata", "--format-version", "1", "--offline"];
    if let Some(h) = host.as_deref() {
        args.extend(["--filter-platform", h]);
    }
    let out = std::process::Command::new("cargo").args(&args).output();
    let parsed = out
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|t| metadata_package(&t, "libz-sys"));
    match parsed {
        Some(p) => p,
        None => {
            eprintln!("oracle bench: warning: could not read libz-sys version/features from cargo metadata");
            ("unknown".to_string(), vec!["unknown".to_string()])
        }
    }
}

/// Capture the C library info (`version` supplied by the caller).
pub fn c_zlib_info(version: String, flags: u64) -> CZlibInfo {
    let (libz_sys_version, libz_sys_features) = libz_sys_info();
    CZlibInfo {
        version,
        compile_flags: compile_flags_hex(flags),
        libz_sys_version,
        libz_sys_features,
    }
}

/// The `Environment` block printed at the top of a bench run.
pub fn render_environment(
    machine: &crate::bench_json::Machine,
    profile: &CargoProfile,
    c: &CZlibInfo,
    git_head: &str,
    git_dirty: bool,
) -> String {
    let mut out = String::from("Environment\n");
    let mut line = |k: &str, v: &str| out.push_str(&format!("  {k}: {v}\n"));
    line("cpu_model", &machine.cpu_model);
    line("cores", &machine.cores.to_string());
    line("governor", &machine.governor);
    line("os", &machine.os);
    line("kernel", &machine.kernel);
    line("uname", &machine.uname);
    line("loadavg_start", &machine.loadavg);
    line("rustc", &machine.rustc);
    line("profile", &profile.profile);
    line("debug_assertions", &profile.debug_assertions.to_string());
    line(
        "release_profile",
        &profile.release_profile_toml.replace('\n', "; "),
    );
    let env: Vec<String> = profile
        .env
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    line(
        "build_env",
        &if env.is_empty() {
            "none".to_string()
        } else {
            env.join(" ")
        },
    );
    line("c_zlib_version", &c.version);
    line("c_zlib_compile_flags", &c.compile_flags);
    line("libz_sys_version", &c.libz_sys_version);
    line("libz_sys_features", &c.libz_sys_features.join(","));
    line("git_head", git_head);
    line("git_dirty", &git_dirty.to_string());
    out.push('\n');
    out
}

/// Logical CPUs, 0 when unknown.
pub fn logical_cpus() -> usize {
    std::thread::available_parallelism().map_or(0, |n| n.get())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_json_roundtrip() {
        let p = CargoProfile {
            profile: "release".into(),
            debug_assertions: false,
            release_profile_toml: "[profile.release]\ndebug = 1".into(),
            env: BTreeMap::from([("RUSTFLAGS".to_string(), "-C x".to_string())]),
        };
        let back: CargoProfile =
            serde_json::from_str(&serde_json::to_string(&p).expect("ser")).expect("de");
        assert_eq!(back, p);
        let c = CZlibInfo {
            version: "1.3".into(),
            compile_flags: "0xa9".into(),
            libz_sys_version: "1.1.2".into(),
            libz_sys_features: vec!["static".into()],
        };
        let back: CZlibInfo =
            serde_json::from_str(&serde_json::to_string(&c).expect("ser")).expect("de");
        assert_eq!(back, c);
    }

    #[test]
    fn release_profile_section_extracted() {
        let toml = "[workspace]\nmembers = [\"a\"]\n\n[profile.dev]\nopt-level = 1\n\n[profile.release]\ndebug = \"line-tables-only\"\nlto = false\n\n[profile.bench]\nx = 1\n";
        assert_eq!(
            release_profile_section(toml),
            "[profile.release]\ndebug = \"line-tables-only\"\nlto = false"
        );
        assert_eq!(release_profile_section("[workspace]\n"), "");
    }

    #[test]
    fn metadata_version_and_features_parsed() {
        let json = r#"{"packages":[{"name":"other","version":"0.1.0","id":"o 0.1.0"},
            {"name":"libz-sys","version":"1.1.29","id":"libz-sys 1.1.29"}],
            "resolve":{"nodes":[{"id":"o 0.1.0","features":[]},
            {"id":"libz-sys 1.1.29","features":["libc","static"]}]}}"#;
        let (v, f) = metadata_package(json, "libz-sys").expect("found");
        assert_eq!(v, "1.1.29");
        assert_eq!(f, vec!["libc", "static"]);
        assert!(metadata_package(json, "absent").is_none());
        assert!(metadata_package("not json", "libz-sys").is_none());
    }

    #[test]
    fn compile_flags_hex_format() {
        assert_eq!(compile_flags_hex(0xa9), "0xa9");
        assert_eq!(compile_flags_hex(0), "0x0");
        let live = compile_flags_hex(crate::ffi::zlib_compile_flags());
        assert!(live.starts_with("0x") && live.len() > 2);
    }

    #[test]
    fn missing_os_release_is_unknown() {
        assert_eq!(os_pretty_name_at("/nonexistent/os-release"), "unknown");
        assert_eq!(pretty_name_from("ID=x\n"), "unknown");
        assert_eq!(
            pretty_name_from("PRETTY_NAME=\"Pop!_OS 24\"\n"),
            "Pop!_OS 24"
        );
    }

    #[test]
    fn profile_env_filters_names() {
        let vars = vec![
            (
                "CARGO_PROFILE_RELEASE_DEBUG".to_string(),
                "true".to_string(),
            ),
            ("RUSTFLAGS".to_string(), "-C x".to_string()),
            ("HOME".to_string(), "/h".to_string()),
        ];
        let m = profile_env_from(vars.into_iter());
        assert_eq!(m.len(), 2);
        assert!(!m.contains_key("HOME"));
    }
}

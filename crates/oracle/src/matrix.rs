//! The bounded parameter matrix the conformance corpus is generated from
//! (SPEC S4; PLAN.md P1-02 sub-matrices A-D).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::ffi::{Framing, Strategy};
use crate::inputs::{find_canterbury_file, Input, Tier};

/// Deterministic seed for every synthetic input and for the corpus
/// generation manifest.
pub const CORPUS_SEED: u64 = 0x636C_6561_6E66_6C61;

const FOUR_MIB: usize = 4 * 1024 * 1024;
const ONE_MIB: usize = 1024 * 1024;

/// One (input, compression parameters) pair the oracle will generate.
#[derive(Debug, Clone)]
pub struct EntrySpec {
    /// Unique id: `<input-id>__<framing>_l<level>_<strategy>_w<wb>_m<mem>[_dict]`.
    pub id: String,
    /// Which input this entry compresses.
    pub input_id: String,
    /// The input's tier (synthetic/canterbury/silesia).
    pub tier: Tier,
    /// Output framing.
    pub framing: Framing,
    /// Compression level, 0..=9.
    pub level: u8,
    /// Compression strategy.
    pub strategy: Strategy,
    /// zlib windowBits magnitude, 9..=15.
    pub window_bits: u8,
    /// zlib memLevel, 1..=9.
    pub mem_level: u8,
    /// Preset dictionary source file, if any.
    pub dictionary: Option<PathBuf>,
    /// Path relative to `testdata/generated/`.
    pub relative_path: String,
}

fn tier_token(tier: Tier) -> &'static str {
    match tier {
        Tier::Synthetic => "synthetic",
        Tier::Canterbury => "canterbury",
        Tier::Silesia => "silesia",
    }
}

fn framing_id_token(framing: Framing) -> &'static str {
    match framing {
        Framing::Raw => "raw",
        Framing::Zlib => "zlib",
        Framing::Gzip => "gzip",
    }
}

fn framing_path_token(framing: Framing) -> &'static str {
    match framing {
        Framing::Raw => "rfc1951",
        Framing::Zlib => "rfc1950",
        Framing::Gzip => "rfc1952",
    }
}

fn strategy_token(strategy: Strategy) -> &'static str {
    match strategy {
        Strategy::Default => "default",
        Strategy::Filtered => "filtered",
        Strategy::HuffmanOnly => "huffman",
        Strategy::Rle => "rle",
        Strategy::Fixed => "fixed",
    }
}

#[allow(clippy::too_many_arguments)]
fn make_entry(
    input_id: &str,
    tier: Tier,
    framing: Framing,
    level: u8,
    strategy: Strategy,
    window_bits: u8,
    mem_level: u8,
    dictionary: Option<PathBuf>,
) -> EntrySpec {
    let dict_suffix = if dictionary.is_some() { "_dict" } else { "" };
    let id = format!(
        "{input_id}__{}_l{level}_{}_w{window_bits}_m{mem_level}{dict_suffix}",
        framing_id_token(framing),
        strategy_token(strategy),
    );
    let dict_path_suffix = if dictionary.is_some() { "-dict" } else { "" };
    let relative_path = format!(
        "{}/{input_id}/{}-l{level}-{}-w{window_bits}-m{mem_level}{dict_path_suffix}.bin",
        tier_token(tier),
        framing_path_token(framing),
        strategy_token(strategy),
    );
    EntrySpec {
        id,
        input_id: input_id.to_string(),
        tier,
        framing,
        level,
        strategy,
        window_bits,
        mem_level,
        dictionary,
        relative_path,
    }
}

fn group_a(input: &Input) -> Vec<EntrySpec> {
    let levels: Vec<u8> = if input.len > FOUR_MIB {
        vec![0, 1, 6, 9]
    } else {
        (0..=9).collect()
    };
    let mut out = Vec::new();
    for &level in &levels {
        for &framing in &[Framing::Raw, Framing::Zlib, Framing::Gzip] {
            out.push(make_entry(
                &input.id,
                input.tier,
                framing,
                level,
                Strategy::Default,
                15,
                8,
                None,
            ));
        }
    }
    out
}

fn group_b(input: &Input) -> Vec<EntrySpec> {
    let mut out = Vec::new();
    if input.len <= FOUR_MIB {
        for level in 1..=9u8 {
            out.push(make_entry(
                &input.id,
                input.tier,
                Framing::Raw,
                level,
                Strategy::Fixed,
                15,
                8,
                None,
            ));
        }
        for &framing in &[Framing::Zlib, Framing::Gzip] {
            out.push(make_entry(
                &input.id,
                input.tier,
                framing,
                6,
                Strategy::Fixed,
                15,
                8,
                None,
            ));
        }
        for &strategy in &[Strategy::Filtered, Strategy::HuffmanOnly, Strategy::Rle] {
            for &level in &[1u8, 6, 9] {
                out.push(make_entry(
                    &input.id,
                    input.tier,
                    Framing::Raw,
                    level,
                    strategy,
                    15,
                    8,
                    None,
                ));
            }
        }
    } else {
        for &strategy in &[Strategy::Fixed, Strategy::HuffmanOnly, Strategy::Rle] {
            out.push(make_entry(
                &input.id,
                input.tier,
                Framing::Raw,
                6,
                strategy,
                15,
                8,
                None,
            ));
        }
    }
    out
}

fn group_c(input: &Input) -> Vec<EntrySpec> {
    let mut out = Vec::new();
    if input.len <= ONE_MIB {
        for wb in 9..=15u8 {
            for &mem in &[1u8, 9] {
                for &framing in &[Framing::Raw, Framing::Zlib] {
                    out.push(make_entry(
                        &input.id,
                        input.tier,
                        framing,
                        6,
                        Strategy::Default,
                        wb,
                        mem,
                        None,
                    ));
                }
            }
        }
        for &wb in &[9u8, 15] {
            for mem in 1..=9u8 {
                out.push(make_entry(
                    &input.id,
                    input.tier,
                    Framing::Raw,
                    1,
                    Strategy::Default,
                    wb,
                    mem,
                    None,
                ));
            }
        }
    }
    out
}

fn dictionary_entries(corpora_root: &Path) -> Vec<EntrySpec> {
    let dict_path = match find_canterbury_file(corpora_root, "lcet10.txt") {
        Some(p) => p,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    for name in ["alice29.txt", "asyoulik.txt", "cp.html"] {
        if find_canterbury_file(corpora_root, name).is_none() {
            continue;
        }
        let input_id = format!("canterbury-{name}");
        for &level in &[1u8, 6, 9] {
            out.push(make_entry(
                &input_id,
                Tier::Canterbury,
                Framing::Zlib,
                level,
                Strategy::Default,
                15,
                8,
                Some(dict_path.clone()),
            ));
        }
    }
    out
}

fn push_dedup(out: &mut Vec<EntrySpec>, seen: &mut HashSet<String>, spec: EntrySpec) {
    if seen.insert(spec.id.clone()) {
        out.push(spec);
    }
}

/// Build the full, deduplicated matrix of entries for `inputs`, including
/// the dictionary sub-matrix if the required Canterbury files are present
/// under `corpora_root`.
pub fn entries(inputs: &[Input], corpora_root: &Path) -> Vec<EntrySpec> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();

    for input in inputs {
        for spec in group_a(input) {
            push_dedup(&mut out, &mut seen, spec);
        }
        for spec in group_b(input) {
            push_dedup(&mut out, &mut seen, spec);
        }
        for spec in group_c(input) {
            push_dedup(&mut out, &mut seen, spec);
        }
    }

    for spec in dictionary_entries(corpora_root) {
        push_dedup(&mut out, &mut seen, spec);
    }

    out
}

/// `quick` = synthetic + canterbury inputs; `full` = synthetic +
/// canterbury + silesia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorpusTier {
    /// synthetic + canterbury.
    Quick,
    /// synthetic + canterbury + silesia.
    Full,
}

/// Collect the input list for `tier` from `corpora_root` (typically
/// `testdata/corpora`).
pub fn inputs_for_tier(tier: CorpusTier, corpora_root: &Path) -> Vec<Input> {
    let mut inputs = crate::inputs::synthetic_inputs();
    inputs.extend(crate::inputs::canterbury_inputs(corpora_root));
    if tier == CorpusTier::Full {
        inputs.extend(crate::inputs::silesia_inputs(corpora_root));
    }
    inputs
}

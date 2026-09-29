use std::collections::HashSet;
use std::path::PathBuf;

use oracle::ffi::{Framing, Strategy};
use oracle::inputs::{generate_synthetic, synthetic_inputs};
use oracle::matrix::{entries, inputs_for_tier, CorpusTier};

fn corpora_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("testdata")
        .join("corpora")
}

#[test]
fn synthetic_inputs_are_deterministic() {
    let a: Vec<Vec<u8>> = synthetic_inputs().iter().map(|i| i.bytes()).collect();
    let b: Vec<Vec<u8>> = synthetic_inputs().iter().map(|i| i.bytes()).collect();
    assert_eq!(a, b);

    let random_64kib = generate_synthetic("random-64KiB");
    let mut hasher = sha2::Sha256::new();
    use sha2::Digest;
    hasher.update(&random_64kib);
    let hash = hasher.finalize();
    let hex = hash.iter().map(|b| format!("{b:02x}")).collect::<String>();
    // Computed once from this implementation and pinned here; any change
    // to SplitMix64, fnv1a64, CORPUS_SEED or the "random-64KiB" id/size
    // must update this constant deliberately.
    assert_eq!(
        hex, "433baa09ac0f3f8dc2b3926a207a43122d3c203d4ea535c2555d7a734a81d3b5",
        "random-64KiB sha256 changed -- update the pinned hash if this is intentional"
    );
}

#[test]
fn window_inputs_repeat_at_exact_distance() {
    for (id, d) in [
        ("window-d32767", 32767usize),
        ("window-d32768", 32768),
        ("window-d32769", 32769),
    ] {
        let bytes = generate_synthetic(id);
        assert_eq!(bytes.len(), d + 1536, "id={id}");
        let r1 = bytes.get(0..512).unwrap();
        let r2 = bytes.get(d..d + 512).unwrap();
        assert_eq!(r1, r2, "id={id}: second R does not repeat at distance {d}");
    }
}

#[test]
fn matrix_ids_unique() {
    let inputs = synthetic_inputs();
    let specs = entries(&inputs, &corpora_root());
    let mut seen = HashSet::new();
    for spec in &specs {
        assert!(
            seen.insert(spec.id.clone()),
            "duplicate entry id: {}",
            spec.id
        );
    }
    assert!(!specs.is_empty());
}

#[test]
fn matrix_covers_all_parameter_values() {
    let root = corpora_root();
    let inputs = inputs_for_tier(CorpusTier::Quick, &root);
    let specs = entries(&inputs, &root);

    let mut levels = HashSet::new();
    let mut strategies = HashSet::new();
    let mut window_bits = HashSet::new();
    let mut mem_levels = HashSet::new();
    let mut framings = HashSet::new();
    let mut dict_count = 0;

    for spec in &specs {
        levels.insert(spec.level);
        strategies.insert(format!("{:?}", spec.strategy));
        window_bits.insert(spec.window_bits);
        mem_levels.insert(spec.mem_level);
        framings.insert(format!("{:?}", spec.framing));
        if spec.dictionary.is_some() {
            dict_count += 1;
        }
    }

    for level in 0..=9u8 {
        assert!(levels.contains(&level), "missing level {level}");
    }
    for strategy in [
        Strategy::Default,
        Strategy::Filtered,
        Strategy::HuffmanOnly,
        Strategy::Rle,
        Strategy::Fixed,
    ] {
        assert!(
            strategies.contains(&format!("{strategy:?}")),
            "missing strategy {strategy:?}"
        );
    }
    for wb in 9..=15u8 {
        assert!(window_bits.contains(&wb), "missing window_bits {wb}");
    }
    for mem in 1..=9u8 {
        assert!(mem_levels.contains(&mem), "missing mem_level {mem}");
    }
    for framing in [Framing::Raw, Framing::Zlib, Framing::Gzip] {
        assert!(
            framings.contains(&format!("{framing:?}")),
            "missing framing {framing:?}"
        );
    }

    let canterbury_present = root.join("canterbury").join("lcet10.txt").is_file();
    if canterbury_present {
        assert!(dict_count >= 1, "expected at least one dictionary entry");
    }
}

#[test]
fn quick_tier_size_is_bounded() {
    let root = corpora_root();
    let inputs = inputs_for_tier(CorpusTier::Quick, &root);
    let specs = entries(&inputs, &root);
    assert!(
        specs.len() >= 300 && specs.len() <= 6000,
        "quick tier has {} entries, expected 300..=6000",
        specs.len()
    );
}

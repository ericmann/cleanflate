use crate::error::{ChecksumKind, Error, ErrorKind, HeaderError, HuffmanError};

fn all_variants() -> Vec<Error> {
    vec![
        Error::BadHeader(HeaderError::ZlibFcheck),
        Error::PresetDictionaryRequired { dict_id: 1 },
        Error::DictionaryMismatch {
            expected: 1,
            actual: 2,
        },
        Error::BadBlockType,
        Error::InvalidStoredLength { len: 1, nlen: 2 },
        Error::InvalidHuffmanTable(HuffmanError::OverSubscribed),
        Error::InvalidSymbol,
        Error::InvalidDistance {
            distance: 1,
            available: 0,
        },
        Error::ChecksumMismatch {
            kind: ChecksumKind::Adler32,
            expected: 1,
            actual: 2,
        },
        Error::Truncated,
        Error::TrailingData { offset: 5 },
        Error::OutputLimitExceeded { limit: 10 },
    ]
}

#[test]
fn every_variant_has_distinct_kind() {
    let variants = all_variants();
    let kinds: Vec<ErrorKind> = variants.iter().map(Error::kind).collect();
    for i in 0..kinds.len() {
        for j in 0..kinds.len() {
            if i != j {
                assert_ne!(
                    kinds.get(i),
                    kinds.get(j),
                    "variants at {i} and {j} share a kind"
                );
            }
        }
    }
}

#[test]
fn kind_names_match_variant_names() {
    let cases: Vec<(Error, &str)> = vec![
        (Error::BadHeader(HeaderError::ZlibFcheck), "BadHeader"),
        (
            Error::PresetDictionaryRequired { dict_id: 0 },
            "PresetDictionaryRequired",
        ),
        (
            Error::DictionaryMismatch {
                expected: 0,
                actual: 0,
            },
            "DictionaryMismatch",
        ),
        (Error::BadBlockType, "BadBlockType"),
        (
            Error::InvalidStoredLength { len: 0, nlen: 0 },
            "InvalidStoredLength",
        ),
        (
            Error::InvalidHuffmanTable(HuffmanError::Incomplete),
            "InvalidHuffmanTable",
        ),
        (Error::InvalidSymbol, "InvalidSymbol"),
        (
            Error::InvalidDistance {
                distance: 0,
                available: 0,
            },
            "InvalidDistance",
        ),
        (
            Error::ChecksumMismatch {
                kind: ChecksumKind::Crc32,
                expected: 0,
                actual: 0,
            },
            "ChecksumMismatch",
        ),
        (Error::Truncated, "Truncated"),
        (Error::TrailingData { offset: 0 }, "TrailingData"),
        (
            Error::OutputLimitExceeded { limit: 0 },
            "OutputLimitExceeded",
        ),
    ];
    for (err, expected_name) in cases {
        assert_eq!(err.kind().as_str(), expected_name);
    }
}

#[test]
fn display_is_nonempty_for_all_variants() {
    for err in all_variants() {
        let text = format!("{err}");
        assert!(!text.is_empty(), "{:?} displayed as empty string", err);
    }
}

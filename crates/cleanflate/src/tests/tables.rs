use crate::tables::{
    fixed_dist_lengths, fixed_litlen_lengths, CODE_LENGTH_ORDER, DIST_BASE, DIST_EXTRA,
    LENGTH_BASE, LENGTH_EXTRA,
};

#[test]
fn length_table_spot_checks_rfc_3_2_5() {
    // code 257 -> index 0, base 3, extra 0
    assert_eq!(LENGTH_BASE.first(), Some(&3));
    assert_eq!(LENGTH_EXTRA.first(), Some(&0));
    // code 265 -> index 8, base 11, extra 1
    assert_eq!(LENGTH_BASE.get(8), Some(&11));
    assert_eq!(LENGTH_EXTRA.get(8), Some(&1));
    // code 284 -> index 27, base 227, extra 5
    assert_eq!(LENGTH_BASE.get(27), Some(&227));
    assert_eq!(LENGTH_EXTRA.get(27), Some(&5));
    // code 285 -> index 28, base 258, extra 0
    assert_eq!(LENGTH_BASE.get(28), Some(&258));
    assert_eq!(LENGTH_EXTRA.get(28), Some(&0));
}

#[test]
fn distance_table_spot_checks() {
    assert_eq!(DIST_BASE.first(), Some(&1));
    assert_eq!(DIST_EXTRA.first(), Some(&0));
    assert_eq!(DIST_BASE.get(4), Some(&5));
    assert_eq!(DIST_EXTRA.get(4), Some(&1));
    assert_eq!(DIST_BASE.get(29), Some(&24577));
    assert_eq!(DIST_EXTRA.get(29), Some(&13));
}

#[test]
fn length_ranges_contiguous() {
    // Each code's range [base, base + 2^extra - 1] must abut the next
    // code's base, EXCEPT the last transition (code 284 -> 285): RFC 1951
    // S3.2.5 gives code 284 base 227 extra 5, a nominal range of
    // 227..=258 (32 values), but 258 is exclusively code 285's fixed
    // value (extra 0). The two ranges intentionally overlap at 258; this
    // is a documented RFC quirk (see PLAN.md's "Length code 284 with
    // extra value 31 decodes arithmetically to 258" interpretation), not
    // a table bug.
    let last = LENGTH_BASE.len() - 1;
    for i in 0..last - 1 {
        let base = *LENGTH_BASE.get(i).expect("in range");
        let extra = *LENGTH_EXTRA.get(i).expect("in range");
        let next_base = *LENGTH_BASE.get(i + 1).expect("in range");
        let range_len = 1u16 << extra;
        assert_eq!(
            base + range_len,
            next_base,
            "code index {i}: range does not abut the next code's base"
        );
    }
    let base = *LENGTH_BASE.get(last).expect("in range");
    let extra = *LENGTH_EXTRA.get(last).expect("in range");
    assert_eq!(base + (1u16 << extra) - 1, 258);
}

#[test]
fn distance_ranges_contiguous() {
    for i in 0..DIST_BASE.len() - 1 {
        let base = *DIST_BASE.get(i).expect("in range");
        let extra = *DIST_EXTRA.get(i).expect("in range");
        let next_base = *DIST_BASE.get(i + 1).expect("in range");
        let range_len = 1u16 << extra;
        assert_eq!(
            base + range_len,
            next_base,
            "code index {i}: range does not abut the next code's base"
        );
    }
    let last = DIST_BASE.len() - 1;
    let base = *DIST_BASE.get(last).expect("in range");
    let extra = *DIST_EXTRA.get(last).expect("in range");
    assert_eq!(base as u32 + (1u32 << extra) - 1, 32768);
}

#[test]
fn fixed_litlen_lengths_rfc_3_2_6() {
    let lengths = fixed_litlen_lengths();
    assert_eq!(lengths.len(), 288);
    for i in 0..=143usize {
        assert_eq!(lengths.get(i), Some(&8), "literal {i}");
    }
    for i in 144..=255usize {
        assert_eq!(lengths.get(i), Some(&9), "literal {i}");
    }
    for i in 256..=279usize {
        assert_eq!(lengths.get(i), Some(&7), "code {i}");
    }
    for i in 280..=287usize {
        assert_eq!(lengths.get(i), Some(&8), "code {i}");
    }

    let dist = fixed_dist_lengths();
    assert_eq!(dist.len(), 32);
    assert!(dist.iter().all(|&l| l == 5));
}

#[test]
fn code_length_order_rfc_3_2_7() {
    let expected: [u8; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    assert_eq!(CODE_LENGTH_ORDER, expected);
}

//! Port of `test/bin.test.ts`.

use vega_lite::bin::{auto_max_bins, bin_to_string, is_bin_params, Bin};
use vega_lite::channel::{
    COLOR, COLUMN, FILL, FILLOPACITY, OPACITY, ROW, SHAPE, SIZE, STROKE, STROKEOPACITY,
    STROKEWIDTH,
};

fn bin_from_json(value: serde_json::Value) -> Bin {
    serde_json::from_value(value).expect("valid bin")
}

#[test]
fn auto_max_bins_should_assign_generate_correct_defaults_for_different_channels() {
    // Not testing case for 10 because it's already tested
    for channel in [
        COLOR,
        FILL,
        STROKE,
        STROKEWIDTH,
        SIZE,
        OPACITY,
        FILLOPACITY,
        STROKEOPACITY,
        SHAPE,
        ROW,
        COLUMN,
    ] {
        assert_eq!(auto_max_bins(Some(channel)), 6, "channel {channel}");
    }
}

#[test]
fn bin_to_string_should_generate_the_correct_key_for_boolean() {
    assert_eq!(bin_to_string(&Bin::Bool(true)), "bin_maxbins_10");
}

#[test]
fn bin_to_string_puts_maxbins_before_minstep() {
    // Matches the insertion order used by the upstream examples:
    // `binToString({maxbins: 100, minstep: 1})`.
    assert_eq!(
        bin_to_string(&bin_from_json(
            serde_json::json!({"maxbins": 100, "minstep": 1})
        )),
        "bin_maxbins_100_minstep_1"
    );
    assert_eq!(
        bin_to_string(&bin_from_json(serde_json::json!({"maxbins": 10}))),
        "bin_maxbins_10"
    );
}

#[test]
fn is_bin_params_should_detect_whether_the_input_is_bin_params_or_not() {
    assert!(!is_bin_params(&Bin::Bool(true)));
    assert!(!is_bin_params(&Bin::Binned));
    assert!(is_bin_params(&bin_from_json(serde_json::json!({}))));
    assert!(is_bin_params(&bin_from_json(
        serde_json::json!({"binned": true})
    )));
    assert!(is_bin_params(&bin_from_json(
        serde_json::json!({"extent": [0, 1]})
    )));
}

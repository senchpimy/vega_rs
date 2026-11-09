//! Scale range resolution for Vega color schemes.
//!
//! These tests pin the behaviour of vega-scale's `configureScheme`: continuous
//! schemes are piecewise RGB interpolators over Vega's anchor palettes, while
//! discretizing scales sample the scheme at evenly spaced interior positions.

use std::collections::HashMap;

use serde_json::{json, Value};
use vega_svg::data::DataStore;
use vega_svg::expression::EmptyEnv;
use vega_svg::scale::{build_scale, Scale};

fn make(spec: Value) -> Scale {
    build_scale("s", &spec, &DataStore::default(), &HashMap::new(), &EmptyEnv).unwrap()
}

#[test]
fn continuous_scheme_uses_vega_anchor_palette() {
    let scale = make(json!({
        "type": "linear",
        "domain": [0, 1],
        "range": "heatmap"
    }));
    // Vega's `heatmap` alias is `yellowgreenblue`.
    assert_eq!(scale.colors.len(), 11);
    assert_eq!(scale.scale(&json!(0.0)), json!("rgb(239, 249, 189)"));
    assert_eq!(scale.scale(&json!(1.0)), json!("rgb(28, 49, 133)"));
    // Piecewise interpolation between the 5th and 6th anchors.
    assert_eq!(scale.scale(&json!(0.413)), json!("rgb(100, 195, 191)"));
}

#[test]
fn ordinal_scheme_uses_d3_category_palettes() {
    let scale = make(json!({
        "type": "ordinal",
        "domain": ["a", "b"],
        "range": {"scheme": "category20"}
    }));
    assert_eq!(scale.colors.len(), 20);
    assert_eq!(scale.colors[0], "#1f77b4");
    assert_eq!(scale.colors[1], "#aec7e8");

    let b = make(json!({
        "type": "ordinal",
        "domain": ["a", "b"],
        "range": {"scheme": "category20b"}
    }));
    assert_eq!(b.colors[0], "#393b79");
    assert_eq!(b.colors[1], "#5254a3");
}

#[test]
fn ordinal_continuous_scheme_is_quantized() {
    // The scheme is quantized to the ordinal domain cardinality.
    let scale = make(json!({
        "type": "ordinal",
        "domain": ["a", "b", "c"],
        "range": {"scheme": "yelloworangebrown"}
    }));
    assert_eq!(scale.colors.len(), 3);
    assert_eq!(scale.scale(&json!("a")), json!("rgb(254, 194, 85)"));
    assert_eq!(scale.scale(&json!("c")), json!("rgb(208, 85, 8)"));
}

#[test]
fn quantize_scheme_samples_interior_positions() {
    let scale = make(json!({
        "type": "quantize",
        "domain": [0, 91],
        "range": "ramp"
    }));
    assert_eq!(
        scale.colors,
        vec![
            "rgb(175, 209, 231)",
            "rgb(134, 188, 220)",
            "rgb(91, 163, 207)",
            "rgb(56, 135, 192)",
            "rgb(27, 105, 173)",
        ]
    );
    // Equal-width bins: [0,18.2), [18.2,36.4), ...
    assert_eq!(scale.scale(&json!(0)), json!("rgb(175, 209, 231)"));
    assert_eq!(scale.scale(&json!(18)), json!("rgb(175, 209, 231)"));
    assert_eq!(scale.scale(&json!(19)), json!("rgb(134, 188, 220)"));
    assert_eq!(scale.scale(&json!(91)), json!("rgb(27, 105, 173)"));
}

#[test]
fn threshold_scheme_uses_domain_breaks() {
    let scale = make(json!({
        "type": "threshold",
        "domain": [30, 70],
        "range": {"scheme": "viridis"}
    }));
    assert_eq!(
        scale.colors,
        vec![
            "rgb(59, 82, 139)",
            "rgb(33, 145, 141)",
            "rgb(93, 201, 99)",
        ]
    );
    assert_eq!(scale.scale(&json!(29)), json!("rgb(59, 82, 139)"));
    assert_eq!(scale.scale(&json!(30)), json!("rgb(33, 145, 141)"));
    assert_eq!(scale.scale(&json!(69.9)), json!("rgb(33, 145, 141)"));
    assert_eq!(scale.scale(&json!(70)), json!("rgb(93, 201, 99)"));
}

#[test]
fn quantile_scheme_uses_quantile_breaks() {
    let scale = make(json!({
        "type": "quantile",
        "domain": [1, 2, 3, 4],
        "range": {"scheme": "magma"}
    }));
    assert_eq!(scale.colors.len(), 5);
    assert_eq!(scale.colors[0], "rgb(44, 17, 96)");
    assert_eq!(scale.stops.len(), 4);
    assert_eq!(scale.scale(&json!(1)), json!("rgb(44, 17, 96)"));
    assert_eq!(scale.scale(&json!(2)), json!("rgb(114, 31, 129)"));
    assert_eq!(scale.scale(&json!(4)), json!("rgb(254, 175, 120)"));
}

#[test]
fn time_scheme_is_quantized_to_two_colors() {
    // Continuous time scales keep their temporal type, so Vega quantizes the
    // scheme to the domain cardinality (two endpoints).
    let scale = make(json!({
        "type": "time",
        "domain": [946684800000.0, 1293840000000.0],
        "range": "ramp"
    }));
    assert_eq!(
        scale.colors,
        vec!["rgb(134, 188, 220)", "rgb(56, 135, 192)"]
    );
}

#[test]
fn diverging_alias_reverses_its_scheme() {
    let scale = make(json!({
        "type": "linear",
        "domain": [0, 1],
        "range": "diverging"
    }));
    // `diverging` is blueOrange with extent [1, 0].
    assert_eq!(scale.scale(&json!(0.0)), json!("rgb(153, 74, 7)"));
    assert_eq!(scale.scale(&json!(1.0)), json!("rgb(19, 75, 133)"));
}

//! Port of `test/compile/mark/rect.test.ts`.

#[path = "mark_util.rs"]
mod mark_util;

use serde_json::json;

use mark_util::{assert_eq_json, assert_match_object, parse_unit_model_with_scale_and_layout};
use vega_lite::compile::mark::rect::encode_entry as rect;

#[test]
fn simple_with_width_and_height_centered() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": {"type": "rect", "width": 50, "height": 49},
        "encoding": {
            "x": {"field": "x", "type": "quantitative"},
            "y": {"type": "quantitative", "field": "y"}
        }
    }));
    let props = rect(&model);
    assert_eq_json(json!({"scale": "x", "field": "x"}), props["xc"].clone());
    assert_eq_json(json!({"value": 50}), props["width"].clone());
    assert_eq_json(json!({"scale": "y", "field": "y"}), props["yc"].clone());
    assert_eq_json(json!({"value": 49}), props["height"].clone());
}

#[test]
fn simple_with_width_and_height_left_align_top_baseline() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": {"type": "rect", "width": 50, "height": 49, "align": "left", "baseline": "top"},
        "encoding": {
            "x": {"field": "x", "type": "quantitative"},
            "y": {"type": "quantitative", "field": "y"}
        }
    }));
    let props = rect(&model);
    assert_eq_json(json!({"scale": "x", "field": "x"}), props["x"].clone());
    assert_eq_json(json!({"value": 50}), props["width"].clone());
    assert_eq_json(json!({"scale": "y", "field": "y"}), props["y"].clone());
    assert_eq_json(json!({"value": 49}), props["height"].clone());
}

#[test]
fn warn_if_align_is_expression() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": {"type": "rect", "width": 50, "height": 49, "align": {"expr": "test"}, "baseline": "top"},
        "encoding": {
            "x": {"field": "x", "type": "quantitative"},
            "y": {"type": "quantitative", "field": "y"}
        }
    }));
    let (_, logger) = vega_lite::log::capture(|| rect(&model));
    assert!(!logger.warns.is_empty());
}

#[test]
fn simple_with_width_and_height_right_align_bottom_baseline() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": {"type": "rect", "width": 50, "height": 49, "align": "right", "baseline": "bottom"},
        "encoding": {
            "x": {"field": "x", "type": "quantitative"},
            "y": {"type": "quantitative", "field": "y"}
        }
    }));
    let props = rect(&model);
    assert_eq_json(json!({"scale": "x", "field": "x"}), props["x2"].clone());
    assert_eq_json(json!({"value": 50}), props["width"].clone());
    assert_eq_json(json!({"scale": "y", "field": "y"}), props["y2"].clone());
    assert_eq_json(json!({"value": 49}), props["height"].clone());
}

#[test]
fn simple_with_fixed_x2_y2() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": {"type": "rect", "x2": -1, "y2": -2},
        "encoding": {
            "x": {"field": "x", "type": "quantitative"},
            "y": {"type": "quantitative", "field": "y"}
        }
    }));
    let props = rect(&model);
    assert_eq_json(json!({"scale": "x", "field": "x"}), props["x"].clone());
    assert_eq_json(json!({"value": -1}), props["x2"].clone());
    assert_eq_json(json!({"scale": "y", "field": "y"}), props["y"].clone());
    assert_eq_json(json!({"value": -2}), props["y2"].clone());
}

#[test]
fn simple_vertical() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "rect",
        "encoding": {
            "x": {"field": "Origin", "type": "nominal"},
            "y": {"type": "quantitative", "field": "Acceleration", "aggregate": "mean"}
        }
    }));
    let props = rect(&model);
    assert_eq_json(json!({"scale": "x", "field": "Origin"}), props["x"].clone());
    assert_eq_json(json!({"signal": "max(0.25, bandwidth('x'))"}), props["width"].clone());
    assert_eq_json(json!({"scale": "y", "field": "mean_Acceleration"}), props["y"].clone());
    assert_eq_json(json!({"scale": "y", "value": 0}), props["y2"].clone());
    assert!(props.get("height").is_none());
}

#[test]
fn simple_vertical_signal_band_size() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": {"type": "rect", "width": {"expr": "a"}},
        "encoding": {
            "x": {"field": "Origin", "type": "nominal"},
            "y": {"type": "quantitative", "field": "Acceleration", "aggregate": "mean"}
        }
    }));
    let props = rect(&model);
    assert_eq_json(json!({"scale": "x", "field": "Origin", "band": 0.5}), props["xc"].clone());
    assert_eq_json(json!({"signal": "a"}), props["width"].clone());
    assert_eq_json(json!({"scale": "y", "field": "mean_Acceleration"}), props["y"].clone());
    assert_eq_json(json!({"scale": "y", "value": 0}), props["y2"].clone());
    assert!(props.get("height").is_none());
}

#[test]
fn simple_vertical_1d() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "rect",
        "encoding": {"y": {"type": "quantitative", "field": "Acceleration", "aggregate": "mean"}}
    }));
    let props = rect(&model);
    assert_match_object(
        &json!({
            "x": {"field": {"group": "width"}},
            "x2": {"value": 0},
            "y": {"scale": "y", "field": "mean_Acceleration"},
            "y2": {"scale": "y", "value": 0}
        }),
        &props,
    );
}

#[test]
fn simple_horizontal() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "rect",
        "encoding": {
            "y": {"field": "Origin", "type": "nominal"},
            "x": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"}
        }
    }));
    let props = rect(&model);
    assert_eq_json(json!({"scale": "y", "field": "Origin"}), props["y"].clone());
    assert_eq_json(json!({"signal": "max(0.25, bandwidth('y'))"}), props["height"].clone());
    assert_eq_json(json!({"scale": "x", "field": "mean_Acceleration"}), props["x"].clone());
    assert_eq_json(json!({"scale": "x", "value": 0}), props["x2"].clone());
    assert!(props.get("width").is_none());
}

#[test]
fn simple_horizontal_with_size_field() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "rect",
        "encoding": {
            "y": {"field": "Origin", "type": "nominal"},
            "x": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"},
            "size": {"aggregate": "mean", "field": "Horsepower", "type": "quantitative"}
        }
    }));
    let (props, _logger) = vega_lite::log::capture(|| rect(&model));
    assert_eq_json(json!({"scale": "y", "field": "Origin"}), props["y"].clone());
    assert_eq_json(json!({"signal": "max(0.25, bandwidth('y'))"}), props["height"].clone());
    assert_eq_json(json!({"scale": "x", "field": "mean_Acceleration"}), props["x"].clone());
    assert_eq_json(json!({"scale": "x", "value": 0}), props["x2"].clone());
    assert!(props.get("width").is_none());
}

#[test]
fn horizontal_bin() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "rect",
        "encoding": {
            "y": {"bin": true, "field": "Horsepower", "type": "quantitative"},
            "x": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"}
        },
        "config": {"rect": {"minBandSize": null}}
    }));
    let props = rect(&model);
    assert_eq_json(
        json!({"scale": "y", "field": "bin_maxbins_10_Horsepower", "offset": 0.5}),
        props["y2"].clone(),
    );
    assert_eq_json(
        json!({"scale": "y", "field": "bin_maxbins_10_Horsepower_end", "offset": 0.5}),
        props["y"].clone(),
    );
    assert!(props.get("height").is_none());
}

#[test]
fn vertical_bin() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "rect",
        "encoding": {
            "x": {"bin": true, "field": "Horsepower", "type": "quantitative"},
            "y": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"}
        },
        "config": {"rect": {"minBandSize": null}}
    }));
    let props = rect(&model);
    assert_eq_json(
        json!({"scale": "x", "field": "bin_maxbins_10_Horsepower", "offset": 0.5}),
        props["x2"].clone(),
    );
    assert_eq_json(
        json!({"scale": "x", "field": "bin_maxbins_10_Horsepower_end", "offset": 0.5}),
        props["x"].clone(),
    );
    assert!(props.get("width").is_none());
}

#[test]
fn simple_ranged() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "rect",
        "encoding": {
            "y": {"field": "hp1", "type": "quantitative"},
            "y2": {"field": "hp2"},
            "x": {"field": "origin1", "type": "ordinal"},
            "x2": {"field": "origin2"}
        }
    }));
    let props = rect(&model);
    assert_eq_json(json!({"scale": "x", "field": "origin1", "band": 0.5}), props["x"].clone());
    assert_eq_json(json!({"scale": "x", "field": "origin2", "band": 0.5}), props["x2"].clone());
    assert_eq_json(json!({"scale": "y", "field": "hp1"}), props["y"].clone());
    assert_eq_json(json!({"scale": "y", "field": "hp2"}), props["y2"].clone());
}

#[test]
fn simple_heatmap() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "rect",
        "encoding": {
            "y": {"field": "Origin", "type": "ordinal"},
            "x": {"field": "Cylinders", "type": "ordinal"},
            "color": {"aggregate": "mean", "field": "Horsepower", "type": "quantitative"}
        }
    }));
    let props = rect(&model);
    assert_eq_json(json!({"scale": "x", "field": "Cylinders"}), props["x"].clone());
    assert_eq_json(json!({"signal": "max(0.25, bandwidth('x'))"}), props["width"].clone());
    assert_eq_json(json!({"scale": "y", "field": "Origin"}), props["y"].clone());
    assert_eq_json(json!({"signal": "max(0.25, bandwidth('y'))"}), props["height"].clone());
}

#[test]
fn vertical_binned_data() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "bar",
        "encoding": {
            "x": {"field": "bin_start", "bin": "binned", "type": "quantitative", "axis": {"tickMinStep": 2}},
            "x2": {"field": "bin_end"},
            "y": {"field": "count", "type": "quantitative"}
        },
        "config": {"bar": {"minBandSize": null}}
    }));
    let props = rect(&model);
    assert_eq_json(json!({"scale": "x", "field": "bin_start", "offset": 1}), props["x2"].clone());
    assert_eq_json(json!({"scale": "x", "field": "bin_end"}), props["x"].clone());
    assert_eq_json(json!({"scale": "y", "field": "count_end"}), props["y"].clone());
    assert_eq_json(json!({"scale": "y", "field": "count_start"}), props["y2"].clone());
    assert!(props.get("width").is_none());
}

#[test]
fn horizontal_binned_data() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "bar",
        "encoding": {
            "y": {"field": "bin_start", "bin": "binned", "type": "quantitative", "axis": {"tickMinStep": 2}},
            "y2": {"field": "bin_end"},
            "x": {"field": "count", "type": "quantitative"}
        },
        "config": {"bar": {"minBandSize": null}}
    }));
    let props = rect(&model);
    assert_eq_json(json!({"scale": "y", "field": "bin_start"}), props["y2"].clone());
    assert_eq_json(json!({"scale": "y", "field": "bin_end", "offset": 1}), props["y"].clone());
    assert_eq_json(json!({"scale": "x", "field": "count_end"}), props["x"].clone());
    assert_eq_json(json!({"scale": "x", "field": "count_start"}), props["x2"].clone());
    assert!(props.get("width").is_none());
}

#[test]
fn with_size_warns() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": {"type": "rect", "width": 50, "height": 49, "size": 20},
        "encoding": {
            "x": {"field": "x", "type": "ordinal"},
            "y": {"field": "y", "type": "ordinal"}
        }
    }));
    let (_, logger) = vega_lite::log::capture(|| rect(&model));
    assert!(!logger.warns.is_empty());
}

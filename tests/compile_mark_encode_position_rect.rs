//! Port of `test/compile/mark/encode/position-rect.test.ts`.

#[path = "mark_util.rs"]
mod mark_util;

use serde_json::json;

use mark_util::{assert_eq_json, parse_unit_model_with_scale_and_layout};
use vega_lite::compile::mark::encode::position_rect::rect_position;

const OFFSETTED_RECT_START_SUFFIX: &str = "offsetted_rect_start";
const OFFSETTED_RECT_END_SUFFIX: &str = "offsetted_rect_end";

#[test]
fn x_mixins_for_signal_reverse() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"values": []},
        "mark": "bar",
        "encoding": {"x": {"bin": true, "field": "x", "type": "quantitative", "scale": {"reverse": {"signal": "r"}}}},
        "config": {"bar": {"minBandSize": null}}
    }));
    let props = rect_position(&model, "x");
    assert_eq_json(
        json!({"signal": "0.5 + (r ? -1 : 1) * -0.5"}),
        props["x"]["offset"].clone(),
    );
    assert_eq_json(
        json!({"signal": "0.5 + (r ? -1 : 1) * 0.5"}),
        props["x2"]["offset"].clone(),
    );
}

#[test]
fn x_mixins_for_x_offset_without_x() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"values": []},
        "mark": "bar",
        "encoding": {
            "xOffset": {"field": "a", "type": "nominal"},
            "y": {"field": "b", "type": "quantitative"}
        }
    }));
    let props = rect_position(&model, "x");
    assert_eq_json(
        json!({"signal": "width", "mult": 0.5, "offset": {"scale": "xOffset", "field": "a"}}),
        props["x"].clone(),
    );
    assert_eq_json(
        json!({"signal": "max(0.25, bandwidth('xOffset'))"}),
        props["width"].clone(),
    );
}

#[test]
fn x_mixins_for_time_unit_band_position_zero() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"values": []},
        "mark": "bar",
        "encoding": {"x": {"timeUnit": "yearmonth", "field": "date", "type": "temporal", "bandPosition": 0}}
    }));
    let props = rect_position(&model, "x");
    assert_eq_json(
        json!(format!("yearmonth_date_{OFFSETTED_RECT_END_SUFFIX}")),
        props["x"]["field"].clone(),
    );
    assert_eq_json(
        json!(format!("yearmonth_date_{OFFSETTED_RECT_START_SUFFIX}")),
        props["x2"]["field"].clone(),
    );
}

#[test]
fn xc_for_tick_with_explicit_size_and_time_unit_band_position_zero() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"values": []},
        "mark": {"type": "tick", "size": {"expr": "20"}},
        "encoding": {
            "x": {"timeUnit": "binnedutcyear", "field": "date", "type": "temporal"},
            "y": {"field": "v", "type": "quantitative"}
        },
        "config": {"tick": {"timeUnitBandPosition": 0}}
    }));
    let props = rect_position(&model, "x");
    assert_eq_json(json!("x"), props["xc"]["scale"].clone());
    assert_eq_json(json!("date"), props["xc"]["field"].clone());
}

#[test]
fn does_not_apply_time_unit_band_position_to_non_centered_bar() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"values": []},
        "mark": "bar",
        "encoding": {
            "x": {"timeUnit": "month", "field": "date", "type": "ordinal"},
            "y": {"aggregate": "mean", "field": "precipitation"}
        }
    }));
    let props = rect_position(&model, "x");
    assert_eq_json(json!("x"), props["x"]["scale"].clone());
    assert_eq_json(json!("month_date"), props["x"]["field"].clone());
    assert!(props["x"].get("band").is_none());
}

#[test]
fn centered_bar_with_size_and_x_offset_uses_band_position_zero() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"values": []},
        "mark": {"type": "bar", "size": 20},
        "encoding": {
            "x": {"field": "a", "type": "nominal"},
            "xOffset": {"field": "b", "type": "nominal"},
            "y": {"field": "c", "type": "quantitative"}
        }
    }));
    let props = rect_position(&model, "x");
    assert_eq_json(json!("x"), props["xc"]["scale"].clone());
    assert_eq_json(json!("a"), props["xc"]["field"].clone());
    assert_eq_json(json!("xOffset"), props["xc"]["offset"]["scale"].clone());
    assert_eq_json(json!("b"), props["xc"]["offset"]["field"].clone());
}

#[test]
fn left_aligned_bar_with_signal_band_size() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"values": []},
        "mark": {"type": "bar", "align": "left", "size": {"signal": "s"}},
        "encoding": {
            "x": {"field": "a", "type": "nominal"},
            "y": {"field": "b", "type": "quantitative"}
        }
    }));
    let props = rect_position(&model, "x");
    let signal = props["x"]["band"]["signal"].as_str().unwrap();
    assert!(signal.contains("(1-"), "{signal}");
    assert_eq_json(json!("s"), props["width"]["signal"].clone());
}

#[test]
fn left_aligned_bar_with_numeric_size() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"values": []},
        "mark": {"type": "bar", "align": "left", "size": 20},
        "encoding": {
            "x": {"field": "a", "type": "nominal"},
            "y": {"field": "b", "type": "quantitative"}
        }
    }));
    let props = rect_position(&model, "x");
    assert_eq_json(json!("x"), props["x"]["scale"].clone());
    assert_eq_json(json!("a"), props["x"]["field"].clone());
    assert_eq_json(json!({"value": 20}), props["width"].clone());
}

#[test]
fn binned_data_with_step_and_start_field_without_end_field() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"values": []},
        "mark": "bar",
        "encoding": {"x": {"bin": {"binned": true, "step": 2}, "field": "x", "type": "quantitative"}},
        "config": {"bar": {"minBandSize": null}}
    }));
    let props = rect_position(&model, "x");
    assert_eq_json(
        json!({"signal": "scale(\"x\", datum[\"x\"] + 2)", "offset": 0}),
        props["x"].clone(),
    );
}

#[test]
fn y_mixins_for_signal_reverse() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"values": []},
        "mark": "bar",
        "encoding": {"y": {"bin": true, "field": "x", "type": "quantitative", "scale": {"reverse": {"signal": "r"}}}},
        "config": {"bar": {"minBandSize": null}}
    }));
    let props = rect_position(&model, "y");
    assert_eq_json(
        json!({"signal": "0.5 + (r ? -1 : 1) * -0.5"}),
        props["y2"]["offset"].clone(),
    );
    assert_eq_json(
        json!({"signal": "0.5 + (r ? -1 : 1) * 0.5"}),
        props["y"]["offset"].clone(),
    );
}

#[test]
fn x_mixins_for_signal_reverse_with_custom_spacing() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"values": []},
        "mark": {"type": "bar", "binSpacing": 2},
        "encoding": {"x": {"bin": true, "field": "x", "type": "quantitative", "scale": {"reverse": {"signal": "r"}}}},
        "config": {"bar": {"minBandSize": null}}
    }));
    let props = rect_position(&model, "x");
    assert_eq_json(
        json!({"signal": "0.5 + (r ? -1 : 1) * -1"}),
        props["x"]["offset"].clone(),
    );
    assert_eq_json(
        json!({"signal": "0.5 + (r ? -1 : 1) * 1"}),
        props["x2"]["offset"].clone(),
    );
}

#[test]
fn y_mixins_for_signal_reverse_with_different_spacing() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"values": []},
        "mark": {"type": "bar", "binSpacing": 2},
        "encoding": {"y": {"bin": true, "field": "x", "type": "quantitative", "scale": {"reverse": {"signal": "r"}}}},
        "config": {"bar": {"minBandSize": null}}
    }));
    let props = rect_position(&model, "y");
    assert_eq_json(
        json!({"signal": "0.5 + (r ? -1 : 1) * -1"}),
        props["y2"]["offset"].clone(),
    );
    assert_eq_json(
        json!({"signal": "0.5 + (r ? -1 : 1) * 1"}),
        props["y"]["offset"].clone(),
    );
}

#[test]
fn warns_for_invalid_binned_spec_without_x2() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"values": []},
        "mark": {"type": "bar", "binSpacing": 2},
        "encoding": {"x": {"bin": "binned", "field": "x", "type": "quantitative", "scale": {"reverse": {"signal": "r"}}}}
    }));
    let (props, logger) = vega_lite::log::capture(|| rect_position(&model, "x"));
    assert!(props.as_object().map(|m| m.is_empty()).unwrap_or(false));
    assert!(!logger.warns.is_empty());
}

#[test]
fn warns_for_invalid_binned_spec_without_y2() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"values": []},
        "mark": {"type": "bar", "binSpacing": 2},
        "encoding": {"y": {"bin": "binned", "field": "y", "type": "quantitative", "scale": {"reverse": {"signal": "r"}}}}
    }));
    let (props, logger) = vega_lite::log::capture(|| rect_position(&model, "y"));
    assert!(props.as_object().map(|m| m.is_empty()).unwrap_or(false));
    assert!(!logger.warns.is_empty());
}

#[test]
fn x_mixins_for_signal_translate() {
    let mut model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"values": []},
        "mark": "bar",
        "encoding": {"x": {"bin": true, "field": "x", "type": "quantitative", "axis": {"translate": {"signal": "t"}}}},
        "config": {"bar": {"minBandSize": null}}
    }));
    model.component.axes = json!({"x": [{"translate": {"signal": "t"}}]});
    let props = rect_position(&model, "x");
    assert_eq_json(json!({"signal": "t + -0.5"}), props["x"]["offset"].clone());
    assert_eq_json(json!({"signal": "t + 0.5"}), props["x2"]["offset"].clone());
}

#[test]
fn x_mixins_for_signal_translate_reverse_and_offset() {
    let mut model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"values": []},
        "mark": {"type": "bar", "xOffset": {"signal": "o"}},
        "encoding": {"x": {"bin": true, "field": "x", "type": "quantitative", "scale": {"reverse": {"signal": "r"}}, "axis": {"translate": {"signal": "t"}}}},
        "config": {"bar": {"minBandSize": null}}
    }));
    model.component.axes = json!({"x": [{"translate": {"signal": "t"}}]});
    let props = rect_position(&model, "x");
    assert_eq_json(
        json!({"signal": "t + (r ? -1 : 1) * (o + -0.5)"}),
        props["x"]["offset"].clone(),
    );
    assert_eq_json(
        json!({"signal": "t + (r ? -1 : 1) * (o + 0.5)"}),
        props["x2"]["offset"].clone(),
    );
}

#[test]
fn default_y_mixins_for_empty_y_encoding() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"values": []},
        "mark": {"type": "bar"},
        "encoding": {"x": {"field": "x", "type": "quantitative", "aggregate": "mean"}}
    }));
    let props = rect_position(&model, "y");
    assert_eq_json(json!({"signal": "0.9 * height"}), props["height"].clone());
}

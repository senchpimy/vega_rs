//! Port of `test/compile/mark/rule.test.ts`.

#[path = "mark_util.rs"]
mod mark_util;

use serde_json::json;

use mark_util::{assert_eq_json, parse_unit_model_with_scale_and_layout};
use vega_lite::compile::mark::rule::encode_entry as rule;

#[test]
fn without_encoding_shows_nothing() {
    let model = parse_unit_model_with_scale_and_layout(&json!({"mark": "rule", "encoding": {}}));
    let props = rule(&model);
    assert!(props.get("x").is_none());
    assert!(props.get("y").is_none());
}

#[test]
fn with_x_only_vertical_rule_fits_height() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "rule",
        "encoding": {"x": {"field": "a", "type": "quantitative"}}
    }));
    let props = rule(&model);
    assert_eq_json(json!({"scale": "x", "field": "a"}), props["x"].clone());
    assert_eq_json(json!({"value": 0}), props["y"].clone());
    assert_eq_json(json!({"field": {"group": "height"}}), props["y2"].clone());
}

#[test]
fn with_y_only_horizontal_rule_fits_width() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "rule",
        "encoding": {"y": {"field": "a", "type": "quantitative"}}
    }));
    let props = rule(&model);
    assert_eq_json(json!({"scale": "y", "field": "a"}), props["y"].clone());
    assert_eq_json(json!({"field": {"group": "width"}}), props["x"].clone());
    assert_eq_json(json!({"value": 0}), props["x2"].clone());
}

#[test]
fn with_y_only_log_scale() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "rule",
        "encoding": {"y": {"field": "a", "type": "quantitative", "scale": {"type": "log"}}}
    }));
    let props = rule(&model);
    assert_eq_json(json!({"scale": "y", "field": "a"}), props["y"].clone());
    assert_eq_json(json!({"value": 0}), props["x2"].clone());
    assert_eq_json(json!({"field": {"group": "width"}}), props["x"].clone());
}

#[test]
fn with_x_and_x2_only() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "rule",
        "encoding": {
            "x": {"field": "a", "type": "quantitative"},
            "x2": {"field": "a2"}
        }
    }));
    let props = rule(&model);
    assert_eq_json(json!({"scale": "x", "field": "a"}), props["x"].clone());
    assert_eq_json(json!({"scale": "x", "field": "a2"}), props["x2"].clone());
    assert_eq_json(json!({"mult": 0.5, "signal": "height"}), props["y"].clone());
}

#[test]
fn with_y_and_y2_only() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "rule",
        "encoding": {
            "y": {"field": "a", "type": "quantitative"},
            "y2": {"field": "a2"}
        }
    }));
    let props = rule(&model);
    assert_eq_json(json!({"scale": "y", "field": "a"}), props["y"].clone());
    assert_eq_json(json!({"scale": "y", "field": "a2"}), props["y2"].clone());
    assert_eq_json(json!({"mult": 0.5, "signal": "width"}), props["x"].clone());
}

#[test]
fn with_x_x2_y() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "rule",
        "encoding": {
            "x": {"field": "a", "type": "quantitative"},
            "x2": {"field": "a2"},
            "y": {"field": "b", "type": "quantitative"}
        }
    }));
    let props = rule(&model);
    assert_eq_json(json!({"scale": "x", "field": "a"}), props["x"].clone());
    assert_eq_json(json!({"scale": "x", "field": "a2"}), props["x2"].clone());
    assert_eq_json(json!({"scale": "y", "field": "b"}), props["y"].clone());
}

#[test]
fn with_x_x2_y_y2() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "rule",
        "encoding": {
            "x": {"field": "a", "type": "quantitative"},
            "x2": {"field": "a2"},
            "y": {"field": "b", "type": "quantitative"},
            "y2": {"field": "b2"}
        }
    }));
    let props = rule(&model);
    assert_eq_json(json!({"scale": "x", "field": "a"}), props["x"].clone());
    assert_eq_json(json!({"scale": "x", "field": "a2"}), props["x2"].clone());
    assert_eq_json(json!({"scale": "y", "field": "b"}), props["y"].clone());
    assert_eq_json(json!({"scale": "y", "field": "b2"}), props["y2"].clone());
}

#[test]
fn with_x_and_y() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "rule",
        "encoding": {
            "x": {"field": "a", "type": "quantitative"},
            "y": {"field": "b", "type": "quantitative"}
        }
    }));
    let props = rule(&model);
    assert_eq_json(json!({"scale": "x", "field": "a"}), props["x"].clone());
    assert_eq_json(json!({"scale": "y", "field": "b"}), props["y"].clone());
}

#[test]
fn with_y_y2_x() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "rule",
        "encoding": {
            "y": {"field": "a", "type": "quantitative"},
            "y2": {"field": "a2"},
            "x": {"field": "b", "type": "quantitative"}
        }
    }));
    let props = rule(&model);
    assert_eq_json(json!({"scale": "y", "field": "a"}), props["y"].clone());
    assert_eq_json(json!({"scale": "y", "field": "a2"}), props["y2"].clone());
    assert_eq_json(json!({"scale": "x", "field": "b"}), props["x"].clone());
}

#[test]
fn nominal_x_quantitative_y_no_y2_emulates_bar() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "rule",
        "encoding": {
            "x": {"field": "a", "type": "ordinal"},
            "y": {"field": "b", "type": "quantitative"}
        }
    }));
    assert_eq_json(json!("vertical"), model.unit.mark_def["orient"].clone());
    let props = rule(&model);
    assert_eq_json(json!({"scale": "x", "field": "a", "band": 0.5}), props["x"].clone());
    assert_eq_json(json!({"scale": "y", "field": "b"}), props["y"].clone());
    assert_eq_json(json!({"scale": "y", "value": 0}), props["y2"].clone());
}

#[test]
fn nominal_y_quantitative_x_no_y2_emulates_bar() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "rule",
        "encoding": {
            "y": {"field": "a", "type": "ordinal"},
            "x": {"field": "b", "type": "quantitative"}
        }
    }));
    assert_eq_json(json!("horizontal"), model.unit.mark_def["orient"].clone());
    let props = rule(&model);
    assert_eq_json(json!({"scale": "x", "field": "b"}), props["x"].clone());
    assert_eq_json(json!({"scale": "x", "value": 0}), props["x2"].clone());
    assert_eq_json(json!({"scale": "y", "field": "a", "band": 0.5}), props["y"].clone());
}

#[test]
fn horizontal_stacked_rule_with_color() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "rule",
        "encoding": {
            "y": {"field": "a", "type": "ordinal"},
            "x": {"aggregate": "sum", "field": "b", "type": "quantitative", "stack": "zero"},
            "color": {"field": "Origin", "type": "nominal"}
        },
        "config": {"mark": {"invalid": null}}
    }));
    let props = rule(&model);
    assert_eq_json(json!({"scale": "x", "field": "sum_b_end"}), props["x"].clone());
    assert_eq_json(json!({"scale": "x", "field": "sum_b_start"}), props["x2"].clone());
    assert_eq_json(json!({"scale": "color", "field": "Origin"}), props["stroke"].clone());
}

#[test]
fn vertical_stacked_rule_with_color() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "rule",
        "encoding": {
            "x": {"field": "a", "type": "ordinal"},
            "y": {"aggregate": "sum", "field": "b", "type": "quantitative", "stack": "zero"},
            "color": {"field": "Origin", "type": "nominal"}
        },
        "config": {"mark": {"invalid": null}}
    }));
    let props = rule(&model);
    assert_eq_json(json!({"scale": "y", "field": "sum_b_end"}), props["y"].clone());
    assert_eq_json(json!({"scale": "y", "field": "sum_b_start"}), props["y2"].clone());
    assert_eq_json(json!({"scale": "color", "field": "Origin"}), props["stroke"].clone());
}

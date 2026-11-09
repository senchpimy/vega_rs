//! Port of `test/compile/legend/properties.test.ts`.

use serde_json::{json, Value};

use vega_lite::compile::legend::properties::{
    clip_height, default_direction, default_label_overlap, default_symbol_type, values,
};
use vega_lite::scale::ScaleType;

#[test]
fn default_direction_should_return_horizontal_for_top_bottom() {
    for orient in ["top", "bottom"] {
        assert_eq!(
            default_direction(Some(orient), "gradient"),
            Some("horizontal".to_string())
        );
    }
}

#[test]
fn default_direction_should_return_undefined_for_left_right() {
    for orient in [Some("left"), Some("right"), None, Some("none")] {
        assert_eq!(default_direction(orient, "gradient"), None);
    }
}

#[test]
fn default_direction_should_return_horizontal_for_quantitative_inner_legend() {
    for orient in ["top-left", "top-right", "bottom-left", "bottom-right"] {
        assert_eq!(
            default_direction(Some(orient), "gradient"),
            Some("horizontal".to_string())
        );
    }
}

#[test]
fn default_direction_should_return_undefined_for_discrete_inner_legend() {
    for orient in ["top-left", "top-right", "bottom-left", "bottom-right"] {
        assert_eq!(default_direction(Some(orient), "symbol"), None);
    }
}

#[test]
fn values_should_return_correct_timestamp_values_for_date_times() {
    let values = values(
        &json!({"values": [{"year": 1970}, {"year": 1980}]}),
        &json!({"field": "a", "type": "temporal"}),
    );
    assert_eq!(
        values,
        Some(json!([
            {"signal": "datetime(1970, 0, 1, 0, 0, 0, 0)"},
            {"signal": "datetime(1980, 0, 1, 0, 0, 0, 0)"}
        ]))
    );
}

#[test]
fn values_should_simply_return_values_for_non_date_time() {
    let values = values(
        &json!({"values": [1, 2, 3, 4]}),
        &json!({"field": "a", "type": "quantitative"}),
    );
    assert_eq!(values, Some(json!([1, 2, 3, 4])));
}

#[test]
fn values_returns_signal_correctly_for_non_date_time() {
    let values = values(
        &json!({"values": {"signal": "a"}}),
        &json!({"field": "a", "type": "quantitative"}),
    );
    assert_eq!(values, Some(json!({"signal": "a"})));
}

#[test]
fn clip_height_should_return_clip_height_for_gradient_legend() {
    assert_eq!(clip_height("gradient"), Some(json!(20)));
}

#[test]
fn clip_height_should_simply_return_for_symbol_legends() {
    assert_eq!(clip_height("symbol"), None);
}

#[test]
fn default_label_overlap_should_return_undefined_for_linear() {
    assert_eq!(default_label_overlap(ScaleType::Linear), None);
}

#[test]
fn default_label_overlap_should_return_greedy_for_log() {
    assert_eq!(
        default_label_overlap(ScaleType::Log),
        Some(Value::String("greedy".to_string()))
    );
}

#[test]
fn default_label_overlap_should_return_greedy_for_symlog() {
    assert_eq!(
        default_label_overlap(ScaleType::Symlog),
        Some(Value::String("greedy".to_string()))
    );
}

#[test]
fn default_label_overlap_should_return_greedy_for_threshold() {
    assert_eq!(
        default_label_overlap(ScaleType::Threshold),
        Some(Value::String("greedy".to_string()))
    );
}

#[test]
fn default_symbol_type_return_stroke_for_line() {
    assert_eq!(
        default_symbol_type("line", "color", Some(&json!({})), None),
        Some(json!("stroke"))
    );
}

#[test]
fn default_symbol_type_return_stroke_for_rule() {
    assert_eq!(
        default_symbol_type("rule", "color", Some(&json!({})), None),
        Some(json!("stroke"))
    );
}

#[test]
fn default_symbol_type_return_square_for_rect() {
    assert_eq!(
        default_symbol_type("rect", "color", Some(&json!({})), None),
        Some(json!("square"))
    );
}

#[test]
fn default_symbol_type_return_circle_for_point() {
    assert_eq!(
        default_symbol_type("point", "color", Some(&json!({})), None),
        Some(json!("circle"))
    );
}

#[test]
fn default_symbol_type_return_the_mark_shape_if_defined() {
    assert_eq!(
        default_symbol_type("point", "color", Some(&json!({})), Some(&json!("triangle"))),
        Some(json!("triangle"))
    );
}

#[test]
fn default_symbol_type_return_the_value_of_the_shape_encoding() {
    assert_eq!(
        default_symbol_type("point", "color", Some(&json!({"value": "triangle"})), None),
        Some(json!("triangle"))
    );
}

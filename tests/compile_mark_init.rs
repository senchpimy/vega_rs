//! Port of `test/compile/mark/init.test.ts`.

use serde_json::{json, Value};

use vega_lite::compile::mark::init::init_markdef;
use vega_lite::compile::model::Model;
use vega_lite::compile::support::{default_config, init_config};
use vega_lite::mark::{GEOSHAPE, POINT, PRIMITIVE_MARKS, TICK, CIRCLE, SQUARE};

fn parse_unit_model_with_scale_and_layout_size(spec: Value) -> Model {
    let config = init_config(spec.get("config"));
    let mut model = Model::new_unit(&spec, None, "", None, &config);
    model.parse_scale();
    model.parse_layout_size();
    model
}

fn mark_def(spec: Value) -> Value {
    parse_unit_model_with_scale_and_layout_size(spec)
        .mark_def()
        .unwrap()
        .clone()
}

fn opacity(spec: Value) -> Option<Value> {
    mark_def(spec).get("opacity").cloned()
}

fn orient(spec: Value) -> Option<String> {
    mark_def(spec)
        .get("orient")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn cursor(spec: Value) -> Option<Value> {
    mark_def(spec).get("cursor").cloned()
}

#[test]
fn preserves_corner_radius_end_for_ranged_bars() {
    let result = init_markdef(
        &json!({"type": "bar", "cornerRadiusEnd": 5}),
        &json!({"x": {"field": "x", "type": "quantitative"}, "x2": {"field": "x2"}}),
        &default_config(),
    );
    assert_eq!(result["cornerRadiusEnd"], json!(5));

    let result = init_markdef(
        &json!({"type": "bar", "cornerRadiusEnd": 5}),
        &json!({"y": {"field": "x", "type": "quantitative"}, "y2": {"field": "x2"}}),
        &default_config(),
    );
    assert_eq!(result["cornerRadiusEnd"], json!(5));
}

#[test]
fn preserves_corner_radius_end_for_vertical_bars() {
    let result = init_markdef(
        &json!({"type": "bar", "cornerRadiusEnd": 5}),
        &json!({"y": {"field": "x", "type": "quantitative"}}),
        &default_config(),
    );
    assert_eq!(result["cornerRadiusEnd"], json!(5));
}

#[test]
fn preserves_corner_radius_end_for_horizontal_bars() {
    let result = init_markdef(
        &json!({"type": "bar", "cornerRadiusEnd": 5}),
        &json!({"x": {"field": "x", "type": "quantitative"}}),
        &default_config(),
    );
    assert_eq!(result["cornerRadiusEnd"], json!(5));
}

#[test]
fn should_return_0_7_by_default_for_unaggregated_point_tick_circle_square() {
    for mark in ["point", "tick", "circle", "square"] {
        let result = opacity(json!({
            "mark": mark,
            "encoding": {
                "y": {"type": "quantitative", "field": "foo"},
                "x": {"type": "quantitative", "field": "bar"}
            }
        }));
        assert_eq!(result, Some(json!(0.7)), "mark={mark}");
    }
}

#[test]
fn should_return_undefined_by_default_for_aggregated_point_tick_circle_square() {
    for mark in ["point", "tick", "circle", "square"] {
        let result = opacity(json!({
            "mark": mark,
            "encoding": {
                "y": {"aggregate": "mean", "type": "quantitative", "field": "foo"},
                "x": {"type": "nominal", "field": "bar"}
            }
        }));
        assert_eq!(result, None, "mark={mark}");
    }
}

#[test]
fn should_use_specified_opacity() {
    for mark in ["point", "tick", "circle", "square"] {
        let result = opacity(json!({
            "mark": {"type": mark, "opacity": 0.9},
            "encoding": {
                "y": {"type": "quantitative", "field": "foo"},
                "x": {"type": "quantitative", "field": "bar"}
            }
        }));
        assert_eq!(result, Some(json!(0.9)), "mark={mark}");
    }
}

#[test]
fn should_return_undefined_by_default_for_other_marks() {
    for mark in PRIMITIVE_MARKS {
        if [POINT, TICK, CIRCLE, SQUARE, GEOSHAPE].contains(&mark) {
            continue;
        }
        let result = opacity(json!({
            "mark": mark.as_str(),
            "encoding": {
                "y": {"type": "quantitative", "field": "foo"},
                "x": {"type": "nominal", "field": "bar"}
            }
        }));
        assert_eq!(result, None, "mark={}", mark.as_str());
    }
}

#[test]
fn should_return_correct_default_for_qxq() {
    assert_eq!(
        orient(json!({
            "mark": "bar",
            "encoding": {
                "y": {"type": "quantitative", "field": "foo"},
                "x": {"type": "quantitative", "field": "bar"}
            }
        })),
        Some("vertical".to_string())
    );
}

#[test]
fn should_return_correct_default_for_t() {
    assert_eq!(
        orient(json!({
            "mark": "bar",
            "encoding": {"x": {"type": "temporal", "field": "bar"}}
        })),
        Some("vertical".to_string())
    );
}

#[test]
fn should_return_correct_default_for_empty_plot() {
    assert_eq!(orient(json!({"mark": "bar", "encoding": {}})), None);
}

#[test]
fn should_return_correct_orient_for_bar_with_both_axes_discrete() {
    assert_eq!(
        orient(json!({
            "mark": "bar",
            "encoding": {
                "x": {"type": "ordinal", "field": "foo"},
                "y": {"type": "ordinal", "field": "bar"}
            }
        })),
        None
    );
}

#[test]
fn should_return_correct_orient_for_vertical_bar() {
    assert_eq!(
        orient(json!({
            "mark": "bar",
            "encoding": {
                "y": {"type": "quantitative", "field": "foo"},
                "x": {"type": "ordinal", "field": "bar"}
            }
        })),
        Some("vertical".to_string())
    );
}

#[test]
fn should_return_correct_orient_for_horizontal_bar() {
    assert_eq!(
        orient(json!({
            "mark": "bar",
            "encoding": {
                "x": {"type": "quantitative", "field": "foo"},
                "y": {"type": "ordinal", "field": "bar"}
            }
        })),
        Some("horizontal".to_string())
    );
}

#[test]
fn should_return_correct_orient_for_vertical_bar_with_raw_temporal_dimension() {
    assert_eq!(
        orient(json!({
            "mark": "bar",
            "encoding": {
                "y": {"type": "quantitative", "field": "foo"},
                "x": {"type": "temporal", "field": "bar"}
            }
        })),
        Some("vertical".to_string())
    );
}

#[test]
fn should_return_correct_orient_for_horizontal_bar_with_raw_temporal_dimension() {
    assert_eq!(
        orient(json!({
            "mark": "bar",
            "encoding": {
                "x": {"type": "quantitative", "field": "foo"},
                "y": {"type": "temporal", "field": "bar"}
            }
        })),
        Some("horizontal".to_string())
    );
}

#[test]
fn should_return_correct_orient_for_vertical_with_aggregation() {
    assert_eq!(
        orient(json!({
            "mark": "bar",
            "encoding": {
                "x": {"type": "quantitative", "field": "foo", "aggregate": "mean"},
                "y": {"type": "quantitative", "field": "bar"}
            }
        })),
        Some("horizontal".to_string())
    );
}

#[test]
fn should_return_correct_orient_for_vertical_tick() {
    assert_eq!(
        orient(json!({
            "mark": "tick",
            "encoding": {
                "x": {"type": "quantitative", "field": "foo"},
                "y": {"type": "ordinal", "field": "bar"}
            }
        })),
        Some("vertical".to_string())
    );
}

#[test]
fn should_return_correct_orient_for_vertical_tick_with_bin() {
    assert_eq!(
        orient(json!({
            "mark": "tick",
            "encoding": {
                "x": {"type": "quantitative", "field": "foo"},
                "y": {"type": "quantitative", "field": "bar", "bin": true}
            }
        })),
        Some("vertical".to_string())
    );
}

#[test]
fn should_return_correct_orient_for_vertical_tick_of_continuous_timeunit_dotplot() {
    assert_eq!(
        orient(json!({
            "mark": "tick",
            "encoding": {
                "x": {"type": "temporal", "field": "foo", "timeUnit": "yearmonthdate"},
                "y": {"type": "ordinal", "field": "bar"}
            }
        })),
        Some("vertical".to_string())
    );
}

#[test]
fn should_return_correct_orient_for_horizontal_tick() {
    assert_eq!(
        orient(json!({
            "mark": "tick",
            "encoding": {
                "y": {"type": "quantitative", "field": "foo"},
                "x": {"type": "ordinal", "field": "bar"}
            }
        })),
        Some("horizontal".to_string())
    );
}

#[test]
fn should_return_correct_orient_for_vertical_rule() {
    assert_eq!(
        orient(json!({"mark": "rule", "encoding": {"x": {"value": 0}}})),
        Some("vertical".to_string())
    );
}

#[test]
fn should_return_correct_orient_for_horizontal_rule() {
    assert_eq!(
        orient(json!({"mark": "rule", "encoding": {"y": {"value": 0}}})),
        Some("horizontal".to_string())
    );
}

#[test]
fn should_return_undefined_for_line_segment_rule() {
    assert_eq!(
        orient(json!({
            "mark": "rule",
            "encoding": {
                "y": {"value": 0},
                "x": {"value": 0},
                "y2": {"value": 100},
                "x2": {"value": 100}
            }
        })),
        None
    );
}

#[test]
fn should_return_undefined_for_line_segment_rule_with_only_x_and_y() {
    assert_eq!(
        orient(json!({
            "mark": "rule",
            "encoding": {"y": {"value": 0}, "x": {"value": 0}}
        })),
        None
    );
}

#[test]
fn should_return_correct_orient_for_horizontal_rules_without_x2() {
    assert_eq!(
        orient(json!({
            "mark": "rule",
            "encoding": {
                "x": {"field": "b", "type": "quantitative"},
                "y": {"field": "a", "type": "ordinal"}
            }
        })),
        Some("horizontal".to_string())
    );
}

#[test]
fn should_return_correct_orient_for_vertical_rules_without_y2() {
    assert_eq!(
        orient(json!({
            "mark": "rule",
            "encoding": {
                "y": {"field": "b", "type": "quantitative"},
                "x": {"field": "a", "type": "ordinal"}
            }
        })),
        Some("vertical".to_string())
    );
}

#[test]
fn should_return_correct_orient_for_vertical_rule_with_range() {
    assert_eq!(
        orient(json!({
            "mark": "rule",
            "encoding": {
                "x": {"type": "ordinal", "field": "foo"},
                "y": {"type": "quantitative", "field": "bar"},
                "y2": {"field": "baz"}
            }
        })),
        Some("vertical".to_string())
    );
}

#[test]
fn should_return_correct_orient_for_horizontal_rule_with_range() {
    assert_eq!(
        orient(json!({
            "mark": "rule",
            "encoding": {
                "y": {"type": "ordinal", "field": "foo"},
                "x": {"type": "quantitative", "field": "bar"},
                "x2": {"field": "baz"}
            }
        })),
        Some("horizontal".to_string())
    );
}

#[test]
fn should_return_correct_orient_for_horizontal_rule_with_range_and_no_ordinal() {
    assert_eq!(
        orient(json!({
            "mark": "rule",
            "encoding": {
                "x": {"type": "quantitative", "field": "bar"},
                "x2": {"field": "baz"}
            }
        })),
        Some("horizontal".to_string())
    );
}

#[test]
fn should_return_correct_orient_for_vertical_rule_with_range_and_no_ordinal() {
    assert_eq!(
        orient(json!({
            "mark": "rule",
            "encoding": {
                "y": {"type": "quantitative", "field": "bar"},
                "y2": {"field": "baz"}
            }
        })),
        Some("vertical".to_string())
    );
}

#[test]
fn should_return_correct_orient_for_bar_with_vertical_binned_data() {
    assert_eq!(
        orient(json!({
            "mark": "bar",
            "encoding": {
                "x": {"field": "bin_start", "bin": "binned", "type": "quantitative", "axis": {"tickMinStep": 2}},
                "x2": {"field": "bin_end"},
                "y": {"field": "count", "type": "quantitative"}
            }
        })),
        Some("vertical".to_string())
    );
}

#[test]
fn should_return_correct_orient_for_bar_with_horizontal_binned_data() {
    assert_eq!(
        orient(json!({
            "mark": "bar",
            "encoding": {
                "y": {"field": "bin_start", "bin": "binned", "type": "quantitative", "axis": {"tickMinStep": 2}},
                "y2": {"field": "bin_end"},
                "x": {"field": "count", "type": "quantitative"}
            }
        })),
        Some("horizontal".to_string())
    );
}

#[test]
fn should_return_correct_orient_for_bar_with_horizontal_un_binned_data() {
    assert_eq!(
        orient(json!({
            "mark": "bar",
            "encoding": {
                "x": {"field": "q_start", "type": "quantitative"},
                "x2": {"field": "q_end"},
                "y": {"field": "count", "type": "quantitative"}
            }
        })),
        Some("horizontal".to_string())
    );
}

#[test]
fn should_return_correct_orient_for_bar_with_vertical_un_binned_data() {
    assert_eq!(
        orient(json!({
            "mark": "bar",
            "encoding": {
                "y": {"field": "q_start", "type": "quantitative"},
                "y2": {"field": "q_end"},
                "x": {"field": "count", "type": "quantitative"}
            }
        })),
        Some("vertical".to_string())
    );
}

#[test]
fn should_return_vertical_orient_for_bar_with_only_y_offset_quantitative() {
    assert_eq!(
        orient(json!({
            "mark": "bar",
            "encoding": {
                "x": {"field": "value", "type": "quantitative"},
                "yOffset": {"field": "density", "type": "quantitative"}
            }
        })),
        Some("vertical".to_string())
    );
}

#[test]
fn should_return_horizontal_orient_for_bar_with_only_x_offset_quantitative() {
    assert_eq!(
        orient(json!({
            "mark": "bar",
            "encoding": {
                "y": {"field": "value", "type": "quantitative"},
                "xOffset": {"field": "density", "type": "quantitative"}
            }
        })),
        Some("horizontal".to_string())
    );
}

#[test]
fn should_return_correct_orient_for_area_with_vertical_binned_data() {
    assert_eq!(
        orient(json!({
            "mark": "area",
            "encoding": {
                "x": {"field": "bin_start", "bin": "binned", "type": "quantitative", "axis": {"tickMinStep": 2}},
                "x2": {"field": "bin_end"},
                "y": {"field": "count", "type": "quantitative"}
            }
        })),
        Some("vertical".to_string())
    );
}

#[test]
fn should_return_correct_orient_for_area_with_horizontal_binned_data() {
    assert_eq!(
        orient(json!({
            "mark": "area",
            "encoding": {
                "y": {"field": "bin_start", "bin": "binned", "type": "quantitative", "axis": {"tickMinStep": 2}},
                "y2": {"field": "bin_end"},
                "x": {"field": "count", "type": "quantitative"}
            }
        })),
        Some("horizontal".to_string())
    );
}

#[test]
fn should_return_vertical_orient_for_area_with_only_y_offset_quantitative() {
    assert_eq!(
        orient(json!({
            "mark": "area",
            "encoding": {
                "x": {"field": "value", "type": "quantitative"},
                "yOffset": {"field": "density", "type": "quantitative"}
            }
        })),
        Some("vertical".to_string())
    );
}

#[test]
fn should_return_horizontal_orient_for_area_with_only_x_offset_quantitative() {
    assert_eq!(
        orient(json!({
            "mark": "area",
            "encoding": {
                "y": {"field": "value", "type": "quantitative"},
                "xOffset": {"field": "density", "type": "quantitative"}
            }
        })),
        Some("horizontal".to_string())
    );
}

#[test]
fn cursor_should_be_undefined_when_no_href_channel_defined() {
    assert_eq!(
        cursor(json!({
            "mark": "bar",
            "encoding": {
                "y": {"type": "quantitative", "field": "foo"},
                "x": {"type": "temporal", "field": "bar"}
            }
        })),
        None
    );
}

#[test]
fn should_return_pointer_cursor_when_href_channel_present() {
    assert_eq!(
        cursor(json!({
            "mark": "bar",
            "params": [{"name": "test", "select": "point"}],
            "encoding": {
                "x": {"field": "a", "type": "ordinal"},
                "y": {"field": "b", "type": "quantitative"},
                "href": {
                    "condition": {"param": "test", "value": "https://vega.github.io/schema/vega-lite/v6.json"},
                    "field": "a",
                    "type": "ordinal"
                }
            }
        })),
        Some(json!("pointer"))
    );
}

#[test]
fn should_return_specified_cursor_when_href_channel_present_but_cursor_specified() {
    assert_eq!(
        cursor(json!({
            "mark": {"type": "bar", "cursor": "auto"},
            "params": [{"name": "test", "select": "point"}],
            "encoding": {
                "x": {"field": "a", "type": "ordinal"},
                "y": {"field": "b", "type": "quantitative"},
                "href": {
                    "condition": {"param": "test", "value": "http://www.google.com"},
                    "field": "a",
                    "type": "ordinal"
                }
            }
        })),
        Some(json!("auto"))
    );
}

#[test]
fn should_return_pointer_cursor_when_href_channel_specified_in_mark_definition() {
    assert_eq!(
        cursor(json!({
            "mark": {"type": "bar", "href": "http://www.google.com"},
            "encoding": {
                "y": {"type": "quantitative", "field": "foo"},
                "x": {"type": "temporal", "field": "bar"}
            }
        })),
        Some(json!("pointer"))
    );
}

#[test]
fn should_return_specified_cursor_when_href_in_mark_definition_but_cursor_also_specified() {
    assert_eq!(
        cursor(json!({
            "mark": {"type": "bar", "href": "http://www.google.com", "cursor": "auto"},
            "encoding": {
                "y": {"type": "quantitative", "field": "foo"},
                "x": {"type": "temporal", "field": "bar"}
            }
        })),
        Some(json!("auto"))
    );
}

#[test]
fn should_return_pointer_cursor_when_href_channel_specified_in_mark_config() {
    assert_eq!(
        cursor(json!({
            "config": {"mark": {"href": "http://www.google.com"}},
            "mark": "bar",
            "encoding": {
                "y": {"type": "quantitative", "field": "foo"},
                "x": {"type": "temporal", "field": "bar"}
            }
        })),
        Some(json!("pointer"))
    );
}

#[test]
fn should_return_specified_cursor_when_href_in_mark_config_but_cursor_also_specified_in_mark() {
    assert_eq!(
        cursor(json!({
            "config": {"mark": {"href": "http://www.google.com"}},
            "mark": {"type": "bar", "cursor": "auto"},
            "encoding": {
                "y": {"type": "quantitative", "field": "foo"},
                "x": {"type": "temporal", "field": "bar"}
            }
        })),
        Some(json!("auto"))
    );
}

#[test]
fn should_not_specify_cursor_in_the_markdef_if_defined_in_the_config() {
    assert_eq!(
        cursor(json!({
            "config": {"mark": {"href": "http://www.google.com", "cursor": "auto"}},
            "mark": "bar",
            "encoding": {
                "y": {"type": "quantitative", "field": "foo"},
                "x": {"type": "temporal", "field": "bar"}
            }
        })),
        None
    );
}

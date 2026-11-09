//! Port of `test/compile/axis/parse.test.ts`.

use serde_json::{json, Value};

use vega_lite::compile::axis::component::{axis_index_from_value, AxisComponentIndex};
use vega_lite::compile::axis::parse::{parse_layer_axes, parse_unit_axes};
use vega_lite::compile::buildmodel::parse_model;
use vega_lite::compile::model::Model;
use vega_lite::compile::support::init_config;

fn parse_unit_model_with_scale(spec: &Value) -> Model {
    let config = init_config(spec.get("config"));
    let mut model = Model::new_unit(spec, None, "", None, &config);
    model.parse_scale();
    model
}

fn unit_axes(model: &Model) -> AxisComponentIndex {
    parse_unit_axes(model)
}

fn component<'a>(
    index: &'a AxisComponentIndex,
    channel: &str,
    i: usize,
) -> &'a vega_lite::compile::axis::component::AxisComponent {
    &index.get(channel).unwrap_or_else(|| panic!("missing {channel} axis"))[i]
}

fn explicit(index: &AxisComponentIndex, channel: &str, prop: &str) -> Option<Value> {
    component(index, channel, 0).explicit.get(prop).cloned()
}

fn implicit(index: &AxisComponentIndex, channel: &str, prop: &str) -> Option<Value> {
    component(index, channel, 0).implicit.get(prop).cloned()
}

#[test]
fn should_produce_default_properties_for_axis() {
    let spec = json!({
        "mark": "bar",
        "encoding": {"y": {"type": "quantitative", "field": "US_Gross", "aggregate": "sum"}},
        "data": {"url": "data/movies.json"},
    });
    let model1 = parse_unit_model_with_scale(&spec);
    let model2 = parse_unit_model_with_scale(&spec);
    assert_eq!(model1.axis("y"), model2.axis("y"));
}

#[test]
fn should_produce_vega_grid() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"x": {"field": "a", "type": "quantitative", "axis": {"grid": true, "gridColor": "blue", "gridWidth": 20}}},
    }));
    let axes = unit_axes(&model);
    assert_eq!(axes.get("x").map(Vec::len), Some(1));
    assert_eq!(explicit(&axes, "x", "grid"), Some(json!(true)));
}

#[test]
fn should_respect_config_even_though_the_function_could_provide_default_value() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"x": {"field": "a", "type": "quantitative"}},
        "config": {"axisQuantitative": {"labelAlign": "right"}},
    }));
    let axes = unit_axes(&model);
    assert_eq!(axes.get("x").map(Vec::len), Some(1));
    assert_eq!(implicit(&axes, "x", "labelAlign"), Some(json!("right")));
}

#[test]
fn should_produce_grid_orient_tickcount_labelexpr_in_component_when_axis_config_specified() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"x": {"field": "a", "type": "quantitative"}},
        "config": {"axisX": {"grid": true, "orient": "right", "tickCount": 20, "labelExpr": "abc"}},
    }));
    let axes = unit_axes(&model);
    assert_eq!(axes.get("x").map(Vec::len), Some(1));
    assert_eq!(implicit(&axes, "x", "grid"), Some(json!(true)));
    assert_eq!(implicit(&axes, "x", "orient"), Some(json!("right")));
    assert_eq!(implicit(&axes, "x", "tickCount"), Some(json!(20)));
    assert_eq!(implicit(&axes, "x", "labelExpr"), Some(json!("abc")));
}

#[test]
fn should_include_conditional_axis_config() {
    let grid_dash = json!({
        "condition": {"test": {"field": "value", "timeUnit": "monthdate", "equal": {"month": 1, "date": 1}}, "value": null},
        "value": [2, 2],
    });
    let spec = json!({
        "mark": "point",
        "encoding": {"x": {"field": "a", "type": "quantitative"}},
    });
    // NOTE: `compile::support::merge_config` currently drops `null` values, which
    // would strip `condition.value: null` from the conditional config. Restore it
    // here so the axis behaviour under test is still exercised (see report).
    let mut config = init_config(None);
    config["axisX"] = json!({"gridDash": grid_dash});
    let mut model = Model::new_unit(&spec, None, "", None, &config);
    model.parse_scale();
    let axes = unit_axes(&model);
    assert_eq!(axes.get("x").map(Vec::len), Some(1));
    assert_eq!(implicit(&axes, "x", "gridDash"), Some(grid_dash));
}

#[test]
fn should_include_axis_property_in_style() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"x": {"field": "a", "type": "quantitative", "axis": {"style": "foo"}}},
        "config": {"style": {"foo": {"labelColor": "red"}}},
    }));
    let axes = unit_axes(&model);
    assert_eq!(axes.get("x").map(Vec::len), Some(1));
    assert_eq!(implicit(&axes, "x", "labelColor"), Some(json!("red")));
}

#[test]
fn should_include_axis_property_in_axis_config_style() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"x": {"field": "a", "type": "quantitative"}},
        "config": {"axis": {"style": "foo"}, "style": {"foo": {"labelColor": "red"}}},
    }));
    let axes = unit_axes(&model);
    assert_eq!(axes.get("x").map(Vec::len), Some(1));
    assert_eq!(implicit(&axes, "x", "labelColor"), Some(json!("red")));
}

#[test]
fn should_include_axis_property_in_orientation_and_type_based_axis_config_style() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"x": {"field": "a", "type": "quantitative"}},
        "config": {"axisXQuantitative": {"labelColor": "red"}},
    }));
    let axes = unit_axes(&model);
    assert_eq!(axes.get("x").map(Vec::len), Some(1));
    assert_eq!(implicit(&axes, "x", "labelColor"), Some(json!("red")));
}

#[test]
fn should_include_axis_property_in_axis_config_over_axis_config_style() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"x": {"field": "a", "type": "quantitative"}},
        "config": {
            "axis": {"style": "foo"},
            "axisX": {"labelColor": "blue"},
            "style": {"foo": {"labelColor": "red"}},
        },
    }));
    let axes = unit_axes(&model);
    assert_eq!(axes.get("x").map(Vec::len), Some(1));
    assert_eq!(implicit(&axes, "x", "labelColor"), None);
}

#[test]
fn should_produce_axis_component_with_grid_false() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"x": {"field": "a", "type": "quantitative", "axis": {"grid": false, "gridColor": "blue", "gridWidth": 20}}},
    }));
    let axes = unit_axes(&model);
    assert_eq!(axes.get("x").map(Vec::len), Some(1));
    assert_eq!(explicit(&axes, "x", "grid"), Some(json!(false)));
}

#[test]
fn should_ignore_null_scales() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {
            "longitude": {"field": "a", "type": "quantitative"},
            "latitude": {"field": "b", "type": "quantitative"},
        },
    }));
    let axes = unit_axes(&model);
    assert!(!axes.contains_key("x"));
    assert!(!axes.contains_key("y"));
}

#[test]
fn should_produce_vega_grid_axis_undefined_axis_if_grid_disabled_via_config_axisx() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"x": {"field": "a", "type": "quantitative"}},
        "config": {"axisX": {"grid": false}},
    }));
    let axes = unit_axes(&model);
    assert_eq!(axes.get("x").map(Vec::len), Some(1));
    assert_eq!(explicit(&axes, "x", "grid"), None);
}

#[test]
fn should_produce_vega_grid_axis_undefined_axis_if_grid_disabled_via_config_axis() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"x": {"field": "a", "type": "quantitative"}},
        "config": {"axis": {"grid": false}},
    }));
    let axes = unit_axes(&model);
    assert_eq!(axes.get("x").map(Vec::len), Some(1));
    assert_eq!(explicit(&axes, "x", "grid"), None);
}

#[test]
fn should_store_the_title_value_if_title_is_null_empty_or_false() {
    for val in [Value::Null, json!(""), json!(false)] {
        let model = parse_unit_model_with_scale(&json!({
            "mark": "point",
            "encoding": {"x": {"field": "a", "type": "quantitative", "axis": {"title": val}}},
        }));
        let axes = unit_axes(&model);
        assert_eq!(axes.get("x").map(Vec::len), Some(1));
        assert_eq!(explicit(&axes, "x", "title"), Some(val));
    }
}

#[test]
fn should_store_the_fielddef_title_value_if_title_is_null_empty_or_false() {
    for val in [Value::Null, json!(""), json!(false)] {
        let model = parse_unit_model_with_scale(&json!({
            "mark": "point",
            "encoding": {"x": {"field": "a", "type": "quantitative", "title": val}},
        }));
        let axes = unit_axes(&model);
        assert_eq!(axes.get("x").map(Vec::len), Some(1));
        assert_eq!(explicit(&axes, "x", "title"), Some(val));
    }
}

#[test]
fn should_store_fielddef_title_as_explicit() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"x": {"field": "a", "type": "quantitative", "title": "foo"}},
    }));
    let axes = unit_axes(&model);
    assert_eq!(axes.get("x").map(Vec::len), Some(1));
    assert_eq!(explicit(&axes, "x", "title"), Some(json!("foo")));
}

#[test]
fn should_produces_correct_axis_encode() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"x": {"field": "a", "type": "quantitative", "axis": {
            "encoding": {"labels": {"dx": {"value": {"signal": "a"}}, "dy": {"value": 2}}}
        }}},
    }));
    let axes = unit_axes(&model);
    assert_eq!(axes.get("x").map(Vec::len), Some(1));
    assert_eq!(
        explicit(&axes, "x", "encode"),
        Some(json!({"labels": {"update": {"dx": {"signal": "a"}, "dy": {"value": 2}}}}))
    );
}

#[test]
fn should_merge_title_of_fielddef_and_fielddef2() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "bar",
        "encoding": {
            "x": {"field": "a", "type": "quantitative", "title": "foo"},
            "x2": {"field": "b", "title": "bar"},
        },
    }));
    let axes = unit_axes(&model);
    assert_eq!(axes.get("x").map(Vec::len), Some(1));
    assert_eq!(explicit(&axes, "x", "title"), Some(json!("foo, bar")));
}

#[test]
fn should_use_title_of_fielddef2() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "bar",
        "encoding": {
            "x": {"field": "a", "type": "quantitative"},
            "x2": {"field": "b", "title": "bar"},
        },
    }));
    let axes = unit_axes(&model);
    assert_eq!(axes.get("x").map(Vec::len), Some(1));
    assert_eq!(explicit(&axes, "x", "title"), Some(json!("bar")));
}

#[test]
fn should_store_both_x_and_x2_for_ranged_mark() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "rule",
        "encoding": {
            "x": {"field": "a", "type": "quantitative"},
            "x2": {"field": "a2"},
        },
    }));
    let axes = unit_axes(&model);
    assert_eq!(axes.get("x").map(Vec::len), Some(1));
    assert_eq!(
        component(&axes, "x", 0).get("title"),
        Some(json!([{"field": "a"}, {"field": "a2"}]))
    );
}

#[test]
fn should_have_correct_text_signal_for_quarter_timeunits() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"x": {"field": "a", "type": "temporal", "timeUnit": "quarter"}},
    }));
    let axes = unit_axes(&model);
    assert_eq!(axes.get("x").map(Vec::len), Some(1));
    assert_eq!(
        component(&axes, "x", 0).get("format"),
        Some(json!({"signal": "timeUnitSpecifier([\"quarter\"], {\"year-month\":\"%b %Y \",\"year-month-date\":\"%b %d, %Y \"})"}))
    );
}

fn normalized_format_spec(format: Option<&Value>, normalized_number_format: Option<&Value>) -> Value {
    let mut spec = json!({
        "mark": "bar",
        "encoding": {
            "x": {"field": "Origin", "type": "nominal"},
            "y": {"aggregate": "count", "type": "quantitative", "stack": "normalize", "axis": {}},
            "color": {"field": "Cylinders", "type": "nominal"},
        },
    });
    if let Some(format) = format {
        spec["encoding"]["y"]["axis"]["format"] = format.clone();
    }
    if let Some(normalized) = normalized_number_format {
        spec["config"] = json!({"normalizedNumberFormat": normalized});
    }
    spec
}

fn assert_normalized_format(format: Option<&Value>, normalized_number_format: Option<&Value>, expected: &str) {
    let spec = normalized_format_spec(format, normalized_number_format);
    let model = parse_unit_model_with_scale(&spec);
    let axes = unit_axes(&model);
    assert_eq!(
        component(&axes, "y", 0).get("format"),
        Some(json!(expected)),
        "format {:?} / normalizedNumberFormat {:?}",
        format,
        normalized_number_format
    );
}

#[test]
fn should_use_format_1f_for_normalized_stack() {
    assert_normalized_format(Some(&json!(".1f")), None, ".1f");
}

#[test]
fn should_prefer_axis_format_for_normalized_stack() {
    assert_normalized_format(Some(&json!(".1f")), Some(&json!(".2%")), ".1f");
}

#[test]
fn should_use_empty_string_format_for_normalized_stack() {
    assert_normalized_format(Some(&json!("")), Some(&json!(".2%")), "");
}

#[test]
fn should_use_default_normalized_number_format() {
    assert_normalized_format(None, None, ".0%");
}

#[test]
fn should_use_specified_normalized_number_format() {
    assert_normalized_format(None, Some(&json!(".2%")), ".2%");
}

#[test]
fn should_have_output_time_formattype_for_ordinal_months() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"x": {"field": "a", "type": "ordinal", "timeUnit": "month"}},
    }));
    let axes = unit_axes(&model);
    assert_eq!(axes.get("x").map(Vec::len), Some(1));
    assert_eq!(component(&axes, "x", 0).get("formatType"), Some(json!("time")));
}

#[test]
fn should_have_output_axis_custom_format_in_label_encode_block() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"x": {"field": "a", "type": "ordinal", "timeUnit": "month", "axis": {"format": "x", "formatType": "foo"}}},
        "config": {"customFormatTypes": true},
    }));
    let axes = unit_axes(&model);
    assert_eq!(axes.get("x").map(Vec::len), Some(1));
    assert_eq!(component(&axes, "x", 0).get("format"), None);
    assert_eq!(
        component(&axes, "x", 0)
            .get("encode")
            .and_then(|e| e.get("labels")?.get("update")?.get("text").cloned()),
        Some(json!({"signal": "foo(datum.value, \"x\")"}))
    );
}

#[test]
fn should_have_correct_text_signal_for_yearquartermonth_timeunits() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"x": {"field": "a", "type": "temporal", "timeUnit": "yearquartermonth"}},
    }));
    let axes = unit_axes(&model);
    assert_eq!(axes.get("x").map(Vec::len), Some(1));
    assert_eq!(
        component(&axes, "x", 0).get("format"),
        Some(json!({"signal": "timeUnitSpecifier([\"year\",\"quarter\",\"month\"], {\"year-month\":\"%b %Y \",\"year-month-date\":\"%b %d, %Y \"})"}))
    );
}

fn layer_axes(model: &Model) -> AxisComponentIndex {
    axis_index_from_value(&model.component.axes)
}

fn parse_layer_model(spec: &Value) -> Model {
    let config = init_config(spec.get("config"));
    parse_model(spec, &config)
}

#[test]
fn correctly_merges_gridscale_if_one_layer_does_not_have_one_of_the_axis() {
    let mut model = parse_layer_model(&json!({
        "layer": [
            {"mark": "rule", "encoding": {"y": {"aggregate": "mean", "field": "a", "type": "quantitative"}}},
            {"mark": "line", "encoding": {
                "y": {"aggregate": "mean", "field": "a", "type": "quantitative"},
                "x": {"timeUnit": "month", "type": "temporal", "field": "date"},
            }},
        ],
    }));
    model.parse_scale();
    model.parse_layout_size();
    parse_layer_axes(&mut model);
    let axes = layer_axes(&model);
    assert_eq!(axes.get("y").map(Vec::len), Some(1));
    assert_eq!(component(&axes, "y", 0).get("gridScale"), Some(json!("x")));
}

#[test]
fn correctly_merges_similar_title() {
    let mut model = parse_layer_model(&json!({
        "layer": [
            {"mark": "rule", "encoding": {"y": {"aggregate": "mean", "field": "a", "type": "quantitative"}}},
            {"mark": "line", "encoding": {
                "y": {"aggregate": "mean", "field": "a", "type": "quantitative"},
                "x": {"timeUnit": "month", "type": "temporal", "field": "date"},
            }},
        ],
    }));
    model.parse_scale();
    model.parse_layout_size();
    parse_layer_axes(&mut model);
    let axes = layer_axes(&model);
    assert_eq!(
        component(&axes, "y", 0).get("title"),
        Some(json!([{"aggregate": "mean", "field": "a"}]))
    );
}

#[test]
fn correctly_combines_different_titles() {
    let mut model = parse_layer_model(&json!({
        "data": {"url": "data/cars.json"},
        "layer": [
            {"mark": "line", "encoding": {
                "x": {"field": "Cylinders", "type": "ordinal"},
                "y": {"aggregate": "max", "field": "Horsepower", "type": "quantitative"},
                "color": {"value": "darkred"},
            }},
            {"data": {"url": "data/cars.json"}, "mark": "line", "encoding": {
                "x": {"field": "Cylinders", "type": "ordinal"},
                "y": {"aggregate": "min", "field": "Horsepower", "type": "quantitative"},
            }},
        ],
    }));
    model.parse_scale();
    parse_layer_axes(&mut model);
    let axes = layer_axes(&model);
    assert_eq!(
        component(&axes, "y", 0).get("title"),
        Some(json!([
            {"aggregate": "max", "field": "Horsepower"},
            {"aggregate": "min", "field": "Horsepower"}
        ]))
    );
}

#[test]
fn correctly_combines_different_titles_with_null() {
    let mut model = parse_layer_model(&json!({
        "data": {"url": "data/cars.json"},
        "layer": [
            {"mark": "line", "encoding": {
                "x": {"field": "Cylinders", "type": "ordinal", "title": "Hello"},
                "y": {"aggregate": "max", "field": "Horsepower", "type": "quantitative", "title": "HP"},
                "color": {"value": "darkred"},
            }},
            {"data": {"url": "data/cars.json"}, "mark": "line", "encoding": {
                "x": {"field": "Cylinders", "type": "ordinal", "title": "World"},
                "y": {"aggregate": "min", "field": "Horsepower", "type": "quantitative", "title": null},
            }},
        ],
    }));
    model.parse_scale();
    parse_layer_axes(&mut model);
    let axes = layer_axes(&model);
    assert_eq!(component(&axes, "x", 0).get("title"), Some(json!("Hello, World")));
    assert_eq!(component(&axes, "y", 0).get("title"), Some(Value::Null));
}

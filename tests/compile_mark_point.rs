//! Port of `test/compile/mark/point.test.ts`.

#[path = "mark_util.rs"]
mod mark_util;

use serde_json::{json, Value};

use mark_util::{assert_eq_json, parse_unit_model_with_scale_and_layout};
use vega_lite::compile::mark::point::{circle_entry, point_entry, square_entry};
use vega_lite::mark::default_mark_config;
use vega_lite::util::internal_field;

fn point_xy(more: Value, config: Value) -> Value {
    let mut encoding = json!({
        "x": {"field": "year", "type": "ordinal"},
        "y": {"field": "yield", "type": "quantitative"}
    });
    if let (Some(enc), Some(extra)) = (encoding.as_object_mut(), more.as_object()) {
        for (k, v) in extra {
            enc.insert(k.clone(), v.clone());
        }
    }
    json!({
        "mark": "point",
        "encoding": encoding,
        "data": {"url": "data/barley.json"},
        "config": config
    })
}

#[test]
fn point_with_x_centered_on_y() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "point",
        "encoding": {"x": {"field": "year", "type": "ordinal"}},
        "data": {"url": "data/barley.json"}
    }));
    let props = point_entry(&model);
    assert_eq_json(json!({"mult": 0.5, "signal": "height"}), props["y"].clone());
    assert_eq_json(json!({"scale": "x", "field": "year"}), props["x"].clone());
}

#[test]
fn applies_bin_range_to_ordinal_bin() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "point",
        "encoding": {"y": {"bin": true, "field": "Miles_per_Gallon", "type": "ordinal"}}
    }));
    let props = point_entry(&model);
    assert_eq_json(
        json!({"scale": "y", "field": "bin_maxbins_10_Miles_per_Gallon_range"}),
        props["y"].clone(),
    );
}

#[test]
fn with_offset_includes_offset_on_y() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "point", "size": 250, "y": "height", "yOffset": 25},
        "data": {"url": "data/barley.json"}
    }));
    let props = point_entry(&model);
    assert_eq_json(
        json!({"field": {"group": "height"}, "offset": 25}),
        props["y"].clone(),
    );
}

#[test]
fn with_stacked_x_uses_stack_end() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "point",
        "encoding": {
            "x": {"aggregate": "sum", "field": "a", "type": "quantitative", "stack": "zero"},
            "color": {"field": "b", "type": "ordinal"}
        },
        "data": {"url": "data/barley.json"}
    }));
    let props = point_entry(&model);
    assert_eq_json(json!({"scale": "x", "field": "sum_a_end"}), props["x"].clone());
}

#[test]
fn interpolate_stacked_x_with_band_half() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "point",
        "encoding": {
            "x": {"field": "a", "type": "quantitative", "bandPosition": 0.5, "stack": "zero"},
            "color": {"field": "b", "type": "ordinal"}
        },
        "data": {"url": "data/barley.json"}
    }));
    let props = point_entry(&model);
    assert_eq_json(
        json!({"signal": "scale(\"x\", 0.5 * datum[\"a_start\"] + 0.5 * datum[\"a_end\"])"}),
        props["x"].clone(),
    );
}

#[test]
fn interpolates_binned_x_band_06() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "point",
        "encoding": {
            "x": {"bin": true, "field": "a", "type": "quantitative", "bandPosition": 0.6}
        },
        "data": {"url": "data/barley.json"}
    }));
    let props = point_entry(&model);
    assert_eq_json(
        json!({"signal": "scale(\"x\", 0.4 * datum[\"bin_maxbins_10_a\"] + 0.6 * datum[\"bin_maxbins_10_a_end\"])"}),
        props["x"].clone(),
    );
}

#[test]
fn interpolates_x_time_unit_band_half() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/barley.json"},
        "mark": "point",
        "encoding": {"x": {"timeUnit": "year", "field": "a"}},
        "config": {"point": {"timeUnitBandPosition": 0.5}}
    }));
    let props = point_entry(&model);
    assert_eq_json(
        json!({"signal": "scale(\"x\", 0.5 * datum[\"year_a\"] + 0.5 * datum[\"year_a_end\"])"}),
        props["x"].clone(),
    );
}

#[test]
fn interpolates_nominal_x_band_06() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "point",
        "encoding": {
            "x": {"field": "a", "type": "nominal", "bandPosition": 0.6, "scale": {"type": "band"}}
        },
        "data": {"url": "data/barley.json"}
    }));
    let props = point_entry(&model);
    assert_eq_json(json!({"scale": "x", "field": "a", "band": 0.6}), props["x"].clone());
}

#[test]
fn supports_encoding_with_expression() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "point",
        "encoding": {"x": {"value": {"expr": "a"}}},
        "data": {"url": "data/barley.json"}
    }));
    let props = point_entry(&model);
    assert_eq_json(json!({"signal": "a"}), props["x"].clone());
}

#[test]
fn point_with_y() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "point",
        "encoding": {"y": {"field": "year", "type": "ordinal"}},
        "data": {"url": "data/barley.json"}
    }));
    let props = point_entry(&model);
    assert_eq_json(json!({"mult": 0.5, "signal": "width"}), props["x"].clone());
    assert_eq_json(json!({"scale": "y", "field": "year"}), props["y"].clone());
}

#[test]
fn with_stacked_y_uses_stack_end() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "point",
        "encoding": {
            "y": {"aggregate": "sum", "field": "a", "type": "quantitative", "stack": "zero"},
            "color": {"field": "b", "type": "ordinal"}
        },
        "data": {"url": "data/barley.json"}
    }));
    let props = point_entry(&model);
    assert_eq_json(json!({"scale": "y", "field": "sum_a_end"}), props["y"].clone());
}

#[test]
fn point_with_x_and_y() {
    let model = parse_unit_model_with_scale_and_layout(&point_xy(json!({}), json!({})));
    let props = point_entry(&model);
    assert_eq_json(json!({"scale": "x", "field": "year"}), props["x"].clone());
    assert_eq_json(json!({"scale": "y", "field": "yield"}), props["y"].clone());
    assert_eq_json(json!({"value": "transparent"}), props["fill"].clone());
    assert_eq_json(
        json!({"value": default_mark_config()["color"]}),
        props["stroke"].clone(),
    );
}

#[test]
fn with_invalid_null() {
    let model = parse_unit_model_with_scale_and_layout(&point_xy(
        json!({}),
        json!({"mark": {"invalid": null}}),
    ));
    let props = point_entry(&model);
    assert_eq_json(
        json!([
            {"scale": "y", "value": 0, "test": "!isValid(datum[\"yield\"]) || !isFinite(+datum[\"yield\"])"},
            {"scale": "y", "field": "yield"}
        ]),
        props["y"].clone(),
    );
    assert_eq_json(json!({"value": "transparent"}), props["fill"].clone());
    assert_eq_json(
        json!({"value": default_mark_config()["color"]}),
        props["stroke"].clone(),
    );
}

#[test]
fn band_x_quantitative_y_offsets_band_half() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/barley.json"},
        "mark": "point",
        "encoding": {
            "x": {"field": "year", "type": "ordinal", "scale": {"type": "band"}},
            "y": {"field": "yield", "type": "quantitative"}
        }
    }));
    let props = point_entry(&model);
    assert_eq_json(json!({"scale": "x", "field": "year", "band": 0.5}), props["x"].clone());
}

#[test]
fn with_size() {
    let model = parse_unit_model_with_scale_and_layout(&point_xy(
        json!({"size": {"aggregate": "count", "type": "quantitative"}}),
        json!({}),
    ));
    let props = point_entry(&model);
    assert_eq_json(
        json!({"scale": "size", "field": internal_field("count")}),
        props["size"].clone(),
    );
}

#[test]
fn with_color() {
    let model = parse_unit_model_with_scale_and_layout(&point_xy(
        json!({"color": {"field": "yield", "type": "quantitative"}}),
        json!({}),
    ));
    let props = point_entry(&model);
    assert_eq_json(json!({"scale": "color", "field": "yield"}), props["stroke"].clone());
}

#[test]
fn with_invalid_color_specified() {
    let model = parse_unit_model_with_scale_and_layout(&point_xy(
        json!({"color": {"field": "yield", "type": "quantitative"}}),
        json!({"scale": {"invalid": {"color": {"value": "red"}}}}),
    ));
    let props = point_entry(&model);
    assert_eq_json(
        json!([
            {"test": "!isValid(datum[\"yield\"]) || !isFinite(+datum[\"yield\"])", "value": "red"},
            {"scale": "color", "field": "yield"}
        ]),
        props["stroke"].clone(),
    );
}

#[test]
fn condition_only_color() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "point",
        "encoding": {
            "x": {"field": "year", "type": "ordinal"},
            "y": {"field": "yield", "type": "quantitative"},
            "color": {"condition": {"param": "test", "field": "yield", "type": "quantitative"}}
        },
        "params": [{"name": "test", "select": "point"}],
        "data": {"url": "data/barley.json"}
    }));
    let props = point_entry(&model);
    let stroke = props["stroke"].as_array().expect("stroke should be conditional array");
    assert_eq!(stroke.len(), 2);
    assert_eq_json(json!("color"), stroke[0]["scale"].clone());
    assert_eq_json(json!("yield"), stroke[0]["field"].clone());
}

#[test]
fn condition_only_color_explicit_test() {
    let model = parse_unit_model_with_scale_and_layout(&point_xy(
        json!({"color": {"condition": {"test": "true", "field": "yield", "type": "quantitative"}}}),
        json!({}),
    ));
    let props = point_entry(&model);
    let stroke = props["stroke"].as_array().expect("stroke should be conditional array");
    assert_eq!(stroke.len(), 2);
    assert_eq_json(json!("true"), stroke[0]["test"].clone());
    assert_eq_json(json!("#4c78a8"), stroke[1]["value"].clone());
}

#[test]
fn with_shape() {
    let model = parse_unit_model_with_scale_and_layout(&point_xy(
        json!({"shape": {"field": "site", "type": "nominal"}}),
        json!({}),
    ));
    let props = point_entry(&model);
    assert_eq_json(json!({"scale": "shape", "field": "site"}), props["shape"].clone());
}

#[test]
fn with_constant_color_shape_size() {
    let model = parse_unit_model_with_scale_and_layout(&point_xy(
        json!({"shape": {"value": "circle"}, "color": {"value": "red"}, "size": {"value": 23}}),
        json!({}),
    ));
    let props = point_entry(&model);
    assert_eq_json(json!({"value": "circle"}), props["shape"].clone());
    assert_eq_json(json!({"value": "red"}), props["stroke"].clone());
    assert_eq_json(json!({"value": 23}), props["size"].clone());
}

#[test]
fn config_mark_size_is_ignored() {
    let model = parse_unit_model_with_scale_and_layout(&point_xy(
        json!({}),
        json!({"mark": {"size": 23}}),
    ));
    let props = point_entry(&model);
    assert!(props.get("size").is_none());
}

#[test]
fn config_point_size_is_ignored() {
    let model = parse_unit_model_with_scale_and_layout(&point_xy(
        json!({}),
        json!({"point": {"size": 23}}),
    ));
    let props = point_entry(&model);
    assert!(props.get("size").is_none());
}

#[test]
fn with_href() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "point",
        "encoding": {"href": {"value": "https://idl.cs.washington.edu/"}}
    }));
    let props = point_entry(&model);
    assert_eq_json(
        json!({"value": "https://idl.cs.washington.edu/"}),
        props["href"].clone(),
    );
}

#[test]
fn square_has_correct_shape() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "square",
        "encoding": {"color": {"value": "blue"}}
    }));
    let props = square_entry(&model);
    assert_eq_json(json!("square"), props["shape"]["value"].clone());
}

#[test]
fn square_is_filled_by_default() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "square",
        "encoding": {"color": {"value": "blue"}}
    }));
    let props = square_entry(&model);
    assert_eq_json(json!("blue"), props["fill"]["value"].clone());
}

#[test]
fn square_config_mark_filled_false_has_transparent_fill() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "square",
        "encoding": {"color": {"value": "blue"}},
        "config": {"mark": {"filled": false}}
    }));
    let props = square_entry(&model);
    assert_eq_json(json!("blue"), props["stroke"]["value"].clone());
    assert_eq_json(json!("transparent"), props["fill"]["value"].clone());
}

#[test]
fn circle_has_correct_shape() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "circle",
        "encoding": {"color": {"value": "blue"}}
    }));
    let props = circle_entry(&model);
    assert_eq_json(json!("circle"), props["shape"]["value"].clone());
}

#[test]
fn circle_is_filled_by_default() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "circle",
        "encoding": {"color": {"value": "blue"}}
    }));
    let props = circle_entry(&model);
    assert_eq_json(json!("blue"), props["fill"]["value"].clone());
}

#[test]
fn circle_config_mark_filled_false_has_transparent_fill() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "circle",
        "encoding": {"color": {"value": "blue"}},
        "config": {"mark": {"filled": false}}
    }));
    let props = circle_entry(&model);
    assert_eq_json(json!("blue"), props["stroke"]["value"].clone());
    assert_eq_json(json!("transparent"), props["fill"]["value"].clone());
}

#[test]
fn circle_converts_expression_in_mark_properties_to_signal() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "circle", "stroke": {"expr": "'red'"}},
        "config": {"mark": {"filled": false}}
    }));
    let props = circle_entry(&model);
    assert_eq_json(json!({"signal": "'red'"}), props["stroke"].clone());
}

#[test]
fn circle_converts_expression_in_encoding_into_signal() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "circle"},
        "encoding": {"x": {"datum": {"expr": "myX"}, "type": "quantitative"}}
    }));
    let props = circle_entry(&model);
    assert_eq_json(json!({"scale": "x", "signal": "myX"}), props["x"].clone());
}

#[test]
fn point_with_x_scales_on_x() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "point",
        "encoding": {"x": {"field": "year", "type": "ordinal"}},
        "data": {"url": "data/barley.json"}
    }));
    assert_eq_json(json!({"scale": "x", "field": "year"}), point_entry(&model)["x"].clone());
}

#[test]
fn point_with_y_scales_on_y() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "point",
        "encoding": {"y": {"field": "year", "type": "ordinal"}},
        "data": {"url": "data/barley.json"}
    }));
    assert_eq_json(json!({"scale": "y", "field": "year"}), point_entry(&model)["y"].clone());
}

#[test]
fn point_with_x_and_y_scales_on_x() {
    let model = parse_unit_model_with_scale_and_layout(&point_xy(json!({}), json!({})));
    assert_eq_json(json!({"scale": "x", "field": "year"}), point_entry(&model)["x"].clone());
}

#[test]
fn point_with_x_and_y_scales_on_y() {
    let model = parse_unit_model_with_scale_and_layout(&point_xy(json!({}), json!({})));
    assert_eq_json(json!({"scale": "y", "field": "yield"}), point_entry(&model)["y"].clone());
}

#[test]
fn point_with_x_and_y_is_unfilled() {
    let model = parse_unit_model_with_scale_and_layout(&point_xy(json!({}), json!({})));
    let props = point_entry(&model);
    assert_eq_json(json!({"value": "transparent"}), props["fill"].clone());
    assert_eq_json(
        json!({"value": default_mark_config()["color"]}),
        props["stroke"].clone(),
    );
}

#[test]
fn with_invalid_null_unfilled() {
    let model = parse_unit_model_with_scale_and_layout(&point_xy(
        json!({}),
        json!({"mark": {"invalid": null}}),
    ));
    let props = point_entry(&model);
    assert_eq_json(json!({"value": "transparent"}), props["fill"].clone());
    assert_eq_json(
        json!({"value": default_mark_config()["color"]}),
        props["stroke"].clone(),
    );
}

//! Port of `test/compile/mark/area.test.ts`.

#[path = "mark_util.rs"]
mod mark_util;

use serde_json::{json, Value};

use mark_util::{assert_eq_json, parse_unit_model_with_scale_and_layout};
use vega_lite::compile::mark::area::encode_entry as area;
use vega_lite::util::internal_field;

fn vertical_area(more: Value) -> Value {
    let mut encoding = json!({
        "x": {"timeUnit": "year", "field": "Year", "type": "temporal"},
        "y": {"aggregate": "count", "type": "quantitative"}
    });
    if let (Some(enc), Some(extra)) = (encoding.as_object_mut(), more.as_object()) {
        for (k, v) in extra {
            enc.insert(k.clone(), v.clone());
        }
    }
    json!({"mark": "area", "encoding": encoding, "data": {"url": "data/cars.json"}})
}

fn horizontal_area(more: Value) -> Value {
    let mut encoding = json!({
        "y": {"timeUnit": "year", "field": "Year", "type": "temporal"},
        "x": {"aggregate": "count", "type": "quantitative"}
    });
    if let (Some(enc), Some(extra)) = (encoding.as_object_mut(), more.as_object()) {
        for (k, v) in extra {
            enc.insert(k.clone(), v.clone());
        }
    }
    json!({"mark": "area", "encoding": encoding, "data": {"url": "data/cars.json"}})
}

#[test]
fn vertical_area_with_log() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "area",
        "encoding": {
            "x": {"bin": true, "type": "quantitative", "field": "IMDB_Rating"},
            "y": {"scale": {"type": "log"}, "type": "quantitative", "field": "US_Gross", "aggregate": "mean"}
        },
        "data": {"url": "data/movies.json"}
    }));
    let props = area(&model);
    assert_eq_json(json!({"signal": "scale('y', domain('y')[0])"}), props["y2"].clone());
    assert!(props.get("height").is_none());
}

#[test]
fn stacked_vertical_area_with_binned_dimension() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "area",
        "encoding": {
            "x": {"bin": true, "type": "quantitative", "field": "IMDB_Rating"},
            "y": {"type": "quantitative", "field": "US_Gross", "aggregate": "sum"},
            "color": {"type": "nominal", "field": "c"}
        },
        "data": {"url": "data/movies.json"}
    }));
    let props = area(&model);
    assert_eq_json(
        json!({"field": "bin_maxbins_10_IMDB_Rating_mid", "scale": "x"}),
        props["x"].clone(),
    );
    let defined = props["defined"]["signal"].as_str().unwrap();
    assert!(defined.contains("bin_maxbins_10_IMDB_Rating_mid"), "{defined}");
}

#[test]
fn vertical_area_with_zero_false() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "area",
        "encoding": {
            "x": {"bin": true, "type": "quantitative", "field": "IMDB_Rating"},
            "y": {"scale": {"zero": false}, "type": "quantitative", "field": "US_Gross", "aggregate": "mean"}
        },
        "data": {"url": "data/movies.json"}
    }));
    let props = area(&model);
    assert_eq_json(
        json!({"signal": "scale('y', inrange(0, domain('y')) ? 0 : domain('y')[0])"}),
        props["y2"].clone(),
    );
    assert!(props.get("height").is_none());
}

#[test]
fn vertical_area_default() {
    let model = parse_unit_model_with_scale_and_layout(&vertical_area(json!({})));
    let props = area(&model);
    assert_eq_json(json!({"scale": "x", "field": "year_Year"}), props["x"].clone());
    assert_eq_json(
        json!({"scale": "y", "field": internal_field("count")}),
        props["y"].clone(),
    );
    assert_eq_json(json!({"scale": "y", "value": 0}), props["y2"].clone());
}

#[test]
fn vertical_area_with_binned_dimension() {
    let model = parse_unit_model_with_scale_and_layout(&vertical_area(json!({})));
    let props = area(&model);
    assert_eq_json(json!({"scale": "x", "field": "year_Year"}), props["x"].clone());
    assert_eq_json(
        json!({"scale": "y", "field": internal_field("count")}),
        props["y"].clone(),
    );
    assert_eq_json(json!({"scale": "y", "value": 0}), props["y2"].clone());
}

#[test]
fn vertical_area_with_discrete_y_and_continuous_y_offset() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "area",
        "encoding": {
            "x": {"timeUnit": "year", "field": "Year", "type": "temporal"},
            "y": {"field": "Origin", "type": "nominal"},
            "yOffset": {"field": "US_Gross", "type": "quantitative"}
        },
        "data": {"url": "data/movies.json"}
    }));
    let props = area(&model);
    assert_eq_json(
        json!({"scale": "y", "field": "Origin", "offset": {"scale": "yOffset", "field": "US_Gross"}}),
        props["y"].clone(),
    );
    assert_eq_json(
        json!({"scale": "y", "field": "Origin", "offset": {"scale": "yOffset", "value": 0}}),
        props["y2"].clone(),
    );
    assert!(props.get("x2").is_none());
    assert!(props.get("y2").is_some());
}

#[test]
fn vertical_area_with_only_y_offset() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "area",
        "encoding": {
            "x": {"field": "a", "type": "nominal"},
            "yOffset": {"field": "b", "type": "quantitative", "aggregate": "sum"}
        },
        "data": {"values": [{"a": "A", "b": 28}, {"a": "B", "b": 55}, {"a": "C", "b": 43}]}
    }));
    let props = area(&model);
    assert_eq_json(
        json!({"value": 0, "offset": {"scale": "yOffset", "field": "sum_b"}}),
        props["y"].clone(),
    );
    assert_eq_json(json!({"field": {"group": "height"}}), props["y2"].clone());
}

#[test]
fn vertical_stacked_area_with_color() {
    let model = parse_unit_model_with_scale_and_layout(&vertical_area(
        json!({"color": {"field": "Origin", "type": "quantitative"}}),
    ));
    let props = area(&model);
    assert_eq_json(
        json!({"scale": "y", "field": internal_field("count_end")}),
        props["y"].clone(),
    );
    assert_eq_json(
        json!({"scale": "y", "field": internal_field("count_start")}),
        props["y2"].clone(),
    );
    assert_eq_json(json!({"value": "vertical"}), props["orient"].clone());
    assert_eq_json(json!({"scale": "color", "field": "Origin"}), props["fill"].clone());
}

#[test]
fn horizontal_area_default() {
    let model = parse_unit_model_with_scale_and_layout(&horizontal_area(json!({})));
    let props = area(&model);
    assert_eq_json(json!({"scale": "y", "field": "year_Year"}), props["y"].clone());
    assert_eq_json(
        json!({"scale": "x", "field": internal_field("count")}),
        props["x"].clone(),
    );
    assert_eq_json(json!({"scale": "x", "value": 0}), props["x2"].clone());
}

#[test]
fn horizontal_area_with_log() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "area",
        "encoding": {
            "y": {"bin": true, "type": "quantitative", "field": "IMDB_Rating"},
            "x": {"scale": {"type": "log"}, "type": "quantitative", "field": "US_Gross", "aggregate": "mean"}
        },
        "data": {"url": "data/movies.json"}
    }));
    let props = area(&model);
    assert_eq_json(json!({"signal": "scale('x', domain('x')[0])"}), props["x2"].clone());
    assert!(props.get("width").is_none());
}

#[test]
fn horizontal_area_with_zero_false() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "area",
        "encoding": {
            "y": {"bin": true, "type": "quantitative", "field": "IMDB_Rating"},
            "x": {"scale": {"zero": false}, "type": "quantitative", "field": "US_Gross", "aggregate": "mean"}
        },
        "data": {"url": "data/movies.json"}
    }));
    let props = area(&model);
    assert_eq_json(
        json!({"signal": "scale('x', inrange(0, domain('x')) ? 0 : domain('x')[0])"}),
        props["x2"].clone(),
    );
    assert!(props.get("width").is_none());
}

#[test]
fn horizontal_stacked_area_with_color() {
    let model = parse_unit_model_with_scale_and_layout(&horizontal_area(
        json!({"color": {"field": "Origin", "type": "nominal"}}),
    ));
    let props = area(&model);
    assert_eq_json(
        json!({"scale": "x", "field": internal_field("count_end")}),
        props["x"].clone(),
    );
    assert_eq_json(
        json!({"scale": "x", "field": internal_field("count_start")}),
        props["x2"].clone(),
    );
    assert_eq_json(json!({"value": "horizontal"}), props["orient"].clone());
    assert_eq_json(json!({"scale": "color", "field": "Origin"}), props["fill"].clone());
}

#[test]
fn ranged_vertical_area_with_aggregate() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "area",
        "encoding": {
            "x": {"timeUnit": "year", "field": "Year", "type": "temporal"},
            "y": {"aggregate": "min", "field": "Weight_in_lbs", "type": "quantitative"},
            "y2": {"aggregate": "max", "field": "Weight_in_lbs"}
        }
    }));
    let props = area(&model);
    assert_eq_json(json!({"scale": "x", "field": "year_Year"}), props["x"].clone());
    assert_eq_json(json!({"scale": "y", "field": "min_Weight_in_lbs"}), props["y"].clone());
    assert_eq_json(json!({"scale": "y", "field": "max_Weight_in_lbs"}), props["y2"].clone());
}

#[test]
fn ranged_horizontal_area_with_aggregate() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "area",
        "encoding": {
            "y": {"timeUnit": "year", "field": "Year", "type": "temporal"},
            "x": {"aggregate": "min", "field": "Weight_in_lbs", "type": "quantitative"},
            "x2": {"aggregate": "max", "field": "Weight_in_lbs"}
        }
    }));
    let props = area(&model);
    assert_eq_json(json!({"scale": "y", "field": "year_Year"}), props["y"].clone());
    assert_eq_json(json!({"scale": "x", "field": "min_Weight_in_lbs"}), props["x"].clone());
    assert_eq_json(json!({"scale": "x", "field": "max_Weight_in_lbs"}), props["x2"].clone());
}

#[test]
fn vertical_area_with_log_has_no_height() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "area",
        "encoding": {
            "x": {"bin": true, "type": "quantitative", "field": "IMDB_Rating"},
            "y": {"scale": {"type": "log"}, "type": "quantitative", "field": "US_Gross", "aggregate": "mean"}
        },
        "data": {"url": "data/movies.json"}
    }));
    assert!(area(&model).get("height").is_none());
}

#[test]
fn stacked_vertical_area_with_binned_dimension_defined() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "area",
        "encoding": {
            "x": {"bin": true, "type": "quantitative", "field": "IMDB_Rating"},
            "y": {"type": "quantitative", "field": "US_Gross", "aggregate": "sum"},
            "color": {"type": "nominal", "field": "c"}
        },
        "data": {"url": "data/movies.json"}
    }));
    let defined = area(&model)["defined"]["signal"].as_str().unwrap().to_string();
    assert!(defined.contains("bin_maxbins_10_IMDB_Rating_mid"));
}

#[test]
fn vertical_area_with_zero_false_has_no_height() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "area",
        "encoding": {
            "x": {"bin": true, "type": "quantitative", "field": "IMDB_Rating"},
            "y": {"scale": {"zero": false}, "type": "quantitative", "field": "US_Gross", "aggregate": "mean"}
        },
        "data": {"url": "data/movies.json"}
    }));
    assert!(area(&model).get("height").is_none());
}

#[test]
fn vertical_area_scale_y() {
    let model = parse_unit_model_with_scale_and_layout(&vertical_area(json!({})));
    assert_eq_json(
        json!({"scale": "y", "field": internal_field("count")}),
        area(&model)["y"].clone(),
    );
}

#[test]
fn vertical_area_y2_zero() {
    let model = parse_unit_model_with_scale_and_layout(&vertical_area(json!({})));
    assert_eq_json(json!({"scale": "y", "value": 0}), area(&model)["y2"].clone());
}

#[test]
fn vertical_area_binned_dimension_scale_x() {
    let model = parse_unit_model_with_scale_and_layout(&vertical_area(json!({})));
    assert_eq_json(json!({"scale": "x", "field": "year_Year"}), area(&model)["x"].clone());
}

#[test]
fn vertical_area_binned_dimension_scale_y() {
    let model = parse_unit_model_with_scale_and_layout(&vertical_area(json!({})));
    assert_eq_json(
        json!({"scale": "y", "field": internal_field("count")}),
        area(&model)["y"].clone(),
    );
}

#[test]
fn vertical_area_binned_dimension_y2_zero() {
    let model = parse_unit_model_with_scale_and_layout(&vertical_area(json!({})));
    assert_eq_json(json!({"scale": "y", "value": 0}), area(&model)["y2"].clone());
}

#[test]
fn vertical_area_y_offset_no_collapse() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "area",
        "encoding": {
            "x": {"timeUnit": "year", "field": "Year", "type": "temporal"},
            "y": {"field": "Origin", "type": "nominal"},
            "yOffset": {"field": "US_Gross", "type": "quantitative"}
        },
        "data": {"url": "data/movies.json"}
    }));
    let props = area(&model);
    assert!(props.get("x2").is_none());
    assert!(props.get("y2").is_some());
}

#[test]
fn vertical_stacked_area_orient() {
    let model = parse_unit_model_with_scale_and_layout(&vertical_area(
        json!({"color": {"field": "Origin", "type": "quantitative"}}),
    ));
    assert_eq_json(json!({"value": "vertical"}), area(&model)["orient"].clone());
}

#[test]
fn vertical_stacked_area_fill() {
    let model = parse_unit_model_with_scale_and_layout(&vertical_area(
        json!({"color": {"field": "Origin", "type": "quantitative"}}),
    ));
    assert_eq_json(json!({"scale": "color", "field": "Origin"}), area(&model)["fill"].clone());
}

#[test]
fn horizontal_area_scale_y() {
    let model = parse_unit_model_with_scale_and_layout(&horizontal_area(json!({})));
    assert_eq_json(json!({"scale": "y", "field": "year_Year"}), area(&model)["y"].clone());
}

#[test]
fn horizontal_area_scale_x() {
    let model = parse_unit_model_with_scale_and_layout(&horizontal_area(json!({})));
    assert_eq_json(
        json!({"scale": "x", "field": internal_field("count")}),
        area(&model)["x"].clone(),
    );
}

#[test]
fn horizontal_area_x2_zero() {
    let model = parse_unit_model_with_scale_and_layout(&horizontal_area(json!({})));
    assert_eq_json(json!({"scale": "x", "value": 0}), area(&model)["x2"].clone());
}

#[test]
fn horizontal_area_with_log_has_no_width() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "area",
        "encoding": {
            "y": {"bin": true, "type": "quantitative", "field": "IMDB_Rating"},
            "x": {"scale": {"type": "log"}, "type": "quantitative", "field": "US_Gross", "aggregate": "mean"}
        },
        "data": {"url": "data/movies.json"}
    }));
    assert!(area(&model).get("width").is_none());
}

#[test]
fn horizontal_area_with_zero_false_has_no_width() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "area",
        "encoding": {
            "y": {"bin": true, "type": "quantitative", "field": "IMDB_Rating"},
            "x": {"scale": {"zero": false}, "type": "quantitative", "field": "US_Gross", "aggregate": "mean"}
        },
        "data": {"url": "data/movies.json"}
    }));
    assert!(area(&model).get("width").is_none());
}

#[test]
fn horizontal_stacked_area_orient() {
    let model = parse_unit_model_with_scale_and_layout(&horizontal_area(
        json!({"color": {"field": "Origin", "type": "nominal"}}),
    ));
    assert_eq_json(json!({"value": "horizontal"}), area(&model)["orient"].clone());
}

#[test]
fn horizontal_stacked_area_fill() {
    let model = parse_unit_model_with_scale_and_layout(&horizontal_area(
        json!({"color": {"field": "Origin", "type": "nominal"}}),
    ));
    assert_eq_json(json!({"scale": "color", "field": "Origin"}), area(&model)["fill"].clone());
}

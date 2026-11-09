//! Port of `test/compile/mark/arc.test.ts`, `line.test.ts`, `geoshape.test.ts`
//! and `image.test.ts`.

#[path = "mark_util.rs"]
mod mark_util;

use serde_json::json;

use mark_util::{assert_eq_json, parse_unit_model_with_scale_and_layout};
use vega_lite::compile::mark::arc::encode_entry as arc;
use vega_lite::compile::mark::geoshape::encode_entry as geoshape;
use vega_lite::compile::mark::image::encode_entry as image;
use vega_lite::compile::mark::line::{encode_entry as line, trail_entry};

#[test]
// These are deliberately short decimal literals (not π or π/2) passed through
// verbatim to assert value passthrough.
#[allow(clippy::approx_constant)]
fn arc_theta_in_mark_def() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "arc", "theta": 1.57, "theta2": 3.14}
    }));
    let props = arc(&model);
    assert_eq_json(json!({"value": 1.57}), props["startAngle"].clone());
    assert_eq_json(json!({"value": 3.14}), props["endAngle"].clone());
    assert!(props.get("theta").is_none());
}

#[test]
fn arc_for_pie() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "arc",
        "encoding": {
            "theta": {"field": "field", "type": "quantitative"},
            "color": {"field": "id", "type": "nominal"}
        }
    }));
    let props = arc(&model);
    assert_eq_json(json!({"scale": "theta", "field": "field_end"}), props["startAngle"].clone());
    assert_eq_json(json!({"scale": "theta", "field": "field_start"}), props["endAngle"].clone());
}

#[test]
fn arc_for_pie_with_theta_offset() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "arc", "thetaOffset": 5},
        "encoding": {
            "theta": {"field": "field", "type": "quantitative"},
            "color": {"field": "id", "type": "nominal"}
        }
    }));
    let props = arc(&model);
    assert_eq_json(
        json!({"scale": "theta", "field": "field_end", "offset": 5}),
        props["startAngle"].clone(),
    );
    assert_eq_json(
        json!({"scale": "theta", "field": "field_start", "offset": 5}),
        props["endAngle"].clone(),
    );
}

#[test]
fn arc_for_radial_histogram() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "arc",
        "encoding": {
            "theta": {"bin": true, "field": "field", "type": "quantitative"},
            "radius": {"aggregate": "count", "type": "quantitative"}
        }
    }));
    let props = arc(&model);
    assert_eq_json(
        json!({"scale": "theta", "field": "bin_maxbins_10_field"}),
        props["endAngle"].clone(),
    );
    assert_eq_json(
        json!({"scale": "theta", "field": "bin_maxbins_10_field_end"}),
        props["startAngle"].clone(),
    );
}

#[test]
fn line_with_x_y() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/barley.json"},
        "mark": "line",
        "encoding": {
            "x": {"field": "year", "type": "ordinal"},
            "y": {"field": "yield", "type": "quantitative"}
        }
    }));
    let props = line(&model);
    assert_eq_json(json!({"scale": "x", "field": "year"}), props["x"].clone());
    assert_eq_json(json!({"scale": "y", "field": "yield"}), props["y"].clone());
}

#[test]
fn line_with_color() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/barley.json"},
        "mark": "line",
        "encoding": {
            "x": {"field": "year", "type": "ordinal"},
            "y": {"field": "yield", "type": "quantitative"},
            "color": {"field": "Acceleration", "type": "quantitative"}
        }
    }));
    assert_eq_json(
        json!({"scale": "color", "field": "Acceleration"}),
        line(&model)["stroke"].clone(),
    );
}

#[test]
fn line_with_size() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/barley.json"},
        "mark": "line",
        "encoding": {
            "x": {"field": "year", "type": "ordinal"},
            "y": {"field": "yield", "type": "quantitative", "aggregate": "mean"},
            "size": {"field": "variety", "type": "quantitative"}
        }
    }));
    assert_eq_json(
        json!({"scale": "size", "field": "variety"}),
        line(&model)["strokeWidth"].clone(),
    );
}

#[test]
fn line_drops_aggregate_size_field() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/barley.json"},
        "mark": "line",
        "encoding": {
            "x": {"field": "year", "type": "ordinal"},
            "y": {"field": "yield", "type": "quantitative", "aggregate": "mean"},
            "size": {"field": "Acceleration", "type": "quantitative", "aggregate": "mean"}
        }
    }));
    let (props, _logger) = vega_lite::log::capture(|| line(&model));
    assert!(props["strokeWidth"].get("scale").is_none());
}

#[test]
fn line_with_stacked_y() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/barley.json"},
        "mark": "line",
        "encoding": {
            "x": {"field": "year", "type": "ordinal"},
            "y": {"field": "yield", "type": "quantitative", "aggregate": "sum", "stack": "zero"},
            "color": {"field": "a", "type": "nominal"}
        }
    }));
    assert_eq_json(
        json!({"scale": "y", "field": "sum_yield_end"}),
        line(&model)["y"].clone(),
    );
}

#[test]
fn line_with_stacked_x() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/barley.json"},
        "mark": "line",
        "encoding": {
            "y": {"field": "year", "type": "ordinal"},
            "x": {"field": "yield", "type": "quantitative", "aggregate": "sum", "stack": "zero"},
            "color": {"field": "a", "type": "nominal"}
        }
    }));
    assert_eq_json(
        json!({"scale": "x", "field": "sum_yield_end"}),
        line(&model)["x"].clone(),
    );
}

#[test]
fn line_with_x() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "line",
        "encoding": {"x": {"field": "year", "type": "ordinal"}},
        "data": {"url": "data/barley.json"}
    }));
    let props = line(&model);
    assert_eq_json(json!({"mult": 0.5, "signal": "height"}), props["y"].clone());
    assert_eq_json(json!({"scale": "x", "field": "year"}), props["x"].clone());
}

#[test]
fn line_with_y() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "line",
        "encoding": {"y": {"field": "year", "type": "ordinal"}},
        "data": {"url": "data/barley.json"}
    }));
    let props = line(&model);
    assert_eq_json(json!({"mult": 0.5, "signal": "width"}), props["x"].clone());
    assert_eq_json(json!({"scale": "y", "field": "year"}), props["y"].clone());
}

#[test]
fn trail_mark_encodes_size() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "trail",
        "encoding": {
            "x": {"field": "year", "type": "ordinal"},
            "y": {"field": "yield", "type": "quantitative"},
            "size": {"field": "yield", "type": "quantitative"}
        }
    }));
    assert_eq_json(
        json!({"scale": "size", "field": "yield"}),
        trail_entry(&model)["size"].clone(),
    );
}

#[test]
fn geoshape_creates_no_properties() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "geoshape",
        "projection": {"type": "albersUsa"},
        "data": {"url": "data/us-10m.json", "format": {"type": "topojson", "feature": "states"}},
        "encoding": {"color": {"value": "black"}, "opacity": {"value": 0.8}},
        "config": {"mark": {"tooltip": null}}
    }));
    let props = geoshape(&model);
    assert_eq_json(
        json!({
            "ariaRoleDescription": {"value": "geoshape"},
            "fill": {"value": "black"},
            "opacity": {"value": 0.8}
        }),
        props,
    );
}

#[test]
fn image_without_explicit_size() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"values": [{"t": "a", "s": 1, "img": "data/ffox.png"}]},
        "mark": "image",
        "encoding": {
            "x": {"field": "t", "type": "nominal"},
            "y": {"field": "s", "type": "quantitative"},
            "url": {"field": "img", "type": "nominal"}
        }
    }));
    let props = image(&model);
    assert_eq_json(json!({"scale": "x", "field": "t", "band": 0.5}), props["x"].clone());
    assert_eq_json(json!({"value": "center"}), props["align"].clone());
    assert_eq_json(json!({"scale": "y", "field": "s"}), props["y"].clone());
    assert_eq_json(json!({"value": "middle"}), props["baseline"].clone());
    assert!(props.get("width").is_none());
    assert!(props.get("height").is_none());
    assert!(props.get("x2").is_none());
    assert!(props.get("y2").is_none());
}

#[test]
fn image_without_size_with_align_baseline() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"values": [{"a": 1, "b": 2, "img": "data/ffox.png"}]},
        "mark": {"type": "image", "align": "left", "baseline": "top"},
        "encoding": {
            "x": {"field": "a", "type": "quantitative"},
            "y": {"field": "b", "type": "quantitative"},
            "url": {"field": "img", "type": "nominal"}
        }
    }));
    let props = image(&model);
    assert_eq_json(json!({"scale": "x", "field": "a"}), props["x"].clone());
    assert_eq_json(json!({"value": "left"}), props["align"].clone());
    assert_eq_json(json!({"scale": "y", "field": "b"}), props["y"].clone());
    assert_eq_json(json!({"value": "top"}), props["baseline"].clone());
    assert!(props.get("width").is_none());
    assert!(props.get("height").is_none());
}

#[test]
fn image_with_x2_y2() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"values": [{"a": 1, "a2": 2, "b": 1, "b2": 2, "img": "data/ffox.png"}]},
        "mark": "image",
        "encoding": {
            "x": {"field": "a", "type": "quantitative"},
            "x2": {"field": "a2"},
            "y": {"field": "b", "type": "quantitative"},
            "y2": {"field": "b2"},
            "url": {"field": "img", "type": "nominal"}
        }
    }));
    let props = image(&model);
    assert_eq_json(json!({"scale": "x", "field": "a"}), props["x"].clone());
    assert_eq_json(json!({"scale": "x", "field": "a2"}), props["x2"].clone());
    assert_eq_json(json!({"scale": "y", "field": "b"}), props["y"].clone());
    assert_eq_json(json!({"scale": "y", "field": "b2"}), props["y2"].clone());
    assert!(props.get("align").is_none());
    assert!(props.get("baseline").is_none());
}

#[test]
fn image_with_explicit_width_height() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"values": [{"a": 1, "b": 2, "img": "data/ffox.png"}]},
        "mark": {"type": "image", "width": 50, "height": 49},
        "encoding": {
            "x": {"field": "a", "type": "quantitative"},
            "y": {"field": "b", "type": "quantitative"},
            "url": {"field": "img", "type": "nominal"}
        }
    }));
    let props = image(&model);
    assert_eq_json(json!({"scale": "x", "field": "a"}), props["xc"].clone());
    assert_eq_json(json!({"value": 50}), props["width"].clone());
    assert_eq_json(json!({"scale": "y", "field": "b"}), props["yc"].clone());
    assert_eq_json(json!({"value": 49}), props["height"].clone());
    assert!(props.get("align").is_none());
    assert!(props.get("baseline").is_none());
}

#[test]
fn image_with_position_value_no_size() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"values": [{"img": "data/ffox.png"}]},
        "mark": "image",
        "encoding": {
            "y": {"value": 100},
            "url": {"field": "img", "type": "nominal"}
        }
    }));
    let props = image(&model);
    assert_eq_json(json!({"value": 100}), props["y"].clone());
    assert_eq_json(json!({"value": "middle"}), props["baseline"].clone());
    assert!(props.get("height").is_none());
}

#[test]
fn line_with_x_y_scale_x() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/barley.json"},
        "mark": "line",
        "encoding": {
            "x": {"field": "year", "type": "ordinal"},
            "y": {"field": "yield", "type": "quantitative"}
        }
    }));
    assert_eq_json(json!({"scale": "x", "field": "year"}), line(&model)["x"].clone());
}

#[test]
fn line_with_x_y_scale_y() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/barley.json"},
        "mark": "line",
        "encoding": {
            "x": {"field": "year", "type": "ordinal"},
            "y": {"field": "yield", "type": "quantitative"}
        }
    }));
    assert_eq_json(json!({"scale": "y", "field": "yield"}), line(&model)["y"].clone());
}

#[test]
fn line_with_x_scales_on_x() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "line",
        "encoding": {"x": {"field": "year", "type": "ordinal"}},
        "data": {"url": "data/barley.json"}
    }));
    assert_eq_json(json!({"scale": "x", "field": "year"}), line(&model)["x"].clone());
}

#[test]
fn line_with_x_centered_on_y() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "line",
        "encoding": {"x": {"field": "year", "type": "ordinal"}},
        "data": {"url": "data/barley.json"}
    }));
    assert_eq_json(json!({"mult": 0.5, "signal": "height"}), line(&model)["y"].clone());
}

#[test]
fn line_with_y_scales_on_y() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "line",
        "encoding": {"y": {"field": "year", "type": "ordinal"}},
        "data": {"url": "data/barley.json"}
    }));
    assert_eq_json(json!({"scale": "y", "field": "year"}), line(&model)["y"].clone());
}

#[test]
fn line_with_y_centered_on_x() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "line",
        "encoding": {"y": {"field": "year", "type": "ordinal"}},
        "data": {"url": "data/barley.json"}
    }));
    assert_eq_json(json!({"mult": 0.5, "signal": "width"}), line(&model)["x"].clone());
}

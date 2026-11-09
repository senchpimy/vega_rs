//! Port of `test/compile/mark/tick.test.ts`.

#[path = "mark_util.rs"]
mod mark_util;

use serde_json::json;

use mark_util::{assert_eq_json, parse_unit_model_with_scale_and_layout};
use vega_lite::compile::mark::tick::encode_entry as tick;

#[test]
fn stacked_x_uses_stack_end() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "tick",
        "encoding": {
            "x": {"aggregate": "sum", "field": "a", "type": "quantitative", "stack": "zero"},
            "color": {"field": "b", "type": "ordinal"}
        },
        "data": {"url": "data/barley.json"}
    }));
    assert_eq_json(json!({"scale": "x", "field": "sum_a_end"}), tick(&model)["xc"].clone());
}

#[test]
fn stacked_y_uses_stack_end() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "tick",
        "encoding": {
            "y": {"aggregate": "sum", "field": "a", "type": "quantitative", "stack": "zero"},
            "color": {"field": "b", "type": "ordinal"}
        },
        "data": {"url": "data/barley.json"}
    }));
    assert_eq_json(json!({"scale": "y", "field": "sum_a_end"}), tick(&model)["yc"].clone());
}

#[test]
fn quantitative_x_centered_on_y() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "tick",
        "encoding": {"x": {"field": "Horsepower", "type": "quantitative"}},
        "data": {"url": "data/cars.json"}
    }));
    let props = tick(&model);
    assert_eq_json(json!({"mult": 0.5, "signal": "height"}), props["yc"].clone());
    assert_eq_json(json!({"scale": "x", "field": "Horsepower"}), props["xc"].clone());
    assert_eq_json(json!({"value": 1}), props["width"].clone());
}

#[test]
fn quantitative_y_centered_on_x() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "tick",
        "encoding": {"y": {"field": "Cylinders", "type": "quantitative"}},
        "data": {"url": "data/cars.json"}
    }));
    let props = tick(&model);
    assert_eq_json(json!({"mult": 0.5, "signal": "width"}), props["xc"].clone());
    assert_eq_json(json!({"scale": "y", "field": "Cylinders"}), props["yc"].clone());
    assert_eq_json(json!({"value": 1}), props["height"].clone());
}

#[test]
fn quantitative_x_and_ordinal_y() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "tick",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Cylinders", "type": "ordinal"}
        },
        "data": {"url": "data/cars.json"}
    }));
    let props = tick(&model);
    assert_eq_json(json!({"scale": "x", "field": "Horsepower"}), props["xc"].clone());
    assert_eq_json(json!({"scale": "y", "field": "Cylinders"}), props["y"].clone());
    assert_eq_json(json!({"value": 1}), props["width"].clone());
    assert_eq_json(json!({"signal": "max(0.25, bandwidth('y'))"}), props["height"].clone());
}

#[test]
fn quantitative_x_and_ordinal_y_with_y_offset() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "tick",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Cylinders", "type": "ordinal"},
            "yOffset": {"field": "Acceleration", "type": "ordinal"}
        },
        "data": {"url": "data/cars.json"}
    }));
    let props = tick(&model);
    assert_eq_json(json!({"scale": "x", "field": "Horsepower"}), props["xc"].clone());
    assert_eq_json(
        json!({"scale": "y", "field": "Cylinders", "offset": {"field": "Acceleration", "scale": "yOffset"}}),
        props["y"].clone(),
    );
    assert_eq_json(json!({"value": 1}), props["width"].clone());
    assert_eq_json(
        json!({"signal": "max(0.25, bandwidth('yOffset'))"}),
        props["height"].clone(),
    );
}

#[test]
fn vertical_ticks_maps_size_to_height() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "tick",
        "config": {"mark": {"orient": "vertical"}},
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Cylinders", "type": "ordinal"},
            "size": {"field": "Acceleration", "type": "quantitative"}
        },
        "data": {"url": "data/cars.json"}
    }));
    assert_eq_json(
        json!({"field": "Acceleration", "scale": "size"}),
        tick(&model)["height"].clone(),
    );
}

#[test]
fn vertical_ticks_with_size_in_mark_def() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "tick", "size": 5},
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Cylinders", "type": "ordinal"}
        },
        "data": {"url": "data/cars.json"}
    }));
    assert_eq_json(json!({"value": 5}), tick(&model)["height"].clone());
}

#[test]
fn vertical_ticks_implicit_maps_size_to_height() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "tick",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Cylinders", "type": "ordinal"},
            "size": {"field": "Acceleration", "type": "quantitative"}
        },
        "data": {"url": "data/cars.json"}
    }));
    assert_eq_json(
        json!({"field": "Acceleration", "scale": "size"}),
        tick(&model)["height"].clone(),
    );
}

#[test]
fn vertical_ticks_without_y() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "tick",
        "encoding": {"x": {"field": "Horsepower", "type": "quantitative"}},
        "data": {"url": "data/cars.json"}
    }));
    assert_eq_json(json!({"signal": "0.75 * height"}), tick(&model)["height"].clone());
}

#[test]
fn horizontal_ticks_without_x() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "tick",
        "encoding": {"y": {"field": "Horsepower", "type": "quantitative"}},
        "data": {"url": "data/cars.json"}
    }));
    assert_eq_json(json!({"signal": "0.75 * width"}), tick(&model)["width"].clone());
}

#[test]
fn quantitative_x_scale() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "tick",
        "encoding": {"x": {"field": "Horsepower", "type": "quantitative"}},
        "data": {"url": "data/cars.json"}
    }));
    assert_eq_json(json!({"scale": "x", "field": "Horsepower"}), tick(&model)["xc"].clone());
}

#[test]
fn quantitative_x_width_is_thickness() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "tick",
        "encoding": {"x": {"field": "Horsepower", "type": "quantitative"}},
        "data": {"url": "data/cars.json"}
    }));
    assert_eq_json(json!({"value": 1}), tick(&model)["width"].clone());
}

#[test]
fn quantitative_y_scale() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "tick",
        "encoding": {"y": {"field": "Cylinders", "type": "quantitative"}},
        "data": {"url": "data/cars.json"}
    }));
    assert_eq_json(json!({"scale": "y", "field": "Cylinders"}), tick(&model)["yc"].clone());
}

#[test]
fn quantitative_y_height_is_thickness() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "tick",
        "encoding": {"y": {"field": "Cylinders", "type": "quantitative"}},
        "data": {"url": "data/cars.json"}
    }));
    assert_eq_json(json!({"value": 1}), tick(&model)["height"].clone());
}

#[test]
fn quantitative_x_and_ordinal_y_scale_x() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "tick",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Cylinders", "type": "ordinal"}
        },
        "data": {"url": "data/cars.json"}
    }));
    assert_eq_json(json!({"scale": "x", "field": "Horsepower"}), tick(&model)["xc"].clone());
}

#[test]
fn quantitative_x_and_ordinal_y_scale_y() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "tick",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Cylinders", "type": "ordinal"}
        },
        "data": {"url": "data/cars.json"}
    }));
    assert_eq_json(json!({"scale": "y", "field": "Cylinders"}), tick(&model)["y"].clone());
}

#[test]
fn quantitative_x_and_ordinal_y_width_is_thickness() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "tick",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Cylinders", "type": "ordinal"}
        },
        "data": {"url": "data/cars.json"}
    }));
    assert_eq_json(json!({"value": 1}), tick(&model)["width"].clone());
}

#[test]
fn y_offset_scale_x() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "tick",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Cylinders", "type": "ordinal"},
            "yOffset": {"field": "Acceleration", "type": "ordinal"}
        },
        "data": {"url": "data/cars.json"}
    }));
    assert_eq_json(json!({"scale": "x", "field": "Horsepower"}), tick(&model)["xc"].clone());
}

#[test]
fn y_offset_width_is_thickness() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "tick",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Cylinders", "type": "ordinal"},
            "yOffset": {"field": "Acceleration", "type": "ordinal"}
        },
        "data": {"url": "data/cars.json"}
    }));
    assert_eq_json(json!({"value": 1}), tick(&model)["width"].clone());
}

#[test]
fn y_offset_height_matches_offset_band() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "tick",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Cylinders", "type": "ordinal"},
            "yOffset": {"field": "Acceleration", "type": "ordinal"}
        },
        "data": {"url": "data/cars.json"}
    }));
    assert_eq_json(
        json!({"signal": "max(0.25, bandwidth('yOffset'))"}),
        tick(&model)["height"].clone(),
    );
}

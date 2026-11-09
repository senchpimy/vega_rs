//! Port of `test/compile/mark/bar.test.ts`.

#[path = "mark_util.rs"]
mod mark_util;

use serde_json::{json, Value};

use mark_util::{assert_eq_json, assert_match_object, parse_unit_model_with_scale_and_layout};
use vega_lite::compile::mark::bar::encode_entry as bar;
use vega_lite::config::DEFAULT_STEP;
use vega_lite::mark::default_bar_config;

fn num(value: f64) -> Value {
    if value == value.trunc() {
        json!(value as i64)
    } else {
        json!(value)
    }
}

fn continuous_band_size() -> Value {
    num(default_bar_config()["continuousBandSize"].as_f64().unwrap())
}

fn bin_spacing() -> Value {
    default_bar_config()["binSpacing"].clone()
}

#[test]
fn simple_vertical() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "x": {"field": "Origin", "type": "nominal"},
            "y": {"type": "quantitative", "field": "Acceleration", "aggregate": "mean"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "x", "field": "Origin"}), props["x"].clone());
    assert_eq_json(json!({"signal": "max(0.25, bandwidth('x'))"}), props["width"].clone());
    assert_eq_json(json!({"scale": "y", "field": "mean_Acceleration"}), props["y"].clone());
    assert_eq_json(json!({"scale": "y", "value": 0}), props["y2"].clone());
    assert!(props.get("height").is_none());
}

#[test]
fn vertical_grouped_bar_with_offset() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "x": {"field": "Origin", "type": "nominal"},
            "xOffset": {"field": "SubOrigin", "type": "nominal"},
            "y": {"type": "quantitative", "field": "Acceleration", "aggregate": "mean"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(
        json!({"scale": "x", "field": "Origin", "offset": {"scale": "xOffset", "field": "SubOrigin"}}),
        props["x"].clone(),
    );
    assert_eq_json(
        json!({"signal": "max(0.25, bandwidth('xOffset'))"}),
        props["width"].clone(),
    );
    assert_eq_json(json!({"scale": "y", "field": "mean_Acceleration"}), props["y"].clone());
    assert_eq_json(json!({"scale": "y", "value": 0}), props["y2"].clone());
    assert!(props.get("height").is_none());
}

#[test]
fn ranged_bars_within_discrete_y_band_when_y_offset_continuous() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "x": {"field": "Origin", "type": "nominal"},
            "y": {"field": "Cylinders", "type": "ordinal"},
            "yOffset": {"field": "Acceleration", "type": "quantitative"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(
        json!({"scale": "y", "field": "Cylinders", "offset": {"scale": "yOffset", "field": "Acceleration"}}),
        props["y"].clone(),
    );
    assert_eq_json(
        json!({"scale": "y", "field": "Cylinders", "offset": {"scale": "yOffset", "value": 0}}),
        props["y2"].clone(),
    );
    assert!(props.get("height").is_none());
}

#[test]
fn horizontal_bar_quantitative_x_x2_y() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "x": {"field": "q_start", "type": "quantitative"},
            "x2": {"field": "q_end"},
            "y": {"type": "quantitative", "field": "Acceleration"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "x", "field": "q_start"}), props["x"].clone());
    assert_eq_json(json!({"scale": "x", "field": "q_end"}), props["x2"].clone());
    assert_eq_json(json!({"scale": "y", "field": "Acceleration"}), props["yc"].clone());
    assert!(props.get("y").is_none());
    assert_eq_json(json!({"value": continuous_band_size()}), props["height"].clone());
}

#[test]
fn vertical_bar_binned_x_x2_y() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "x": {"field": "bin_start", "type": "quantitative", "bin": "binned"},
            "x2": {"field": "bin_end"},
            "y": {"type": "quantitative", "field": "Acceleration", "aggregate": "mean"}
        },
        "config": {"bar": {"minBandSize": null}}
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "x", "field": "bin_end"}), props["x"].clone());
    assert_eq_json(json!({"scale": "x", "field": "bin_start", "offset": 1}), props["x2"].clone());
    assert_eq_json(json!({"scale": "y", "field": "mean_Acceleration"}), props["y"].clone());
    assert_eq_json(json!({"scale": "y", "value": 0}), props["y2"].clone());
}

#[test]
fn vertical_bar_domain_including_zero() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "x": {"field": "Origin", "type": "nominal"},
            "y": {"type": "quantitative", "field": "Acceleration", "aggregate": "mean", "scale": {"domain": [-1, 1]}}
        }
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "x", "field": "Origin"}), props["x"].clone());
    assert_eq_json(json!({"signal": "max(0.25, bandwidth('x'))"}), props["width"].clone());
    assert_eq_json(json!({"scale": "y", "field": "mean_Acceleration"}), props["y"].clone());
    assert_eq_json(json!({"scale": "y", "value": 0}), props["y2"].clone());
    assert!(props.get("height").is_none());
}

#[test]
fn vertical_bar_domain_excluding_zero() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "x": {"field": "Origin", "type": "nominal"},
            "y": {"type": "quantitative", "field": "Acceleration", "aggregate": "mean", "scale": {"domain": [1, 2]}}
        }
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "y", "field": "mean_Acceleration"}), props["y"].clone());
    assert_eq_json(json!({"signal": "scale('y', domain('y')[0])"}), props["y2"].clone());
    assert!(props.get("height").is_none());
}

#[test]
fn vertical_bar_zero_false() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "x": {"field": "Origin", "type": "nominal"},
            "y": {"type": "quantitative", "field": "Acceleration", "aggregate": "mean", "scale": {"zero": false}}
        }
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "y", "field": "mean_Acceleration"}), props["y"].clone());
    assert_eq_json(
        json!({"signal": "scale('y', inrange(0, domain('y')) ? 0 : domain('y')[0])"}),
        props["y2"].clone(),
    );
}

#[test]
fn vertical_bar_explicit_domain_excludes_zero() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "x": {"field": "Origin", "type": "nominal"},
            "y": {"type": "quantitative", "field": "Acceleration", "aggregate": "mean", "scale": {"domain": [1, 5]}}
        }
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "y", "field": "mean_Acceleration"}), props["y"].clone());
    assert_eq_json(json!({"signal": "scale('y', domain('y')[0])"}), props["y2"].clone());
}

#[test]
fn vertical_bar_log_scale() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "x": {"field": "Origin", "type": "nominal"},
            "y": {"type": "quantitative", "field": "Acceleration", "aggregate": "mean", "scale": {"type": "log"}}
        }
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "y", "field": "mean_Acceleration"}), props["y"].clone());
    assert_eq_json(json!({"signal": "scale('y', domain('y')[0])"}), props["y2"].clone());
    assert!(props.get("height").is_none());
}

#[test]
fn simple_horizontal() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "y": {"field": "Origin", "type": "nominal"},
            "x": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "y", "field": "Origin"}), props["y"].clone());
    assert_eq_json(json!({"signal": "max(0.25, bandwidth('y'))"}), props["height"].clone());
    assert_eq_json(json!({"scale": "x", "field": "mean_Acceleration"}), props["x"].clone());
    assert_eq_json(json!({"scale": "x", "value": 0}), props["x2"].clone());
    assert!(props.get("width").is_none());
}

#[test]
fn round_horizontal_bar_ends() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": {"type": "bar", "cornerRadiusEnd": 5},
        "encoding": {
            "y": {"field": "Origin", "type": "nominal"},
            "x": {"field": "Acceleration", "type": "quantitative", "stack": null}
        }
    }));
    let props = bar(&model);
    let round_right = json!([
        {"test": "scale(\"x\", datum[\"Acceleration\"]) > scale(\"x\", 0)", "value": 5},
        {"value": 0}
    ]);
    let round_left = json!([
        {"test": "scale(\"x\", datum[\"Acceleration\"]) < scale(\"x\", 0)", "value": 5},
        {"value": 0}
    ]);
    assert_match_object(
        &json!({
            "cornerRadiusTopRight": round_right,
            "cornerRadiusBottomRight": round_right,
            "cornerRadiusTopLeft": round_left,
            "cornerRadiusBottomLeft": round_left,
        }),
        &props,
    );
}

#[test]
fn round_vertical_bar_ends() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": {"type": "bar", "cornerRadiusEnd": 5},
        "encoding": {
            "x": {"field": "Origin", "type": "nominal"},
            "y": {"field": "Acceleration", "type": "quantitative", "stack": null}
        }
    }));
    let props = bar(&model);
    let top = json!([
        {"test": "scale(\"y\", datum[\"Acceleration\"]) < scale(\"y\", 0)", "value": 5},
        {"value": 0}
    ]);
    let bottom = json!([
        {"test": "scale(\"y\", datum[\"Acceleration\"]) > scale(\"y\", 0)", "value": 5},
        {"value": 0}
    ]);
    assert_match_object(
        &json!({
            "cornerRadiusTopLeft": top,
            "cornerRadiusTopRight": top,
            "cornerRadiusBottomLeft": bottom,
            "cornerRadiusBottomRight": bottom,
        }),
        &props,
    );
}

#[test]
fn fallback_corner_radius_properties() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": {"type": "bar", "cornerRadiusEnd": 5, "cornerRadiusBottomLeft": 8, "cornerRadius": 2},
        "encoding": {
            "x": {"field": "Origin", "type": "nominal"},
            "y": {"field": "Acceleration", "type": "quantitative", "stack": null}
        }
    }));
    let props = bar(&model);
    let top = "scale(\"y\", datum[\"Acceleration\"]) < scale(\"y\", 0)";
    let bottom = "scale(\"y\", datum[\"Acceleration\"]) > scale(\"y\", 0)";
    assert_match_object(
        &json!({
            "cornerRadiusTopLeft": [{"test": top, "value": 5}, {"value": 2}],
            "cornerRadiusTopRight": [{"test": top, "value": 5}, {"value": 2}],
            "cornerRadiusBottomLeft": [{"test": bottom, "value": 5}, {"value": 8}],
            "cornerRadiusBottomRight": [{"test": bottom, "value": 5}, {"value": 2}],
        }),
        &props,
    );
}

#[test]
fn config_corner_radius_end_does_not_override_mark_corner_radii() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": {"type": "bar", "cornerRadius": 2, "cornerRadiusTopLeft": 8},
        "encoding": {
            "x": {"field": "Origin", "type": "nominal"},
            "y": {"field": "Acceleration", "type": "quantitative", "stack": null}
        },
        "config": {"bar": {"cornerRadiusEnd": 5}}
    }));
    let props = bar(&model);
    assert_eq_json(json!({"value": 8}), props["cornerRadiusTopLeft"].clone());
    assert_eq_json(json!({"value": 2}), props["cornerRadius"].clone());
    assert!(props.get("cornerRadiusBottomLeft").is_none());
}

#[test]
fn ranged_bar_mark_corner_radius_end_all_corners() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/population.json"},
        "mark": {"type": "bar", "cornerRadiusEnd": 5},
        "encoding": {
            "y": {"field": "age", "type": "ordinal"},
            "x": {"field": "people", "aggregate": "q1", "type": "quantitative"},
            "x2": {"field": "people", "aggregate": "q3"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(json!({"value": 5}), props["cornerRadius"].clone());
    assert!(props.get("cornerRadiusTopRight").is_none());
    assert!(props.get("cornerRadiusBottomRight").is_none());
}

#[test]
fn ranged_bar_config_corner_radius_end_does_not_override_mark() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/population.json"},
        "mark": {"type": "bar", "cornerRadius": 2},
        "encoding": {
            "y": {"field": "age", "type": "ordinal"},
            "x": {"field": "people", "aggregate": "q1", "type": "quantitative"},
            "x2": {"field": "people", "aggregate": "q3"}
        },
        "config": {"bar": {"cornerRadiusEnd": 5}}
    }));
    let props = bar(&model);
    assert_eq_json(json!({"value": 2}), props["cornerRadius"].clone());
}

#[test]
fn horizontal_with_height_band() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": {"type": "bar", "height": {"band": 0.6}},
        "encoding": {
            "y": {"field": "Origin", "type": "nominal"},
            "x": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "y", "field": "Origin", "band": 0.2}), props["y"].clone());
    assert_eq_json(json!({"signal": "max(0.25, 0.6 * bandwidth('y'))"}), props["height"].clone());
    assert_eq_json(json!({"scale": "x", "field": "mean_Acceleration"}), props["x"].clone());
    assert_eq_json(json!({"scale": "x", "value": 0}), props["x2"].clone());
    assert!(props.get("width").is_none());
}

#[test]
fn vertical_bar_binned_y_y2_x() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "y": {"field": "bin_start", "type": "quantitative", "bin": "binned"},
            "y2": {"field": "bin_end"},
            "x": {"type": "quantitative", "field": "Acceleration", "aggregate": "mean"}
        },
        "config": {"bar": {"minBandSize": null}}
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "y", "field": "bin_end", "offset": 1}), props["y"].clone());
    assert_eq_json(json!({"scale": "y", "field": "bin_start"}), props["y2"].clone());
    assert_eq_json(json!({"scale": "x", "field": "mean_Acceleration"}), props["x"].clone());
    assert_eq_json(json!({"scale": "x", "value": 0}), props["x2"].clone());
}

#[test]
fn horizontal_bar_quantitative_y_y2_x() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "y": {"field": "q_start", "type": "quantitative"},
            "y2": {"field": "q_end"},
            "x": {"type": "quantitative", "field": "Acceleration"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "y", "field": "q_start"}), props["y"].clone());
    assert_eq_json(json!({"scale": "y", "field": "q_end"}), props["y2"].clone());
    assert_eq_json(json!({"scale": "x", "field": "Acceleration"}), props["xc"].clone());
    assert!(props.get("x").is_none());
    assert_eq_json(json!({"value": continuous_band_size()}), props["width"].clone());
}

#[test]
fn horizontal_with_point_scale() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "y": {"field": "Origin", "type": "nominal", "scale": {"type": "point"}},
            "x": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "y", "field": "Origin"}), props["yc"].clone());
    assert_eq_json(json!({"value": DEFAULT_STEP - 2.0}), props["height"].clone());
    assert_eq_json(json!({"scale": "x", "field": "mean_Acceleration"}), props["x"].clone());
    assert_eq_json(json!({"scale": "x", "value": 0}), props["x2"].clone());
    assert!(props.get("width").is_none());
}

#[test]
fn horizontal_with_size_value() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "y": {"field": "Origin", "type": "nominal"},
            "x": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"},
            "size": {"value": 5}
        }
    }));
    let props = bar(&model);
    assert_eq_json(json!({"value": 5}), props["height"].clone());
    assert_eq_json(json!({"scale": "y", "field": "Origin", "band": 0.5}), props["yc"].clone());
}

#[test]
fn horizontal_with_size_in_mark_def() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": {"type": "bar", "size": 5},
        "encoding": {
            "y": {"field": "Origin", "type": "nominal"},
            "x": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(json!({"value": 5}), props["height"].clone());
    assert_eq_json(json!({"scale": "y", "field": "Origin", "band": 0.5}), props["yc"].clone());
}

#[test]
fn horizontal_with_size_field() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "y": {"field": "Origin", "type": "nominal"},
            "x": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"},
            "size": {"aggregate": "mean", "field": "Horsepower", "type": "quantitative"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "y", "field": "Origin", "band": 0.5}), props["yc"].clone());
    assert_eq_json(json!({"scale": "size", "field": "mean_Horsepower"}), props["height"].clone());
    assert_eq_json(json!({"scale": "x", "field": "mean_Acceleration"}), props["x"].clone());
    assert_eq_json(json!({"scale": "x", "value": 0}), props["x2"].clone());
    assert!(props.get("width").is_none());
}

#[test]
fn horizontal_binned() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "y": {"bin": true, "field": "Horsepower", "type": "quantitative"},
            "x": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"}
        },
        "config": {"bar": {"minBandSize": null}}
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "y", "field": "bin_maxbins_10_Horsepower"}), props["y2"].clone());
    assert_eq_json(
        json!({"scale": "y", "field": "bin_maxbins_10_Horsepower_end", "offset": bin_spacing()}),
        props["y"].clone(),
    );
    assert!(props.get("height").is_none());
}

#[test]
fn horizontal_binned_with_offset() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": {"type": "bar", "yOffset": 5},
        "encoding": {
            "y": {"bin": true, "field": "Horsepower", "type": "quantitative"},
            "x": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"}
        },
        "config": {"bar": {"minBandSize": null}}
    }));
    let props = bar(&model);
    assert_eq_json(
        json!({"scale": "y", "field": "bin_maxbins_10_Horsepower", "offset": 5}),
        props["y2"].clone(),
    );
    assert_eq_json(
        json!({"scale": "y", "field": "bin_maxbins_10_Horsepower_end", "offset": num(bin_spacing().as_f64().unwrap() + 5.0)}),
        props["y"].clone(),
    );
    assert!(props.get("height").is_none());
}

#[test]
fn horizontal_binned_with_offset_signal() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": {"type": "bar", "yOffset": {"signal": "a"}},
        "encoding": {
            "y": {"bin": true, "field": "Horsepower", "type": "quantitative"},
            "x": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"}
        },
        "config": {"bar": {"minBandSize": null}}
    }));
    let props = bar(&model);
    assert_eq_json(
        json!({"scale": "y", "field": "bin_maxbins_10_Horsepower", "offset": {"signal": "0.5 + (a + -0.5)"}}),
        props["y2"].clone(),
    );
    assert_eq_json(
        json!({"scale": "y", "field": "bin_maxbins_10_Horsepower_end", "offset": {"signal": "0.5 + (a + 0.5)"}}),
        props["y"].clone(),
    );
    assert!(props.get("height").is_none());
}

#[test]
fn horizontal_binned_sort_descending() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "y": {"bin": true, "field": "Horsepower", "type": "quantitative", "sort": "descending"},
            "x": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"}
        },
        "config": {"bar": {"minBandSize": null}}
    }));
    let props = bar(&model);
    assert_eq_json(
        json!({"scale": "y", "field": "bin_maxbins_10_Horsepower", "offset": bin_spacing()}),
        props["y2"].clone(),
    );
    assert_eq_json(
        json!({"scale": "y", "field": "bin_maxbins_10_Horsepower_end"}),
        props["y"].clone(),
    );
    assert!(props.get("height").is_none());
}

#[test]
fn horizontal_binned_reverse() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "y": {"bin": true, "field": "Horsepower", "type": "quantitative", "scale": {"reverse": true}},
            "x": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"}
        },
        "config": {"bar": {"minBandSize": null}}
    }));
    let props = bar(&model);
    assert_eq_json(
        json!({"scale": "y", "field": "bin_maxbins_10_Horsepower", "offset": bin_spacing()}),
        props["y2"].clone(),
    );
    assert_eq_json(
        json!({"scale": "y", "field": "bin_maxbins_10_Horsepower_end"}),
        props["y"].clone(),
    );
    assert!(props.get("height").is_none());
}

#[test]
fn vertical_binned() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "x": {"bin": true, "field": "Horsepower", "type": "quantitative"},
            "y": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"}
        },
        "config": {"bar": {"minBandSize": null}}
    }));
    let props = bar(&model);
    assert_eq_json(
        json!({"scale": "x", "field": "bin_maxbins_10_Horsepower", "offset": bin_spacing()}),
        props["x2"].clone(),
    );
    assert_eq_json(
        json!({"scale": "x", "field": "bin_maxbins_10_Horsepower_end"}),
        props["x"].clone(),
    );
    assert!(props.get("width").is_none());
}

#[test]
fn vertical_binned_with_width() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": {"type": "bar", "width": 5},
        "encoding": {
            "x": {"bin": true, "field": "Horsepower", "type": "quantitative"},
            "y": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(
        json!({"signal": "scale(\"x\", 0.5 * datum[\"bin_maxbins_10_Horsepower\"] + 0.5 * datum[\"bin_maxbins_10_Horsepower_end\"])"}),
        props["xc"].clone(),
    );
    assert_eq_json(json!({"value": 5}), props["width"].clone());
}

#[test]
fn vertical_binned_sort_descending() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "x": {"bin": true, "field": "Horsepower", "type": "quantitative", "sort": "descending"},
            "y": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"}
        },
        "config": {"bar": {"minBandSize": null}}
    }));
    let props = bar(&model);
    assert_eq_json(
        json!({"scale": "x", "field": "bin_maxbins_10_Horsepower"}),
        props["x2"].clone(),
    );
    assert_eq_json(
        json!({"scale": "x", "field": "bin_maxbins_10_Horsepower_end", "offset": bin_spacing()}),
        props["x"].clone(),
    );
    assert!(props.get("width").is_none());
}

#[test]
fn horizontal_binned_with_ordinal() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "y": {"bin": true, "field": "Horsepower", "type": "ordinal"},
            "x": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(
        json!({"scale": "y", "field": "bin_maxbins_10_Horsepower_range"}),
        props["y"].clone(),
    );
    assert_eq_json(json!({"signal": "max(0.25, bandwidth('y'))"}), props["height"].clone());
}

#[test]
fn vertical_binned_with_ordinal() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "x": {"bin": true, "field": "Horsepower", "type": "ordinal"},
            "y": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(
        json!({"scale": "x", "field": "bin_maxbins_10_Horsepower_range"}),
        props["x"].clone(),
    );
    assert_eq_json(json!({"signal": "max(0.25, bandwidth('x'))"}), props["width"].clone());
}

#[test]
fn horizontal_binned_with_no_spacing() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "y": {"bin": true, "field": "Horsepower", "type": "quantitative"},
            "x": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"}
        },
        "config": {"bar": {"binSpacing": 0, "minBandSize": null}}
    }));
    let props = bar(&model);
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
fn vertical_binned_with_no_spacing() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "x": {"bin": true, "field": "Horsepower", "type": "quantitative"},
            "y": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"}
        },
        "config": {"bar": {"binSpacing": 0, "minBandSize": null}}
    }));
    let props = bar(&model);
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
fn horizontal_binned_with_size() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "y": {"bin": true, "field": "Horsepower", "type": "quantitative"},
            "x": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"},
            "size": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(
        json!({"signal": "scale(\"y\", 0.5 * datum[\"bin_maxbins_10_Horsepower\"] + 0.5 * datum[\"bin_maxbins_10_Horsepower_end\"])"}),
        props["yc"].clone(),
    );
    assert_eq_json(json!({"scale": "size", "field": "mean_Acceleration"}), props["height"].clone());
}

#[test]
fn vertical_binned_with_size() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "x": {"bin": true, "field": "Horsepower", "type": "quantitative"},
            "y": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"},
            "size": {"aggregate": "mean", "field": "Acceleration", "type": "quantitative"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(
        json!({"signal": "scale(\"x\", 0.5 * datum[\"bin_maxbins_10_Horsepower\"] + 0.5 * datum[\"bin_maxbins_10_Horsepower_end\"])"}),
        props["xc"].clone(),
    );
    assert_eq_json(json!({"scale": "size", "field": "mean_Acceleration"}), props["width"].clone());
}

#[test]
fn vertical_with_log() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "x": {"field": "Origin", "type": "nominal"},
            "y": {"scale": {"type": "log"}, "type": "quantitative", "field": "Acceleration", "aggregate": "mean"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(json!({"signal": "scale('y', domain('y')[0])"}), props["y2"].clone());
    assert!(props.get("height").is_none());
}

#[test]
fn horizontal_with_log() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "y": {"field": "Origin", "type": "nominal"},
            "x": {"scale": {"type": "log"}, "type": "quantitative", "field": "Acceleration", "aggregate": "mean"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(json!({"signal": "scale('x', domain('x')[0])"}), props["x2"].clone());
    assert!(props.get("width").is_none());
}

#[test]
fn vertical_with_fit_mode() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "width": 120,
        "height": 120,
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "x": {"field": "Origin", "type": "nominal"},
            "y": {"aggregate": "mean", "field": "Horsepower", "type": "quantitative"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "x", "field": "Origin"}), props["x"].clone());
    assert_eq_json(json!({"signal": "max(0.25, bandwidth('x'))"}), props["width"].clone());
}

#[test]
fn horizontal_with_fit_mode() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "width": 120,
        "height": 120,
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "y": {"field": "Origin", "type": "nominal"},
            "x": {"aggregate": "mean", "field": "Horsepower", "type": "quantitative"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "y", "field": "Origin"}), props["y"].clone());
    assert_eq_json(json!({"signal": "max(0.25, bandwidth('y'))"}), props["height"].clone());
}

#[test]
fn vertical_with_zero_false() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "x": {"field": "Origin", "type": "nominal"},
            "y": {"scale": {"zero": false}, "type": "quantitative", "field": "Acceleration", "aggregate": "mean"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(
        json!({"signal": "scale('y', inrange(0, domain('y')) ? 0 : domain('y')[0])"}),
        props["y2"].clone(),
    );
}

#[test]
fn horizontal_with_zero_false() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "y": {"field": "Origin", "type": "nominal"},
            "x": {"scale": {"zero": false}, "type": "quantitative", "field": "Acceleration", "aggregate": "mean"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(
        json!({"signal": "scale('x', inrange(0, domain('x')) ? 0 : domain('x')[0])"}),
        props["x2"].clone(),
    );
}

#[test]
fn one_dimensional_vertical() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": {"type": "bar", "width": {"band": 0.5}},
        "encoding": {
            "y": {"type": "quantitative", "field": "Acceleration", "aggregate": "mean"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "y", "field": "mean_Acceleration"}), props["y"].clone());
    assert_eq_json(json!({"scale": "y", "value": 0}), props["y2"].clone());
    assert!(props.get("height").is_none());
    assert_eq_json(json!({"mult": 0.5, "signal": "width"}), props["xc"].clone());
    assert_eq_json(json!({"mult": 0.5, "field": {"group": "width"}}), props["width"].clone());
}

#[test]
fn one_dimensional_vertical_with_size_value() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "bar",
        "encoding": {
            "y": {"type": "quantitative", "field": "US_Gross", "aggregate": "sum"},
            "size": {"value": 5}
        },
        "data": {"url": "data/movies.json"}
    }));
    let props = bar(&model);
    assert_eq_json(json!({"value": 5}), props["width"].clone());
}

#[test]
fn one_dimensional_vertical_config_mark_size() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "bar",
        "encoding": {"y": {"type": "quantitative", "field": "US_Gross", "aggregate": "sum"}},
        "data": {"url": "data/movies.json"},
        "config": {"mark": {"size": 5}}
    }));
    let props = bar(&model);
    assert_eq_json(json!({"signal": "0.9 * width"}), props["width"].clone());
}

#[test]
fn one_dimensional_vertical_config_bar_discrete_band_size() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/movies.json"},
        "mark": "bar",
        "encoding": {"y": {"type": "quantitative", "field": "US_Gross", "aggregate": "sum"}},
        "config": {"bar": {"discreteBandSize": 5}}
    }));
    let props = bar(&model);
    assert_eq_json(json!({"value": 5}), props["width"].clone());
}

#[test]
fn two_dimensional_vertical_point_scale_config_bar_discrete_band_size() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/movies.json"},
        "mark": "bar",
        "encoding": {
            "y": {"type": "quantitative", "field": "US_Gross", "aggregate": "sum"},
            "x": {"type": "nominal", "field": "Major_Genre", "scale": {"type": "point"}}
        },
        "config": {"bar": {"discreteBandSize": 5}}
    }));
    let props = bar(&model);
    assert_eq_json(json!({"value": 5}), props["width"].clone());
}

#[test]
fn two_dimensional_vertical_config_bar_discrete_band_size() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/movies.json"},
        "mark": "bar",
        "encoding": {
            "y": {"type": "quantitative", "field": "US_Gross", "aggregate": "sum"},
            "x": {"type": "nominal", "field": "Major_Genre"}
        },
        "config": {"bar": {"discreteBandSize": 5}}
    }));
    let props = bar(&model);
    assert_eq_json(json!({"field": "Major_Genre", "scale": "x", "band": 0.5}), props["xc"].clone());
    assert_eq_json(json!({"value": 5}), props["width"].clone());
}

#[test]
fn one_dimensional_vertical_config_bar_size() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/movies.json"},
        "mark": "bar",
        "encoding": {"y": {"type": "quantitative", "field": "US_Gross", "aggregate": "sum"}},
        "config": {"bar": {"size": 5}}
    }));
    let props = bar(&model);
    assert_eq_json(json!({"value": 5}), props["width"].clone());
}

#[test]
fn one_dimensional_vertical_config_style_bar_size() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/movies.json"},
        "mark": "bar",
        "encoding": {"y": {"type": "quantitative", "field": "US_Gross", "aggregate": "sum"}},
        "config": {"style": {"bar": {"size": 5}}}
    }));
    let props = bar(&model);
    assert_eq_json(json!({"value": 5}), props["width"].clone());
}

#[test]
fn one_dimensional_horizontal() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "bar",
        "encoding": {"x": {"type": "quantitative", "field": "US_Gross", "aggregate": "sum"}},
        "data": {"url": "data/movies.json"}
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "x", "field": "sum_US_Gross"}), props["x"].clone());
    assert_eq_json(json!({"scale": "x", "value": 0}), props["x2"].clone());
    assert!(props.get("width").is_none());
    assert_eq_json(json!({"mult": 0.5, "signal": "height"}), props["yc"].clone());
}

#[test]
fn qxq_horizontal() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "x": {"field": "Acceleration", "type": "quantitative"},
            "y": {"field": "Horsepower", "type": "quantitative"}
        },
        "config": {"mark": {"orient": "horizontal"}}
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "x", "field": "Acceleration_end"}), props["x"].clone());
    assert_eq_json(json!({"scale": "x", "field": "Acceleration_start"}), props["x2"].clone());
    assert_eq_json(json!({"scale": "y", "field": "Horsepower"}), props["yc"].clone());
    assert_eq_json(json!({"value": continuous_band_size()}), props["height"].clone());
}

#[test]
fn qxq_vertical() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "x": {"field": "Acceleration", "type": "quantitative"},
            "y": {"field": "Horsepower", "type": "quantitative"}
        },
        "config": {"mark": {"orient": "vertical"}}
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "x", "field": "Acceleration"}), props["xc"].clone());
    assert_eq_json(json!({"value": continuous_band_size()}), props["width"].clone());
    assert_eq_json(json!({"scale": "y", "field": "Horsepower_end"}), props["y"].clone());
    assert_eq_json(json!({"scale": "y", "field": "Horsepower_start"}), props["y2"].clone());
}

#[test]
fn oxn() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "bar",
        "encoding": {
            "x": {"field": "Origin", "type": "nominal"},
            "y": {"field": "Cylinders", "type": "ordinal"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "x", "field": "Origin"}), props["x"].clone());
    assert_eq_json(json!({"signal": "max(0.25, bandwidth('x'))"}), props["width"].clone());
    assert_eq_json(json!({"scale": "y", "field": "Cylinders"}), props["y"].clone());
    assert_eq_json(json!({"signal": "max(0.25, bandwidth('y'))"}), props["height"].clone());
}

#[test]
fn ranged_bar_vertical_aggregate() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/population.json"},
        "mark": "bar",
        "encoding": {
            "x": {"field": "age", "type": "ordinal"},
            "y": {"field": "people", "aggregate": "q1", "type": "quantitative"},
            "y2": {"field": "people", "aggregate": "q3"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "x", "field": "age"}), props["x"].clone());
    assert_eq_json(json!({"scale": "y", "field": "q1_people"}), props["y"].clone());
    assert_eq_json(json!({"scale": "y", "field": "q3_people"}), props["y2"].clone());
}

#[test]
fn ranged_bar_horizontal_aggregate() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/population.json"},
        "mark": "bar",
        "encoding": {
            "y": {"field": "age", "type": "ordinal"},
            "x": {"field": "people", "aggregate": "q1", "type": "quantitative"},
            "x2": {"field": "people", "aggregate": "q3"}
        }
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "y", "field": "age"}), props["y"].clone());
    assert_eq_json(json!({"scale": "x", "field": "q1_people"}), props["x"].clone());
    assert_eq_json(json!({"scale": "x", "field": "q3_people"}), props["x2"].clone());
}

#[test]
fn ranged_bar_config_corner_radius_end() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/population.json"},
        "mark": "bar",
        "encoding": {
            "y": {"field": "age", "type": "ordinal"},
            "x": {"field": "people", "aggregate": "q1", "type": "quantitative"},
            "x2": {"field": "people", "aggregate": "q3"}
        },
        "config": {"bar": {"cornerRadiusEnd": 5}}
    }));
    let props = bar(&model);
    assert_eq_json(json!({"value": 5}), props["cornerRadius"].clone());
    assert!(props.get("cornerRadiusTopRight").is_none());
    assert!(props.get("cornerRadiusBottomRight").is_none());
}

#[test]
fn vertical_binned_data_default_offset() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "bar",
        "encoding": {
            "x": {"field": "bin_start", "bin": "binned", "type": "quantitative", "axis": {"tickMinStep": 2}},
            "x2": {"field": "bin_end"},
            "y": {"field": "count", "type": "quantitative"}
        },
        "config": {"bar": {"minBandSize": null}}
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "x", "field": "bin_start", "offset": 1}), props["x2"].clone());
    assert_eq_json(json!({"scale": "x", "field": "bin_end"}), props["x"].clone());
    assert_eq_json(json!({"scale": "y", "field": "count_end"}), props["y"].clone());
    assert_eq_json(json!({"scale": "y", "field": "count_start"}), props["y2"].clone());
    assert!(props.get("width").is_none());
}

#[test]
fn vertical_binned_data_custom_offset() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "bar", "binSpacing": 10},
        "encoding": {
            "x": {"field": "bin_start", "bin": "binned", "type": "quantitative", "axis": {"tickMinStep": 2}},
            "x2": {"field": "bin_end"},
            "y": {"field": "count", "type": "quantitative"}
        },
        "config": {"bar": {"minBandSize": null}}
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "x", "field": "bin_start", "offset": 5.5}), props["x2"].clone());
    assert_eq_json(json!({"scale": "x", "field": "bin_end", "offset": -4.5}), props["x"].clone());
    assert_eq_json(json!({"scale": "y", "field": "count_end"}), props["y"].clone());
    assert_eq_json(json!({"scale": "y", "field": "count_start"}), props["y2"].clone());
    assert!(props.get("width").is_none());
}

#[test]
fn horizontal_binned_data_default_offset() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "bar",
        "encoding": {
            "y": {"field": "bin_start", "bin": "binned", "type": "quantitative", "axis": {"tickMinStep": 2}},
            "y2": {"field": "bin_end"},
            "x": {"field": "count", "type": "quantitative"}
        },
        "config": {"bar": {"minBandSize": null}}
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "y", "field": "bin_start"}), props["y2"].clone());
    assert_eq_json(json!({"scale": "y", "field": "bin_end", "offset": 1}), props["y"].clone());
    assert_eq_json(json!({"scale": "x", "field": "count_end"}), props["x"].clone());
    assert_eq_json(json!({"scale": "x", "field": "count_start"}), props["x2"].clone());
    assert!(props.get("width").is_none());
}

#[test]
fn horizontal_binned_data_custom_offset() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "bar", "binSpacing": 10},
        "encoding": {
            "y": {"field": "bin_start", "bin": "binned", "type": "quantitative", "axis": {"tickMinStep": 2}},
            "y2": {"field": "bin_end"},
            "x": {"field": "count", "type": "quantitative"}
        },
        "config": {"bar": {"minBandSize": null}}
    }));
    let props = bar(&model);
    assert_eq_json(json!({"scale": "y", "field": "bin_start", "offset": -4.5}), props["y2"].clone());
    assert_eq_json(json!({"scale": "y", "field": "bin_end", "offset": 5.5}), props["y"].clone());
    assert_eq_json(json!({"scale": "x", "field": "count_end"}), props["x"].clone());
    assert_eq_json(json!({"scale": "x", "field": "count_start"}), props["x2"].clone());
    assert!(props.get("width").is_none());
}

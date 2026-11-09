//! Port of the remaining `test/compile/mark/encode/*.test.ts` suites.

#[path = "mark_util.rs"]
mod mark_util;

use serde_json::json;

use mark_util::{assert_eq_json, parse_unit_model_with_scale_and_layout};
use vega_lite::compile::mark::encode::color::color;
use vega_lite::compile::mark::encode::nonposition::{non_position, NonPositionOpt};
use vega_lite::compile::mark::encode::offset::position_offset;
use vega_lite::compile::mark::encode::position_point::point_position;
use vega_lite::compile::mark::encode::position_range::range_position;
use vega_lite::compile::mark::encode::text::text;
use vega_lite::compile::mark::encode::valueref::{mid_point, MidPointParams};
use vega_lite::compile::support::default_config;

#[test]
fn nonposition_respects_default_value() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "point",
        "encoding": {
            "x": {"field": "Acceleration", "type": "quantitative"},
            "y": {"field": "Horsepower", "type": "quantitative"}
        }
    }));
    let mixins = non_position("opacity", &model, NonPositionOpt::default());
    assert_eq_json(json!({"value": 0.7}), mixins["opacity"].clone());
}

#[test]
fn offset_value_for_channel() {
    let mark_def = json!({"type": "point", "x2Offset": 100});
    let offset = position_offset("x2", &mark_def, &json!({}), None, None);
    assert_eq_json(json!(100), offset.offset.unwrap());
}

#[test]
fn offset_undefined_when_not_defined() {
    let mark_def = json!({"type": "point", "x2Offset": 100});
    let offset = position_offset("x", &mark_def, &json!({}), None, None);
    assert!(offset.offset.is_none());
}

#[test]
fn point_position_lat_lng() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/zipcodes.csv", "format": {"type": "csv"}},
        "mark": "point",
        "encoding": {
            "longitude": {"field": "longitude", "type": "quantitative"},
            "latitude": {"field": "latitude", "type": "quantitative"}
        }
    }));
    for channel in ["x", "y"] {
        let mixins = point_position(channel, &model, Some("zeroOrMin"), None);
        assert_eq_json(
            json!(model.get_name(channel)),
            mixins[channel]["field"].clone(),
        );
    }
}

#[test]
fn range_position_lat_lng_with_size() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"url": "data/zipcodes.csv"},
        "mark": {"type": "image", "width": 42, "height": 42},
        "encoding": {
            "longitude": {"field": "longitude", "type": "quantitative"},
            "latitude": {"field": "latitude", "type": "quantitative"}
        }
    }));
    for (channel, size_channel) in [("x", "width"), ("y", "height")] {
        let mixins = range_position(channel, &model, Some("zeroOrMin"), "zeroOrMin");
        assert_eq_json(
            json!(model.get_name(channel)),
            mixins[format!("{channel}c")]["field"].clone(),
        );
        assert_eq_json(json!({"value": 42}), mixins[size_channel].clone());
    }
}

#[test]
fn range_position_lat_lng_without_size() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "data": {"values": [
            {"latitude": 0, "longitude": 0, "latitude2": 30, "longitude2": 30},
            {"latitude": -10, "longitude": -10, "latitude2": 20, "longitude2": 20}
        ]},
        "mark": "rect",
        "encoding": {
            "longitude": {"field": "longitude", "type": "quantitative"},
            "latitude": {"field": "latitude", "type": "quantitative"},
            "longitude2": {"field": "longitude2"},
            "latitude2": {"field": "latitude2"}
        }
    }));
    for channel in ["x", "y"] {
        let mixins = range_position(channel, &model, Some("zeroOrMin"), "zeroOrMin");
        assert_eq_json(
            json!(model.get_name(channel)),
            mixins[channel]["field"].clone(),
        );
        assert_eq_json(
            json!(model.get_name(&format!("{channel}2"))),
            mixins[format!("{channel}2")]["field"].clone(),
        );
    }
}

#[test]
fn text_supports_signals() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "text",
        "encoding": {"text": {"value": {"signal": "Hello"}}},
        "data": {"url": "data/population.json"}
    }));
    assert_eq_json(json!({"signal": "Hello"}), text(&model, "text")["text"].clone());
}

#[test]
fn text_supports_datum_with_format() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "text",
        "encoding": {"text": {"datum": 0.12342342134324, "type": "quantitative", "format": ".2f"}},
        "data": {"url": "data/population.json"}
    }));
    assert_eq_json(
        json!({"signal": "format(0.12342342134324, \".2f\")"}),
        text(&model, "text")["text"].clone(),
    );
}

#[test]
fn mid_point_width() {
    let reference = mid_point(&MidPointParams {
        channel: "x",
        channel_def: Some(&json!({"value": "width"})),
        channel2_def: None,
        mark_def: &json!({"type": "point"}),
        config: &default_config(),
        scale_name: "",
        scale: None,
        stack: None,
        offset: None,
        default_ref: Some(json!({"value": 0})),
        band_position: None,
    });
    assert_eq_json(json!({"field": {"group": "width"}}), reference);
}

#[test]
fn mid_point_height() {
    let reference = mid_point(&MidPointParams {
        channel: "y",
        channel_def: Some(&json!({"value": "height"})),
        channel2_def: None,
        mark_def: &json!({"type": "point"}),
        config: &default_config(),
        scale_name: "",
        scale: None,
        stack: None,
        offset: None,
        default_ref: Some(json!({"value": 0})),
        band_position: None,
    });
    assert_eq_json(json!({"field": {"group": "height"}}), reference);
}

#[test]
fn mid_point_datum() {
    let reference = mid_point(&MidPointParams {
        channel: "y",
        channel_def: Some(&json!({"datum": 5})),
        channel2_def: None,
        mark_def: &json!({"type": "point"}),
        config: &default_config(),
        scale_name: "x",
        scale: None,
        stack: None,
        offset: None,
        default_ref: Some(json!({"value": 0})),
        band_position: None,
    });
    assert_eq_json(json!({"scale": "x", "value": 5}), reference);
}

#[test]
fn mid_point_datum_signal() {
    let reference = mid_point(&MidPointParams {
        channel: "y",
        channel_def: Some(&json!({"datum": {"signal": "foo"}})),
        channel2_def: None,
        mark_def: &json!({"type": "point"}),
        config: &default_config(),
        scale_name: "x",
        scale: None,
        stack: None,
        offset: None,
        default_ref: Some(json!({"value": 0})),
        band_position: None,
    });
    assert_eq_json(json!({"scale": "x", "signal": "foo"}), reference);
}

#[test]
fn mid_point_binned_data() {
    let reference = mid_point(&MidPointParams {
        channel: "x",
        channel_def: Some(&json!({"field": "bin_start", "bin": "binned", "type": "quantitative"})),
        channel2_def: Some(&json!({"field": "bin_end"})),
        mark_def: &json!({"type": "point"}),
        config: &default_config(),
        scale_name: "x",
        scale: None,
        stack: None,
        offset: None,
        default_ref: Some(json!({"value": 0})),
        band_position: None,
    });
    assert_eq_json(
        json!({"signal": "scale(\"x\", 0.5 * datum[\"bin_start\"] + 0.5 * datum[\"bin_end\"])"}),
        reference,
    );
}

// -- color -----------------------------------------------------------------

#[test]
fn color_mapped_to_fill_for_bar() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "bar",
        "encoding": {
            "x": {"field": "gender", "type": "nominal", "axis": null},
            "color": {"field": "gender", "type": "nominal", "scale": {"range": ["#EA98D2", "#659CCA"]}}
        },
        "data": {"url": "data/population.json"}
    }));
    assert_eq_json(
        json!({"field": "gender", "scale": "color"}),
        color(&model, None)["fill"].clone(),
    );
}

#[test]
fn color_mapped_to_stroke_for_point() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "point",
        "encoding": {
            "x": {"field": "gender", "type": "nominal", "axis": null},
            "color": {"field": "gender", "type": "nominal", "scale": {"range": ["#EA98D2", "#659CCA"]}}
        },
        "data": {"url": "data/population.json"}
    }));
    let mixins = color(&model, None);
    assert_eq_json(json!({"field": "gender", "scale": "color"}), mixins["stroke"].clone());
    assert_eq_json(json!("transparent"), mixins["fill"]["value"].clone());
}

#[test]
fn adds_transparent_fill_when_stroke_encoded() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "point",
        "encoding": {
            "x": {"field": "gender", "type": "nominal", "axis": null},
            "stroke": {"field": "gender", "type": "nominal", "scale": {"range": ["#EA98D2", "#659CCA"]}}
        },
        "data": {"url": "data/population.json"}
    }));
    let mixins = color(&model, None);
    assert_eq_json(json!({"field": "gender", "scale": "stroke"}), mixins["stroke"].clone());
    assert_eq_json(json!("transparent"), mixins["fill"]["value"].clone());
}

#[test]
fn combines_color_with_fill_when_filled_false() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "point",
        "encoding": {
            "x": {"field": "gender", "type": "nominal", "axis": null},
            "fill": {"field": "gender", "type": "nominal", "scale": {"range": ["#EA98D2", "#659CCA"]}},
            "color": {"field": "gender", "type": "nominal", "scale": {"range": ["#EA98D2", "#659CCA"]}}
        },
        "data": {"url": "data/population.json"}
    }));
    let mixins = color(&model, None);
    assert_eq_json(json!({"field": "gender", "scale": "color"}), mixins["stroke"].clone());
    assert_eq_json(json!({"field": "gender", "scale": "fill"}), mixins["fill"].clone());
}

#[test]
fn ignores_color_property_if_fill_specified() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "point", "color": "red"},
        "encoding": {
            "x": {"field": "gender", "type": "nominal", "axis": null},
            "fill": {"field": "gender", "type": "nominal", "scale": {"range": ["#EA98D2", "#659CCA"]}}
        },
        "data": {"url": "data/population.json"}
    }));
    let mixins = color(&model, None);
    assert_eq_json(json!({"value": "red"}), mixins["stroke"].clone());
    assert_eq_json(json!({"field": "gender", "scale": "fill"}), mixins["fill"].clone());
}

#[test]
fn stroke_property_over_color_property() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "point", "color": "red", "stroke": "blue"},
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative"}
        }
    }));
    let (props, logger) = vega_lite::log::capture(|| color(&model, None));
    assert_eq_json(json!({"value": "blue"}), props["stroke"].clone());
    assert!(!logger.warns.is_empty());
}

#[test]
fn combines_color_with_fill() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "point", "color": "red", "fill": "blue"},
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative"}
        }
    }));
    assert_eq_json(json!({"value": "red"}), color(&model, None)["stroke"].clone());
}

#[test]
fn applies_color_property() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "point", "color": "red"},
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative"}
        }
    }));
    assert_eq_json(json!({"value": "red"}), color(&model, None)["stroke"].clone());
}

#[test]
fn mark_specific_color_over_general_mark_config() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "point",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative"}
        },
        "config": {"mark": {"color": "blue"}, "point": {"color": "red"}}
    }));
    assert_eq_json(json!({"value": "red"}), color(&model, None)["stroke"].clone());
}

#[test]
fn stroke_mark_config_over_color_mark_config() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "point",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative"}
        },
        "config": {"mark": {"color": "red", "stroke": "blue"}}
    }));
    assert_eq_json(json!({"value": "blue"}), color(&model, None)["stroke"].clone());
}

#[test]
fn stroke_mark_config_over_color_point_config() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "point",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative"}
        },
        "config": {"point": {"color": "red", "stroke": "blue"}}
    }));
    assert_eq_json(json!({"value": "blue"}), color(&model, None)["stroke"].clone());
}

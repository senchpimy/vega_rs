//! Port of `test/compile/legend/parse.test.ts`.

use serde_json::{json, Value};

use vega_lite::channel::{
    COLOR, FILLOPACITY, OPACITY, SHAPE, SIZE, STROKEOPACITY, STROKEWIDTH,
};
use vega_lite::channeldef::is_field_def;
use vega_lite::compile::legend::component::{legends_from_value, LegendComponent};
use vega_lite::compile::legend::parse::{parse_legend, parse_legend_for_channel};
use vega_lite::compile::model::Model;
use vega_lite::compile::support::init_config;
use vega_lite::type_::GEOJSON;

fn parse_unit_model_with_scale(spec: &Value) -> Model {
    let config = init_config(spec.get("config"));
    let mut model = Model::new_unit(spec, None, "", None, &config);
    model.parse_scale();
    model
}

fn parse_layer_model(spec: &Value) -> Model {
    let config = init_config(spec.get("config"));
    Model::new_layer(spec, None, "", None, &config)
}

fn component(model: &Model, channel: &str) -> Option<LegendComponent> {
    legends_from_value(&model.component.legends)
        .remove(channel)
}

#[test]
fn parse_unit_legend_should_return_correct_expression_for_time_unit_month() {
    let mut unit_model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {
            "x": {"field": "a", "type": "temporal"},
            "color": {"field": "a", "type": "temporal", "timeUnit": "month"}
        }
    }));

    parse_legend(&mut unit_model);
    let color = component(&unit_model, COLOR).expect("color legend");
    assert_eq!(
        color.get("format"),
        Some(json!({
            "signal": "timeUnitSpecifier([\"month\"], {\"year-month\":\"%b %Y \",\"year-month-date\":\"%b %d, %Y \"})"
        }))
    );
}

#[test]
fn parse_unit_legend_should_return_correct_expression_for_time_unit_quarter() {
    let mut unit_model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {
            "x": {"field": "a", "type": "temporal"},
            "color": {"field": "a", "type": "temporal", "timeUnit": "quarter"}
        }
    }));

    parse_legend(&mut unit_model);
    let color = component(&unit_model, COLOR).expect("color legend");
    assert_eq!(
        color.get("format"),
        Some(json!({
            "signal": "timeUnitSpecifier([\"quarter\"], {\"year-month\":\"%b %Y \",\"year-month-date\":\"%b %d, %Y \"})"
        }))
    );
}

#[test]
fn parse_unit_legend_should_not_produce_a_vega_legend_object_on_shape_with_type_geojson() {
    let spec = json!({
        "mark": "geoshape",
        "data": {"url": "data/income.json"},
        "transform": [
            {
                "lookup": "id",
                "from": {
                    "data": {
                        "url": "data/us-10m.json",
                        "format": {"type": "topojson", "feature": "states"}
                    },
                    "key": "id"
                },
                "as": "geo"
            }
        ],
        "encoding": {
            "shape": {"field": "geo", "type": "geojson"}
        }
    });

    let mut unit_model = parse_unit_model_with_scale(&spec);
    let channel_def = &unit_model.encoding().unwrap()[SHAPE];
    assert!(is_field_def(channel_def));
    assert_eq!(
        channel_def.get("type").and_then(Value::as_str),
        Some(GEOJSON.as_str())
    );

    parse_legend(&mut unit_model);
    assert!(component(&unit_model, SHAPE).is_none());
}

#[test]
fn parse_legend_for_channel_should_produce_vega_legend_object_for_color() {
    let mut model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {
            "x": {"field": "a", "type": "nominal"},
            "color": {"field": "a", "type": "quantitative"}
        }
    }));

    let def = parse_legend_for_channel(&mut model, COLOR).combine();
    assert_eq!(def.get("title"), Some(&json!("a")));
    assert_eq!(def.get("stroke"), Some(&json!("color")));
}

#[test]
fn parse_legend_for_channel_should_produce_vega_legend_object_for_trail_color() {
    let mut model = parse_unit_model_with_scale(&json!({
        "mark": "trail",
        "encoding": {
            "x": {"field": "a", "type": "nominal"},
            "color": {"field": "a", "type": "quantitative"}
        }
    }));

    let def = parse_legend_for_channel(&mut model, COLOR).combine();
    assert_eq!(def.get("title"), Some(&json!("a")));
    assert_eq!(def.get("stroke"), Some(&json!("color")));
}

#[test]
fn parse_legend_for_channel_should_produce_vega_legend_object_for_trail_size() {
    let mut model = parse_unit_model_with_scale(&json!({
        "mark": "trail",
        "encoding": {
            "x": {"field": "a", "type": "nominal"},
            "size": {"field": "a", "type": "quantitative"}
        }
    }));

    let def = parse_legend_for_channel(&mut model, SIZE).combine();
    assert_eq!(def.get("title"), Some(&json!("a")));
    assert_eq!(def.get("strokeWidth"), Some(&json!("size")));
}

#[test]
fn parse_legend_for_channel_should_produce_no_legend_title_when_title_is_null_empty_or_false() {
    for val in [Value::Null, json!(""), json!(false)] {
        let mut model = parse_unit_model_with_scale(&json!({
            "mark": "point",
            "encoding": {
                "x": {"field": "a", "type": "nominal"},
                "color": {
                    "field": "a",
                    "type": "quantitative",
                    "legend": {"title": val}
                }
            }
        }));

        let def = parse_legend_for_channel(&mut model, COLOR).combine();
        assert_eq!(def.get("title"), Some(&val));
    }
}

#[test]
fn parse_legend_for_channel_should_store_field_def_title_as_explicit() {
    let mut model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {
            "x": {"field": "a", "type": "nominal"},
            "color": {
                "field": "a",
                "type": "quantitative",
                "legend": {"title": "foo"}
            }
        }
    }));

    let def = parse_legend_for_channel(&mut model, COLOR).combine();
    assert_eq!(def.get("title"), Some(&json!("foo")));
}

fn channel_test(channel: &str) {
    let mut encoding = json!({
        "x": {"field": "a", "type": "nominal"}
    });
    encoding[channel] =
        json!({"field": "a", "type": if channel == "shape" { "nominal" } else { "quantitative" }});

    let spec = json!({
        "mark": "point",
        "encoding": encoding
    });

    let mut model = parse_unit_model_with_scale(&spec);
    let def = parse_legend_for_channel(&mut model, channel).combine();

    let channel_def = &model.encoding().unwrap()[channel];
    if is_field_def(channel_def) {
        assert_ne!(
            channel_def.get("type").and_then(Value::as_str),
            Some(GEOJSON.as_str())
        );
    }

    let opacity = def
        .get("encode")
        .and_then(|e| e.get("symbols"))
        .and_then(|s| s.get("update"))
        .and_then(|u| u.get("opacity"));
    if channel != OPACITY {
        assert_eq!(
            opacity.and_then(|o| o.get("value")),
            Some(&json!(0.7)),
            "channel {channel}"
        );
    } else {
        assert_eq!(opacity, None, "channel {channel}");
    }
    assert_eq!(def.get("title"), Some(&json!("a")), "channel {channel}");
}

#[test]
fn parse_legend_for_channel_size() {
    channel_test(SIZE);
}

#[test]
fn parse_legend_for_channel_shape() {
    channel_test(SHAPE);
}

#[test]
fn parse_legend_for_channel_opacity() {
    channel_test(OPACITY);
}

#[test]
fn parse_legend_for_channel_strokewidth() {
    channel_test(STROKEWIDTH);
}

fn opacity_channel_has_no_legend(channel: &str) {
    let mut encoding = json!({
        "x": {"field": "a", "type": "nominal"}
    });
    encoding[channel] = json!({"field": "a", "type": "quantitative"});

    let spec = json!({
        "mark": "point",
        "encoding": encoding
    });

    let model = parse_unit_model_with_scale(&spec);
    assert_eq!(
        vega_lite::compile::legend::parse::model_legend(&model, channel),
        None,
        "channel {channel}"
    );
}

#[test]
fn parse_legend_for_channel_fill_opacity_has_no_legend_initialized() {
    opacity_channel_has_no_legend(FILLOPACITY);
}

#[test]
fn parse_legend_for_channel_stroke_opacity_has_no_legend_initialized() {
    opacity_channel_has_no_legend(STROKEOPACITY);
}

#[test]
fn parse_non_unit_legend_should_correctly_merge_orient_by_favoring_explicit_orient() {
    let mut model = parse_layer_model(&json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
        "description": "Google's stock price over time.",
        "data": {"url": "data/stocks.csv"},
        "layer": [
            {
                "mark": "line",
                "encoding": {
                    "x": {"field": "date", "type": "temporal"},
                    "y": {"field": "price", "type": "quantitative"},
                    "color": {"field": "symbol", "type": "nominal"}
                }
            },
            {
                "mark": {"type": "point", "filled": true},
                "encoding": {
                    "x": {"field": "date", "type": "temporal"},
                    "y": {"field": "price", "type": "quantitative"},
                    "color": {"field": "symbol", "type": "nominal", "legend": {"orient": "left"}}
                }
            }
        ]
    }));
    model.parse_scale();
    parse_legend(&mut model);

    let color = component(&model, COLOR).expect("color legend");
    assert_eq!(color.explicit.get("orient"), Some(&json!("left")));
}

#[test]
fn parse_non_unit_legend_should_correctly_merge_legend_that_exists_only_on_one_plot() {
    let mut model = parse_layer_model(&json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
        "description": "Google's stock price over time.",
        "data": {"url": "data/stocks.csv"},
        "layer": [
            {
                "mark": "line",
                "encoding": {
                    "x": {"field": "date", "type": "temporal"},
                    "y": {"field": "price", "type": "quantitative"}
                }
            },
            {
                "mark": {"type": "point", "filled": true},
                "encoding": {
                    "x": {"field": "date", "type": "temporal"},
                    "y": {"field": "price", "type": "quantitative"},
                    "color": {"field": "symbol", "type": "nominal"}
                }
            }
        ]
    }));
    model.parse_scale();
    parse_legend(&mut model);

    assert!(component(&model, COLOR).is_some());
    assert!(component(&model.children[0], COLOR).is_none());
    assert!(component(&model.children[1], COLOR).is_none());
}

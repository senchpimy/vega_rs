//! Port of `test/compile/legend/encode.test.ts`.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use vega_lite::channel::COLOR;
use vega_lite::compile::buildmodel::parse_model;
use vega_lite::compile::legend::component::LegendComponent;
use vega_lite::compile::legend::encode::{self, LegendEncodeParams};
use vega_lite::compile::legend::properties::get_legend_type;
use vega_lite::compile::model::Model;
use vega_lite::compile::support::init_config;

fn symbol_legend() -> LegendComponent {
    let mut explicit = BTreeMap::new();
    explicit.insert("type".to_string(), json!("symbol"));
    LegendComponent::new(explicit, BTreeMap::new())
}

fn gradient_legend() -> LegendComponent {
    let mut explicit = BTreeMap::new();
    explicit.insert("type".to_string(), json!("gradient"));
    LegendComponent::new(explicit, BTreeMap::new())
}

fn empty_spec() -> Map<String, Value> {
    Map::new()
}

fn parse_unit_model_with_scale(spec: &Value) -> Model {
    let config = init_config(spec.get("config"));
    let mut model = Model::new_unit(spec, None, "", None, &config);
    model.parse_scale();
    model
}

fn legend_type_for_color() -> String {
    get_legend_type(&json!({}), COLOR, None, Some(vega_lite::scale::ScaleType::Ordinal))
}

#[test]
fn symbols_should_not_have_fill_stroke_dash_or_stroke_dash_offset() {
    let legend_type = legend_type_for_color();
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {
            "x": {"field": "a", "type": "nominal"},
            "color": {"field": "a", "type": "nominal"}
        }
    }));
    let field_or_datum_def = json!({"field": "a", "type": "nominal"});
    let symbol = encode::symbols(
        &empty_spec(),
        &LegendEncodeParams {
            field_or_datum_def: &field_or_datum_def,
            model: &model,
            channel: COLOR,
            legend_cmpt: &symbol_legend(),
            legend_type: &legend_type,
        },
    )
    .expect("symbol encode");

    assert_eq!(symbol.get("fill"), Some(&json!({"value": "transparent"})));
    assert_eq!(symbol.get("strokeDash"), None);
    assert_eq!(symbol.get("strokeDashOffset"), None);
}

#[test]
fn symbols_should_have_fill_if_a_color_encoding_exists() {
    let legend_type = legend_type_for_color();
    let model = parse_unit_model_with_scale(&json!({
        "mark": {"type": "circle", "opacity": 0.3},
        "encoding": {
            "x": {"field": "a", "type": "nominal"},
            "color": {"field": "a", "type": "nominal"},
            "size": {"field": "a", "type": "quantitative"}
        }
    }));
    let field_or_datum_def = json!({"field": "a", "type": "quantitative"});
    let symbol = encode::symbols(
        &empty_spec(),
        &LegendEncodeParams {
            field_or_datum_def: &field_or_datum_def,
            model: &model,
            channel: "size",
            legend_cmpt: &symbol_legend(),
            legend_type: &legend_type,
        },
    )
    .expect("symbol encode");

    assert_eq!(symbol.get("fill"), Some(&json!({"value": "black"})));
    assert_eq!(symbol.get("fillOpacity"), Some(&json!({"value": 0.3})));
}

#[test]
fn symbols_should_have_default_opacity() {
    let legend_type = legend_type_for_color();
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {
            "color": {"field": "a", "type": "nominal"}
        }
    }));
    let field_or_datum_def = json!({"field": "a", "type": "nominal"});
    let symbol = encode::symbols(
        &empty_spec(),
        &LegendEncodeParams {
            field_or_datum_def: &field_or_datum_def,
            model: &model,
            channel: COLOR,
            legend_cmpt: &symbol_legend(),
            legend_type: &legend_type,
        },
    )
    .expect("symbol encode");

    assert_eq!(symbol["opacity"]["value"], json!(0.7));
}

#[test]
fn symbols_should_use_symbol_opacity_when_set() {
    let mut explicit = BTreeMap::new();
    explicit.insert("type".to_string(), json!("symbol"));
    explicit.insert("symbolOpacity".to_string(), json!(0.9));
    let legend_cmpt = LegendComponent::new(explicit, BTreeMap::new());

    let legend_type = legend_type_for_color();
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {
            "color": {"field": "a", "type": "nominal"}
        }
    }));
    let field_or_datum_def = json!({"field": "a", "type": "nominal"});
    let symbol = encode::symbols(
        &empty_spec(),
        &LegendEncodeParams {
            field_or_datum_def: &field_or_datum_def,
            model: &model,
            channel: COLOR,
            legend_cmpt: &legend_cmpt,
            legend_type: &legend_type,
        },
    )
    .expect("symbol encode");

    assert_eq!(symbol.get("opacity"), None);
}

#[test]
fn symbols_should_return_the_maximum_value_when_there_is_a_condition() {
    let legend_type = legend_type_for_color();
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {
            "color": {"field": "a", "type": "nominal"},
            "opacity": {
                "condition": {"param": "brush", "value": 1},
                "value": 0
            }
        }
    }));
    let field_or_datum_def = json!({"field": "a", "type": "nominal"});
    let symbol = encode::symbols(
        &empty_spec(),
        &LegendEncodeParams {
            field_or_datum_def: &field_or_datum_def,
            model: &model,
            channel: COLOR,
            legend_cmpt: &symbol_legend(),
            legend_type: &legend_type,
        },
    )
    .expect("symbol encode");

    assert_eq!(symbol["opacity"]["value"], json!(1));
}

#[test]
fn gradient_should_have_default_opacity() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {
            "color": {"field": "a", "type": "nominal"}
        }
    }));
    let field_or_datum_def = json!({"field": "a", "type": "quantitative"});
    let gradient = encode::gradient(
        &empty_spec(),
        &LegendEncodeParams {
            field_or_datum_def: &field_or_datum_def,
            model: &model,
            channel: COLOR,
            legend_cmpt: &gradient_legend(),
            legend_type: "gradient",
        },
    )
    .expect("gradient encode");

    assert_eq!(gradient["opacity"]["value"], json!(0.7));
}

#[test]
fn labels_returns_correct_expression_for_custom_format_type() {
    let field_or_datum_def = json!({
        "field": "a",
        "type": "temporal",
        "legend": {"format": "abc", "formatType": "customDateFormat"}
    });
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"color": field_or_datum_def},
        "config": {"customFormatTypes": true}
    }));

    let label = encode::labels(
        &empty_spec(),
        &LegendEncodeParams {
            field_or_datum_def: &field_or_datum_def,
            model: &model,
            channel: COLOR,
            legend_cmpt: &symbol_legend(),
            legend_type: "symbol",
        },
    )
    .expect("labels encode");

    assert_eq!(
        label.get("text"),
        Some(&json!({"signal": "customDateFormat(datum.value, \"abc\")"}))
    );
}

#[test]
fn labels_returns_correct_expression_for_custom_format_type_from_config_number_format_type() {
    let field_or_datum_def = json!({"field": "a", "type": "quantitative"});
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"color": field_or_datum_def},
        "config": {
            "customFormatTypes": true,
            "numberFormat": "abc",
            "numberFormatType": "customDateFormat"
        }
    }));

    let label = encode::labels(
        &empty_spec(),
        &LegendEncodeParams {
            field_or_datum_def: &field_or_datum_def,
            model: &model,
            channel: COLOR,
            legend_cmpt: &symbol_legend(),
            legend_type: "symbol",
        },
    )
    .expect("labels encode");

    assert_eq!(
        label.get("text"),
        Some(&json!({"signal": "customDateFormat(datum.value, \"abc\")"}))
    );
}

#[test]
fn labels_returns_correct_expression_for_custom_format_type_from_config_time_format_type() {
    let field_or_datum_def = json!({"field": "a", "type": "temporal"});
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"color": field_or_datum_def},
        "config": {
            "customFormatTypes": true,
            "timeFormat": "abc",
            "timeFormatType": "customDateFormat"
        }
    }));

    let label = encode::labels(
        &empty_spec(),
        &LegendEncodeParams {
            field_or_datum_def: &field_or_datum_def,
            model: &model,
            channel: COLOR,
            legend_cmpt: &symbol_legend(),
            legend_type: "symbol",
        },
    )
    .expect("labels encode");

    assert_eq!(
        label.get("text"),
        Some(&json!({"signal": "customDateFormat(datum.value, \"abc\")"}))
    );
}

#[test]
fn labels_prefers_time_unit_over_config_time_format_type() {
    let field_or_datum_def = json!({"field": "a", "type": "temporal", "timeUnit": "date"});
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"color": field_or_datum_def},
        "config": {
            "customFormatTypes": true,
            "timeFormat": "abc",
            "timeFormatType": "customDateFormat"
        }
    }));

    let label = encode::labels(
        &empty_spec(),
        &LegendEncodeParams {
            field_or_datum_def: &field_or_datum_def,
            model: &model,
            channel: COLOR,
            legend_cmpt: &symbol_legend(),
            legend_type: "symbol",
        },
    );

    assert_eq!(label, None);
}

// `parse_model` is imported to make sure the normalizing entry point still
// compiles alongside the legend port.
#[allow(dead_code)]
fn _unused(spec: &Value) -> Model {
    let config = init_config(spec.get("config"));
    parse_model(spec, &config)
}

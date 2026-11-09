//! Port of `test/compile/selection/scales.test.ts`.

use serde_json::{json, Value};

use vega_lite::compile::buildmodel::build_model;
use vega_lite::compile::model::Model;
use vega_lite::compile::scale::domain::assemble_domain;
use vega_lite::compile::selection::assemble::{
    assemble_selection_scale_domain, assemble_top_level_signals, assemble_unit_selection_signals,
};
use vega_lite::compile::selection::message;
use vega_lite::compile::selection::parse::{parse_selections, parse_unit_selection};
use vega_lite::compile::support::init_config;
use vega_lite::log;
use vega_lite::normalize::normalize;

fn parse_unit_model_with_scale_and_selection(spec: &Value) -> Model {
    let config = init_config(spec.get("config"));
    let mut model = Model::new_unit(spec, None, "", None, &config);
    model.parse_scale();
    parse_selections(&mut model);
    model
}

fn parse_unit_only(spec: &Value) -> Model {
    let config = init_config(spec.get("config"));
    Model::new_unit(spec, None, "", None, &config)
}

fn parse_model(spec: &Value) -> Model {
    let config = init_config(spec.get("config"));
    let normalized = normalize(spec, Some(&config)).unwrap();
    build_model(&normalized, None, "", None, &config)
}

/// Local equivalent of `assembleScalesForModel`'s `domainRaw` branch (the scale
/// assembly module still has a placeholder for selection domains).
fn domain_raw(model: &Model, channel: &str) -> Value {
    let component = model.get_scale_component(channel).unwrap();
    let Some(extent) = component.get("selectionExtent") else {
        return Value::Null;
    };
    let domain = assemble_domain(model, channel);
    assemble_selection_scale_domain(model, &extent, &component, &domain)
}

#[test]
fn is_assembled_from_selection_parameter() {
    let spec = json!({
        "params": [{"name": "var"}],
        "vconcat": [
            {
                "mark": "area",
                "params": [
                    {"name": "brush", "select": {"type": "interval", "encodings": ["x"]}},
                    {"name": "brush2", "select": {"type": "point", "fields": ["price"], "resolve": "intersect"}}
                ],
                "encoding": {
                    "x": {"field": "date", "type": "temporal"},
                    "y": {"field": "price", "type": "quantitative"}
                }
            },
            {
                "params": [{"name": "brush3", "select": {"type": "point", "fields": ["symbol"]}}],
                "mark": "area",
                "encoding": {
                    "x": {"field": "date", "type": "temporal", "scale": {"domain": {"param": "brush", "encoding": "x"}}},
                    "y": {"field": "price", "type": "quantitative", "scale": {"domain": {"param": "brush2", "field": "price"}}},
                    "color": {"field": "symbol", "type": "nominal", "scale": {"domain": {"param": "brush3"}}},
                    "opacity": {"field": "symbol", "type": "ordinal", "scale": {"domain": {"param": "var"}}}
                }
            }
        ],
        "resolve": {"scale": {"color": "independent", "opacity": "independent"}}
    });

    let mut model = parse_model(&spec);
    model.parse_scale();
    parse_selections(&mut model);

    let child = &model.children[1];
    assert_eq!(domain_raw(child, "x"), json!({"signal": "brush[\"date\"]"}));
    assert_eq!(
        domain_raw(child, "y"),
        json!({"signal": "brush2[\"price\"]"})
    );
    assert_eq!(
        domain_raw(child, "color"),
        json!({"signal": "brush3[\"symbol\"]"})
    );
    assert_eq!(domain_raw(child, "opacity"), json!({"signal": "var"}));
}

#[test]
fn should_be_merged_for_layered_views() {
    let spec = json!({
        "data": {"url": "data/sp500.csv"},
        "vconcat": [
            {
                "layer": [{
                    "mark": "point",
                    "encoding": {
                        "x": {"field": "date", "type": "temporal", "scale": {"domain": {"param": "brush"}}},
                        "y": {"field": "price", "type": "quantitative"}
                    }
                }]
            },
            {
                "mark": "area",
                "params": [{"name": "brush", "select": {"type": "interval", "encodings": ["x"]}}],
                "encoding": {
                    "x": {"field": "date", "type": "temporal"},
                    "y": {"field": "price", "type": "quantitative"}
                }
            }
        ]
    });

    let mut model = parse_model(&spec);
    model.parse_scale();
    parse_selections(&mut model);
    assert_eq!(
        domain_raw(&model.children[0], "x"),
        json!({"signal": "brush[\"date\"]"})
    );
}

#[test]
fn should_handle_nested_field_references() {
    let mut model = parse_unit_model_with_scale_and_selection(&json!({
        "params": [{"name": "grid", "select": "interval", "bind": "scales"}],
        "data": {"values": [{"nested": {"a": "1", "b": 28}}, {"nested": {"a": "2", "b": 55}}, {"nested": {"a": "3", "b": 43}}]},
        "mark": "point",
        "encoding": {
            "y": {"field": "nested.a", "type": "quantitative"},
            "x": {"field": "nested.b", "type": "quantitative"}
        }
    }));

    assert_eq!(
        domain_raw(&model, "x"),
        json!({"signal": "grid[\"nested\\\\.b\"]"})
    );
    assert_eq!(
        domain_raw(&model, "y"),
        json!({"signal": "grid[\"nested\\\\.a\"]"})
    );

    let spec = json!({
        "vconcat": [
            {
                "mark": "area",
                "params": [{"name": "brush", "select": {"type": "interval", "encodings": ["x"]}}],
                "encoding": {
                    "x": {"field": "nested.a", "type": "temporal"},
                    "y": {"field": "price", "type": "quantitative"}
                }
            },
            {
                "mark": "area",
                "encoding": {
                    "x": {"field": "date", "type": "temporal", "scale": {"domain": {"param": "brush", "encoding": "x"}}},
                    "y": {"field": "price", "type": "quantitative"}
                }
            },
            {
                "mark": "area",
                "encoding": {
                    "x": {"field": "date", "type": "temporal", "scale": {"domain": {"param": "brush", "field": "nested.a"}}},
                    "y": {"field": "price", "type": "quantitative"}
                }
            }
        ],
        "resolve": {"scale": {"color": "independent", "opacity": "independent"}}
    });

    model = parse_model(&spec);
    model.parse_scale();
    parse_selections(&mut model);

    assert_eq!(
        domain_raw(&model.children[1], "x"),
        json!({"signal": "brush[\"nested\\\\.a\"]"})
    );
    assert_eq!(
        domain_raw(&model.children[2], "x"),
        json!({"signal": "brush[\"nested\\\\.a\"]"})
    );
}

#[test]
fn should_respect_ordering_of_explicit_scale_domains() {
    let model = parse_unit_model_with_scale_and_selection(&json!({
        "mark": "point",
        "encoding": {
            "x": {"type": "quantitative", "field": "Horsepower", "scale": {"domain": [250, 0]}},
            "y": {"type": "quantitative", "field": "Miles_per_Gallon"}
        },
        "params": [{"name": "pan", "select": "interval", "bind": "scales"}]
    }));

    assert_eq!(
        domain_raw(&model, "x"),
        json!({"signal": "isValid(pan[\"Horsepower\"]) && reverse(pan[\"Horsepower\"])"})
    );
}

#[test]
fn signals_should_be_marked_as_push_outer() {
    let spec = json!({
        "repeat": {
            "row": ["Horsepower", "Acceleration"],
            "column": ["Miles_per_Gallon", "Acceleration"]
        },
        "spec": {
            "data": {"url": "data/cars.json"},
            "mark": "point",
            "params": [{"name": "grid", "select": {"type": "interval", "resolve": "global"}, "bind": "scales"}],
            "encoding": {
                "x": {"field": {"repeat": "column"}, "type": "quantitative"},
                "y": {"field": {"repeat": "row"}, "type": "quantitative"},
                "color": {"field": "Origin", "type": "nominal"}
            }
        }
    });
    let mut model = parse_model(&spec);
    model.parse_scale();
    parse_selections(&mut model);

    let signals = assemble_unit_selection_signals(&model.children[0], vec![]);
    for name in ["grid_Horsepower", "grid_Miles_per_Gallon"] {
        let found: Vec<&Value> = signals
            .iter()
            .filter(|s| s.get("name").and_then(Value::as_str) == Some(name))
            .collect();
        assert_eq!(found.len(), 1, "{name}");
        assert_eq!(found[0].get("push"), Some(&json!("outer")));
        assert!(found[0].get("value").is_none());
        assert!(found[0].get("update").is_none());
    }
}

#[test]
fn signals_should_be_assembled_at_the_top_level() {
    let repeat_spec = json!({
        "repeat": {
            "row": ["Horsepower", "Acceleration"],
            "column": ["Miles_per_Gallon", "Acceleration"]
        },
        "spec": {
            "data": {"url": "data/cars.json"},
            "mark": "point",
            "params": [{"name": "grid", "select": {"type": "interval", "resolve": "global"}, "bind": "scales"}],
            "encoding": {
                "x": {"field": {"repeat": "column"}, "type": "quantitative"},
                "y": {"field": {"repeat": "row"}, "type": "quantitative"},
                "color": {"field": "Origin", "type": "nominal"}
            }
        }
    });
    let mut repeat_model = parse_model(&repeat_spec);
    repeat_model.parse_scale();
    parse_selections(&mut repeat_model);

    let signals = assemble_top_level_signals(&repeat_model.children[0], vec![]);
    assert_eq!(
        signals
            .iter()
            .filter(|s| s.get("name").and_then(Value::as_str) == Some("grid_Horsepower"))
            .count(),
        1
    );
    let named: Vec<&Value> = signals
        .iter()
        .filter(|s| s.get("name").and_then(Value::as_str) == Some("grid"))
        .collect();
    assert_eq!(named.len(), 1);
    assert_eq!(
        named[0]["update"],
        json!("{\"Miles_per_Gallon\": grid_Miles_per_Gallon, \"Horsepower\": grid_Horsepower}")
    );

    let signals2 = assemble_top_level_signals(&repeat_model.children[1], signals);
    assert_eq!(
        signals2
            .iter()
            .filter(|s| s.get("name").and_then(Value::as_str) == Some("grid_Acceleration"))
            .count(),
        1
    );
    let named2: Vec<&Value> = signals2
        .iter()
        .filter(|s| s.get("name").and_then(Value::as_str) == Some("grid"))
        .collect();
    assert_eq!(named2.len(), 1);
    assert_eq!(
        named2[0]["update"],
        json!("{\"Miles_per_Gallon\": grid_Miles_per_Gallon, \"Horsepower\": grid_Horsepower, \"Acceleration\": grid_Acceleration}")
    );

    let concat_spec = json!({
        "data": {"url": "data/cars.json"},
        "hconcat": [
            {
                "mark": "point",
                "encoding": {
                    "x": {"type": "quantitative", "field": "Miles_per_Gallon"},
                    "y": {"type": "quantitative", "field": "Weight_in_lbs"}
                },
                "params": [{"name": "selector001", "select": "interval", "bind": "scales"}]
            },
            {
                "mark": "point",
                "encoding": {
                    "x": {"type": "quantitative", "field": "Acceleration"},
                    "y": {"type": "quantitative", "field": "Horsepower"}
                },
                "params": [{"name": "selector001", "select": {"type": "interval"}, "bind": "scales"}]
            }
        ]
    });
    let mut concat_model = parse_model(&concat_spec);
    concat_model.parse_scale();
    parse_selections(&mut concat_model);

    let signals3 = assemble_top_level_signals(
        &concat_model.children[1],
        assemble_top_level_signals(&concat_model.children[0], vec![]),
    );
    let named_selector: Vec<&Value> = signals3
        .iter()
        .filter(|s| s.get("name").and_then(Value::as_str) == Some("selector001"))
        .collect();
    assert_eq!(
        named_selector[0]["update"],
        json!("{\"Miles_per_Gallon\": selector001_Miles_per_Gallon, \"Weight_in_lbs\": selector001_Weight_in_lbs, \"Acceleration\": selector001_Acceleration, \"Horsepower\": selector001_Horsepower}")
    );
}

#[test]
fn should_correctly_nest_fields_for_top_level_signals() {
    let spec = json!({
        "repeat": {"row": ["p.x", "p.y"], "column": ["p.x", "p.y"]},
        "spec": {
            "mark": "point",
            "params": [{"name": "sel11", "select": "interval", "bind": "scales"}],
            "encoding": {
                "x": {"field": {"repeat": "column"}, "type": "quantitative"},
                "y": {"field": {"repeat": "row"}, "type": "quantitative"}
            },
            "data": {"values": [{"p": {"x": 1, "y": 1}}, {"p": {"x": 2, "y": 1}}, {"p": {"x": 1, "y": 2}}, {"p": {"x": 3, "y": 3}}, {"p": {"x": 3, "y": 2}}]}
        }
    });

    let mut model = parse_model(&spec);
    model.parse_scale();
    parse_selections(&mut model);

    let signals = assemble_top_level_signals(&model.children[2], vec![]);
    let named: Vec<&Value> = signals
        .iter()
        .filter(|s| s.get("name").and_then(Value::as_str) == Some("sel11"))
        .collect();
    assert_eq!(named.len(), 1);
    assert_eq!(
        named[0]["update"],
        json!("{\"p\\\\.x\": sel11_p_x, \"p\\\\.y\": sel11_p_y}")
    );
}

#[test]
fn should_not_bind_for_unavailable_or_unsupported_scales() {
    let mut model = parse_unit_only(&json!({
        "data": {"url": "data/cars.json"},
        "params": [{"name": "grid", "select": "interval", "bind": "scales"}],
        "mark": "circle",
        "encoding": {"y": {"field": "Miles_per_Gallon", "type": "quantitative"}}
    }));
    model.parse_scale();
    let (_, logger) = log::capture(|| {
        let components = parse_unit_selection(
            &mut model,
            &[json!({"name": "grid", "select": "interval", "bind": "scales"})],
        );
        model.component.selection = serde_json::to_value(&components).unwrap();
    });
    assert_eq!(
        logger.warns[0],
        json!(vega_lite::compile::selection::message::cannot_project_on_channel_without_field("x"))
    );

    let mut model2 = parse_unit_only(&json!({
        "data": {"url": "data/cars.json"},
        "params": [{"name": "grid", "select": {"type": "interval"}, "bind": "scales"}],
        "mark": "circle",
        "encoding": {
            "x": {"field": "Origin", "type": "nominal"},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative"}
        }
    }));
    model2.parse_scale();
    let (_, logger2) = log::capture(|| {
        let components = parse_unit_selection(
            &mut model2,
            &[json!({"name": "grid", "select": {"type": "interval"}, "bind": "scales"})],
        );
        model2.component.selection = serde_json::to_value(&components).unwrap();
    });
    assert_eq!(logger2.warns[0], json!(message::SCALE_BINDINGS_CONTINUOUS));
}

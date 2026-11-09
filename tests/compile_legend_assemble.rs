//! Port of `test/compile/legend/assemble.test.ts`.

use serde_json::{json, Value};

use vega_lite::compile::legend::assemble::assemble_legends;
use vega_lite::compile::legend::parse::parse_legend;
use vega_lite::compile::model::Model;
use vega_lite::compile::support::init_config;
use vega_lite::log;

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

fn labels_text(legend: &Value) -> Option<&Value> {
    legend
        .get("encode")
        .and_then(|e| e.get("labels"))
        .and_then(|l| l.get("update"))
        .and_then(|u| u.get("text"))
}

#[test]
fn correctly_applies_label_expr() {
    let mut model = parse_unit_model_with_scale(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "point",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative"},
            "color": {
                "field": "Origin",
                "type": "nominal",
                "legend": {"labelExpr": "datum.label[0]"}
            }
        }
    }));
    parse_legend(&mut model);

    let legends = assemble_legends(&model);
    assert_eq!(labels_text(&legends[0]), Some(&json!({"signal": "datum.label[0]"})));
}

#[test]
fn correctly_applies_custom_formatter_to_label_expr() {
    let mut model = parse_unit_model_with_scale(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "point",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative"},
            "color": {
                "field": "Origin",
                "type": "nominal",
                "legend": {
                    "format": {"a": "b"},
                    "formatType": "myFormat",
                    "labelExpr": "datum.label[0]"
                }
            }
        },
        "config": {"customFormatTypes": true}
    }));
    parse_legend(&mut model);

    let legends = assemble_legends(&model);
    assert_eq!(
        labels_text(&legends[0]),
        Some(&json!({"signal": "myFormat(datum.value, {\"a\":\"b\"})[0]"}))
    );
}

#[test]
fn correctly_applies_label_expr_for_time_unit() {
    let mut model = parse_unit_model_with_scale(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "point",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative"},
            "color": {
                "timeUnit": "month",
                "field": "date",
                "type": "temporal",
                "legend": {"labelExpr": "datum.label[0]"}
            }
        }
    }));
    parse_legend(&mut model);

    let legends = assemble_legends(&model);
    assert_eq!(labels_text(&legends[0]), Some(&json!({"signal": "datum.label[0]"})));
}

#[test]
fn merges_legend_of_the_same_field_with_the_default_type() {
    let mut model = parse_unit_model_with_scale(&json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
        "data": {"url": "data/cars.json"},
        "mark": "point",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative"},
            "color": {"field": "Origin", "type": "nominal"},
            "shape": {"field": "Origin", "type": "nominal"}
        }
    }));
    parse_legend(&mut model);

    let legends = assemble_legends(&model);
    assert_eq!(legends.len(), 1);
    assert_eq!(legends[0].get("title"), Some(&json!("Origin")));
    assert_eq!(legends[0].get("stroke"), Some(&json!("color")));
}

#[test]
fn merges_legend_of_the_same_field_and_favors_symbol_legend_over_gradient() {
    let mut model = parse_unit_model_with_scale(&json!({
        "data": {
            "values": [
                {"a": "A", "b": 28},
                {"a": "B", "b": 55}
            ]
        },
        "mark": "bar",
        "encoding": {
            "x": {"field": "a", "type": "ordinal"},
            "y": {"field": "b", "type": "quantitative"},
            "color": {"field": "b", "type": "quantitative"},
            "size": {"field": "b", "type": "quantitative"}
        }
    }));

    parse_legend(&mut model);
    let legends = assemble_legends(&model);
    assert_eq!(legends.len(), 1);
    assert_eq!(legends[0].get("title"), Some(&json!("b")));
    assert_eq!(legends[0].get("fill"), Some(&json!("color")));
    assert_eq!(legends[0].get("size"), Some(&json!("size")));
    assert_eq!(
        legends[0]
            .get("encode")
            .and_then(|e| e.get("symbols"))
            .and_then(|s| s.get("update"))
            .and_then(|u| u.get("fill")),
        None
    );
}

#[test]
fn sets_aria_to_false_if_set_in_config() {
    let mut model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {
            "color": {"field": "b", "type": "quantitative"}
        },
        "config": {"aria": false}
    }));

    parse_legend(&mut model);
    let legends = assemble_legends(&model);
    assert_eq!(legends.len(), 1);
    assert_eq!(legends[0].get("aria"), Some(&json!(false)));
}

#[test]
fn does_not_merge_legends_for_different_discrete_fields() {
    let mut model = parse_unit_model_with_scale(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "point",
        "encoding": {
            "color": {"field": "Origin", "type": "nominal"},
            "shape": {"field": "Cylinders", "type": "ordinal"}
        }
    }));

    parse_legend(&mut model);
    let legends = assemble_legends(&model);
    assert_eq!(legends.len(), 2);
    let titles: Vec<Value> = legends
        .iter()
        .map(|l| l.get("title").cloned().unwrap_or(Value::Null))
        .collect();
    assert!(titles.contains(&json!("Origin")));
    assert!(titles.contains(&json!("Cylinders")));
}

#[test]
fn does_not_merge_legends_when_same_field_is_discrete_vs_continuous() {
    let mut model = parse_unit_model_with_scale(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "point",
        "encoding": {
            "color": {"field": "Horsepower", "type": "nominal"},
            "size": {"field": "Horsepower", "type": "quantitative"}
        }
    }));

    parse_legend(&mut model);
    let legends = assemble_legends(&model);
    assert_eq!(legends.len(), 2);
}

#[test]
fn merges_same_discrete_field_and_unions_array_domains() {
    let mut model = parse_unit_model_with_scale(&json!({
        "data": {
            "values": [{"cat": "A"}, {"cat": "B"}, {"cat": "C"}]
        },
        "mark": "point",
        "encoding": {
            "color": {"field": "cat", "type": "nominal", "scale": {"domain": ["A", "B"]}},
            "shape": {"field": "cat", "type": "nominal", "scale": {"domain": ["B", "C"]}}
        }
    }));

    let (legends, local_logger) = log::capture(|| {
        parse_legend(&mut model);
        assemble_legends(&model)
    });

    assert!(local_logger.warns.contains(&json!(
        "Unioning discrete legend values from color and shape."
    )));
    assert_eq!(legends.len(), 1);
    assert_eq!(legends[0].get("values"), Some(&json!(["A", "B", "C"])));
}

#[test]
fn preserves_explicit_legend_values_over_unioned_domains() {
    let mut model = parse_unit_model_with_scale(&json!({
        "data": {
            "values": [{"cat": "A"}, {"cat": "B"}, {"cat": "C"}]
        },
        "mark": "point",
        "encoding": {
            "color": {
                "field": "cat",
                "type": "nominal",
                "scale": {"domain": ["A", "B"]},
                "legend": {"values": ["B", "A"]}
            },
            "shape": {"field": "cat", "type": "nominal", "scale": {"domain": ["B", "C"]}}
        }
    }));

    parse_legend(&mut model);
    let legends = assemble_legends(&model);
    assert_eq!(legends.len(), 1);
    assert_eq!(legends[0].get("values"), Some(&json!(["B", "A"])));
}

#[test]
fn keeps_same_array_when_both_discrete_domains_are_equal() {
    let mut model = parse_unit_model_with_scale(&json!({
        "data": {
            "values": [{"cat": "A"}, {"cat": "B"}]
        },
        "mark": "point",
        "encoding": {
            "color": {"field": "cat", "type": "nominal", "scale": {"domain": ["A", "B"]}},
            "shape": {"field": "cat", "type": "nominal", "scale": {"domain": ["A", "B"]}}
        }
    }));

    parse_legend(&mut model);
    let legends = assemble_legends(&model);
    assert_eq!(legends.len(), 1);
    assert_eq!(legends[0].get("values"), Some(&json!(["A", "B"])));
}

#[test]
fn merges_discrete_field_without_arrays_and_does_not_set_values() {
    let mut model = parse_unit_model_with_scale(&json!({
        "data": {
            "values": [{"cat": "A"}, {"cat": "B"}, {"cat": "C"}]
        },
        "mark": "point",
        "encoding": {
            "color": {"field": "cat", "type": "nominal"},
            "shape": {"field": "cat", "type": "nominal"}
        }
    }));

    parse_legend(&mut model);
    let legends = assemble_legends(&model);
    assert_eq!(legends.len(), 1);
    assert_eq!(legends[0].get("values"), None);
}

#[test]
fn does_not_merge_different_fields_even_if_discrete_domain_arrays_are_identical() {
    let mut model = parse_unit_model_with_scale(&json!({
        "data": {
            "values": [{"cat1": "A", "cat2": "A"}]
        },
        "mark": "point",
        "encoding": {
            "color": {"field": "cat1", "type": "nominal", "scale": {"domain": ["A", "B"]}},
            "shape": {"field": "cat2", "type": "nominal", "scale": {"domain": ["A", "B"]}}
        }
    }));

    parse_legend(&mut model);
    let legends = assemble_legends(&model);
    assert_eq!(legends.len(), 2);
}

#[test]
fn does_not_merge_when_explicit_orient_conflicts_for_the_same_field() {
    let mut model = parse_unit_model_with_scale(&json!({
        "data": {"url": "data/cars.json"},
        "mark": "point",
        "encoding": {
            "color": {"field": "Origin", "type": "nominal", "legend": {"orient": "left"}},
            "shape": {"field": "Origin", "type": "nominal", "legend": {"orient": "right"}}
        }
    }));

    parse_legend(&mut model);
    let legends = assemble_legends(&model);
    assert_eq!(legends.len(), 2);
}

#[test]
fn merges_continuous_same_field_without_setting_values() {
    let mut model = parse_unit_model_with_scale(&json!({
        "data": {
            "values": [{"v": 1}, {"v": 2}]
        },
        "mark": "trail",
        "encoding": {
            "color": {"field": "v", "type": "quantitative"},
            "size": {"field": "v", "type": "quantitative"}
        }
    }));

    parse_legend(&mut model);
    let legends = assemble_legends(&model);
    assert_eq!(legends.len(), 1);
    assert_eq!(legends[0].get("values"), None);
}

#[test]
fn layer_groups_by_child_field_when_shared_and_unions_values_from_unioned_domains() {
    let mut model = parse_layer_model(&json!({
        "layer": [
            {
                "mark": "point",
                "encoding": {
                    "color": {"field": "cat", "type": "nominal", "scale": {"domain": ["A", "B"]}},
                    "shape": {"field": "cat", "type": "nominal", "scale": {"domain": ["A", "C"]}}
                }
            },
            {
                "mark": "point",
                "encoding": {
                    "color": {"field": "cat", "type": "nominal", "scale": {"domain": ["B", "D"]}},
                    "shape": {"field": "cat", "type": "nominal", "scale": {"domain": ["C", "E"]}}
                }
            }
        ]
    }));

    model.parse_scale();
    parse_legend(&mut model);

    let legends = assemble_legends(&model);
    assert_eq!(legends.len(), 1);
    assert_eq!(legends[0].get("title"), Some(&json!("cat")));
    let values = legends[0]
        .get("values")
        .and_then(Value::as_array)
        .cloned()
        .expect("values");
    for value in ["A", "B", "C", "D", "E"] {
        assert!(values.contains(&json!(value)), "missing {value}: {values:?}");
    }
    assert_eq!(values.len(), 5);
}

#[test]
fn layer_ambiguous_child_fields_fallback_to_channel_grouping() {
    let mut model = parse_layer_model(&json!({
        "layer": [
            {
                "mark": "point",
                "encoding": {
                    "color": {"field": "a", "type": "nominal"}
                }
            },
            {
                "mark": "point",
                "encoding": {
                    "color": {"field": "b", "type": "nominal"}
                }
            }
        ]
    }));

    model.parse_scale();
    parse_legend(&mut model);

    let legends = assemble_legends(&model);
    assert_eq!(legends.len(), 1);
    assert_eq!(legends[0].get("title"), Some(&json!("a, b")));
}

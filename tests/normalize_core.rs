//! Port of `test/normalize/core.test.ts`.

use serde_json::{json, Map, Value};
use vega_lite::log;
use vega_lite::normalize::normalize;

fn spec_with_channel(channel: &str) -> Value {
    let mut encoding = Map::new();
    encoding.insert(
        channel.to_string(),
        json!({"field": "MPAA_Rating", "type": "ordinal", "spacing": 13}),
    );
    encoding.insert(
        "x".to_string(),
        json!({"field": "Worldwide_Gross", "type": "quantitative"}),
    );
    encoding.insert(
        "y".to_string(),
        json!({"field": "US_DVD_Sales", "type": "quantitative"}),
    );
    json!({
        "name": "faceted",
        "width": 123,
        "height": 234,
        "description": "faceted spec",
        "data": {"url": "data/movies.json"},
        "mark": "point",
        "encoding": Value::Object(encoding),
    })
}

#[test]
fn throws_errors_for_invalid_spec() {
    let result = normalize(&json!({}), None);
    assert_eq!(result, Err(log::message::invalid_spec(&json!({}))));
}

#[test]
fn should_ignore_columns_from_repeat_with_row_column() {
    let spec = json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
        "repeat": {"column": ["Horsepower", "Miles_per_Gallon", "Acceleration", "Displacement"]},
        "columns": 2,
        "spec": {
            "data": {"url": "data/cars.json"},
            "mark": "bar",
            "encoding": {
                "x": {"field": {"repeat": "column"}, "bin": true, "type": "quantitative"},
                "y": {"aggregate": "count", "type": "quantitative"},
                "color": {"field": "Origin", "type": "nominal"}
            }
        }
    });
    let (normalized, local_logger) = log::capture(|| normalize(&spec, None));
    let normalized = normalized.unwrap();
    assert_eq!(normalized["columns"], json!(4));
    assert_eq!(
        local_logger.warns[0],
        json!("The \"columns\" property cannot be used when \"repeat\" has nested row/column.")
    );
}

#[test]
fn normalizes_repeat_layer() {
    let spec = json!({
        "data": {"url": "data/movies.json"},
        "repeat": {"layer": ["US_Gross", "Worldwide_Gross"]},
        "spec": {
            "mark": "line",
            "encoding": {
                "x": {"bin": true, "field": "IMDB_Rating", "type": "quantitative"},
                "y": {"aggregate": "mean", "field": {"repeat": "layer"}, "type": "quantitative"}
            }
        }
    });
    let normalized = normalize(&spec, None).unwrap();
    assert_eq!(normalized["layer"].as_array().unwrap().len(), 2);
}

#[test]
fn should_convert_single_extended_spec_with_facet_into_composite_spec() {
    assert_facet_channel("facet");
}

#[test]
fn should_convert_single_extended_spec_with_row_into_composite_spec() {
    assert_facet_channel("row");
}

#[test]
fn should_convert_single_extended_spec_with_column_into_composite_spec() {
    assert_facet_channel("column");
}

fn assert_facet_channel(channel: &str) {
    let spec = spec_with_channel(channel);
    let field_def = json!({"field": "MPAA_Rating", "type": "ordinal"});
    let expected_facet = if channel == "facet" {
        field_def.clone()
    } else {
        json!({channel: field_def})
    };
    let spacing = if channel == "facet" {
        json!(13)
    } else {
        json!({channel: 13})
    };
    assert_eq!(
        normalize(&spec, None).unwrap(),
        json!({
            "name": "faceted",
            "description": "faceted spec",
            "data": {"url": "data/movies.json"},
            "spacing": spacing,
            "facet": expected_facet,
            "spec": {
                "mark": "point",
                "width": 123,
                "height": 234,
                "encoding": {
                    "x": {"field": "Worldwide_Gross", "type": "quantitative"},
                    "y": {"field": "US_DVD_Sales", "type": "quantitative"}
                }
            }
        })
    );
}

#[test]
fn should_drop_facet_if_row_is_also_specified() {
    assert_drop_facet("row");
}

#[test]
fn should_drop_facet_if_column_is_also_specified() {
    assert_drop_facet("column");
}

fn assert_drop_facet(channel: &str) {
    let mut encoding = Map::new();
    encoding.insert(
        channel.to_string(),
        json!({"field": "MPAA_Rating", "type": "ordinal"}),
    );
    encoding.insert("facet".to_string(), json!({"field": "todrop", "type": "ordinal"}));
    encoding.insert(
        "x".to_string(),
        json!({"field": "Worldwide_Gross", "type": "quantitative"}),
    );
    encoding.insert(
        "y".to_string(),
        json!({"field": "US_DVD_Sales", "type": "quantitative"}),
    );
    let spec = json!({
        "data": {"url": "data/movies.json"},
        "mark": "point",
        "encoding": Value::Object(encoding),
    });
    let (normalized, local_logger) = log::capture(|| normalize(&spec, None));
    assert_eq!(
        normalized.unwrap(),
        json!({
            "data": {"url": "data/movies.json"},
            "facet": {channel: {"field": "MPAA_Rating", "type": "ordinal"}},
            "spec": {
                "mark": "point",
                "encoding": {
                    "x": {"field": "Worldwide_Gross", "type": "quantitative"},
                    "y": {"field": "US_DVD_Sales", "type": "quantitative"}
                }
            }
        })
    );
    assert_eq!(
        local_logger.warns[0],
        json!(format!(
            "Facet encoding dropped as {} is also specified.",
            channel
        ))
    );
}

#[test]
fn should_drop_columns_from_facet_with_row_column() {
    let spec = json!({
        "data": {"url": "data/cars.json"},
        "facet": {"column": {"field": "a", "type": "nominal"}},
        "columns": 2,
        "spec": {
            "mark": "bar",
            "encoding": {
                "x": {"field": {"repeat": "column"}, "bin": true, "type": "quantitative"},
                "y": {"aggregate": "count", "type": "quantitative"},
                "color": {"field": "Origin", "type": "nominal"}
            }
        }
    });
    let (normalized, local_logger) = log::capture(|| normalize(&spec, None));
    let normalized = normalized.unwrap();
    assert!(normalized.get("columns").is_none());
    assert_eq!(
        local_logger.warns[0],
        json!("The \"columns\" property cannot be used when \"facet\" has nested row/column.")
    );
}

#[test]
fn should_produce_correct_layered_specs_for_mean_point_and_vertical_error_bar() {
    let spec = json!({
        "description": "A error bar plot showing mean, min, and max in the US population distribution of age groups in 2000.",
        "data": {"url": "data/population.json"},
        "transform": [{"calculate": "(datum.sex==1) ? 'Men':'Women'", "as": "sex"}],
        "facet": {"row": {"field": "sex", "type": "ordinal"}},
        "spec": {
            "layer": [
                {
                    "mark": "errorbar",
                    "encoding": {
                        "x": {"field": "age", "type": "ordinal"},
                        "y": {"field": "people", "type": "quantitative"}
                    }
                },
                {
                    "mark": {"type": "point", "opacity": 1, "filled": true},
                    "encoding": {
                        "x": {"field": "age", "type": "ordinal"},
                        "y": {"field": "people", "type": "quantitative", "aggregate": "mean"}
                    }
                }
            ]
        }
    });

    assert_eq!(
        normalize(&spec, None).unwrap(),
        json!({
            "description": "A error bar plot showing mean, min, and max in the US population distribution of age groups in 2000.",
            "data": {"url": "data/population.json"},
            "transform": [{"calculate": "(datum.sex==1) ? 'Men':'Women'", "as": "sex"}],
            "facet": {"row": {"field": "sex", "type": "ordinal"}},
            "spec": {
                "layer": [
                    {
                        "transform": [
                            {
                                "aggregate": [
                                    {"op": "stderr", "field": "people", "as": "extent_people"},
                                    {"op": "mean", "field": "people", "as": "center_people"}
                                ],
                                "groupby": ["age"]
                            },
                            {
                                "calculate": "datum['center_people'] + datum['extent_people']",
                                "as": "upper_people"
                            },
                            {
                                "calculate": "datum['center_people'] - datum['extent_people']",
                                "as": "lower_people"
                            }
                        ],
                        "mark": {"type": "rule", "ariaRoleDescription": "errorbar", "style": "errorbar-rule"},
                        "encoding": {
                            "y": {"field": "lower_people", "type": "quantitative", "title": "people"},
                            "y2": {"field": "upper_people"},
                            "x": {"field": "age", "type": "ordinal"},
                            "tooltip": [
                                {"field": "center_people", "title": "Mean of people", "type": "quantitative"},
                                {"field": "upper_people", "title": "Mean + stderr of people", "type": "quantitative"},
                                {"field": "lower_people", "title": "Mean - stderr of people", "type": "quantitative"},
                                {"field": "age", "type": "ordinal"}
                            ]
                        }
                    },
                    {
                        "mark": {"type": "point", "opacity": 1, "filled": true},
                        "encoding": {
                            "x": {"field": "age", "type": "ordinal"},
                            "y": {"field": "people", "type": "quantitative", "aggregate": "mean"}
                        }
                    }
                ]
            }
        })
    );
}

#[test]
fn correctly_passes_shared_projection_and_encoding_to_children_of_layer() {
    let output = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "projection": {"type": "mercator"},
            "encoding": {"x": {"field": "age", "type": "ordinal"}},
            "layer": [
                {"mark": "point"},
                {
                    "layer": [
                        {"mark": "rule"},
                        {"mark": "text", "encoding": {"text": {"field": "a", "type": "nominal"}}}
                    ]
                }
            ]
        }),
        None,
    )
    .unwrap();

    assert_eq!(
        output,
        json!({
            "data": {"url": "data/population.json"},
            "layer": [
                {
                    "projection": {"type": "mercator"},
                    "mark": "point",
                    "encoding": {"x": {"field": "age", "type": "ordinal"}}
                },
                {
                    "layer": [
                        {
                            "projection": {"type": "mercator"},
                            "mark": "rule",
                            "encoding": {"x": {"field": "age", "type": "ordinal"}}
                        },
                        {
                            "projection": {"type": "mercator"},
                            "mark": "text",
                            "encoding": {
                                "x": {"field": "age", "type": "ordinal"},
                                "text": {"field": "a", "type": "nominal"}
                            }
                        }
                    ]
                }
            ]
        })
    );
}

#[test]
fn correctly_passes_shared_encoding_type_scale_to_children_of_layer() {
    let output = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "encoding": {
                "x": {"type": "quantitative", "scale": {"type": "log"}, "axis": {"title": null}},
                "color": {"scale": {"range": ["blue", "green"]}}
            },
            "layer": [
                {"mark": "point", "encoding": {"x": {"field": "a"}}},
                {
                    "layer": [
                        {"mark": "rule", "encoding": {"x": {"field": "b"}}},
                        {
                            "mark": "text",
                            "encoding": {
                                "x": {"field": "c"},
                                "color": {"field": "b1"},
                                "text": {"field": "a", "type": "nominal"}
                            }
                        },
                        {
                            "mark": "text",
                            "encoding": {
                                "x": {"value": 1},
                                "color": {"condition": {"test": "a", "field": "b"}, "value": "red"}
                            }
                        },
                        {"mark": "text"}
                    ]
                }
            ]
        }),
        None,
    )
    .unwrap();

    assert_eq!(
        output,
        json!({
            "data": {"url": "data/population.json"},
            "layer": [
                {
                    "mark": "point",
                    "encoding": {
                        "x": {"field": "a", "type": "quantitative", "scale": {"type": "log"}, "axis": {"title": null}}
                    }
                },
                {
                    "layer": [
                        {
                            "mark": "rule",
                            "encoding": {
                                "x": {"field": "b", "type": "quantitative", "scale": {"type": "log"}, "axis": {"title": null}}
                            }
                        },
                        {
                            "mark": "text",
                            "encoding": {
                                "x": {"field": "c", "type": "quantitative", "scale": {"type": "log"}, "axis": {"title": null}},
                                "color": {"field": "b1", "scale": {"range": ["blue", "green"]}},
                                "text": {"field": "a", "type": "nominal"}
                            }
                        },
                        {
                            "mark": "text",
                            "encoding": {
                                "x": {"value": 1},
                                "color": {
                                    "condition": {"test": "a", "field": "b", "scale": {"range": ["blue", "green"]}},
                                    "value": "red"
                                }
                            }
                        },
                        {"mark": "text"}
                    ]
                }
            ]
        })
    );
}

#[test]
fn correctly_overrides_shared_projection_and_throws_warnings() {
    let (output, local_logger) = log::capture(|| {
        normalize(
            &json!({
                "data": {"url": "data/population.json"},
                "projection": {"type": "mercator"},
                "encoding": {"x": {"field": "age", "type": "ordinal"}},
                "layer": [
                    {"projection": {"type": "albersUsa"}, "mark": "rule"},
                    {"mark": "text", "encoding": {"x": {"field": "a", "type": "nominal"}}}
                ]
            }),
            None,
        )
    });

    assert_eq!(local_logger.warns.len(), 1);
    assert_eq!(
        local_logger.warns[0],
        json!(format!(
            "Layer's shared projection {} is overridden by a child projection {}.",
            vega_lite::util::stringify(&json!({"type": "mercator"})),
            vega_lite::util::stringify(&json!({"type": "albersUsa"}))
        ))
    );

    assert_eq!(
        output.unwrap(),
        json!({
            "data": {"url": "data/population.json"},
            "layer": [
                {
                    "projection": {"type": "albersUsa"},
                    "mark": "rule",
                    "encoding": {"x": {"field": "age", "type": "ordinal"}}
                },
                {
                    "projection": {"type": "mercator"},
                    "mark": "text",
                    "encoding": {"x": {"field": "a", "type": "nominal"}}
                }
            ]
        })
    );
}

//! Port of `test/compositemark/boxplot.test.ts`.

use serde_json::{json, Value};
use vega_lite::log;
use vega_lite::normalize::normalize;

/// Port of vega-util's `array`: wrap a non-array value in a single-element array.
fn tooltip_array(value: &Value) -> Value {
    match value {
        Value::Array(_) => value.clone(),
        other => Value::Array(vec![other.clone()]),
    }
}

/// Rough `toMatchObject` implementation: every key in `expected` must be
/// present in `actual` with a matching (recursively partial) value.
fn assert_match_object(actual: &Value, expected: &Value, path: &str) {
    match expected {
        Value::Object(expected_map) => {
            let actual_map = actual
                .as_object()
                .unwrap_or_else(|| panic!("expected object at {path}, got {actual}"));
            for (key, expected_value) in expected_map {
                let actual_value = actual_map
                    .get(key)
                    .unwrap_or_else(|| panic!("missing key {path}.{key} in {actual}"));
                assert_match_object(actual_value, expected_value, &format!("{path}.{key}"));
            }
        }
        Value::Array(expected_items) => {
            let actual_items = actual
                .as_array()
                .unwrap_or_else(|| panic!("expected array at {path}, got {actual}"));
            assert_eq!(
                actual_items.len(),
                expected_items.len(),
                "array length mismatch at {path}"
            );
            for (i, expected_item) in expected_items.iter().enumerate() {
                assert_match_object(&actual_items[i], expected_item, &format!("{path}[{i}]"));
            }
        }
        _ => assert_eq!(actual, expected, "value mismatch at {path}"),
    }
}

#[test]
fn should_produce_error_if_both_axes_have_aggregate_boxplot() {
    assert!(normalize(
        &json!({
            "description": "A box plot showing median, min, and max in the US population distribution of age groups in 2000.",
            "data": {"url": "data/population.json"},
            "mark": {"type": "boxplot", "extent": "min-max"},
            "encoding": {
                "x": {"aggregate": "boxplot", "field": "people", "type": "quantitative"},
                "y": {"aggregate": "boxplot", "field": "people", "type": "quantitative", "axis": {"title": "Population"}},
                "color": {"value": "skyblue"}
            }
        }),
        None,
    )
    .is_err());
}

#[test]
fn should_produce_correct_transform_groupby_for_vertical_boxplot_with_two_quantitative_axes_and_use_default_orientation(
) {
    let output = normalize(
        &json!({
            "description": "A box plot showing median, min, and max in the US population distribution of age groups in 2000.",
            "data": {"url": "data/population.json"},
            "mark": {"type": "boxplot", "extent": "min-max", "size": 5},
            "encoding": {
                "x": {"field": "age", "type": "quantitative"},
                "y": {"field": "people", "type": "quantitative", "axis": {"title": "Population"}},
                "color": {"value": "skyblue"}
            }
        }),
        None,
    )
    .unwrap();

    assert_eq!(
        output["transform"],
        json!([
            {
                "aggregate": [
                    {"op": "q1", "field": "people", "as": "lower_box_people"},
                    {"op": "q3", "field": "people", "as": "upper_box_people"},
                    {"op": "median", "field": "people", "as": "mid_box_people"},
                    {"op": "min", "field": "people", "as": "lower_whisker_people"},
                    {"op": "max", "field": "people", "as": "upper_whisker_people"}
                ],
                "groupby": ["age"]
            }
        ])
    );
}

#[test]
fn should_produce_correct_transform_with_values_that_include_periods_in_field_names() {
    let output = normalize(
        &json!({
            "description": "A box plot showing median, min, and max in the US population distribution of age groups in 2000.",
            "data": {
                "values": [
                    {"foo.bar": 10}, {"foo.bar": 20}, {"foo.bar": 30}, {"foo.bar": 40},
                    {"foo.bar": 50}, {"foo.bar": 60}, {"foo.bar": 70}, {"foo.bar": 80},
                    {"foo.bar": 90}, {"foo.bar": 100}
                ]
            },
            "mark": {"type": "boxplot", "extent": "min-max", "size": 5},
            "encoding": {
                "x": {"field": "one\\.two", "type": "quantitative"},
                "y": {"field": "three\\.four", "type": "quantitative", "axis": {"title": "Population"}},
                "color": {"value": "skyblue"}
            }
        }),
        None,
    )
    .unwrap();

    assert_eq!(
        output["transform"],
        json!([
            {
                "aggregate": [
                    {"op": "q1", "field": "three\\.four", "as": "lower_box_three.four"},
                    {"op": "q3", "field": "three\\.four", "as": "upper_box_three.four"},
                    {"op": "median", "field": "three\\.four", "as": "mid_box_three.four"},
                    {"op": "min", "field": "three\\.four", "as": "lower_whisker_three.four"},
                    {"op": "max", "field": "three\\.four", "as": "upper_whisker_three.four"}
                ],
                "groupby": ["one\\.two"]
            }
        ])
    );
}

#[test]
fn should_produce_error_if_neither_x_nor_y_axis_is_specified() {
    assert!(normalize(
        &json!({
            "description": "A box plot showing median, min, and max in the US population distribution of age groups in 2000.",
            "data": {"url": "data/population.json"},
            "mark": {"type": "boxplot", "extent": "min-max"},
            "encoding": {"color": {"value": "skyblue"}}
        }),
        None,
    )
    .is_err());
}

#[test]
fn should_produce_warning_if_continuous_axis_has_aggregate_property() {
    let (_, logger) = log::capture(|| {
        normalize(
            &json!({
                "description": "A box plot showing median, min, and max in the US population distribution of age groups in 2000.",
                "data": {"url": "data/population.json"},
                "mark": {"type": "boxplot", "extent": "min-max", "size": 14},
                "encoding": {
                    "x": {"field": "age", "type": "ordinal"},
                    "y": {"aggregate": "min", "field": "people", "type": "quantitative", "axis": {"title": "Population"}},
                    "color": {"value": "skyblue"}
                }
            }),
            None,
        )
    });

    assert_eq!(
        logger.warns[0],
        json!("Continuous axis should not have customized aggregation function min; boxplot already agregates the axis.")
    );
}

#[test]
fn should_produce_error_if_build_1d_boxplot_with_a_discrete_axis() {
    assert!(normalize(
        &json!({
            "description": "A box plot showing median, min, and max in the US population distribution of age groups in 2000.",
            "data": {"url": "data/population.json"},
            "mark": "boxplot",
            "encoding": {"x": {"field": "age", "type": "ordinal"}}
        }),
        None,
    )
    .is_err());
}

#[test]
fn should_produce_error_if_both_axes_are_discrete() {
    assert!(normalize(
        &json!({
            "description": "A box plot showing median, min, and max in the US population distribution of age groups in 2000.",
            "data": {"url": "data/population.json"},
            "mark": {"type": "boxplot", "extent": "min-max"},
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "y": {"field": "age", "type": "ordinal", "axis": {"title": "age"}},
                "color": {"value": "skyblue"}
            }
        }),
        None,
    )
    .is_err());
}

#[test]
fn should_produce_error_if_in_2d_boxplot_both_axes_are_not_valid_field_definitions() {
    assert!(normalize(
        &json!({
            "description": "A box plot showing median, min, and max in the US population distribution of age groups in 2000.",
            "data": {"url": "data/population.json"},
            "mark": {"type": "boxplot", "extent": "min-max"},
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "y": {"type": "ordinal", "axis": {"title": "age"}},
                "color": {"value": "skyblue"}
            }
        }),
        None,
    )
    .is_err());
}

#[test]
fn should_produce_error_if_1d_boxplot_only_axis_is_discrete() {
    assert!(normalize(
        &json!({
            "description": "A box plot showing median, min, and max in the US population distribution of age groups in 2000.",
            "data": {"url": "data/population.json"},
            "mark": "boxplot",
            "encoding": {"x": {"field": "age", "type": "ordinal"}, "color": {"value": "skyblue"}}
        }),
        None,
    )
    .is_err());
}

#[test]
fn should_produce_correct_transform_groupby_for_vertical_boxplot_with_orient() {
    let output = normalize(
        &json!({
            "description": "A box plot showing median, min, and max in the US population distribution of age groups in 2000.",
            "data": {"url": "data/population.json"},
            "mark": {"type": "boxplot", "orient": "vertical", "extent": "min-max"},
            "encoding": {
                "x": {"field": "age", "type": "quantitative"},
                "y": {"field": "people", "type": "quantitative", "axis": {"title": "Population"}},
                "color": {"value": "skyblue"}
            }
        }),
        None,
    )
    .unwrap();

    assert_eq!(output["transform"][0]["groupby"], json!(["age"]));
    assert_eq!(
        output["transform"][0]["aggregate"][0],
        json!({"op": "q1", "field": "people", "as": "lower_box_people"})
    );
}

#[test]
fn should_produce_correct_transform_groupby_for_horizontal_boxplot_with_orient() {
    let output = normalize(
        &json!({
            "description": "A box plot showing median, min, and max in the US population distribution of age groups in 2000.",
            "data": {"url": "data/population.json"},
            "mark": {"type": "boxplot", "orient": "horizontal", "extent": "min-max"},
            "encoding": {
                "y": {"field": "age", "type": "quantitative"},
                "x": {"field": "people", "type": "quantitative", "axis": {"title": "Population"}},
                "color": {"value": "skyblue"}
            }
        }),
        None,
    )
    .unwrap();

    assert_eq!(output["transform"][0]["groupby"], json!(["age"]));
    assert_eq!(
        output["transform"][0]["aggregate"][0],
        json!({"op": "q1", "field": "people", "as": "lower_box_people"})
    );
}

#[test]
fn should_produce_correct_transform_groupby_for_vertical_boxplot_with_aggregate() {
    let output = normalize(
        &json!({
            "description": "A box plot showing median, min, and max in the US population distribution of age groups in 2000.",
            "data": {"url": "data/population.json"},
            "mark": {"type": "boxplot", "extent": "min-max"},
            "encoding": {
                "x": {"field": "age", "type": "quantitative"},
                "y": {"field": "people", "type": "quantitative", "aggregate": "boxplot", "axis": {"title": "Population"}},
                "color": {"value": "skyblue"}
            }
        }),
        None,
    )
    .unwrap();

    assert_eq!(output["transform"][0]["groupby"], json!(["age"]));
    assert_eq!(
        output["transform"][0]["aggregate"][0],
        json!({"op": "q1", "field": "people", "as": "lower_box_people"})
    );
}

#[test]
fn should_produce_correct_transform_groupby_for_horizontal_boxplot_with_aggregate() {
    let output = normalize(
        &json!({
            "description": "A box plot showing median, min, and max in the US population distribution of age groups in 2000.",
            "data": {"url": "data/population.json"},
            "mark": {"type": "boxplot", "extent": "min-max"},
            "encoding": {
                "y": {"field": "age", "type": "quantitative"},
                "x": {"field": "people", "type": "quantitative", "aggregate": "boxplot", "axis": {"title": "Population"}},
                "color": {"value": "skyblue"}
            }
        }),
        None,
    )
    .unwrap();

    assert_eq!(output["transform"][0]["groupby"], json!(["age"]));
    assert_eq!(
        output["transform"][0]["aggregate"][0],
        json!({"op": "q1", "field": "people", "as": "lower_box_people"})
    );
}

#[test]
fn should_produce_correct_transform_for_vertical_boxplot_with_min_and_max() {
    let output = normalize(
        &json!({
            "description": "A box plot showing median, min, and max in the US population distribution of age groups in 2000.",
            "data": {"url": "data/population.json"},
            "mark": {"type": "boxplot", "extent": "min-max"},
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "y": {"field": "people", "type": "quantitative", "axis": {"title": "Population"}},
                "color": {"value": "skyblue"}
            }
        }),
        None,
    )
    .unwrap();

    assert_eq!(
        output["transform"],
        json!([
            {
                "aggregate": [
                    {"op": "q1", "field": "people", "as": "lower_box_people"},
                    {"op": "q3", "field": "people", "as": "upper_box_people"},
                    {"op": "median", "field": "people", "as": "mid_box_people"},
                    {"op": "min", "field": "people", "as": "lower_whisker_people"},
                    {"op": "max", "field": "people", "as": "upper_whisker_people"}
                ],
                "groupby": ["age"]
            }
        ])
    );
}

#[test]
fn should_not_overwrite_transform_with_boxplots_transforms() {
    let output = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": {"type": "boxplot", "extent": "min-max"},
            "transform": [{"calculate": "age * 2", "as": "age2"}],
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "y": {"field": "people", "type": "quantitative", "title": "Population"}
            }
        }),
        None,
    )
    .unwrap();

    let transforms = output["transform"].as_array().unwrap();
    assert!(!transforms.is_empty());
    assert_eq!(transforms[0], json!({"calculate": "age * 2", "as": "age2"}));
}

#[test]
fn should_produce_correct_layered_specs_for_iqr_boxplot_with_boxplot_mark_type() {
    let output = normalize(
        &json!({
            "description": "A box plot showing median, min, and max in the US population distribution of age groups in 2000.",
            "data": {"url": "data/population.json"},
            "mark": "boxplot",
            "encoding": {
                "x": {"field": "age", "type": "quantitative"},
                "y": {"field": "people", "type": "quantitative", "title": "Population"},
                "color": {"value": "skyblue"}
            }
        }),
        None,
    )
    .unwrap();

    let outliers_unit = json!({
        "transform": [
            {
                "filter": "(datum['people'] < datum['lower_box_people'] - 1.5 * (datum['upper_box_people'] - datum['lower_box_people'])) || (datum['people'] > datum['upper_box_people'] + 1.5 * (datum['upper_box_people'] - datum['lower_box_people']))"
            }
        ],
        "mark": {"type": "point", "style": "boxplot-outliers"},
        "encoding": {
            "x": {"field": "age", "type": "quantitative"},
            "y": {"field": "people", "type": "quantitative"}
        }
    });

    let whiskers_unit = json!({
        "transform": [
            {
                "filter": "(datum['lower_box_people'] - 1.5 * (datum['upper_box_people'] - datum['lower_box_people']) <= datum['people']) && (datum['people'] <= datum['upper_box_people'] + 1.5 * (datum['upper_box_people'] - datum['lower_box_people']))"
            },
            {
                "aggregate": [
                    {"as": "lower_whisker_people", "field": "people", "op": "min"},
                    {"as": "upper_whisker_people", "field": "people", "op": "max"},
                    {"as": "lower_box_people", "field": "lower_box_people", "op": "min"},
                    {"as": "upper_box_people", "field": "upper_box_people", "op": "max"}
                ],
                "groupby": ["age"]
            }
        ],
        "layer": [
            {
                "mark": {"type": "rule", "style": "boxplot-rule"},
                "encoding": {
                    "x": {"field": "age", "type": "quantitative"},
                    "y": {"field": "lower_whisker_people", "type": "quantitative", "title": "Population"},
                    "y2": {"field": "lower_box_people"}
                }
            },
            {
                "mark": {"type": "rule", "style": "boxplot-rule"},
                "encoding": {
                    "x": {"field": "age", "type": "quantitative"},
                    "y": {"field": "upper_box_people", "type": "quantitative", "title": "Population"},
                    "y2": {"field": "upper_whisker_people"}
                }
            }
        ]
    });

    let box_unit = json!({
        "transform": [
            {
                "aggregate": [
                    {"op": "q1", "field": "people", "as": "lower_box_people"},
                    {"op": "q3", "field": "people", "as": "upper_box_people"},
                    {"op": "median", "field": "people", "as": "mid_box_people"},
                    {"op": "min", "field": "people", "as": "min_people"},
                    {"op": "max", "field": "people", "as": "max_people"}
                ],
                "groupby": ["age"]
            }
        ],
        "layer": [
            {
                "mark": {"type": "bar", "style": "boxplot-box", "size": 14},
                "encoding": {
                    "x": {"field": "age", "type": "quantitative"},
                    "y": {"field": "lower_box_people", "type": "quantitative", "title": "Population"},
                    "y2": {"field": "upper_box_people"},
                    "color": {"value": "skyblue"}
                }
            },
            {
                "mark": {"type": "tick", "orient": "horizontal", "style": "boxplot-median", "color": "white", "size": 14},
                "encoding": {
                    "x": {"field": "age", "type": "quantitative"},
                    "y": {"field": "mid_box_people", "type": "quantitative", "title": "Population"}
                }
            }
        ]
    });

    assert_match_object(
        &output,
        &json!({
            "layer": [
                {
                    "transform": [
                        {
                            "joinaggregate": [
                                {"op": "q1", "field": "people", "as": "lower_box_people"},
                                {"op": "q3", "field": "people", "as": "upper_box_people"}
                            ],
                            "groupby": ["age"]
                        }
                    ],
                    "layer": [outliers_unit, whiskers_unit]
                },
                box_unit
            ]
        }),
        "$",
    );
}

#[test]
fn should_produce_correct_layered_specs_for_vertical_boxplot_with_default_orientation() {
    let output = normalize(
        &json!({
            "description": "A box plot showing median, min, and max in the US population distribution of age groups in 2000.",
            "data": {"url": "data/population.json"},
            "mark": {"type": "boxplot", "extent": 1.5},
            "encoding": {
                "x": {"field": "age", "type": "quantitative"},
                "y": {"field": "people", "type": "quantitative", "axis": {"title": "Population"}},
                "color": {"value": "skyblue"}
            }
        }),
        None,
    )
    .unwrap();

    let partial_outlier_unit = json!({
        "transform": [{
            "joinaggregate": [
                {"op": "q1", "field": "people", "as": "lower_box_people"},
                {"op": "q3", "field": "people", "as": "upper_box_people"}
            ],
            "groupby": ["age"]
        }]
    });
    let partial_box_unit = json!({
        "transform": [{
            "aggregate": [
                {"op": "q1", "field": "people", "as": "lower_box_people"},
                {"op": "q3", "field": "people", "as": "upper_box_people"},
                {"op": "median", "field": "people", "as": "mid_box_people"},
                {"op": "min", "field": "people", "as": "min_people"},
                {"op": "max", "field": "people", "as": "max_people"}
            ],
            "groupby": ["age"]
        }]
    });

    assert_match_object(
        &output,
        &json!({
            "description": "A box plot showing median, min, and max in the US population distribution of age groups in 2000.",
            "data": {"url": "data/population.json"},
            "layer": [partial_outlier_unit, partial_box_unit]
        }),
        "$",
    );
}

#[test]
fn should_produce_correct_layered_specs_for_vertical_iqr_boxplot_where_color_encodes_mean() {
    let normalized = normalize(
        &json!({
            "description": "A box plot showing median, min, and max in the US population distribution of age groups in 2000.",
            "data": {"url": "data/population.json"},
            "mark": {"type": "boxplot", "extent": 1.5},
            "encoding": {
                "x": {"field": "age", "type": "quantitative"},
                "y": {"field": "people", "type": "quantitative", "title": "Population"},
                "color": {"aggregate": "mean", "field": "people", "type": "quantitative", "title": "Mean Population"}
            }
        }),
        None,
    )
    .unwrap();

    assert_eq!(
        normalized["layer"][1]["layer"][0],
        json!({
            "mark": {
                "type": "bar",
                "style": "boxplot-box",
                "size": 14,
                "orient": "vertical",
                "ariaRoleDescription": "box"
            },
            "encoding": {
                "x": {"field": "age", "type": "quantitative"},
                "y": {"field": "lower_box_people", "type": "quantitative", "title": "Population"},
                "y2": {"field": "upper_box_people"},
                "color": {"field": "mean_people", "type": "quantitative", "title": "Mean Population"},
                "tooltip": [
                    {"field": "max_people", "title": "Max of Population", "type": "quantitative"},
                    {"field": "upper_box_people", "title": "Q3 of Population", "type": "quantitative"},
                    {"field": "mid_box_people", "title": "Median of Population", "type": "quantitative"},
                    {"field": "lower_box_people", "title": "Q1 of Population", "type": "quantitative"},
                    {"field": "min_people", "title": "Min of Population", "type": "quantitative"},
                    {"field": "age", "type": "quantitative"},
                    {"field": "mean_people", "title": "Mean Population", "type": "quantitative"}
                ]
            }
        })
    );
}

#[test]
fn should_only_include_custom_tooltip_without_aggregate_in_outliers_layer() {
    let with_tooltip = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": "boxplot",
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "y": {"field": "people", "type": "quantitative"},
                "tooltip": {"field": "year", "type": "quantitative"}
            }
        }),
        None,
    )
    .unwrap();
    let without_tooltip = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": "boxplot",
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "y": {"field": "people", "type": "quantitative"}
            }
        }),
        None,
    )
    .unwrap();

    assert_ne!(with_tooltip, without_tooltip);

    let mut stripped = with_tooltip.clone();
    let inner = &mut stripped["layer"][0]["layer"][0]["encoding"];
    let map = inner.as_object_mut().unwrap();
    map.remove("tooltip");

    assert_eq!(stripped, without_tooltip);
}

#[test]
fn should_only_include_custom_tooltip_with_aggregate_in_box_and_whiskers_layer() {
    let normalized = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": "boxplot",
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "y": {"field": "people", "type": "quantitative"},
                "tooltip": {"field": "people", "aggregate": "mean", "type": "quantitative"}
            }
        }),
        None,
    )
    .unwrap();

    let whisker_layer = &normalized["layer"][0]["layer"][1];
    for whisker in whisker_layer["layer"].as_array().unwrap() {
        assert_eq!(
            tooltip_array(&whisker["encoding"]["tooltip"]),
            json!([{"title": "Mean of people", "type": "quantitative", "field": "mean_people"}])
        );
    }

    let whisker_aggregate = whisker_layer["transform"][1]["aggregate"]
        .as_array()
        .unwrap();
    assert_eq!(
        whisker_aggregate.last().unwrap(),
        &json!({"op": "mean", "as": "mean_people", "field": "people"})
    );

    let box_layer = &normalized["layer"][1];
    for box_unit in box_layer["layer"].as_array().unwrap() {
        assert_eq!(
            tooltip_array(&box_unit["encoding"]["tooltip"]),
            json!([{"title": "Mean of people", "type": "quantitative", "field": "mean_people"}])
        );
    }
    assert_eq!(
        box_layer["transform"][0]["aggregate"][0],
        json!({"op": "mean", "as": "mean_people", "field": "people"})
    );

    assert!(normalized["layer"][0]["layer"][0]["encoding"]
        .get("tooltip")
        .is_none());
}

#[test]
fn should_include_aggregate_and_non_aggregate_custom_tooltips() {
    let normalized = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": "boxplot",
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "y": {"field": "people", "type": "quantitative"},
                "tooltip": [
                    {"field": "people", "aggregate": "mean", "type": "quantitative"},
                    {"field": "year", "type": "quantitative"}
                ]
            }
        }),
        None,
    )
    .unwrap();

    let whisker_layer = &normalized["layer"][0]["layer"][1];
    for whisker in whisker_layer["layer"].as_array().unwrap() {
        assert_eq!(
            tooltip_array(&whisker["encoding"]["tooltip"]),
            json!([{"title": "Mean of people", "type": "quantitative", "field": "mean_people"}])
        );
    }

    let whisker_aggregate = whisker_layer["transform"][1]["aggregate"]
        .as_array()
        .unwrap();
    assert_eq!(
        whisker_aggregate.last().unwrap(),
        &json!({"op": "mean", "as": "mean_people", "field": "people"})
    );

    let box_layer = &normalized["layer"][1];
    assert_eq!(
        box_layer["transform"][0]["aggregate"][0],
        json!({"op": "mean", "as": "mean_people", "field": "people"})
    );

    assert_eq!(
        normalized["layer"][0]["layer"][0]["encoding"]["tooltip"],
        json!({"field": "year", "type": "quantitative"})
    );
}

#[test]
fn should_include_timeunit_transform_in_filtered_layer_mixins_transform() {
    let field = "Date";
    let time_unit = "year";
    let normalized = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": "boxplot",
            "encoding": {
                "x": {"field": field, "type": "temporal", "timeUnit": time_unit},
                "y": {"field": "Anomaly", "type": "quantitative"}
            }
        }),
        None,
    )
    .unwrap();

    let filtered_layer_mixins = &normalized["layer"][1];
    assert_eq!(
        filtered_layer_mixins["transform"][0],
        json!({"timeUnit": {"unit": "year"}, "field": field, "as": "year_Date"})
    );
}

#[test]
fn should_produce_correct_y_axis_title_when_mark_extent_is_not_explicitly_set() {
    let normalized = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": {"type": "boxplot"},
            "encoding": {
                "x": {"field": "age", "type": "quantitative"},
                "y": {"field": "people", "type": "quantitative", "axis": {"title": "Population"}},
                "color": {"value": "skyblue"}
            }
        }),
        None,
    )
    .unwrap();

    assert_eq!(
        normalized["layer"][0]["layer"][0]["encoding"]["y"]["axis"]["title"],
        json!("Population")
    );
}

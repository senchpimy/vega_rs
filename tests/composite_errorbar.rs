//! Port of `test/compositemark/errorbar.test.ts`.

use serde_json::{json, Value};
use vega_lite::log;
use vega_lite::normalize::normalize;

fn is_layer(spec: &Value) -> bool {
    spec.get("layer").is_some()
}

fn is_unit(spec: &Value) -> bool {
    spec.get("mark").is_some()
}

fn some_aggregate_op(spec: &Value, expected: &str) -> bool {
    spec.get("transform")
        .and_then(|t| t.as_array())
        .and_then(|t| t.first())
        .and_then(|t| t.get("aggregate"))
        .and_then(|a| a.as_array())
        .map(|a| a.iter().any(|def| def["op"] == json!(expected)))
        .unwrap_or(false)
}

#[test]
fn should_produce_correct_layered_specs_for_mean_point_and_vertical_error_bar() {
    let output = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": "errorbar",
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "y": {"field": "people", "type": "quantitative"}
            }
        }),
        None,
    )
    .unwrap();

    assert_eq!(
        output,
        json!({
            "data": {"url": "data/population.json"},
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
        })
    );
}

#[test]
fn should_produce_error_if_both_axes_have_aggregate_errorbar() {
    let result = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": {"type": "errorbar"},
            "encoding": {
                "x": {"aggregate": "errorbar", "field": "people", "type": "quantitative"},
                "y": {"aggregate": "errorbar", "field": "people", "type": "quantitative"},
                "color": {"value": "skyblue"}
            }
        }),
        None,
    );
    assert!(result.is_err());
}

#[test]
fn should_produce_warning_if_continuous_axis_has_aggregate_property() {
    let (_, logger) = log::capture(|| {
        normalize(
            &json!({
                "data": {"url": "data/population.json"},
                "mark": "errorbar",
                "encoding": {
                    "x": {"field": "age", "type": "ordinal"},
                    "y": {"aggregate": "min", "field": "people", "type": "quantitative"},
                    "color": {"value": "skyblue"}
                }
            }),
            None,
        )
    });
    assert_eq!(
        logger.warns[0],
        json!("Continuous axis should not have customized aggregation function min; errorbar already agregates the axis.")
    );
}

#[test]
fn should_produce_error_if_1d_errorbar_with_discrete_axis() {
    assert!(normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": "errorbar",
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
            "data": {"url": "data/population.json"},
            "mark": {"type": "errorbar"},
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "y": {"field": "age", "type": "ordinal"},
                "color": {"value": "skyblue"}
            }
        }),
        None,
    )
    .is_err());
}

#[test]
fn should_produce_error_if_both_axes_are_not_valid_field_definitions() {
    assert!(normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": {"type": "errorbar"},
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "y": {"type": "ordinal"},
                "color": {"value": "skyblue"}
            }
        }),
        None,
    )
    .is_err());
}

#[test]
fn should_produce_error_if_1d_errorbar_only_axis_is_discrete() {
    assert!(normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": "errorbar",
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "color": {"value": "skyblue"}
            }
        }),
        None,
    )
    .is_err());
}

#[test]
fn should_produce_correct_layered_specs_for_vertical_errorbar_with_ticks() {
    let output = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": {"type": "errorbar", "ticks": {"size": 10, "color": "red", "opacity": 0.5}},
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "y": {"field": "people", "type": "quantitative"}
            }
        }),
        None,
    )
    .unwrap();

    assert!(is_layer(&output));
    assert!(output["layer"].as_array().unwrap().iter().any(|unit| {
        unit["mark"]["type"] == json!("tick")
            && unit["mark"]["size"] == json!(10)
            && unit["mark"]["color"] == json!("red")
            && unit["mark"]["opacity"] == json!(0.5)
    }));
}

#[test]
fn should_produce_correct_specs_with_customized_title() {
    let output = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": {"type": "errorbar"},
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "y": {"field": "people", "type": "quantitative", "title": "population"}
            }
        }),
        None,
    )
    .unwrap();

    assert!(is_unit(&output));
    assert_eq!(output["encoding"]["y"]["title"], json!("population"));
}

#[test]
fn should_not_overwrite_transform_with_errorbars_transforms() {
    let output = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": "errorbar",
            "transform": [{"calculate": "age * 2", "as": "age2"}],
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "y": {"field": "people", "type": "quantitative", "title": "population"}
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
fn should_produce_correct_tooltip_title_for_errorbar_with_stdev_extent() {
    let output = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": {"type": "errorbar", "extent": "stdev"},
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "y": {"field": "people", "type": "quantitative"}
            }
        }),
        None,
    )
    .unwrap();

    assert!(is_unit(&output));
    assert_eq!(
        output["encoding"]["tooltip"],
        json!([
            {"field": "center_people", "title": "Mean of people", "type": "quantitative"},
            {"field": "upper_people", "title": "Mean + stdev of people", "type": "quantitative"},
            {"field": "lower_people", "title": "Mean - stdev of people", "type": "quantitative"},
            {"field": "age", "type": "ordinal"}
        ])
    );
}

#[test]
fn should_produce_correct_tooltip_title_for_errorbar_with_stderr_extent() {
    let output = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": {"type": "errorbar", "extent": "stderr"},
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "y": {"field": "people", "type": "quantitative"}
            }
        }),
        None,
    )
    .unwrap();

    assert_eq!(
        output["encoding"]["tooltip"],
        json!([
            {"field": "center_people", "title": "Mean of people", "type": "quantitative"},
            {"field": "upper_people", "title": "Mean + stderr of people", "type": "quantitative"},
            {"field": "lower_people", "title": "Mean - stderr of people", "type": "quantitative"},
            {"field": "age", "type": "ordinal"}
        ])
    );
}

#[test]
fn should_produce_correct_tooltip_title_for_errorbar_with_ci_extent() {
    let output = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": {"type": "errorbar", "extent": "ci"},
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "y": {"field": "people", "type": "quantitative"}
            }
        }),
        None,
    )
    .unwrap();

    assert_eq!(
        output["encoding"]["tooltip"],
        json!([
            {"field": "upper_people", "title": "Ci1 of people", "type": "quantitative"},
            {"field": "lower_people", "title": "Ci0 of people", "type": "quantitative"},
            {"field": "center_people", "title": "Mean of people", "type": "quantitative"},
            {"field": "age", "type": "ordinal"}
        ])
    );
}

#[test]
fn should_produce_correct_tooltip_title_for_errorbar_with_iqr_extent() {
    let output = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": {"type": "errorbar", "extent": "iqr"},
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "y": {"field": "people", "type": "quantitative"}
            }
        }),
        None,
    )
    .unwrap();

    assert_eq!(
        output["encoding"]["tooltip"],
        json!([
            {"field": "upper_people", "title": "Q3 of people", "type": "quantitative"},
            {"field": "lower_people", "title": "Q1 of people", "type": "quantitative"},
            {"field": "center_people", "title": "Median of people", "type": "quantitative"},
            {"field": "age", "type": "ordinal"}
        ])
    );
}

#[test]
fn should_drop_size_in_encoding_channel() {
    let output = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": {"type": "errorbar"},
            "encoding": {
                "x": {"field": "age", "type": "quantitative"},
                "size": {"value": 3}
            }
        }),
        None,
    )
    .unwrap();

    assert!(is_unit(&output));
    assert!(output["encoding"].get("size").is_none());
}

#[test]
fn should_apply_mark_size_to_only_size_of_ticks() {
    let output = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": {"type": "errorbar", "size": 10, "ticks": true},
            "encoding": {"x": {"field": "age", "type": "quantitative"}}
        }),
        None,
    )
    .unwrap();

    assert!(is_layer(&output));
    for unit in output["layer"].as_array().unwrap() {
        if unit.get("mark").is_some() {
            if unit["mark"]["type"] == json!("tick") {
                assert_eq!(unit["mark"]["size"], json!(10));
            } else {
                assert!(unit["mark"].get("size").is_none());
            }
        }
    }
}

#[test]
fn should_override_mark_size_with_mark_ticks_size() {
    let output = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": {"type": "errorbar", "size": 10, "ticks": {"size": 5}},
            "encoding": {"x": {"field": "age", "type": "quantitative"}}
        }),
        None,
    )
    .unwrap();

    assert!(is_layer(&output));
    for unit in output["layer"].as_array().unwrap() {
        if unit.get("mark").is_some() {
            if unit["mark"]["type"] == json!("tick") {
                assert_eq!(unit["mark"]["size"], json!(5));
            } else {
                assert!(unit["mark"].get("size").is_none());
            }
        }
    }
}

#[test]
fn should_apply_mark_thickness_to_ticks_and_rule() {
    let output = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": {"type": "errorbar", "thickness": 10, "ticks": true},
            "encoding": {"x": {"field": "age", "type": "quantitative"}}
        }),
        None,
    )
    .unwrap();

    assert!(is_layer(&output));
    for unit in output["layer"].as_array().unwrap() {
        if unit.get("mark").is_some() {
            if unit["mark"]["type"] == json!("tick") {
                assert_eq!(unit["mark"]["thickness"], json!(10));
            } else {
                assert_eq!(unit["mark"]["size"], json!(10));
            }
        }
    }
}

#[test]
fn should_override_mark_thickness_with_ticks_and_rule() {
    let output = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": {
                "type": "errorbar",
                "thickness": 10,
                "ticks": {"thickness": 5},
                "rule": {"size": 7}
            },
            "encoding": {"x": {"field": "age", "type": "quantitative"}}
        }),
        None,
    )
    .unwrap();

    assert!(is_layer(&output));
    for unit in output["layer"].as_array().unwrap() {
        if unit.get("mark").is_some() {
            if unit["mark"]["type"] == json!("tick") {
                assert_eq!(unit["mark"]["thickness"], json!(5));
            } else {
                assert_eq!(unit["mark"]["size"], json!(7));
            }
        }
    }
}

fn is_part_of_extent(extent: &str, op: &str) -> bool {
    match extent {
        "ci" => op == "ci0" || op == "ci1",
        "iqr" => op == "q1" || op == "q3",
        other => other == op,
    }
}

#[test]
fn should_produce_correct_layer_spec_for_all_centers_and_extents() {
    let centers: [Option<&str>; 3] = [Some("mean"), Some("median"), None];
    let extents: [Option<&str>; 5] = [Some("stderr"), Some("stdev"), Some("ci"), Some("iqr"), None];

    let warns = |center: Option<&str>, extent: Option<&str>| -> bool {
        matches!(
            (center, extent),
            (Some("mean"), Some("iqr")) | (Some("median"), Some("stderr" | "stdev" | "ci"))
        )
    };

    for center in centers {
        for extent in extents {
            let mut mark = serde_json::Map::new();
            mark.insert("type".to_string(), json!("errorbar"));
            if let Some(center) = center {
                mark.insert("center".to_string(), json!(center));
            }
            if let Some(extent) = extent {
                mark.insert("extent".to_string(), json!(extent));
            }

            let spec = json!({
                "data": {"url": "data/population.json"},
                "mark": Value::Object(mark),
                "encoding": {
                    "x": {"field": "people", "type": "quantitative"},
                    "y": {"field": "people", "type": "quantitative"}
                }
            });

            let (output, logger) = log::capture(|| normalize(&spec, None));
            let output = output.unwrap();

            if warns(center, extent) {
                assert!(!logger.warns.is_empty());
            }

            let aggregate_transform = &output["transform"][0];
            assert!(aggregate_transform.get("aggregate").is_some());

            match extent {
                Some("iqr") => assert!(some_aggregate_op(&output, "median")),
                Some("ci") => assert!(some_aggregate_op(&output, "mean")),
                Some(extent) => {
                    if let Some(center) = center {
                        assert!(some_aggregate_op(&output, center));
                    } else {
                        assert!(some_aggregate_op(&output, "mean"));
                    }
                    assert!(aggregate_transform["aggregate"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|def| is_part_of_extent(extent, def["op"].as_str().unwrap_or(""))));
                }
                None => {
                    if center == Some("median") {
                        // Upstream: `center === 'median' && !extent` is handled by
                        // the `extent === 'iqr'` branch, which expects a median op.
                        assert!(some_aggregate_op(&output, "median"));
                    } else {
                        assert!(aggregate_transform["aggregate"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|def| is_part_of_extent(
                                "stderr",
                                def["op"].as_str().unwrap_or("")
                            )));
                    }
                }
            }
        }
    }
}

#[test]
fn should_produce_correct_specs_for_vertical_errorbar_with_aggregated_upper_and_lower_bound() {
    let data = json!({
        "values": [
            {"age": 1, "people": 1, "people2": 2},
            {"age": 2, "people": 4, "people2": 8},
            {"age": 3, "people": 13, "people2": 18},
            {"age": 4, "people": 2, "people2": 28},
            {"age": 5, "people": 19, "people2": 23},
            {"age": 6, "people": 10, "people2": 20},
            {"age": 7, "people": 2, "people2": 5}
        ]
    });

    let output = normalize(
        &json!({
            "data": data,
            "mark": "errorbar",
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "y": {"field": "people", "type": "quantitative"},
                "y2": {"field": "people2"}
            }
        }),
        None,
    )
    .unwrap();

    assert_eq!(
        output,
        json!({
            "data": data,
            "transform": [
                {"calculate": "datum['people2']", "as": "upper_people"},
                {"calculate": "datum['people']", "as": "lower_people"}
            ],
            "mark": {"type": "rule", "ariaRoleDescription": "errorbar", "style": "errorbar-rule"},
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "y": {"field": "lower_people", "type": "quantitative", "title": "people"},
                "y2": {"field": "upper_people"},
                "tooltip": [
                    {"field": "upper_people", "title": "people2", "type": "quantitative"},
                    {"field": "lower_people", "title": "people", "type": "quantitative"},
                    {"field": "age", "type": "ordinal"}
                ]
            }
        })
    );
}

#[test]
fn should_produce_correct_specs_for_horizontal_errorbar_with_aggregated_upper_and_lower_bound() {
    let output = normalize(
        &json!({
            "data": {"url": "data.csv"},
            "mark": "errorbar",
            "encoding": {
                "y": {"field": "age", "type": "ordinal"},
                "x": {"field": "people", "type": "quantitative"},
                "x2": {"field": "people2"}
            }
        }),
        None,
    )
    .unwrap();

    for i in 0..2 {
        let calculate = &output["transform"][i];
        assert!(
            (calculate["calculate"] == json!("datum['people']")
                && calculate["as"] == json!("lower_people"))
                || (calculate["calculate"] == json!("datum['people2']")
                    && calculate["as"] == json!("upper_people"))
        );
    }

    assert!(is_unit(&output));
    assert_eq!(output["encoding"]["x"]["field"], json!("lower_people"));
    assert_eq!(output["encoding"]["x2"]["field"], json!("upper_people"));
}

#[test]
fn should_produce_warning_if_upper_and_lower_bound_are_aggregated_with_center_or_extent() {
    let (_, logger) = log::capture(|| {
        normalize(
            &json!({
                "data": {"url": "data.csv"},
                "mark": {"type": "errorbar", "extent": "stdev", "center": "mean"},
                "encoding": {
                    "x": {"field": "age", "type": "ordinal"},
                    "y": {"field": "people", "type": "quantitative"},
                    "y2": {"field": "people2"}
                }
            }),
            None,
        )
    });

    assert_eq!(
        logger.warns[0],
        json!("extent and center are not needed when data are aggregated.")
    );
}

#[test]
fn should_produce_error_if_upper_and_lower_bound_have_both_x2_and_y2() {
    assert!(normalize(
        &json!({
            "data": {"url": "data.csv"},
            "mark": "errorbar",
            "encoding": {
                "x": {"field": "age", "type": "quantitative"},
                "x2": {"field": "age2"},
                "y": {"field": "people", "type": "quantitative"},
                "y2": {"field": "people2"}
            }
        }),
        None,
    )
    .is_err());
}

#[test]
fn should_produce_warning_if_second_continuous_axis_has_aggregate_property() {
    let (_, logger) = log::capture(|| {
        normalize(
            &json!({
                "data": {"url": "data.csv"},
                "mark": "errorbar",
                "encoding": {
                    "x": {"field": "age", "type": "ordinal"},
                    "y": {"field": "people", "type": "quantitative"},
                    "y2": {"field": "people2", "aggregate": "min"}
                }
            }),
            None,
        )
    });

    assert_eq!(
        logger.warns[0],
        json!("Continuous axis should not have customized aggregation function min; errorbar already agregates the axis.")
    );
}

#[test]
fn should_produce_correct_tooltip_title_for_ranged_errorbar() {
    let output = normalize(
        &json!({
            "data": {"url": "data.csv"},
            "mark": "errorbar",
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "y": {"field": "people", "type": "quantitative"},
                "y2": {"field": "people2"}
            }
        }),
        None,
    )
    .unwrap();

    assert!(is_unit(&output));
    assert_eq!(
        output["encoding"]["tooltip"],
        json!([
            {"field": "upper_people", "title": "people2", "type": "quantitative"},
            {"field": "lower_people", "title": "people", "type": "quantitative"},
            {"field": "age", "type": "ordinal"}
        ])
    );
}

#[test]
fn should_produce_correct_specs_for_vertical_errorbar_with_aggregated_error_input() {
    let output = normalize(
        &json!({
            "data": {"url": "data.csv"},
            "mark": "errorbar",
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "y": {"field": "people", "type": "quantitative"},
                "yError": {"field": "people_error"}
            }
        }),
        None,
    )
    .unwrap();

    assert_eq!(
        output,
        json!({
            "data": {"url": "data.csv"},
            "transform": [
                {"calculate": "datum['people'] + datum['people_error']", "as": "upper_people"},
                {"calculate": "datum['people'] - datum['people_error']", "as": "lower_people"}
            ],
            "mark": {"type": "rule", "ariaRoleDescription": "errorbar", "style": "errorbar-rule"},
            "encoding": {
                "y": {"field": "lower_people", "type": "quantitative", "title": "people"},
                "y2": {"field": "upper_people"},
                "x": {"field": "age", "type": "ordinal"},
                "tooltip": [
                    {"field": "people", "title": "people", "type": "quantitative"},
                    {"field": "upper_people", "title": "people + people_error", "type": "quantitative"},
                    {"field": "lower_people", "title": "people - people_error", "type": "quantitative"},
                    {"field": "age", "type": "ordinal"}
                ]
            }
        })
    );
}

#[test]
fn should_produce_correct_specs_for_horizontal_errorbar_with_aggregated_error_input() {
    let output = normalize(
        &json!({
            "data": {"url": "data.csv"},
            "mark": "errorbar",
            "encoding": {
                "y": {"field": "age", "type": "ordinal"},
                "x": {"field": "people", "type": "quantitative"},
                "xError": {"field": "people_error"}
            }
        }),
        None,
    )
    .unwrap();

    for i in 0..2 {
        let calculate = &output["transform"][i];
        assert!(
            (calculate["calculate"] == json!("datum['people'] - datum['people_error']")
                && calculate["as"] == json!("lower_people"))
                || (calculate["calculate"] == json!("datum['people'] + datum['people_error']")
                    && calculate["as"] == json!("upper_people"))
        );
    }

    assert!(is_unit(&output));
    assert_eq!(output["encoding"]["x"]["field"], json!("lower_people"));
    assert_eq!(output["encoding"]["x2"]["field"], json!("upper_people"));
}

#[test]
fn should_produce_correct_layered_specs_for_horizontal_errorbar_with_two_aggregated_errors() {
    let output = normalize(
        &json!({
            "data": {"url": "data.csv"},
            "mark": "errorbar",
            "encoding": {
                "y": {"field": "age", "type": "ordinal"},
                "x": {"field": "people", "type": "quantitative"},
                "xError": {"field": "people_error"},
                "xError2": {"field": "people_error2"}
            }
        }),
        None,
    )
    .unwrap();

    for i in 0..2 {
        let calculate = &output["transform"][i];
        assert!(
            (calculate["calculate"] == json!("datum['people'] + datum['people_error']")
                && calculate["as"] == json!("upper_people"))
                || (calculate["calculate"] == json!("datum['people'] + datum['people_error2']")
                    && calculate["as"] == json!("lower_people"))
        );
    }

    if is_layer(&output) {
        let unit = &output["layer"][0];
        assert_eq!(
            unit["encoding"]["tooltip"],
            json!([
                {"field": "people", "title": "people", "type": "quantitative"},
                {"field": "upper_people", "title": "people + people_error", "type": "quantitative"},
                {"field": "lower_people", "title": "people + people_error2", "type": "quantitative"},
                {"field": "age", "type": "ordinal"}
            ])
        );
    }
}

#[test]
fn should_produce_warning_if_errors_are_aggregated_with_center_or_extent() {
    let (_, logger) = log::capture(|| {
        normalize(
            &json!({
                "data": {"url": "data.csv"},
                "mark": {"type": "errorbar", "extent": "stdev", "center": "mean"},
                "encoding": {
                    "x": {"field": "age", "type": "ordinal"},
                    "y": {"field": "people", "type": "quantitative"},
                    "yError": {"field": "people_error"}
                }
            }),
            None,
        )
    });

    assert_eq!(
        logger.warns[0],
        json!("extent and center are not needed when data are aggregated.")
    );
}

#[test]
fn should_produce_error_with_both_xerror_and_yerror() {
    assert!(normalize(
        &json!({
            "data": {"url": "data.csv"},
            "mark": "errorbar",
            "encoding": {
                "x": {"field": "age", "type": "quantitative"},
                "xError": {"field": "age"},
                "y": {"field": "people", "type": "quantitative"},
                "yError": {"field": "people_error"}
            }
        }),
        None,
    )
    .is_err());
}

#[test]
fn should_produce_error_for_xerror2_without_xerror() {
    assert!(normalize(
        &json!({
            "data": {"url": "data.csv"},
            "mark": "errorbar",
            "encoding": {
                "x": {"field": "age", "type": "quantitative"},
                "xError2": {"field": "age"},
                "y": {"field": "people", "type": "quantitative"}
            }
        }),
        None,
    )
    .is_err());
}

#[test]
fn should_produce_error_for_yerror2_without_yerror() {
    assert!(normalize(
        &json!({
            "data": {"url": "data.csv"},
            "mark": "errorbar",
            "encoding": {
                "x": {"field": "age", "type": "quantitative"},
                "yError2": {"field": "people_error2"},
                "y": {"field": "people", "type": "quantitative"}
            }
        }),
        None,
    )
    .is_err());
}

#[test]
fn should_produce_warning_if_error_axis_has_aggregate_property() {
    let (_, logger) = log::capture(|| {
        normalize(
            &json!({
                "data": {"url": "data.csv"},
                "mark": "errorbar",
                "encoding": {
                    "x": {"field": "age", "type": "ordinal"},
                    "y": {"field": "people", "type": "quantitative"},
                    "yError": {"field": "people_error", "aggregate": "min"}
                }
            }),
            None,
        )
    });

    assert_eq!(
        logger.warns[0],
        json!("Continuous axis should not have customized aggregation function min; errorbar already agregates the axis.")
    );
}

#[test]
fn should_produce_error_if_both_error_and_upper_lower_bound_are_aggregated() {
    assert!(normalize(
        &json!({
            "data": {"url": "data.csv"},
            "mark": "errorbar",
            "encoding": {
                "x": {"field": "age", "type": "quantitative"},
                "xError": {"field": "people_error"},
                "x2": {"field": "people_error2"},
                "y": {"field": "people", "type": "quantitative"}
            }
        }),
        None,
    )
    .is_err());
}

#[test]
fn should_produce_correct_tooltip_title_for_preaggregated_error_value() {
    let output = normalize(
        &json!({
            "data": {"url": "data.csv"},
            "mark": "errorbar",
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "y": {"field": "people", "type": "quantitative"},
                "yError": {"field": "people_error"}
            }
        }),
        None,
    )
    .unwrap();

    assert!(is_unit(&output));
    assert_eq!(
        output["encoding"]["tooltip"],
        json!([
            {"field": "people", "title": "people", "type": "quantitative"},
            {"field": "upper_people", "title": "people + people_error", "type": "quantitative"},
            {"field": "lower_people", "title": "people - people_error", "type": "quantitative"},
            {"field": "age", "type": "ordinal"}
        ])
    );
}

#[test]
fn should_produce_correct_tooltip_title_for_preaggregated_error_values() {
    let output = normalize(
        &json!({
            "data": {"url": "data.csv"},
            "mark": "errorbar",
            "encoding": {
                "x": {"field": "age", "type": "ordinal"},
                "y": {"field": "people", "type": "quantitative"},
                "yError": {"field": "people_error"},
                "yError2": {"field": "people_error2"}
            }
        }),
        None,
    )
    .unwrap();

    assert!(is_unit(&output));
    assert_eq!(
        output["encoding"]["tooltip"],
        json!([
            {"field": "people", "title": "people", "type": "quantitative"},
            {"field": "upper_people", "title": "people + people_error", "type": "quantitative"},
            {"field": "lower_people", "title": "people + people_error2", "type": "quantitative"},
            {"field": "age", "type": "ordinal"}
        ])
    );
}

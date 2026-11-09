//! Port of `test/compositemark/errorband.test.ts`.

use serde_json::{json, Value};
use vega_lite::log;
use vega_lite::normalize::normalize;

fn layer(spec: &Value) -> &Vec<Value> {
    spec.get("layer")
        .and_then(|l| l.as_array())
        .expect("expected a layer spec")
}

#[test]
fn should_produce_correct_layered_specs_for_mean_point_and_vertical_error_band() {
    let output = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": "errorband",
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
            "layer": [
                {
                    "mark": {
                        "opacity": 0.3,
                        "type": "area",
                        "style": "errorband-band",
                        "ariaRoleDescription": "errorband"
                    },
                    "encoding": {
                        "y": {
                            "field": "lower_people",
                            "type": "quantitative",
                            "title": "people"
                        },
                        "y2": {"field": "upper_people"},
                        "x": {"field": "age", "type": "ordinal"},
                        "tooltip": [
                            {"field": "center_people", "title": "Mean of people", "type": "quantitative"},
                            {"field": "upper_people", "title": "Mean + stderr of people", "type": "quantitative"},
                            {"field": "lower_people", "title": "Mean - stderr of people", "type": "quantitative"},
                            {"field": "age", "type": "ordinal"}
                        ]
                    }
                }
            ]
        })
    );
}

#[test]
fn should_produce_rect_and_rule_in_1d_error_band() {
    let output = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": {"type": "errorband", "borders": true},
            "encoding": {"y": {"field": "people", "type": "quantitative"}}
        }),
        None,
    )
    .unwrap();

    let layers = layer(&output);
    assert!(layers
        .iter()
        .any(|unit| unit["mark"]["type"] == json!("rect")));
    assert!(layers
        .iter()
        .any(|unit| unit["mark"]["type"] == json!("rule")));
}

#[test]
fn should_produce_area_and_line_in_2d_error_band() {
    let output = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": {"type": "errorband", "borders": true},
            "encoding": {
                "y": {"field": "people", "type": "quantitative"},
                "x": {"field": "age", "type": "ordinal"}
            }
        }),
        None,
    )
    .unwrap();

    let layers = layer(&output);
    assert!(layers
        .iter()
        .any(|unit| unit["mark"]["type"] == json!("area")));
    assert!(layers
        .iter()
        .any(|unit| unit["mark"]["type"] == json!("line")));
}

#[test]
fn should_produce_interpolation_in_2d_error_band() {
    let output = normalize(
        &json!({
            "data": {"url": "data/population.json"},
            "mark": {"type": "errorband", "interpolate": "monotone"},
            "encoding": {
                "y": {"field": "people", "type": "quantitative"},
                "x": {"field": "age", "type": "ordinal"}
            }
        }),
        None,
    )
    .unwrap();

    let layers = layer(&output);
    assert!(layers
        .iter()
        .all(|unit| unit["mark"]["interpolate"] == json!("monotone")));
}

#[test]
fn should_produce_no_interpolation_in_1d_error_band() {
    let (output, logger) = log::capture(|| {
        normalize(
            &json!({
                "data": {"url": "data/population.json"},
                "mark": {"type": "errorband", "interpolate": "bundle", "tension": 1},
                "encoding": {"y": {"field": "people", "type": "quantitative"}}
            }),
            None,
        )
    });
    let output = output.unwrap();

    let layers = layer(&output);
    assert!(layers
        .iter()
        .all(|unit| unit["mark"].get("interpolate").is_none()));
    assert_eq!(
        logger.warns[0],
        json!("1D error band does not support interpolate.")
    );
}

#[test]
fn should_warn_when_1d_error_band_has_interpolate_property() {
    let (_, logger) = log::capture(|| {
        normalize(
            &json!({
                "data": {"url": "data/population.json"},
                "mark": {"type": "errorband", "interpolate": "monotone"},
                "encoding": {"y": {"field": "people", "type": "quantitative"}}
            }),
            None,
        )
    });

    assert_eq!(
        logger.warns[0],
        json!("1D error band does not support interpolate.")
    );
}

#[test]
fn should_warn_when_1d_error_band_has_tension_property() {
    let (_, logger) = log::capture(|| {
        normalize(
            &json!({
                "data": {"url": "data/population.json"},
                "mark": {"type": "errorband", "tension": 1},
                "encoding": {"y": {"field": "people", "type": "quantitative"}}
            }),
            None,
        )
    });

    assert_eq!(
        logger.warns[0],
        json!("1D error band does not support tension.")
    );
}

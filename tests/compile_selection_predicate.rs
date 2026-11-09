//! Port of `test/compile/selection/predicate.test.ts`.

use serde_json::{json, Value};

use vega_lite::compile::model::Model;
use vega_lite::compile::selection::parse::{
    parse_selection_predicate, parse_unit_selection,
};
use vega_lite::compile::support::init_config;
use vega_lite::compile_predicate::expression_with_model;

fn predicate_model() -> Model {
    let spec = json!({
        "mark": "circle",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative"},
            "color": {
                "field": "Cylinders",
                "type": "ordinal",
                "condition": {"param": "one", "value": "grey"}
            },
            "opacity": {
                "field": "Acceleration",
                "type": "quantitative",
                "condition": {"param": "two", "empty": false, "value": 0.5}
            },
            "size": {
                "value": 50,
                "condition": {"param": "varHelloWorld", "field": "Displacement", "type": "quantitative"}
            }
        }
    });
    let config = init_config(None);
    let mut model = Model::new_unit(&spec, None, "", None, &config);
    model.parse_scale();
    let components = parse_unit_selection(
        &mut model,
        &[
            json!({"name": "one", "select": "point"}),
            json!({"name": "two", "select": {"type": "point", "resolve": "union"}}),
            json!({"name": "thr-ee", "select": {"type": "interval", "resolve": "intersect"}}),
        ],
    );
    model.component.selection = serde_json::to_value(&components).unwrap();
    model
}

fn predicate(model: &Model, pred: Value) -> String {
    parse_selection_predicate(model, &pred, "datum")
}

#[test]
fn generates_the_predicate_expression() {
    let model = predicate_model();

    // Different resolutions.
    assert_eq!(
        predicate(&model, json!({"param": "one"})),
        "!length(data(\"one_store\")) || vlSelectionIdTest(\"one_store\", datum)"
    );
    assert_eq!(
        predicate(&model, json!({"param": "two"})),
        "!length(data(\"two_store\")) || vlSelectionIdTest(\"two_store\", datum, \"union\")"
    );
    assert_eq!(
        predicate(&model, json!({"param": "thr-ee"})),
        "!length(data(\"thr_ee_store\")) || vlSelectionTest(\"thr_ee_store\", datum, \"intersect\")"
    );

    // Different emptiness.
    assert_eq!(
        predicate(&model, json!({"param": "one", "empty": true})),
        "!length(data(\"one_store\")) || vlSelectionIdTest(\"one_store\", datum)"
    );
    assert_eq!(
        predicate(&model, json!({"param": "two", "empty": false})),
        "length(data(\"two_store\")) && vlSelectionIdTest(\"two_store\", datum, \"union\")"
    );

    // Variable parameters.
    assert_eq!(
        predicate(&model, json!({"param": "helloworld"})),
        "!!helloworld"
    );
}

#[test]
fn generates_a_selection_filter() {
    let model = predicate_model();

    assert_eq!(
        expression_with_model(&model, &json!({"param": "one"})),
        "!length(data(\"one_store\")) || vlSelectionIdTest(\"one_store\", datum)"
    );
    assert_eq!(
        expression_with_model(&model, &json!({"not": {"param": "two", "empty": false}})),
        "!(length(data(\"two_store\")) && vlSelectionIdTest(\"two_store\", datum, \"union\"))"
    );
    assert_eq!(
        expression_with_model(&model, &json!({"not": {"param": "varHelloWorld"}})),
        "!(!!varHelloWorld)"
    );
}

#[test]
fn composes_selection_and_field_predicates() {
    let model = predicate_model();
    assert_eq!(
        expression_with_model(
            &model,
            &json!({"and": [
                {"param": "one"},
                {"field": "Weight_in_lbs", "gt": 3000}
            ]})
        ),
        "(!length(data(\"one_store\")) || vlSelectionIdTest(\"one_store\", datum)) && \
         (datum[\"Weight_in_lbs\"]>3000)"
    );
}

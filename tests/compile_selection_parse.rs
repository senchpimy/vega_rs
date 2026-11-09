//! Port of `test/compile/selection/parse.test.ts`.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use vega_lite::compile::model::Model;
use vega_lite::compile::selection::component::SelectionComponent;
use vega_lite::compile::selection::parse::{materialize_selections, parse_unit_selection};
use vega_lite::compile::selection::project;
use vega_lite::compile::selection::selector::parse_selector;
use vega_lite::compile::support::init_config;

fn parse_unit_model(spec: &Value) -> Model {
    let config = init_config(spec.get("config"));
    Model::new_unit(spec, None, "", None, &config)
}

fn parse_unit_model_with_scale(spec: &Value) -> Model {
    let mut model = parse_unit_model(spec);
    model.parse_scale();
    model
}

fn parse_sel(model: &mut Model, defs: &[Value]) -> BTreeMap<String, SelectionComponent> {
    let components = parse_unit_selection(model, defs);
    model.component.selection = serde_json::to_value(&components).unwrap();
    components
}

fn items(component: &SelectionComponent) -> Value {
    serde_json::to_value(&component.project.items).unwrap()
}

fn default_model() -> Model {
    let mut model = parse_unit_model(&json!({
        "mark": "circle",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative"},
            "color": {"field": "Origin", "type": "nominal"}
        }
    }));
    model.parse_scale();
    model
}

#[test]
fn parses_default_selection_definitions() {
    let model = &mut default_model();
    let component = parse_sel(
        model,
        &[
            json!({"name": "one", "select": "point"}),
            json!({"name": "two", "select": {"type": "interval"}}),
        ],
    );

    assert_eq!(
        component.keys().cloned().collect::<Vec<_>>(),
        vec!["one".to_string(), "two".to_string()]
    );

    let one = &component["one"];
    assert_eq!(one.name, "one");
    assert_eq!(one.type_, "point");
    assert!(items(one).as_array().unwrap().contains(
        &json!({"field": "_vgsid_", "index": 0, "type": "E", "signals": {"data": "one__vgsid_"}})
    ));
    assert_eq!(
        serde_json::to_value(&one.events).unwrap(),
        serde_json::to_value(parse_selector("click", "scope")).unwrap()
    );

    let two = &component["two"];
    assert_eq!(two.name, "two");
    assert_eq!(two.type_, "interval");
    assert_eq!(
        two.translate,
        Some(json!(
            "[pointerdown, window:pointerup] > window:pointermove!"
        ))
    );
    assert_eq!(two.zoom, Some(json!("wheel!")));
    let two_items = items(two);
    assert!(two_items.as_array().unwrap().contains(&json!({
        "field": "Horsepower", "channel": "x", "index": 0, "type": "R",
        "signals": {"data": "two_Horsepower", "visual": "two_x"}
    })));
    assert!(two_items.as_array().unwrap().contains(&json!({
        "field": "Miles_per_Gallon", "channel": "y", "index": 1, "type": "R",
        "signals": {"data": "two_Miles_per_Gallon", "visual": "two_y"}
    })));
    assert_eq!(
        serde_json::to_value(&two.events).unwrap(),
        serde_json::to_value(parse_selector(
            "[pointerdown[!event.item || event.item.mark.name !== \"two_brush\"], window:pointerup] > window:pointermove!",
            "scope"
        ))
        .unwrap()
    );
}

#[test]
fn supports_inline_default_overrides() {
    let model = &mut default_model();
    let component = parse_sel(
        model,
        &[
            json!({"name": "one", "select": {"type": "point", "on": "dblclick", "fields": ["Cylinders"]}}),
            json!({"name": "two", "select": {"type": "point", "on": "pointerover", "toggle": "event.ctrlKey", "encodings": ["color"]}}),
            json!({"name": "three", "select": {"type": "interval", "on": "[pointerdown[!event.shiftKey], pointerup] > pointermove", "encodings": ["y"], "translate": false, "zoom": "wheel[event.altKey]"}}),
        ],
    );

    assert_eq!(
        component.keys().cloned().collect::<Vec<_>>(),
        vec!["one", "three", "two"]
    );

    assert!(items(&component["one"])
        .as_array()
        .unwrap()
        .contains(&json!({"field": "Cylinders", "index": 0, "type": "E", "signals": {"data": "one_Cylinders"}})));
    assert_eq!(
        serde_json::to_value(&component["one"].events).unwrap(),
        serde_json::to_value(parse_selector("dblclick", "scope")).unwrap()
    );

    assert_eq!(component["two"].toggle, Some(json!("event.ctrlKey")));
    assert!(items(&component["two"])
        .as_array()
        .unwrap()
        .contains(&json!({"field": "Origin", "channel": "color", "index": 0, "type": "E", "signals": {"data": "two_Origin", "visual": "two_color"}})));

    assert_eq!(component["three"].translate, Some(Value::Bool(false)));
    assert_eq!(component["three"].zoom, Some(json!("wheel[event.altKey]")));
    assert!(items(&component["three"])
        .as_array()
        .unwrap()
        .contains(&json!({"field": "Miles_per_Gallon", "channel": "y", "index": 0, "type": "R", "signals": {"data": "three_Miles_per_Gallon", "visual": "three_y"}})));
    assert_eq!(
        serde_json::to_value(&component["three"].events).unwrap(),
        serde_json::to_value(parse_selector(
            "[pointerdown[!event.shiftKey], pointerup] > pointermove",
            "scope"
        ))
        .unwrap()
    );
}

#[test]
fn respects_selection_configs() {
    let model = &mut default_model();
    model.config["selection"] = json!({
        "point": {"on": "pointerover", "encodings": ["color"], "toggle": "event.ctrlKey"},
        "interval": {"on": "[pointerdown[!event.shiftKey], pointerup] > pointermove", "encodings": ["y"], "zoom": "wheel[event.altKey]"}
    });

    let component = parse_sel(
        model,
        &[
            json!({"name": "one", "select": "point"}),
            json!({"name": "two", "select": {"type": "interval"}}),
        ],
    );

    let one = &component["one"];
    assert_eq!(one.toggle, Some(json!("event.ctrlKey")));
    assert!(items(one)
        .as_array()
        .unwrap()
        .contains(&json!({"field": "Origin", "channel": "color", "index": 0, "type": "E", "signals": {"data": "one_Origin", "visual": "one_color"}})));
    assert_eq!(
        serde_json::to_value(&one.events).unwrap(),
        serde_json::to_value(parse_selector("pointerover", "scope")).unwrap()
    );

    let two = &component["two"];
    assert!(!vega_lite::compile::selection::component::truthy(
        two.translate.as_ref()
    ));
    assert_eq!(two.zoom, Some(json!("wheel[event.altKey]")));
    assert!(items(two)
        .as_array()
        .unwrap()
        .contains(&json!({"field": "Miles_per_Gallon", "channel": "y", "index": 0, "type": "R", "signals": {"data": "two_Miles_per_Gallon", "visual": "two_y"}})));
    assert_eq!(
        serde_json::to_value(&two.events).unwrap(),
        serde_json::to_value(parse_selector(
            "[pointerdown[!event.shiftKey], pointerup] > pointermove",
            "scope"
        ))
        .unwrap()
    );
}

#[test]
fn uses_enumerated_types_for_interval_selections() {
    let mut m = parse_unit_model(&json!({
        "mark": "circle",
        "encoding": {
            "x": {"field": "Origin", "type": "nominal"},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative"}
        }
    }));
    m.parse_scale();
    let c = parse_sel(
        &mut m,
        &[json!({"name": "one", "select": {"type": "interval", "encodings": ["x"]}})],
    );
    assert!(items(&c["one"])
        .as_array()
        .unwrap()
        .contains(&json!({"field": "Origin", "channel": "x", "index": 0, "type": "E", "signals": {"data": "one_Origin", "visual": "one_x"}})));

    let mut m = parse_unit_model(&json!({
        "mark": "bar",
        "encoding": {
            "x": {"field": "Origin", "type": "nominal"},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative"}
        }
    }));
    m.parse_scale();
    let c = parse_sel(
        &mut m,
        &[json!({"name": "one", "select": {"type": "interval", "encodings": ["x"]}})],
    );
    assert!(items(&c["one"])
        .as_array()
        .unwrap()
        .contains(&json!({"field": "Origin", "channel": "x", "index": 0, "type": "E", "signals": {"data": "one_Origin", "visual": "one_x"}})));
}

#[test]
fn uses_ranged_types_for_point_selections() {
    let mut m = parse_unit_model_with_scale(&json!({
        "mark": "circle",
        "encoding": {
            "x": {"field": "Acceleration", "type": "quantitative", "bin": true},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative"}
        }
    }));
    let c = parse_sel(
        &mut m,
        &[json!({"name": "one", "select": {"type": "point", "encodings": ["x"]}})],
    );
    assert!(items(&c["one"])
        .as_array()
        .unwrap()
        .contains(&json!({"field": "Acceleration", "channel": "x", "index": 0, "type": "R-RE", "signals": {"data": "one_Acceleration", "visual": "one_x"}})));
}

#[test]
fn infers_from_initial_values() {
    let model = &mut default_model();
    let component = parse_sel(
        model,
        &[
            json!({"name": "one", "select": "point", "value": [{"Origin": 5}]}),
            json!({"name": "two", "select": "point", "value": [{"color": 10}]}),
            json!({"name": "three", "select": {"type": "interval"}, "value": {"x": [10, 100]}}),
        ],
    );

    assert!(items(&component["one"]).as_array().unwrap().contains(
        &json!({"field": "Origin", "index": 0, "type": "E", "signals": {"data": "one_Origin"}})
    ));
    assert!(items(&component["two"])
        .as_array()
        .unwrap()
        .contains(&json!({"channel": "color", "field": "Origin", "index": 0, "type": "E", "signals": {"data": "two_Origin", "visual": "two_color"}})));
    assert!(items(&component["three"])
        .as_array()
        .unwrap()
        .contains(&json!({"field": "Horsepower", "channel": "x", "index": 0, "type": "R", "signals": {"data": "three_Horsepower", "visual": "three_x"}})));
}

#[test]
fn escapes_flattened_fields() {
    let model = &mut default_model();
    let component = parse_sel(
        model,
        &[
            json!({"name": "one", "select": {"type": "point", "fields": ["nested.a", "nested.b.aa"]}}),
        ],
    );

    assert_eq!(
        items(&component["one"]),
        json!([
            {"field": "nested.a", "index": 0, "type": "E", "signals": {"data": "one_nested_a"}},
            {"field": "nested.b.aa", "index": 1, "type": "E", "signals": {"data": "one_nested_b_aa"}}
        ])
    );

    assert_eq!(
        project::signals(model, &component["one"], vec![]),
        vec![json!({
            "name": "one_tuple_fields",
            "value": [
                {"field": "nested\\.a", "type": "E"},
                {"field": "nested\\.b\\.aa", "type": "E"}
            ]
        })]
    );
}

#[test]
fn does_not_add_duplicate_projections() {
    let model = &mut default_model();
    let component = parse_sel(
        model,
        &[
            json!({"name": "one", "select": {"type": "point", "fields": ["Horsepower", "Miles_per_Gallon"], "encodings": ["x", "y"]}}),
        ],
    );

    assert_eq!(
        items(&component["one"]),
        json!([
            {"field": "Horsepower", "channel": "x", "index": 0, "type": "E", "signals": {"data": "one_Horsepower", "visual": "one_x"}},
            {"field": "Miles_per_Gallon", "channel": "y", "index": 1, "type": "E", "signals": {"data": "one_Miles_per_Gallon", "visual": "one_y"}}
        ])
    );
}

#[test]
fn materializes_a_selection() {
    // The dataflow/lookup transform has not been ported; the materialization is
    // represented by a lookup output node carrying the filter parameter.
    let mut model = default_model();
    let _ = parse_sel(
        &mut model,
        &[
            json!({"name": "index", "select": {"type": "point", "on": "pointerover", "encodings": ["x"], "nearest": true}}),
        ],
    );
    materialize_selections(&mut model, "source_0");
    assert!(model
        .component
        .data
        .output_nodes
        .contains_key("lookup_index"));
    assert_eq!(
        model.component.data.output_nodes["lookup_index"],
        json!({"name": "lookup_index", "type": "filter", "param": "index", "source": "source_0"})
    );
}

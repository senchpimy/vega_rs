//! Port of `test/compile/selection/legends.test.ts`.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use vega_lite::compile::model::Model;
use vega_lite::compile::selection::assemble::{
    assemble_top_level_signals, assemble_unit_selection_signals,
};
use vega_lite::compile::selection::component::SelectionComponent;
use vega_lite::compile::selection::legends;
use vega_lite::compile::selection::message;
use vega_lite::compile::selection::parse::parse_unit_selection;
use vega_lite::compile::support::init_config;
use vega_lite::log;

fn parse_unit_model(spec: &Value) -> Model {
    let config = init_config(spec.get("config"));
    Model::new_unit(spec, None, "", None, &config)
}

fn defs() -> Vec<Value> {
    vec![
        json!({"name": "one", "select": {"type": "point", "fields": ["Origin"]}, "bind": "legend"}),
        json!({"name": "two", "select": {"type": "point", "fields": ["Origin"]}, "bind": {"legend": "dblclick, pointerover"}}),
        json!({"name": "four", "select": {"type": "point", "encodings": ["color"]}, "bind": "legend"}),
        json!({"name": "seven", "select": {"type": "point", "fields": ["Origin"], "on": "click"}, "bind": {"legend": "pointerover"}}),
        json!({"name": "eight", "select": {"type": "point", "encodings": ["color"], "on": "click", "clear": "dblclick"}, "bind": {"legend": "pointerover"}}),
        json!({"name": "nine", "select": "point", "bind": {"input": "range", "min": 0, "max": 10, "step": 1}}),
        json!({"name": "ten", "value": [{"Origin": "USA"}, {"Origin": "Japan"}], "select": {"type": "point", "fields": ["Origin"]}, "bind": "legend"}),
    ]
}

fn legend_model() -> Model {
    let mut model = parse_unit_model(&json!({
        "mark": "circle",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative"},
            "color": {"field": "Origin", "type": "nominal"},
            "size": {"field": "Cylinders", "type": "ordinal"}
        }
    }));
    model.parse_scale();

    let components = parse_unit_selection(&mut model, &defs());
    model.component.selection = serde_json::to_value(&components).unwrap();

    // Emulate `parseLegends()` -> `parseInteractiveLegend`.
    for channel in ["color", "size"] {
        let mut legend_cmpt = vega_lite::compile::legend::component::LegendComponent::default();
        legends::parse_interactive_legend(&mut model, channel, &mut legend_cmpt);
    }
    model
}

fn components(model: &Model) -> BTreeMap<String, SelectionComponent> {
    vega_lite::compile::selection::component::selection_components(model)
}

#[test]
fn drops_invalid_legend_binding_without_projection() {
    let mut model = parse_unit_model(&json!({
        "mark": "circle",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative"},
            "color": {"field": "Origin", "type": "nominal"},
            "size": {"field": "Cylinders", "type": "ordinal"}
        }
    }));
    model.parse_scale();

    let (sel_cmpts, logger) = log::capture(|| {
        let components = parse_unit_selection(
            &mut model,
            &[
                json!({"name": "three", "select": {"type": "point", "fields": ["Origin", "Cylinders"]}, "bind": "legend"}),
                json!({"name": "five", "select": {"type": "point", "encodings": ["color", "size"]}, "bind": "legend"}),
                json!({"name": "six", "select": "point", "bind": "legend"}),
            ],
        );
        model.component.selection = serde_json::to_value(&components).unwrap();
        components
    });

    assert!(!legends::defined(&sel_cmpts["three"]));
    assert_eq!(
        logger.warns[0],
        json!(message::LEGEND_BINDINGS_MUST_HAVE_PROJECTION)
    );
    assert!(!legends::defined(&sel_cmpts["five"]));
    assert_eq!(
        logger.warns[1],
        json!(message::LEGEND_BINDINGS_MUST_HAVE_PROJECTION)
    );
    assert!(!legends::defined(&sel_cmpts["six"]));
    assert_eq!(
        logger.warns[2],
        json!(message::LEGEND_BINDINGS_MUST_HAVE_PROJECTION)
    );
}

#[test]
fn identifies_transform_invocation() {
    let model = legend_model();
    let sel_cmpts = components(&model);
    assert!(legends::defined(&sel_cmpts["one"]));
    assert!(legends::defined(&sel_cmpts["two"]));
    assert!(legends::defined(&sel_cmpts["four"]));
    assert!(legends::defined(&sel_cmpts["seven"]));
    assert!(legends::defined(&sel_cmpts["eight"]));
    assert!(!legends::defined(&sel_cmpts["nine"]));
    assert!(legends::defined(&sel_cmpts["ten"]));
}

#[test]
fn adds_legend_binding_top_level_signal() {
    let model = legend_model();
    let signals = assemble_top_level_signals(&model, vec![]);

    for expected in [
        json!({
            "name": "one_Origin_legend",
            "value": null,
            "on": [
                {
                    "events": [
                        {"source": "view", "type": "click", "markname": "Origin_legend_symbols"},
                        {"source": "view", "type": "click", "markname": "Origin_legend_labels"},
                        {"source": "view", "type": "click", "markname": "Origin_legend_entries"},
                    ],
                    "update": "isDefined(datum.value) ? datum.value : item().items[0].items[0].datum.value",
                    "force": true,
                },
                {
                    "events": [{"source": "view", "type": "click"}],
                    "update": "!event.item || !datum ? null : one_Origin_legend",
                    "force": true,
                },
            ],
        }),
        json!({
            "name": "two_Origin_legend",
            "value": null,
            "on": [
                {
                    "events": [
                        {"source": "view", "type": "dblclick", "markname": "Origin_legend_symbols"},
                        {"source": "view", "type": "pointerover", "markname": "Origin_legend_symbols"},
                        {"source": "view", "type": "dblclick", "markname": "Origin_legend_labels"},
                        {"source": "view", "type": "pointerover", "markname": "Origin_legend_labels"},
                        {"source": "view", "type": "dblclick", "markname": "Origin_legend_entries"},
                        {"source": "view", "type": "pointerover", "markname": "Origin_legend_entries"},
                    ],
                    "update": "isDefined(datum.value) ? datum.value : item().items[0].items[0].datum.value",
                    "force": true,
                },
                {
                    "events": [
                        {"source": "view", "type": "dblclick"},
                        {"source": "view", "type": "pointerover"},
                    ],
                    "update": "!event.item || !datum ? null : two_Origin_legend",
                    "force": true,
                },
            ],
        }),
        json!({
            "name": "four_Origin_legend",
            "value": null,
            "on": [
                {
                    "events": [
                        {"source": "view", "type": "click", "markname": "Origin_legend_symbols"},
                        {"source": "view", "type": "click", "markname": "Origin_legend_labels"},
                        {"source": "view", "type": "click", "markname": "Origin_legend_entries"},
                    ],
                    "update": "isDefined(datum.value) ? datum.value : item().items[0].items[0].datum.value",
                    "force": true,
                },
                {
                    "events": [{"source": "view", "type": "click"}],
                    "update": "!event.item || !datum ? null : four_Origin_legend",
                    "force": true,
                },
            ],
        }),
        json!({
            "name": "seven_Origin_legend",
            "value": null,
            "on": [
                {
                    "events": [
                        {"source": "view", "type": "pointerover", "markname": "Origin_legend_symbols"},
                        {"source": "view", "type": "pointerover", "markname": "Origin_legend_labels"},
                        {"source": "view", "type": "pointerover", "markname": "Origin_legend_entries"},
                    ],
                    "update": "isDefined(datum.value) ? datum.value : item().items[0].items[0].datum.value",
                    "force": true,
                },
                {
                    "events": [{"source": "view", "type": "pointerover"}],
                    "update": "!event.item || !datum ? null : seven_Origin_legend",
                    "force": true,
                },
            ],
        }),
        json!({
            "name": "eight_Origin_legend",
            "value": null,
            "on": [
                {
                    "events": [
                        {"source": "view", "type": "pointerover", "markname": "Origin_legend_symbols"},
                        {"source": "view", "type": "pointerover", "markname": "Origin_legend_labels"},
                        {"source": "view", "type": "pointerover", "markname": "Origin_legend_entries"},
                    ],
                    "update": "isDefined(datum.value) ? datum.value : item().items[0].items[0].datum.value",
                    "force": true,
                },
                {
                    "events": [{"source": "view", "type": "pointerover"}],
                    "update": "!event.item || !datum ? null : eight_Origin_legend",
                    "force": true,
                },
            ],
        }),
    ] {
        assert!(
            signals.contains(&expected),
            "missing {expected}\n{}",
            serde_json::to_string_pretty(&signals).unwrap()
        );
    }
}

#[test]
fn updates_tuple_signal_to_use_bound_top_level_signal() {
    let model = legend_model();
    let signals = assemble_unit_selection_signals(&model, vec![]);
    for expected in [
        json!({"name": "one_tuple", "update": "one_Origin_legend !== null ? {fields: one_tuple_fields, values: [one_Origin_legend]} : null"}),
        json!({"name": "two_tuple", "update": "two_Origin_legend !== null ? {fields: two_tuple_fields, values: [two_Origin_legend]} : null"}),
        json!({"name": "four_tuple", "update": "four_Origin_legend !== null ? {fields: four_tuple_fields, values: [four_Origin_legend]} : null"}),
    ] {
        assert!(signals.contains(&expected), "missing {expected}");
    }
}

#[test]
fn preserves_explicit_event_triggers() {
    let model = legend_model();
    let signals = assemble_unit_selection_signals(&model, vec![]);

    assert!(signals.contains(&json!({
        "name": "seven_tuple",
        "on": [
            {
                "events": [{"source": "scope", "type": "click", "filter": ["event.item && indexof(event.item.mark.role, \"legend\") < 0"]}],
                "update": "datum && item().mark.marktype !== 'group' && indexof(item().mark.role, 'legend') < 0 ? {unit: \"\", fields: seven_tuple_fields, values: [(item().isVoronoi ? datum.datum : datum)[\"Origin\"]]} : null",
                "force": true,
            },
            {
                "events": [{"signal": "seven_Origin_legend"}],
                "update": "seven_Origin_legend !== null ? {fields: seven_tuple_fields, values: [seven_Origin_legend]} : null",
            },
        ],
    })));

    assert!(signals.contains(&json!({
        "name": "eight_tuple",
        "on": [
            {
                "events": [{"source": "scope", "type": "click", "filter": ["event.item && indexof(event.item.mark.role, \"legend\") < 0"]}],
                "update": "datum && item().mark.marktype !== 'group' && indexof(item().mark.role, 'legend') < 0 ? {unit: \"\", fields: eight_tuple_fields, values: [(item().isVoronoi ? datum.datum : datum)[\"Origin\"]]} : null",
                "force": true,
            },
            {
                "events": [{"signal": "eight_Origin_legend"}],
                "update": "eight_Origin_legend !== null ? {fields: eight_tuple_fields, values: [eight_Origin_legend]} : null",
            },
            {
                "events": [{"source": "view", "type": "dblclick"}],
                "update": "null",
            },
        ],
    })));
}

#[test]
fn respects_initialization() {
    let model = legend_model();
    let signals = assemble_top_level_signals(&model, vec![]);
    assert!(signals.contains(&json!({
        "name": "ten_Origin_legend",
        "on": [
            {
                "events": [
                    {"source": "view", "type": "click", "markname": "Origin_legend_symbols"},
                    {"source": "view", "type": "click", "markname": "Origin_legend_labels"},
                    {"source": "view", "type": "click", "markname": "Origin_legend_entries"},
                ],
                "update": "isDefined(datum.value) ? datum.value : item().items[0].items[0].datum.value",
                "force": true,
            },
            {
                "events": [{"source": "view", "type": "click"}],
                "update": "!event.item || !datum ? null : ten_Origin_legend",
                "force": true,
            },
        ],
    })));
}

//! Tests for the selection assembly helpers.
//!
//! Upstream has no `test/compile/selection/assemble.test.ts`; these cases cover
//! the exported helpers of `src/compile/selection/assemble.ts` directly.

use serde_json::json;

use vega_lite::compile::model::Model;
use vega_lite::compile::selection::assemble::{
    assemble_facet_signals, assemble_init, assemble_projection,
};
use vega_lite::compile::selection::component::{
    ProjectionSignals, SelectionComponent, SelectionProjection,
};
use vega_lite::compile::support::init_config;
use vega_lite::parameter::assemble_parameter_signals;

#[test]
fn assemble_init_handles_scalars_arrays_and_datetimes() {
    assert_eq!(assemble_init(&json!(50), true), json!("50"));
    assert_eq!(assemble_init(&json!("Japan"), true), json!("\"Japan\""));
    assert_eq!(assemble_init(&json!([50, 70]), true), json!("[50, 70]"));
    assert_eq!(
        assemble_init(&json!([[50, 70], [23, 54]]), true),
        json!("[[50, 70], [23, 54]]")
    );
    assert_eq!(assemble_init(&json!(50), false), json!(50));
    assert_eq!(
        // Local datetimes follow the example-corpus timezone
        // (America/Los_Angeles, UTC-8 in January).
        assemble_init(&json!({"year": 1970, "month": 1, "date": 1}), false),
        json!(28_800_000)
    );
    assert_eq!(
        assemble_init(&json!({"year": 1970, "month": 1, "date": 1}), true),
        json!("datetime(1970, 0, 1, 0, 0, 0, 0)")
    );
}

#[test]
fn assemble_projection_strips_internal_properties() {
    let proj = SelectionProjection {
        type_: "E".to_string(),
        field: "nested.a".to_string(),
        index: 2,
        channel: Some("x".to_string()),
        geo_channel: None,
        signals: Some(ProjectionSignals {
            data: Some("sel_nested_a".to_string()),
            visual: Some("sel_x".to_string()),
        }),
        has_legend: true,
    };
    assert_eq!(
        assemble_projection(&proj),
        json!({"type": "E", "field": "nested\\.a", "channel": "x"})
    );
}

#[test]
fn assemble_facet_signals_prepends_facet_signal() {
    let config = init_config(None);
    let mut model = Model::new_unit(
        &json!({"mark": "point", "encoding": {"x": {"field": "a", "type": "quantitative"}}}),
        None,
        "",
        None,
        &config,
    );
    model.component.selection = json!({
        "one": {"name": "one", "type": "point", "resolve": "global", "project": {"items": []}}
    });

    let signals = assemble_facet_signals(&model, vec![]);
    assert_eq!(
        signals[0],
        json!({
            "name": "facet",
            "value": {},
            "on": [{
                "events": [{"source": "scope", "type": "pointermove"}],
                "update": "isTuple(facet) ? facet : group(\"cell\").datum",
            }],
        })
    );

    model.component.selection = json!({});
    assert_eq!(
        assemble_facet_signals(&model, vec![]),
        Vec::<serde_json::Value>::new()
    );
}

#[test]
fn assemble_parameter_signals_skips_selection_parameters() {
    let params = vec![
        json!({"name": "a", "value": 5}),
        json!({"name": "b", "expr": "a * 2"}),
        json!({"name": "c", "expr": "a + 1", "bind": {"input": "range"}}),
        json!({"name": "sel", "select": "point"}),
    ];
    assert_eq!(
        assemble_parameter_signals(&params),
        vec![
            json!({"name": "a", "value": 5}),
            json!({"name": "b", "update": "a * 2"}),
            json!({"name": "c", "bind": {"input": "range"}, "init": "a + 1"}),
        ]
    );
}

#[test]
fn empty_on_arrays_are_removed_from_assembled_signals() {
    use vega_lite::compile::selection::assemble::assemble_unit_selection_signals;

    let config = init_config(None);
    let mut model = Model::new_unit(
        &json!({"mark": "point", "encoding": {"x": {"field": "a", "type": "quantitative"}}}),
        None,
        "",
        None,
        &config,
    );
    let component = SelectionComponent {
        name: "one".to_string(),
        type_: "point".to_string(),
        resolve: "global".to_string(),
        events: None,
        ..Default::default()
    };
    model.component.selection = serde_json::to_value(
        [("one".to_string(), component)]
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>(),
    )
    .unwrap();

    let signals = assemble_unit_selection_signals(&model, vec![]);
    let tuple = signals
        .iter()
        .find(|s| s.get("name").and_then(|v| v.as_str()) == Some("one_tuple"))
        .unwrap();
    assert!(tuple.get("on").is_none());
}

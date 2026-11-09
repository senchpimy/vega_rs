//! Port of `test/compile/axis/assemble.test.ts`.

use serde_json::{json, Map, Value};

use vega_lite::compile::axis::assemble::assemble_axis;
use vega_lite::compile::axis::component::AxisComponent;
use vega_lite::compile::axis::parse::{assemble_model_axes, parse_axes_and_headers};
use vega_lite::compile::model::Model;
use vega_lite::compile::support::{default_config, init_config};

fn axis_component(explicit: Value) -> AxisComponent {
    let explicit: Map<String, Value> = explicit.as_object().unwrap().clone();
    AxisComponent::from_json_map(explicit, Map::new(), false)
}

#[test]
fn outputs_grid_axis_with_only_grid_encode_blocks() {
    let axis_cmpt = axis_component(json!({
        "orient": "left",
        "grid": true,
        "encode": {
            "grid": {"update": {"stroke": {"value": "red"}}},
            "labels": {"update": {"fill": {"value": "red"}}},
        },
    }));
    let axis = assemble_axis(&axis_cmpt, "grid", &default_config(), false).unwrap();
    assert!(axis.get("encode").and_then(|e| e.get("labels")).is_none());
}

#[test]
fn outputs_grid_axis_with_custom_zindex() {
    let axis_cmpt = axis_component(json!({"orient": "left", "grid": true, "zindex": 3}));
    let axis = assemble_axis(&axis_cmpt, "grid", &default_config(), false).unwrap();
    assert_eq!(axis["zindex"], json!(3));
}

#[test]
fn outputs_main_axis_without_grid_encode_blocks() {
    let axis_cmpt = axis_component(json!({
        "orient": "left",
        "encode": {
            "grid": {"update": {"stroke": {"value": "red"}}},
            "labels": {"update": {"fill": {"value": "red"}}},
        },
    }));
    let axis = assemble_axis(&axis_cmpt, "main", &default_config(), false).unwrap();
    assert!(axis.get("encode").and_then(|e| e.get("grid")).is_none());
}

#[test]
fn correctly_assemble_title_fielddefs() {
    let axis_cmpt = axis_component(json!({
        "orient": "left",
        "title": [
            {"aggregate": "max", "field": "a"},
            {"aggregate": "min", "field": "b"},
        ],
    }));
    let axis = assemble_axis(&axis_cmpt, "main", &default_config(), false).unwrap();
    assert_eq!(axis["title"], json!("Max of a, Min of b"));
}

#[test]
fn correctly_redirect_color_signal() {
    let axis_cmpt = axis_component(json!({"labelColor": {"signal": "a"}}));
    let axis = assemble_axis(&axis_cmpt, "main", &default_config(), false).unwrap();
    assert_eq!(
        axis["encode"]["labels"]["update"]["fill"],
        json!({"signal": "a"})
    );
}

#[test]
fn correctly_applies_conditional_axis_tick_size() {
    let axis_cmpt = axis_component(json!({
        "tickSize": {
            "condition": {"test": "datum.index === 0 || datum.index === 1", "value": 4},
            "value": 2,
        },
    }));
    let axis = assemble_axis(&axis_cmpt, "main", &default_config(), false).unwrap();
    assert_eq!(
        axis["tickSize"],
        json!({"signal": "datum.index === 0 || datum.index === 1 ? 4 : 2"})
    );
}

#[test]
fn correctly_applies_conditional_signal_for_axis_tick_size() {
    let axis_cmpt = axis_component(json!({
        "tickSize": {
            "condition": {"test": "datum.index === 0 || datum.index === 1", "signal": "a"},
            "signal": "b",
        },
    }));
    let axis = assemble_axis(&axis_cmpt, "main", &default_config(), false).unwrap();
    assert_eq!(
        axis["tickSize"],
        json!({"signal": "datum.index === 0 || datum.index === 1 ? a : b"})
    );
}

#[test]
fn outputs_grid_axis_with_aria_false() {
    let axis_cmpt = axis_component(json!({"orient": "left", "grid": true}));
    let axis = assemble_axis(&axis_cmpt, "grid", &default_config(), false).unwrap();
    assert_eq!(axis["aria"], json!(false));
}

#[test]
fn outputs_aria_false_if_set_in_config() {
    let axis_cmpt = axis_component(json!({"orient": "left"}));
    let mut config = default_config();
    config["aria"] = json!(false);
    let axis = assemble_axis(&axis_cmpt, "main", &config, false).unwrap();
    assert_eq!(axis["aria"], json!(false));
}

#[test]
fn correctly_applies_custom_formatter_to_label_expr() {
    let spec = json!({
        "data": {"url": "data/cars.json"},
        "mark": "point",
        "encoding": {
            "y": {
                "field": "Miles_per_Gallon",
                "type": "quantitative",
                "axis": {
                    "format": {"a": "b"},
                    "formatType": "myFormat",
                    "labelExpr": "datum.label[0]",
                },
            },
        },
        "config": {"customFormatTypes": true},
    });
    let config = init_config(spec.get("config"));
    let mut model = Model::new_unit(&spec, None, "", None, &config);
    model.parse_scale();
    parse_axes_and_headers(&mut model);

    let axes = assemble_model_axes(&model);
    assert_eq!(
        axes[1]["encode"]["labels"]["update"]["text"],
        json!({"signal": "myFormat(datum.value, {\"a\":\"b\"})[0]"})
    );
}

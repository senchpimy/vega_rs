//! Port of `test/config.test.ts`.

use serde_json::{json, Map, Value};

use vega_lite::config::{
    default_color, default_config, default_font_size, init_config, is_vg_scheme,
    strip_and_redirect_config,
};
use vega_lite::mark::PRIMITIVE_MARKS;

/// Recursive subset matcher implementing Jest's `toMatchObject`.
fn assert_match_object(actual: &Value, expected: &Value) {
    match (actual, expected) {
        (Value::Object(actual), Value::Object(expected)) => {
            for (key, expected_value) in expected {
                let actual_value = actual
                    .get(key)
                    .unwrap_or_else(|| panic!("expected key `{key}` in {actual:?}"));
                assert_match_object(actual_value, expected_value);
            }
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(
                actual.len(),
                expected.len(),
                "array lengths differ: {actual:?} vs {expected:?}"
            );
            for (actual_value, expected_value) in actual.iter().zip(expected) {
                assert_match_object(actual_value, expected_value);
            }
        }
        _ => assert_eq!(actual, expected),
    }
}

fn named_value(name: &str, value: Value) -> Value {
    let mut signal = Map::new();
    signal.insert("name".to_string(), Value::String(name.to_string()));
    signal.insert("value".to_string(), value);
    Value::Object(signal)
}

fn with_prop(mut object: Value, key: &str, value: Value) -> Value {
    object
        .as_object_mut()
        .expect("object")
        .insert(key.to_string(), value);
    object
}

#[test]
fn init_config_converts_exprs_in_config_to_signals() {
    let config = init_config(&json!({
        "background": {"expr": "'yellow'"},
        "padding": {"expr": "5"},
        "mark": {"color": {"expr": "'red'"}},
        "point": {"color": {"expr": "'green'"}},
        "scale": {
            "bandPaddingInner": {"expr": "0.1"},
        },
        "style": {
            "foo": {"color": {"expr": "'blue'"}},
        },
        "title": {"color": {"expr": "'violet'"}},
    }));

    assert_match_object(
        &config,
        &json!({
            "background": {"signal": "'yellow'"},
            "padding": {"signal": "5"},
            "mark": {"color": {"signal": "'red'"}},
            "point": {"color": {"signal": "'green'"}},
            "scale": {
                "bandPaddingInner": {"signal": "0.1"},
            },
            "style": {
                "foo": {"color": {"signal": "'blue'"}},
            },
            "title": {"color": {"signal": "'violet'"}},
        }),
    );
}

#[test]
fn init_config_produces_correct_default_color_font_and_font_size_config() {
    let config = init_config(&json!({"color": true, "fontSize": true, "font": "abc"}));

    let mut expected = default_config();
    let expected_object = expected.as_object_mut().unwrap();

    expected_object.insert(
        "signals".to_string(),
        json!([
            named_value("fontSize", default_font_size()),
            named_value("color", default_color()),
        ]),
    );

    let mark = with_prop(
        expected_object["mark"].clone(),
        "color",
        json!({"signal": "color.blue"}),
    );
    expected_object.insert("mark".to_string(), mark);
    expected_object.insert(
        "rule".to_string(),
        json!({"color": {"signal": "color.gray0"}}),
    );
    expected_object.insert(
        "text".to_string(),
        json!({
            "color": {"signal": "color.gray0"},
            "font": "abc",
            "fontSize": {"signal": "fontSize.text"},
        }),
    );
    expected_object.insert(
        "style".to_string(),
        json!({
            "guide-label": {
                "fill": {"signal": "color.gray0"},
                "font": "abc",
                "fontSize": {"signal": "fontSize.guideLabel"},
            },
            "guide-title": {
                "fill": {"signal": "color.gray0"},
                "font": "abc",
                "fontSize": {"signal": "fontSize.guideTitle"},
            },
            "group-title": {
                "fill": {"signal": "color.gray0"},
                "font": "abc",
                "fontSize": {"signal": "fontSize.groupTitle"},
            },
            "group-subtitle": {
                "fill": {"signal": "color.gray0"},
                "font": "abc",
                "fontSize": {"signal": "fontSize.groupSubtitle"},
            },
            "cell": {
                "stroke": {"signal": "color.gray8"},
            },
        }),
    );
    expected_object.insert(
        "axis".to_string(),
        json!({
            "domainColor": {"signal": "color.gray13"},
            "gridColor": {"signal": "color.gray8"},
            "tickColor": {"signal": "color.gray13"},
        }),
    );
    expected_object.insert(
        "range".to_string(),
        json!({
            "category": [
                {"signal": "color.blue"},
                {"signal": "color.orange"},
                {"signal": "color.red"},
                {"signal": "color.teal"},
                {"signal": "color.green"},
                {"signal": "color.yellow"},
                {"signal": "color.purple"},
                {"signal": "color.pink"},
                {"signal": "color.brown"},
                {"signal": "color.grey8"},
            ],
        }),
    );

    assert_eq!(config, expected);
}

#[test]
fn init_config_correctly_merges_color_signals_for_color_and_font_size_config() {
    let config = init_config(&json!({
        "color": {"newColor": "red"},
        "fontSize": {"newFontSize": 123},
    }));

    let mut font_size = default_font_size();
    font_size
        .as_object_mut()
        .unwrap()
        .insert("newFontSize".to_string(), json!(123));

    let mut color = default_color();
    color
        .as_object_mut()
        .unwrap()
        .insert("newColor".to_string(), json!("red"));

    assert_eq!(
        config["signals"],
        json!([
            named_value("fontSize", font_size),
            named_value("color", color),
        ])
    );
}

#[test]
fn init_config_correctly_merges_object_valued_configs() {
    let cfg = init_config(&json!({
        "selection": {
            "point": {"on": "pointerover"},
            "interval": {"encodings": ["x"]},
        },
    }));

    assert!(cfg["selection"].get("point").is_some());
    assert!(cfg["selection"].get("interval").is_some());

    // Overrides correctly.
    assert_eq!(cfg["selection"]["point"]["on"], json!("pointerover"));
    assert_eq!(cfg["selection"]["interval"]["encodings"], json!(["x"]));

    // Preserves defaults.
    assert_eq!(cfg["selection"]["point"]["toggle"], json!("event.shiftKey"));
    assert_eq!(cfg["selection"]["interval"]["zoom"], json!("wheel!"));
}

fn example_config() -> Value {
    let default = default_config();
    let mut config = default.as_object().unwrap().clone();

    let mut mark = default["mark"].as_object().unwrap().clone();
    mark.insert("opacity".to_string(), json!(0.3));
    mark.insert("tooltip".to_string(), json!({"content": "encoding"}));
    config.insert("mark".to_string(), Value::Object(mark));

    let mut bar = Map::new();
    bar.insert("opacity".to_string(), json!(0.5));
    for (key, value) in default["bar"].as_object().unwrap() {
        bar.insert(key.clone(), value.clone());
    }
    config.insert("bar".to_string(), Value::Object(bar));

    config.insert(
        "axis".to_string(),
        json!({
            "gridDash": {
                "condition": {
                    "test": {"field": "value", "timeUnit": "month", "equal": 1},
                    "value": null,
                },
                "value": [2, 2],
            },
        }),
    );

    config.insert("view".to_string(), json!({"fill": "#eee"}));
    config.insert(
        "title".to_string(),
        json!({
            "color": "red",
            "fontWeight": "bold",
            "align": "center",
            "baseline": "middle",
            "dx": 5,
            "dy": 5,
            "angle": 1,
            "limit": 5,
        }),
    );
    config.insert(
        "boxplot".to_string(),
        json!({
            "rule": {"fill": "red"},
            "median": {"color": "white"},
        }),
    );

    Value::Object(config)
}

fn strip_example() -> Value {
    let config = example_config();
    strip_and_redirect_config(&config).expect("config should not be empty")
}

#[test]
fn strip_and_redirect_config_should_not_cause_side_effect_to_the_input() {
    let config = example_config();
    let copy = config.clone();
    let _output = strip_and_redirect_config(&config);
    assert_eq!(config, copy);
}

#[test]
fn strip_and_redirect_config_should_remove_vl_only_mark_config_but_keep_vega_mark_config() {
    let output = strip_example();
    assert!(output["mark"].get("color").is_none());
    assert_eq!(output["mark"]["opacity"], json!(0.3));
}

#[test]
fn strip_and_redirect_config_should_remove_vl_tooltip_object() {
    let output = strip_example();
    assert!(output["mark"].get("tooltip").is_none());
}

#[test]
fn strip_and_redirect_config_should_remove_conditional_axis_config() {
    let output = strip_example();
    assert!(output.get("axis").is_none());
}

#[test]
fn strip_and_redirect_config_should_redirect_mark_config_to_style_and_remove_vl_only_mark_specific_config(
) {
    let output = strip_example();

    for mark in PRIMITIVE_MARKS {
        assert!(
            output.get(mark.as_str()).is_none(),
            "top-level `{}` config should have been redirected",
            mark.as_str()
        );
    }

    assert!(output["style"]["bar"].get("binSpacing").is_none());
    assert!(output["style"]["cell"].get("width").is_none());
    assert!(output["style"]["cell"].get("height").is_none());
    assert_eq!(output["style"]["cell"]["fill"], json!("#eee"));

    assert_eq!(output["style"]["bar"]["opacity"], json!(0.5));
}

#[test]
fn strip_and_redirect_config_should_redirect_title_to_style_group_title_and_rename_color_to_fill() {
    let output = strip_example();

    assert!(output.get("title").is_none());
    assert_eq!(output["style"]["group-title"]["fontWeight"], json!("bold"));
    assert_eq!(output["style"]["group-title"]["fill"], json!("red"));
}

#[test]
fn strip_and_redirect_config_redirects_align_baseline_dx_dy_and_limit_of_title_to_group_subtitle() {
    let output = strip_example();

    assert_eq!(
        output["style"]["group-subtitle"],
        json!({
            "align": "center",
            "baseline": "middle",
            "dx": 5,
            "dy": 5,
            "limit": 5,
        })
    );
}

#[test]
fn strip_and_redirect_config_should_remove_empty_config_object() {
    let output = strip_example();
    assert!(output.get("axisTop").is_none());
}

#[test]
fn strip_and_redirect_config_should_keep_subtitle_config_in_config_title_if_specified() {
    let cfg = init_config(&json!({"title": {"subtitleColor": "red"}}));
    let output = strip_and_redirect_config(&cfg).expect("non-empty config");
    assert_eq!(output["title"], json!({"subtitleColor": "red"}));
}

#[test]
// The 6.28 bounds are arbitrary numeric config values being round-tripped, not
// uses of the TAU constant; changing them would weaken the passthrough check.
#[allow(clippy::approx_constant)]
fn strip_and_redirect_config_converts_params_config_into_signals() {
    let cfg = init_config(&json!({
        "signals": [
            {"name": "x", "update": "foo"},
        ],
        "params": [
            {"name": "a", "expr": "bar"},
            {"name": "b", "bind": {"input": "range", "min": -6.28, "max": 6.28}},
        ],
    }));

    let output = strip_and_redirect_config(&cfg).expect("non-empty config");
    assert_eq!(
        output["signals"],
        json!([
            {"name": "x", "update": "foo"},
            {"name": "a", "update": "bar"},
            {"name": "b", "bind": {"input": "range", "min": -6.28, "max": 6.28}},
        ])
    );
}

#[test]
fn is_vg_scheme_should_return_true_for_valid_scheme_object() {
    assert!(is_vg_scheme(&json!({"scheme": "viridis", "count": 2})));
}

#[test]
fn is_vg_scheme_should_return_false_for_non_scheme_object() {
    assert!(!is_vg_scheme(&json!(["#EA98D2", "#659CCA"])));
}

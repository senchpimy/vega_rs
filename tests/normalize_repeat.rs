//! Port of `test/normalize/repeat.test.ts`.

use serde_json::{json, Value};
use vega_lite::log;
use vega_lite::normalize::base::RepeaterValue;
use vega_lite::normalize::repeater::{replace_repeater_in_encoding, replace_repeater_in_facet};

fn row_repeater(value: &str) -> RepeaterValue {
    RepeaterValue {
        row: Some(value.to_string()),
        ..Default::default()
    }
}

fn column_repeater(value: &str) -> RepeaterValue {
    RepeaterValue {
        column: Some(value.to_string()),
        ..Default::default()
    }
}

#[test]
fn should_resolve_repeated_fields() {
    let repeater = row_repeater("foo");
    let resolved = replace_repeater_in_encoding(
        &json!({
            "x": {"field": {"repeat": "row"}, "type": "quantitative"},
            "y": {"field": "bar", "type": "quantitative"}
        }),
        Some(&repeater),
    );
    assert_eq!(
        resolved,
        json!({
            "x": {"field": "foo", "type": "quantitative"},
            "y": {"field": "bar", "type": "quantitative"}
        })
    );
}

#[test]
fn should_resolve_repeated_datums() {
    let repeater = row_repeater("foo");
    let resolved = replace_repeater_in_encoding(
        &json!({
            "x": {"datum": {"repeat": "row"}, "type": "quantitative"},
            "y": {"field": "bar", "type": "quantitative"}
        }),
        Some(&repeater),
    );
    assert_eq!(
        resolved,
        json!({
            "x": {"datum": "foo", "type": "quantitative"},
            "y": {"field": "bar", "type": "quantitative"}
        })
    );
}

#[test]
fn should_show_warning_if_repeat_in_field_def_cannot_be_resolved() {
    let repeater = column_repeater("foo");
    let (resolved, local_logger) = log::capture(|| {
        replace_repeater_in_encoding(
            &json!({
                "x": {"field": {"repeat": "row"}, "type": "quantitative"},
                "y": {"field": "bar", "type": "quantitative"}
            }),
            Some(&repeater),
        )
    });
    assert_eq!(
        local_logger.warns[0],
        json!("Unknown repeated value \"row\".")
    );
    assert_eq!(
        resolved,
        json!({"y": {"field": "bar", "type": "quantitative"}})
    );
}

#[test]
fn should_support_arrays_of_field_defs() {
    let repeater = row_repeater("foo");
    let resolved = replace_repeater_in_encoding(
        &json!({
            "detail": [
                {"field": {"repeat": "row"}, "type": "quantitative"},
                {"field": "bar", "type": "quantitative"}
            ]
        }),
        Some(&repeater),
    );
    assert_eq!(
        resolved,
        json!({
            "detail": [
                {"field": "foo", "type": "quantitative"},
                {"field": "bar", "type": "quantitative"}
            ]
        })
    );
}

#[test]
fn should_replace_fields_in_sort() {
    let repeater = row_repeater("foo");
    let resolved = replace_repeater_in_encoding(
        &json!({
            "x": {"field": "bar", "type": "quantitative", "sort": {"field": {"repeat": "row"}, "op": "min"}}
        }),
        Some(&repeater),
    );
    assert_eq!(
        resolved,
        json!({
            "x": {"field": "bar", "type": "quantitative", "sort": {"field": "foo", "op": "min"}}
        })
    );
}

#[test]
fn should_replace_fields_in_conditionals() {
    let repeater = row_repeater("foo");
    let resolved = replace_repeater_in_encoding(
        &json!({
            "color": {
                "condition": {"param": "test", "field": {"repeat": "row"}, "type": "quantitative"},
                "value": "red"
            }
        }),
        Some(&repeater),
    );
    assert_eq!(
        resolved,
        json!({
            "color": {
                "condition": {"param": "test", "field": "foo", "type": "quantitative"},
                "value": "red"
            }
        })
    );
}

#[test]
fn should_replace_fields_in_reversed_conditionals() {
    let repeater = row_repeater("foo");
    let resolved = replace_repeater_in_encoding(
        &json!({
            "color": {
                "condition": {"param": "test", "value": "red"},
                "field": {"repeat": "row"},
                "type": "quantitative"
            }
        }),
        Some(&repeater),
    );
    assert_eq!(
        resolved,
        json!({
            "color": {
                "condition": {"param": "test", "value": "red"},
                "field": "foo",
                "type": "quantitative"
            }
        })
    );
}

#[test]
fn should_show_warning_if_repeat_in_conditional_cannot_be_resolved() {
    let repeater = column_repeater("foo");
    let (resolved, local_logger) = log::capture(|| {
        replace_repeater_in_encoding(
            &json!({
                "color": {
                    "condition": {"param": "test", "field": {"repeat": "row"}, "type": "quantitative"},
                    "value": "red"
                }
            }),
            Some(&repeater),
        )
    });
    assert_eq!(
        local_logger.warns[0],
        json!("Unknown repeated value \"row\".")
    );
    assert_eq!(resolved, json!({"color": {"value": "red"}}));
}

#[test]
fn should_show_warning_if_repeat_in_a_condition_field_def_cannot_be_resolved() {
    let repeater = column_repeater("foo");
    let (resolved, local_logger) = log::capture(|| {
        replace_repeater_in_encoding(
            &json!({
                "color": {
                    "condition": {"param": "test", "value": "red"},
                    "field": {"repeat": "row"},
                    "type": "quantitative"
                }
            }),
            Some(&repeater),
        )
    });
    assert_eq!(
        local_logger.warns[0],
        json!("Unknown repeated value \"row\".")
    );
    assert_eq!(
        resolved,
        json!({
            "color": {"condition": {"param": "test", "value": "red"}}
        })
    );
}

#[test]
fn should_resolve_repeated_fields_in_facet() {
    let repeater = row_repeater("foo");
    let resolved = replace_repeater_in_facet(
        &json!({
            "row": {"field": {"repeat": "row"}, "type": "quantitative"},
            "column": {"field": "bar", "type": "quantitative"}
        }),
        Some(&repeater),
    );
    assert_eq!(
        resolved,
        json!({
            "row": {"field": "foo", "type": "quantitative"},
            "column": {"field": "bar", "type": "quantitative"}
        })
    );
}

#[allow(dead_code)]
fn _unused(_: Value) {}

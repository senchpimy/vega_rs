//! Port of `test/predicate.test.ts`.

use serde_json::{json, Value};
use vega_lite::compile_predicate::expression;
use vega_lite::predicate::{
    field_filter_expression_with_options, is_field_equal_predicate, is_field_lte_predicate,
    is_field_one_of_predicate, is_field_range_predicate, is_field_valid_predicate,
};

fn without(array: &[Value], excluded: &[Value]) -> Vec<Value> {
    array
        .iter()
        .filter(|item| !excluded.contains(item))
        .cloned()
        .collect()
}

fn all_filters() -> Vec<Value> {
    vec![
        json!({"field": "color", "equal": "red"}),
        json!({"field": "x", "lte": "z"}),
        json!({"field": "color", "oneOf": ["red", "yellow"]}),
        json!({"field": "x", "range": [0, 5]}),
        json!({"field": "x", "valid": true}),
        json!("datum[\"x\"]===5"),
    ]
}

// ---------------------------------------------------------------------------
// isEqualFilter
// ---------------------------------------------------------------------------

#[test]
fn should_return_true_for_an_equal_filter() {
    let equal_filter = json!({"field": "color", "equal": "red"});
    assert!(is_field_equal_predicate(&equal_filter));
}

#[test]
fn should_return_false_for_other_filters_equal() {
    let equal_filter = json!({"field": "color", "equal": "red"});
    for filter in without(&all_filters(), &[equal_filter]) {
        assert!(!is_field_equal_predicate(&filter));
    }
}

// ---------------------------------------------------------------------------
// islessThanEqualsFilter
// ---------------------------------------------------------------------------

#[test]
fn should_return_true_for_less_than_equals_to_filter() {
    let less_than_equals_filter = json!({"field": "x", "lte": "z"});
    assert!(is_field_lte_predicate(&less_than_equals_filter));
}

#[test]
fn should_return_false_for_other_filters_lte() {
    let less_than_equals_filter = json!({"field": "x", "lte": "z"});
    for filter in without(&all_filters(), &[less_than_equals_filter]) {
        assert!(!is_field_lte_predicate(&filter));
    }
}

// ---------------------------------------------------------------------------
// isOneOfFilter
// ---------------------------------------------------------------------------

#[test]
fn should_return_true_for_an_in_filter() {
    let one_of_filter = json!({"field": "color", "oneOf": ["red", "yellow"]});
    assert!(is_field_one_of_predicate(&one_of_filter));
}

#[test]
fn should_return_false_for_other_filters_one_of() {
    let one_of_filter = json!({"field": "color", "oneOf": ["red", "yellow"]});
    for filter in without(&all_filters(), &[one_of_filter]) {
        assert!(!is_field_one_of_predicate(&filter));
    }
}

// ---------------------------------------------------------------------------
// isRangeFilter
// ---------------------------------------------------------------------------

#[test]
fn should_return_true_for_a_range_filter() {
    let range_filter = json!({"field": "x", "range": [0, 5]});
    assert!(is_field_range_predicate(&range_filter));
}

#[test]
fn should_return_true_for_a_range_predicate_with_range_signal() {
    let range_signal_filter = json!({"field": "x", "range": {"signal": "range"}});
    assert!(is_field_range_predicate(&range_signal_filter));
}

#[test]
fn should_return_false_for_other_filters_range() {
    let range_filter = json!({"field": "x", "range": [0, 5]});
    for filter in without(&all_filters(), &[range_filter]) {
        assert!(!is_field_range_predicate(&filter));
    }
}

// ---------------------------------------------------------------------------
// isValidFilter
// ---------------------------------------------------------------------------

#[test]
fn should_return_true_for_a_valid_filter() {
    let valid_filter = json!({"field": "x", "valid": true});
    assert!(is_field_valid_predicate(&valid_filter));
}

#[test]
fn should_return_false_for_other_filters_valid() {
    let valid_filter = json!({"field": "x", "valid": true});
    for filter in without(&all_filters(), &[valid_filter]) {
        assert!(!is_field_valid_predicate(&filter));
    }
}

// ---------------------------------------------------------------------------
// expression
// ---------------------------------------------------------------------------

#[test]
fn should_return_a_correct_expression_for_an_equal_filter() {
    let expr = expression(&json!({"field": "color", "equal": "red"}));
    assert_eq!(expr, "datum[\"color\"]===\"red\"");
}

#[test]
fn should_return_correct_expression_for_less_than() {
    let expr = expression(&json!({"field": "x", "lt": 1}));
    assert_eq!(expr, "datum[\"x\"]<1");
}

#[test]
fn should_return_correct_expression_for_greater_than() {
    let expr = expression(&json!({"field": "x", "gt": "aardvark"}));
    assert_eq!(expr, "datum[\"x\"]>\"aardvark\"");
}

#[test]
fn should_return_correct_expression_for_less_than_equals() {
    let expr = expression(&json!({"field": "x", "lte": "zyzzyva"}));
    assert_eq!(expr, "datum[\"x\"]<=\"zyzzyva\"");
}

#[test]
fn should_return_correct_expression_for_greater_than_equals() {
    let expr = expression(&json!({"field": "x", "gte": 1}));
    assert_eq!(expr, "datum[\"x\"]>=1");
}

#[test]
fn should_return_correct_expression_for_valid() {
    let expr = expression(&json!({"field": "x", "valid": true}));
    assert_eq!(expr, "isValid(datum[\"x\"]) && isFinite(+datum[\"x\"])");
}

#[test]
fn should_return_a_correct_expression_for_an_equal_filter_with_datetime_object() {
    let expr = expression(&json!({
        "field": "date",
        "equal": {
            "month": "January"
        }
    }));
    assert_eq!(expr, "datum[\"date\"]===time(datetime(2012, 0, 1, 0, 0, 0, 0))");
}

#[test]
fn should_return_a_correct_expression_for_an_equal_filter_with_time_unit_and_datetime_object() {
    let expr = expression(&json!({
        "timeUnit": "month",
        "field": "date",
        "equal": {
            "month": "January"
        }
    }));
    assert_eq!(
        expr,
        "time(datetime(2012, month(datum[\"date\"]), 1, 0, 0, 0, 0))===time(datetime(2012, 0, 1, 0, 0, 0, 0))"
    );
}

#[test]
fn should_return_a_correct_expression_for_an_equal_filter_with_datetime_object_with_flat_value() {
    let expr = expression(&json!({
        "timeUnit": "month",
        "field": "date",
        "equal": "January"
    }));
    assert_eq!(
        expr,
        "time(datetime(2012, month(datum[\"date\"]), 1, 0, 0, 0, 0))===time(datetime(2012, 0, 1, 0, 0, 0, 0))"
    );
}

#[test]
fn should_return_a_correct_expression_for_an_less_than_filter_with_datetime_object() {
    let expr = expression(&json!({
        "field": "date",
        "lt": {
            "month": "February"
        }
    }));
    assert_eq!(expr, "datum[\"date\"]<time(datetime(2012, 1, 1, 0, 0, 0, 0))");
}

#[test]
fn should_return_a_correct_expression_for_an_greater_than_filter_with_time_unit_and_datetime_object() {
    let expr = expression(&json!({
        "timeUnit": "month",
        "field": "date",
        "gt": {
            "month": "January"
        }
    }));
    assert_eq!(
        expr,
        "time(datetime(2012, month(datum[\"date\"]), 1, 0, 0, 0, 0))>time(datetime(2012, 0, 1, 0, 0, 0, 0))"
    );
}

#[test]
fn should_return_a_correct_expression_for_an_greater_than_equals_filter_with_datetime_object() {
    let expr = expression(&json!({
        "timeUnit": "month",
        "field": "date",
        "gte": "January"
    }));
    assert_eq!(
        expr,
        "time(datetime(2012, month(datum[\"date\"]), 1, 0, 0, 0, 0))>=time(datetime(2012, 0, 1, 0, 0, 0, 0))"
    );
}

#[test]
fn should_return_a_correct_expression_for_an_in_filter() {
    let expr = expression(&json!({"field": "color", "oneOf": ["red", "yellow"]}));
    assert_eq!(expr, "indexof([\"red\",\"yellow\"], datum[\"color\"]) !== -1");
}

#[test]
fn should_return_a_correct_expression_for_a_range_filter() {
    let expr = expression(&json!({"field": "x", "range": [0, 5]}));
    assert_eq!(expr, "inrange(datum[\"x\"], [0, 5])");
}

#[test]
fn should_return_a_correct_expression_for_a_range_filter_with_signal_range() {
    let expr = expression(&json!({"field": "x", "range": {"signal": "r"}}));
    assert_eq!(expr, "inrange(datum[\"x\"], [r[0], r[1]])");
}

#[test]
fn should_return_a_correct_expression_for_a_range_filter_with_no_lower_bound() {
    let expr = expression(&json!({"field": "x", "range": [null, 5]}));
    assert_eq!(expr, "datum[\"x\"] <= 5");
}

#[test]
fn should_return_a_correct_expression_for_a_range_filter_with_no_upper_bound() {
    let expr = expression(&json!({"field": "x", "range": [0, null]}));
    assert_eq!(expr, "datum[\"x\"] >= 0");
}

#[test]
fn should_return_true_for_a_range_filter_with_no_bound() {
    let expr = expression(&json!({"field": "x", "range": [null, null]}));
    assert_eq!(expr, "true");
}

#[test]
fn should_return_a_correct_expression_for_an_expression_filter() {
    let expr = expression(&json!("datum[\"x\"]===5"));
    assert_eq!(expr, "datum[\"x\"]===5");
}

// ---------------------------------------------------------------------------
// Composed filters
// ---------------------------------------------------------------------------

#[test]
fn generates_expressions_for_composed_filters() {
    let mut expr = expression(&json!({"not": {"field": "color", "equal": "red"}}));
    assert_eq!(expr, "!(datum[\"color\"]===\"red\")");

    expr = expression(&json!({
        "and": [
            {"field": "color", "equal": "red"},
            {"field": "x", "range": [0, 5]}
        ]
    }));

    assert_eq!(
        expr,
        "(datum[\"color\"]===\"red\") && (inrange(datum[\"x\"], [0, 5]))"
    );

    expr = expression(&json!({
        "and": [
            {"field": "color", "oneOf": ["red", "yellow"]},
            {
                "or": [{"field": "x", "range": [0, null]}, "datum.price > 10", {"not": "datum[\"x\"]===5"}]
            }
        ]
    }));

    assert_eq!(
        expr,
        "(indexof([\"red\",\"yellow\"], datum[\"color\"]) !== -1) && \
         ((datum[\"x\"] >= 0) || (datum.price > 10) || (!(datum[\"x\"]===5)))"
    );
}

// ---------------------------------------------------------------------------
// fieldFilterExpression
// ---------------------------------------------------------------------------

#[test]
fn generates_a_range_predicate_using_inequalities_when_use_in_range_is_false() {
    let expr =
        field_filter_expression_with_options(&json!({"field": "x", "range": [0, 5]}), false, "datum");
    assert_eq!(expr, "datum[\"x\"] >= 0 && datum[\"x\"] <= 5");
}

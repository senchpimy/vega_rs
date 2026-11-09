//! Port of `test/compile/data/expressions.test.ts`.

use vega_lite::compile::data::expressions::get_dependent_fields;

#[test]
fn calculates_right_dependent_fields_for_simple_expression() {
    let deps: Vec<String> = get_dependent_fields("datum.x + datum.y")
        .into_iter()
        .collect();
    assert_eq!(deps, vec!["x".to_string(), "y".to_string()]);
}

#[test]
fn calculates_right_dependent_fields_for_complex_expression() {
    let deps: Vec<String> = get_dependent_fields("toString(datum.x) + 12")
        .into_iter()
        .collect();
    assert_eq!(deps, vec!["x".to_string()]);
}

#[test]
fn calculates_right_dependent_fields_for_nested_field() {
    let mut deps: Vec<String> = get_dependent_fields("datum.x.y").into_iter().collect();
    deps.sort();
    assert_eq!(deps, vec!["x".to_string(), "x.y".to_string()]);

    let deps: Vec<String> = get_dependent_fields("datum[\"x.y\"]").into_iter().collect();
    assert_eq!(deps, vec!["x.y".to_string()]);
}

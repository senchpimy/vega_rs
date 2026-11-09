//! Port of `test/util.test.ts`.

use serde_json::{json, Value};
use std::collections::BTreeSet;
use vega_lite::util::*;

#[test]
fn var_name_replaces_all_non_alphanumeric_characters_with_underscore() {
    assert_eq!(var_name("bin-mpg$!@#%_+1"), "bin_mpg_______1");
}

#[test]
fn var_name_prepends_underscore_if_the_string_starts_with_number() {
    assert_eq!(var_name("1a"), "_1a");
}

#[test]
fn stringifies_numbers() {
    assert_eq!(stringify(&json!(12)), "12");
}

#[test]
fn stringifies_booleans() {
    assert_eq!(stringify(&json!(true)), "true");
}

#[test]
fn stringifies_strings() {
    assert_eq!(stringify(&json!("foo")), "\"foo\"");
}

#[test]
fn stringifies_objects() {
    assert_eq!(stringify(&json!({"foo": 42})), "{\"foo\":42}");
}

#[test]
fn hashes_numbers_as_numbers() {
    assert_eq!(hash(&json!(12)), json!(12));
}

#[test]
fn hashes_booleans_as_strings_so_that_they_can_be_used_as_keys() {
    assert_eq!(hash(&json!(true)), json!("true"));
}

#[test]
fn hashes_strings_as_strings() {
    assert_eq!(hash(&json!("foo")), json!("foo"));
}

#[test]
fn hashes_objects() {
    assert_eq!(hash(&json!({"foo": 42})), json!("{\"foo\":42}"));
}

#[test]
fn delete_nested_property_removes_a_property_from_an_object() {
    let mut original = json!({
        "property1": {"property1": "value1"},
        "property2": {"property5": "value2"},
        "property3": {"property6": "value3", "property7": "value4"},
    });
    let new_object = json!({
        "property2": {"property5": "value2"},
        "property3": {"property6": "value3", "property7": "value4"},
    });
    delete_nested_property(&mut original, &["property1".to_string()]);
    assert_eq!(stringify(&original), stringify(&new_object));
}

#[test]
fn delete_nested_property_removes_nested_properties() {
    let mut original = json!({
        "property1": {"property4": "value1"},
        "property2": {"property5": "value2"},
        "property3": {"property6": "value3", "property7": "value4"},
    });
    let new_object = json!({
        "property2": {"property5": "value2"},
        "property3": {"property6": "value3", "property7": "value4"},
    });
    delete_nested_property(
        &mut original,
        &["property1".to_string(), "property4".to_string()],
    );
    assert_eq!(stringify(&original), stringify(&new_object));
}

#[test]
fn delete_nested_property_stops_when_it_does_not_empty_the_last_element() {
    let mut original = json!({
        "property1": {"property4": "value1"},
        "property2": {"property5": "value2"},
        "property3": {"property6": "value3", "property7": "value4"},
    });
    let new_object = json!({
        "property1": {"property4": "value1"},
        "property2": {"property5": "value2"},
        "property3": {"property6": "value3"},
    });
    delete_nested_property(
        &mut original,
        &["property3".to_string(), "property7".to_string()],
    );
    assert_eq!(stringify(&original), stringify(&new_object));
}

#[test]
fn split_access_path_should_support_escaping() {
    assert_eq!(split_access_path("y\\[foo\\]"), vec!["y[foo]"]);
}

#[test]
fn access_path_with_datum_should_parse_foo() {
    assert_eq!(access_path_with_datum("foo", "datum"), "datum[\"foo\"]");
}

#[test]
fn access_path_with_datum_should_parse_foo_bar() {
    assert_eq!(
        access_path_with_datum("foo.bar", "datum"),
        "datum[\"foo\"] && datum[\"foo\"][\"bar\"]"
    );
}

#[test]
fn access_path_with_datum_should_support_custom_datum() {
    assert_eq!(access_path_with_datum("foo", "parent"), "parent[\"foo\"]");
}

#[test]
fn access_path_with_datum_should_support_escaped_brackets() {
    assert_eq!(
        access_path_with_datum("y\\[foo\\]", "datum"),
        "datum[\"y[foo]\"]"
    );
}

#[test]
fn flat_access_with_datum_should_parse_foo_bar() {
    assert_eq!(flat_access_with_datum("foo.bar", "datum"), "datum[\"foo.bar\"]");
}

#[test]
fn flat_access_with_datum_should_return_string_value_of_field_name() {
    assert_eq!(
        flat_access_with_datum("foo[\"bar\"].baz", "datum"),
        "datum[\"foo.bar.baz\"]"
    );
}

#[test]
fn flat_access_with_datum_should_support_custom_datum() {
    assert_eq!(flat_access_with_datum("foo", "parent"), "parent[\"foo\"]");
}

#[test]
fn flat_access_with_datum_should_support_escaped_brackets() {
    assert_eq!(
        flat_access_with_datum("y\\[foo\\]", "datum"),
        "datum[\"y[foo]\"]"
    );
}

#[test]
fn access_path_depth_should_return_1_if_the_field_is_not_nested() {
    assert_eq!(access_path_depth("foo"), 1);
}

#[test]
fn access_path_depth_should_return_1_if_dot_is_escaped() {
    assert_eq!(access_path_depth("foo\\.bar"), 1);
}

#[test]
fn access_path_depth_should_return_2_for_foo_bar() {
    assert_eq!(access_path_depth("foo.bar"), 2);
}

#[test]
fn access_path_depth_should_return_1_for_escaped_brackets() {
    assert_eq!(access_path_depth("y\\[foo\\]"), 1);
}

#[test]
fn replace_path_in_field_should_convert_nested_accesses() {
    assert_eq!(replace_path_in_field("foo[\"bar\"].baz"), "foo\\.bar\\.baz");
}

#[test]
fn replace_path_in_field_should_keep_escaped_dot() {
    assert_eq!(replace_path_in_field("foo\\.bar"), "foo\\.bar");
}

#[test]
fn replace_path_in_field_should_keep_escaped_brackets() {
    assert_eq!(replace_path_in_field("y\\[foo\\]"), "y\\[foo\\]");
}

#[test]
fn replace_path_in_field_should_keep_escaped_single_quotes() {
    assert_eq!(replace_path_in_field("foo\\'"), "foo\\'");
}

#[test]
fn replace_path_in_field_should_keep_escaped_double_quotes() {
    assert_eq!(replace_path_in_field("foo\\\""), "foo\\\"");
}

#[test]
fn prefix_generator_should_return_the_correct_value_for_simple_nested_field() {
    let input: BTreeSet<String> = ["a.b".to_string()].into_iter().collect();
    let expected: BTreeSet<String> = ["a".to_string(), "a[b]".to_string()].into_iter().collect();
    assert_eq!(prefix_generator(&input), expected);
}

#[test]
fn prefix_generator_should_return_the_correct_value_for_multilevel_nested_field() {
    let input: BTreeSet<String> = ["a[b].c.d".to_string()].into_iter().collect();
    let expected: BTreeSet<String> = ["a", "a[b]", "a[b][c]", "a[b][c][d]"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(prefix_generator(&input), expected);
}

fn set<T: Ord>(values: impl IntoIterator<Item = T>) -> BTreeSet<T> {
    values.into_iter().collect()
}

#[test]
fn set_equal_should_return_true_for_equal_sets() {
    assert!(set_equal(&set([1, 2, 3]), &set([3, 2, 1])));
    assert!(set_equal(&set::<i32>([]), &set::<i32>([])));
}

#[test]
fn set_equal_should_return_false_for_unequal_sets() {
    assert!(!set_equal(&set([1, 2, 3]), &set([2, 3])));
    assert!(!set_equal(&set([1, 2]), &set([2, 3])));
    assert!(!set_equal(&set([1]), &set::<i32>([])));
    assert!(!set_equal(&set::<i32>([]), &set([1])));
}

#[test]
fn has_intersection_should_return_true_for_sets_that_intersect() {
    assert!(has_intersection(&set([1, 2, 3]), &set([3, 4])));
    assert!(has_intersection(&set([1]), &set([1, 2])));
    assert!(has_intersection(&set([1, 2]), &set([1])));
}

#[test]
fn has_intersection_should_return_false_for_distinct_sets() {
    assert!(!has_intersection(&set([1, 2, 3]), &set([4, 5])));
    assert!(!has_intersection(&set([1]), &set::<i32>([])));
    assert!(!has_intersection(&set::<i32>([]), &set([1])));
}

#[test]
fn field_intersection_should_return_the_correct_value_for_2_string_sets() {
    let a = set(["a.b".to_string(), "d".to_string()]);
    let b = set(["a[b]".to_string()]);
    assert!(field_intersection(Some(&a), Some(&b)));
}

#[test]
fn field_intersection_should_return_the_correct_value_for_2_nested_but_different_string_sets() {
    let a = set(["a.b.c".to_string()]);
    let b = set(["a.b.d".to_string()]);
    assert!(field_intersection(Some(&a), Some(&b)));
    let c = set(["z.b.c".to_string()]);
    assert!(!field_intersection(Some(&a), Some(&c)));
}

#[test]
fn field_intersection_should_return_true_if_one_input_is_undefined() {
    let a = set(["a".to_string()]);
    assert!(field_intersection(None, Some(&a)));
    assert!(field_intersection(Some(&a), None));
    assert!(field_intersection(None, None));
}

#[test]
fn field_intersection_should_return_the_correct_value_for_2_nodes() {
    // FilterNode(null, null, 'datum.foo > 1')
    let filter_produced: BTreeSet<String> = BTreeSet::new();
    let filter_dependent: BTreeSet<String> = set(["foo".to_string()]);
    // PivotTransformNode(null, {pivot: 'a', value: 'b'})
    let pivot_produced: Option<&BTreeSet<String>> = None;
    let pivot_dependent: BTreeSet<String> = set(["a".to_string(), "b".to_string()]);

    assert!(!field_intersection(
        Some(&filter_produced),
        Some(&filter_dependent)
    ));
    assert!(field_intersection(pivot_produced, Some(&filter_dependent)));
    assert!(!field_intersection(
        Some(&filter_produced),
        Some(&pivot_dependent)
    ));
}

#[test]
fn unique_should_collapse_the_same_numbers() {
    let values = vec![json!(1), json!(2), json!(3), json!(2)];
    let identity = |d: &Value| d.clone();
    assert_eq!(unique(&values, identity), vec![json!(1), json!(2), json!(3)]);
}

#[test]
fn unique_should_collapse_the_same_items_with_strings() {
    let values = vec![json!(1), json!(2), json!("a"), json!("a")];
    let identity = |d: &Value| d.clone();
    assert_eq!(
        unique(&values, identity),
        vec![json!(1), json!(2), json!("a")]
    );
}

#[test]
fn keys_should_return_keys() {
    assert_eq!(keys(&json!({"a": 12, "b": 42})), vec!["a", "b"]);
}

#[test]
fn vals_should_return_values() {
    assert_eq!(vals(&json!({"a": 12, "b": 42})), vec![json!(12), json!(42)]);
}

#[test]
fn entries_should_return_entries() {
    assert_eq!(
        entries(&json!({"a": 12, "b": 42})),
        vec![
            ("a".to_string(), json!(12)),
            ("b".to_string(), json!(42))
        ]
    );
}

#[test]
fn unique_id_should_return_new_id() {
    assert_ne!(unique_id(None), unique_id(None));
}

#[test]
fn is_equal_should_return_false_when_dict_is_a_subset_of_other() {
    let a = json!({"a": 1}).as_object().unwrap().clone();
    let b = json!({"a": 1, "b": 2}).as_object().unwrap().clone();
    assert!(!is_equal(&a, &b));
}

#[test]
fn is_equal_should_return_false_when_other_is_a_subset_of_dict() {
    let a = json!({"a": 1, "b": 2}).as_object().unwrap().clone();
    let b = json!({"a": 1}).as_object().unwrap().clone();
    assert!(!is_equal(&a, &b));
}

#[test]
fn is_equal_should_return_true_when_dicts_are_equal() {
    let a = json!({"a": 1, "b": 2}).as_object().unwrap().clone();
    let b = json!({"a": 1, "b": 2}).as_object().unwrap().clone();
    assert!(is_equal(&a, &b));
}

#[test]
fn is_equal_should_return_false_when_key_values_differ() {
    let a = json!({"a": 1}).as_object().unwrap().clone();
    let b = json!({"a": 2}).as_object().unwrap().clone();
    assert!(!is_equal(&a, &b));
}

#[test]
fn replace_all_should_replace_all_occurrences() {
    assert_eq!(replace_all("abababa", "a", "c"), "cbcbcbc");
}

#[test]
fn replace_all_should_work_with_special_characters() {
    assert_eq!(replace_all("a/c", "/", "b"), "abc");
    assert_eq!(replace_all("a\\c", "\\", "b"), "abc");
    assert_eq!(replace_all("a[c", "[", "b"), "abc");
}

#[test]
fn is_numeric_should_accept_numbers() {
    assert!(is_numeric(&json!("12")));
    assert!(is_numeric(&json!("0")));
    assert!(is_numeric(&json!("1.2")));
    assert!(is_numeric(&json!("1e2")));
}

#[test]
fn is_numeric_should_reject_other_variables() {
    assert!(!is_numeric(&json!("")));
    assert!(!is_numeric(&json!("foo")));
    assert!(!is_numeric(&json!("0a")));
    assert!(!is_numeric(&json!("a0")));
    assert!(!is_numeric(&json!("true")));
    assert!(!is_numeric(&json!("1.2.3")));
}

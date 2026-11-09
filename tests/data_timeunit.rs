//! Port of `test/compile/data/timeunit.test.ts`.
//!
//! Encoding-driven cases are reconstructed directly as `TimeUnitComponent`s
//! (the encoding/model parser is out of scope), preserving the exact upstream
//! expected transforms.

use indexmap::IndexMap;
use serde_json::json;
use vega_lite::compile::data::dataflow::{
    assemble, clone_node, dependent_fields, hash, parent, produced_fields,
    PlaceholderDataFlowNode,
};
use vega_lite::compile::data::support::{
    normalize_time_unit, OFFSETTED_RECT_END_SUFFIX, OFFSETTED_RECT_START_SUFFIX,
};
use vega_lite::compile::data::timeunit::{TimeUnitComponent, TimeUnitNode};

fn component(
    as_: Option<&str>,
    field: &str,
    time_unit: serde_json::Value,
    rect_band_position: Option<f64>,
) -> TimeUnitComponent {
    TimeUnitComponent {
        as_: as_.map(str::to_string),
        field: field.to_string(),
        time_unit,
        rect_band_position,
    }
}

fn node_with(component: TimeUnitComponent) -> vega_lite::compile::data::dataflow::NodeRef {
    let mut units = IndexMap::new();
    units.insert("key".into(), component);
    TimeUnitNode::new(None, units)
}

#[test]
fn transform_with_simple_time_unit() {
    let t = json!({"field": "date", "as": "month_date", "timeUnit": "month"});
    let node = TimeUnitNode::make_from_transform(None, &t);
    assert_eq!(
        assemble(&node),
        json!([{
            "type": "timeunit",
            "field": "date",
            "as": ["month_date", "month_date_end"],
            "units": ["month"]
        }])
    );
}

#[test]
fn transform_with_time_unit_params() {
    let t = json!({"field": "date", "as": "month_date", "timeUnit": {"unit": "month", "utc": true, "step": 10}});
    let node = TimeUnitNode::make_from_transform(None, &t);
    assert_eq!(
        assemble(&node),
        json!([{
            "type": "timeunit",
            "field": "date",
            "timezone": "utc",
            "step": 10,
            "as": ["month_date", "month_date_end"],
            "units": ["month"]
        }])
    );
}

#[test]
fn hash_should_generate_the_correct_hash_for_point() {
    let t = json!({"field": "a", "as": "month_a", "timeUnit": "month"});
    let node = TimeUnitNode::make_from_transform(None, &t);
    assert_eq!(
        hash(&node),
        json!("TimeUnit {\"{\\\"as\\\":\\\"month_a\\\",\\\"field\\\":\\\"a\\\",\\\"timeUnit\\\":{\\\"unit\\\":\\\"month\\\"}}\":{\"as\":\"month_a\",\"field\":\"a\",\"timeUnit\":{\"unit\":\"month\"}}}")
    );
}

#[test]
fn hash_should_generate_the_correct_hash_for_time_unit_params() {
    let t = json!({"field": "a", "as": "utcmonth_step_10_a", "timeUnit": {"unit": "month", "utc": true, "step": 10}});
    let node = TimeUnitNode::make_from_transform(None, &t);
    assert_eq!(
        hash(&node),
        json!("TimeUnit {\"{\\\"as\\\":\\\"utcmonth_step_10_a\\\",\\\"field\\\":\\\"a\\\",\\\"timeUnit\\\":{\\\"step\\\":10,\\\"unit\\\":\\\"month\\\",\\\"utc\\\":true}}\":{\"as\":\"utcmonth_step_10_a\",\"field\":\"a\",\"timeUnit\":{\"step\":10,\"unit\":\"month\",\"utc\":true}}}")
    );
}

#[test]
fn clone_should_never_clone_parent() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let time_unit = TimeUnitNode::new(Some(&parent_node), Default::default());
    assert!(parent(&clone_node(&time_unit)).is_none());
}

#[test]
fn point_time_unit_produces_a_single_transform() {
    let node = node_with(component(
        Some("month_a"),
        "a",
        normalize_time_unit(&json!("month")).unwrap(),
        None,
    ));
    assert_eq!(
        assemble(&node),
        json!([{
            "type": "timeunit",
            "field": "a",
            "as": ["month_a", "month_a_end"],
            "units": ["month"]
        }])
    );
}

#[test]
fn bar_with_band_position_0_produces_offsetted_rect_formulas() {
    let node = node_with(component(
        Some("month_a"),
        "a",
        normalize_time_unit(&json!("month")).unwrap(),
        Some(0.0),
    ));
    assert_eq!(
        assemble(&node),
        json!([
            {
                "type": "timeunit",
                "field": "a",
                "as": ["month_a", "month_a_end"],
                "units": ["month"]
            },
            {
                "type": "formula",
                "expr": "0.5 * timeOffset('month', datum['month_a'], -1) + 0.5 * datum['month_a']",
                "as": format!("month_a_{OFFSETTED_RECT_START_SUFFIX}")
            },
            {
                "type": "formula",
                "expr": "0.5 * datum['month_a'] + 0.5 * datum['month_a_end']",
                "as": format!("month_a_{OFFSETTED_RECT_END_SUFFIX}")
            }
        ])
    );
}

#[test]
fn bar_with_band_position_0_and_escaped_field_name() {
    let node = node_with(component(
        Some("month_'a'"),
        "\\'a\\'",
        normalize_time_unit(&json!("month")).unwrap(),
        Some(0.0),
    ));
    assert_eq!(
        assemble(&node),
        json!([
            {
                "type": "timeunit",
                "field": "\\'a\\'",
                "as": ["month_'a'", "month_'a'_end"],
                "units": ["month"]
            },
            {
                "type": "formula",
                "expr": "0.5 * timeOffset('month', datum['month_\\'a\\''], -1) + 0.5 * datum['month_\\'a\\'']",
                "as": format!("month_'a'_{OFFSETTED_RECT_START_SUFFIX}")
            },
            {
                "type": "formula",
                "expr": "0.5 * datum['month_\\'a\\''] + 0.5 * datum['month_\\'a\\'_end']",
                "as": format!("month_'a'_{OFFSETTED_RECT_END_SUFFIX}")
            }
        ])
    );
}

#[test]
fn bar_with_step_and_band_position_0() {
    let node = node_with(component(
        Some("month_step_2_a"),
        "a",
        normalize_time_unit(&json!({"unit": "month", "step": 2})).unwrap(),
        Some(0.0),
    ));
    assert_eq!(
        assemble(&node),
        json!([
            {
                "type": "timeunit",
                "field": "a",
                "as": ["month_step_2_a", "month_step_2_a_end"],
                "units": ["month"],
                "step": 2
            },
            {
                "type": "formula",
                "expr": "0.5 * timeOffset('month', datum['month_step_2_a'], -2) + 0.5 * datum['month_step_2_a']",
                "as": format!("month_step_2_a_{OFFSETTED_RECT_START_SUFFIX}")
            },
            {
                "type": "formula",
                "expr": "0.5 * datum['month_step_2_a'] + 0.5 * datum['month_step_2_a_end']",
                "as": format!("month_step_2_a_{OFFSETTED_RECT_END_SUFFIX}")
            }
        ])
    );
}

#[test]
fn binned_time_unit_produces_an_offset_formula() {
    let t = json!({"field": "a", "timeUnit": "binnedyearmonth"});
    let node = TimeUnitNode::make_from_transform(None, &t);
    assert_eq!(
        assemble(&node),
        json!([{
            "type": "formula",
            "expr": "timeOffset('month', datum['a'], 1)",
            "as": "a_end"
        }])
    );
}

#[test]
fn binned_time_unit_with_band_position_0() {
    let node = node_with(component(
        None,
        "a",
        normalize_time_unit(&json!("binnedyearmonth")).unwrap(),
        Some(0.0),
    ));
    assert_eq!(
        assemble(&node),
        json!([
            {
                "type": "formula",
                "expr": "timeOffset('month', datum['a'], 1)",
                "as": "a_end"
            },
            {
                "type": "formula",
                "expr": "0.5 * timeOffset('month', datum['a'], -1) + 0.5 * datum['a']",
                "as": format!("a_{OFFSETTED_RECT_START_SUFFIX}")
            },
            {
                "type": "formula",
                "expr": "0.5 * datum['a'] + 0.5 * datum['a_end']",
                "as": format!("a_{OFFSETTED_RECT_END_SUFFIX}")
            }
        ])
    );
}

#[test]
fn binned_time_unit_unescapes_path_dots() {
    let t = json!({"field": "a\\.b", "timeUnit": "binnedyearmonth"});
    let node = TimeUnitNode::make_from_transform(None, &t);
    assert_eq!(
        assemble(&node),
        json!([{
            "type": "formula",
            "expr": "timeOffset('month', datum['a.b'], 1)",
            "as": "a.b_end"
        }])
    );
}

#[test]
fn binned_utc_time_unit_unescapes_quotes() {
    let t = json!({"field": "monthly(DATE_TRUNC(\\'day\\',date))", "timeUnit": "binnedutcyearmonth"});
    let node = TimeUnitNode::make_from_transform(None, &t);
    assert_eq!(
        assemble(&node),
        json!([{
            "type": "formula",
            "expr": "utcOffset('month', datum['monthly(DATE_TRUNC(\\'day\\',date))'], 1)",
            "as": "monthly(DATE_TRUNC('day',date))_end"
        }])
    );
}

#[test]
fn produced_and_dependent_fields() {
    let node = node_with(component(
        Some("month_a"),
        "a",
        normalize_time_unit(&json!("month")).unwrap(),
        None,
    ));
    let produced: Vec<String> = produced_fields(&node).unwrap().into_iter().collect();
    assert_eq!(produced, vec!["month_a".to_string()]);
    let deps: Vec<String> = dependent_fields(&node).into_iter().collect();
    assert_eq!(deps, vec!["a".to_string()]);
}

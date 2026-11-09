//! Port of `test/compile/data/aggregate.test.ts`.
//!
//! The `makeFromEncoding` tests need a fully parsed unit model (encodings,
//! scaled domains, marks, ...) and are skipped.  All transform-level tests,
//! merge/assemble/clone/hash tests are ported.

use indexmap::{IndexMap, IndexSet};
use serde_json::json;
use vega_lite::compile::data::aggregate::{AggregateNode, Measure, Measures};
use vega_lite::compile::data::dataflow::{
    assemble, clone_node, dependent_fields, hash, parent, produced_fields,
    PlaceholderDataFlowNode,
};

fn set(values: &[&str]) -> IndexSet<String> {
    values.iter().map(|s| s.to_string()).collect()
}

fn measure(aliases: &[&str], aggregate_param: Option<f64>) -> Measure {
    Measure {
        aliases: set(aliases),
        aggregate_param,
    }
}

fn measures(entries: Vec<(&str, Vec<(&str, Measure)>)>) -> Measures {
    entries
        .into_iter()
        .map(|(field, ops)| {
            (
                field.to_string(),
                ops.into_iter()
                    .map(|(op, m)| (op.to_string(), m))
                    .collect(),
            )
        })
        .collect()
}

#[test]
fn clone_should_have_correct_type() {
    let agg = AggregateNode::new(None, IndexSet::new(), Measures::new());
    let clone = clone_node(&agg);
    assert_eq!(assemble(&clone), assemble(&agg));
}

#[test]
fn clone_should_have_made_a_deep_copy() {
    let agg = AggregateNode::new(None, set(&["foo"]), Measures::new());
    let clone = clone_node(&agg);
    AggregateNode::add_dimensions(&clone, &["bar".to_string()]);
    let clone_deps: Vec<String> = dependent_fields(&clone).into_iter().collect();
    let agg_deps: Vec<String> = dependent_fields(&agg).into_iter().collect();
    assert_eq!(clone_deps, vec!["foo".to_string(), "bar".to_string()]);
    assert_eq!(agg_deps, vec!["foo".to_string()]);
}

#[test]
fn clone_should_never_clone_parent() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let aggregate = AggregateNode::new(Some(&parent_node), IndexSet::new(), Measures::new());
    assert!(parent(&clone_node(&aggregate)).is_none());
}

#[test]
fn hash_should_generate_the_correct_hash() {
    let agg = AggregateNode::new(
        None,
        set(&["Origin"]),
        measures(vec![
            ("*", vec![("count", measure(&["__count"], None))]),
            ("Acceleration", vec![("sum", measure(&["sum_Acceleration"], None))]),
        ]),
    );
    assert_eq!(
        hash(&agg),
        json!("Aggregate {\"dimensions\":\"Set(\\\"Origin\\\")\",\"measures\":{\"*\":{\"count\":{\"aliases\":\"Set(\\\"__count\\\")\"}},\"Acceleration\":{\"sum\":{\"aliases\":\"Set(\\\"sum_Acceleration\\\")\"}}}}")
    );
}

#[test]
fn make_from_transform_should_produce_the_correct_summary_component() {
    let t = json!({
        "aggregate": [
            {"op": "mean", "field": "Displacement", "as": "Displacement_mean"},
            {"op": "sum", "field": "Acceleration", "as": "Acceleration_sum"}
        ],
        "groupby": ["Group"]
    });
    let agg = AggregateNode::make_from_transform(None, &t).unwrap();
    assert_eq!(
        assemble(&agg),
        json!({
            "type": "aggregate",
            "groupby": ["Group"],
            "ops": ["mean", "sum"],
            "fields": ["Displacement", "Acceleration"],
            "as": ["Displacement_mean", "Acceleration_sum"]
        })
    );
}

#[test]
fn make_from_transform_should_support_multiple_aggregations_for_the_same_field() {
    let t = json!({
        "aggregate": [
            {"op": "mean", "field": "Displacement", "as": "Displacement_mean"},
            {"op": "max", "field": "Displacement", "as": "Displacement_max"},
            {"op": "sum", "field": "Acceleration", "as": "Acceleration_sum"}
        ],
        "groupby": ["Group"]
    });
    let agg = AggregateNode::make_from_transform(None, &t).unwrap();
    assert_eq!(
        assemble(&agg),
        json!({
            "type": "aggregate",
            "groupby": ["Group"],
            "ops": ["mean", "max", "sum"],
            "fields": ["Displacement", "Displacement", "Acceleration"],
            "as": ["Displacement_mean", "Displacement_max", "Acceleration_sum"]
        })
    );
}

#[test]
fn make_from_transform_should_support_exponential() {
    let t = json!({
        "aggregate": [
            {"op": "sum", "field": "Acceleration", "as": "Acceleration_sum"},
            {"op": {"exponential": 0.3}, "field": "Displacement", "as": "Displacement_exponential"}
        ],
        "groupby": ["Group"]
    });
    let agg = AggregateNode::make_from_transform(None, &t).unwrap();
    assert_eq!(
        assemble(&agg),
        json!({
            "type": "aggregate",
            "groupby": ["Group"],
            "ops": ["sum", "exponential"],
            "fields": ["Acceleration", "Displacement"],
            "as": ["Acceleration_sum", "Displacement_exponential"],
            "aggregate_params": [null, 0.3]
        })
    );
}

#[test]
fn make_from_transform_should_support_exponentialb() {
    let t = json!({
        "aggregate": [
            {"op": "sum", "field": "Acceleration", "as": "Acceleration_sum"},
            {"op": {"exponentialb": 0.3}, "field": "Displacement", "as": "Displacement_exponentialb"}
        ],
        "groupby": ["Group"]
    });
    let agg = AggregateNode::make_from_transform(None, &t).unwrap();
    assert_eq!(
        assemble(&agg),
        json!({
            "type": "aggregate",
            "groupby": ["Group"],
            "ops": ["sum", "exponentialb"],
            "fields": ["Acceleration", "Displacement"],
            "as": ["Acceleration_sum", "Displacement_exponentialb"],
            "aggregate_params": [null, 0.3]
        })
    );
}

#[test]
fn make_from_transform_should_preserve_an_aggregate_param_of_0() {
    let t = json!({
        "aggregate": [{"op": {"exponential": 0}, "field": "Displacement", "as": "Displacement_exponential"}],
        "groupby": ["Group"]
    });
    let agg = AggregateNode::make_from_transform(None, &t).unwrap();
    assert_eq!(
        assemble(&agg),
        json!({
            "type": "aggregate",
            "groupby": ["Group"],
            "ops": ["exponential"],
            "fields": ["Displacement"],
            "as": ["Displacement_exponential"],
            "aggregate_params": [0]
        })
    );
}

#[test]
fn produced_fields_should_produce_the_correct_fields() {
    let t = json!({
        "aggregate": [
            {"op": "mean", "field": "Displacement", "as": "AvgDisplacement"},
            {"op": "sum", "field": "Acceleration", "as": "Acceleration_sum"}
        ],
        "groupby": ["Group"]
    });
    let agg = AggregateNode::make_from_transform(None, &t).unwrap();
    let fields: Vec<String> = produced_fields(&agg).unwrap().into_iter().collect();
    assert_eq!(fields, vec!["AvgDisplacement".to_string(), "Acceleration_sum".to_string()]);
}

#[test]
fn should_not_merge_aggregate_nodes_with_different_dimensions() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let agg1 = AggregateNode::new(Some(&parent_node), set(&["a", "b"]), Measures::new());
    let agg2 = AggregateNode::new(Some(&parent_node), set(&["a"]), Measures::new());
    assert!(!AggregateNode::merge(&agg1, &agg2));
}

#[test]
fn should_merge_aggregate_nodes_with_same_dimensions() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let agg1 = AggregateNode::new(
        Some(&parent_node),
        set(&["a", "b"]),
        measures(vec![("a", vec![("mean", measure(&["a_mean"], None))])]),
    );
    let agg2 = AggregateNode::new(
        Some(&parent_node),
        set(&["a", "b"]),
        measures(vec![("b", vec![("mean", measure(&["b_mean"], None))])]),
    );
    assert!(AggregateNode::merge(&agg1, &agg2));
    let fields: Vec<String> = produced_fields(&agg1).unwrap().into_iter().collect();
    assert_eq!(fields, vec!["a_mean".to_string(), "b_mean".to_string()]);
}

#[test]
fn should_merge_aggregate_nodes_without_losing_aggregate_param() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let agg1 = AggregateNode::new(
        Some(&parent_node),
        set(&["a", "b"]),
        measures(vec![("a", vec![("sum", measure(&["a_sum"], None))])]),
    );
    let agg2 = AggregateNode::new(
        Some(&parent_node),
        set(&["a", "b"]),
        measures(vec![(
            "b",
            vec![("exponential", measure(&["b_exponential"], Some(0.5)))],
        )]),
    );
    assert!(AggregateNode::merge(&agg1, &agg2));
    assert_eq!(
        assemble(&agg1),
        json!({
            "ops": ["sum", "exponential"],
            "type": "aggregate",
            "as": ["a_sum", "b_exponential"],
            "fields": ["a", "b"],
            "groupby": ["a", "b"],
            "aggregate_params": [null, 0.5]
        })
    );
}

#[test]
fn should_merge_aggregate_nodes_without_losing_an_aggregate_param_of_0() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let agg1 = AggregateNode::new(
        Some(&parent_node),
        set(&["a"]),
        measures(vec![("b", vec![("exponential", measure(&["b_exp1"], Some(0.0)))])]),
    );
    let agg2 = AggregateNode::new(
        Some(&parent_node),
        set(&["a"]),
        measures(vec![("b", vec![("exponential", measure(&["b_exp2"], Some(0.0)))])]),
    );
    assert!(AggregateNode::merge(&agg1, &agg2));
    assert_eq!(
        assemble(&agg1),
        json!({
            "ops": ["exponential", "exponential"],
            "type": "aggregate",
            "as": ["b_exp1", "b_exp2"],
            "fields": ["b", "b"],
            "groupby": ["a"],
            "aggregate_params": [0, 0]
        })
    );
}

#[test]
fn should_warn_when_merging_aggregate_nodes_with_conflicting_aggregate_params() {
    let (_, logger) = vega_lite::log::capture(|| {
        let parent_node = PlaceholderDataFlowNode::new(None, "parent");
        let agg1 = AggregateNode::new(
            Some(&parent_node),
            set(&["a"]),
            measures(vec![("b", vec![("exponential", measure(&["b_exp1"], Some(0.3)))])]),
        );
        let agg2 = AggregateNode::new(
            Some(&parent_node),
            set(&["a"]),
            measures(vec![("b", vec![("exponential", measure(&["b_exp2"], Some(0.7)))])]),
        );
        assert!(AggregateNode::merge(&agg1, &agg2));
    });
    assert_eq!(
        logger.warns[0],
        json!("The \"exponential\" aggregate of the field \"b\" is used with different parameters (0.3 and 0.7). Using 0.7.")
    );
}

#[test]
fn assemble_should_escape_nested_accesses() {
    let agg = AggregateNode::new(
        None,
        set(&["foo.bar"]),
        measures(vec![("foo.baz", vec![("mean", measure(&["foo_baz_mean"], None))])]),
    );
    assert_eq!(
        assemble(&agg),
        json!({
            "as": ["foo_baz_mean"],
            "fields": ["foo\\.baz"],
            "groupby": ["foo\\.bar"],
            "ops": ["mean"],
            "type": "aggregate"
        })
    );
}

#[test]
fn make_from_transform_returns_null_when_there_is_nothing_to_aggregate() {
    assert!(AggregateNode::make_from_transform(None, &json!({})).is_none());
}

#[allow(dead_code)]
fn _unused_measures_type(_: IndexMap<String, IndexMap<String, Measure>>) {}

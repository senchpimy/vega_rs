//! Port of `test/compile/data/facet.test.ts`.

use serde_json::{json, Value};

use vega_lite::compile::data::dataflow::{dependent_fields, hash};
use vega_lite::compile::data::facet::FacetNode;
use vega_lite::compile::model::Model;
use vega_lite::compile::support::init_config;

fn parse_facet_model_with_scale(spec: &Value) -> Model {
    let config = init_config(spec.get("config"));
    let mut model = Model::new_facet(spec, None, "", &config);
    model.parse_scale();
    model
}

fn assemble_facet(spec: &Value) -> Vec<Value> {
    let model = parse_facet_model_with_scale(spec);
    let node = FacetNode::new(None, &model, "facetName", "dataName");
    FacetNode::assemble_facet(&node)
}

#[test]
fn should_calculate_column_distinct_if_child_has_an_independent_discrete_scale_with_step() {
    let data = assemble_facet(&json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
        "description": "A trellis bar chart showing the US population distribution of age groups and gender in 2000.",
        "data": {"url": "data/population.json"},
        "facet": {"column": {"field": "gender", "type": "nominal"}},
        "spec": {
            "mark": "bar",
            "encoding": {
                "y": {"aggregate": "sum", "field": "people", "type": "quantitative", "axis": {"title": "population"}},
                "x": {"field": "age", "type": "ordinal"},
                "color": {"field": "gender", "type": "nominal", "scale": {"range": ["#EA98D2", "#659CCA"]}}
            }
        },
        "resolve": {"scale": {"x": "independent"}},
        "config": {"view": {"fill": "yellow"}}
    }));

    assert_eq!(
        data[0],
        json!({
            "name": "column_domain",
            "source": "dataName",
            "transform": [{
                "type": "aggregate",
                "groupby": ["gender"],
                "fields": ["age"],
                "ops": ["distinct"],
                "as": ["distinct_age"]
            }]
        })
    );
}

#[test]
fn should_calculate_column_and_row_distinct_if_child_has_an_independent_discrete_scale_with_step_and_the_facet_has_both_row_and_column(
) {
    let data = assemble_facet(&json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
        "data": {
            "values": [
                {"r": "r1", "c": "c1", "a": "a1", "b": "b1"},
                {"r": "r1", "c": "c1", "a": "a2", "b": "b2"},
                {"r": "r2", "c": "c2", "a": "a1", "b": "b1"},
                {"r": "r3", "c": "c2", "a": "a3", "b": "b2"}
            ]
        },
        "facet": {
            "row": {"field": "r", "type": "nominal"},
            "column": {"field": "c", "type": "nominal"}
        },
        "spec": {
            "mark": "rect",
            "encoding": {
                "y": {"field": "b", "type": "nominal"},
                "x": {"field": "a", "type": "nominal"}
            }
        },
        "resolve": {"scale": {"x": "independent", "y": "independent"}}
    }));

    // crossed data
    assert_eq!(
        data[0],
        json!({
            "name": "cross_column_domain_row_domain",
            "source": "dataName",
            "transform": [{
                "type": "aggregate",
                "groupby": ["r", "c"],
                "fields": ["a", "b"],
                "ops": ["distinct", "distinct"]
            }]
        })
    );

    assert_eq!(
        data[1],
        json!({
            "name": "column_domain",
            "source": "cross_column_domain_row_domain",
            "transform": [{
                "type": "aggregate",
                "groupby": ["c"],
                "fields": ["distinct_a"],
                "ops": ["max"],
                "as": ["distinct_a"]
            }]
        })
    );

    assert_eq!(
        data[2],
        json!({
            "name": "row_domain",
            "source": "cross_column_domain_row_domain",
            "transform": [{
                "type": "aggregate",
                "groupby": ["r"],
                "fields": ["distinct_b"],
                "ops": ["max"],
                "as": ["distinct_b"]
            }]
        })
    );
}

#[test]
fn should_calculate_column_and_row_sort_array() {
    let data = assemble_facet(&json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
        "data": {"name": "a"},
        "facet": {
            "row": {"field": "r", "type": "nominal", "sort": ["r1", "r2"]},
            "column": {"field": "c", "type": "nominal", "sort": ["c1", "c2"]}
        },
        "spec": {
            "mark": "rect",
            "encoding": {
                "y": {"field": "b", "type": "quantitative"},
                "x": {"field": "a", "type": "quantitative"}
            }
        }
    }));

    assert_eq!(
        data[0],
        json!({
            "name": "column_domain",
            "source": "dataName",
            "transform": [{
                "type": "aggregate",
                "groupby": ["c"],
                "fields": ["column_c_sort_index"],
                "ops": ["max"],
                "as": ["column_c_sort_index"]
            }]
        })
    );

    assert_eq!(
        data[1],
        json!({
            "name": "row_domain",
            "source": "dataName",
            "transform": [{
                "type": "aggregate",
                "groupby": ["r"],
                "fields": ["row_r_sort_index"],
                "ops": ["max"],
                "as": ["row_r_sort_index"]
            }]
        })
    );
}

#[test]
fn should_calculate_column_and_row_sort_field() {
    let data = assemble_facet(&json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
        "data": {"name": "a"},
        "facet": {
            "row": {"field": "r", "type": "nominal", "sort": {"op": "median", "field": "b"}},
            "column": {"field": "c", "type": "nominal", "sort": {"field": "a"}}
        },
        "spec": {
            "mark": "rect",
            "encoding": {
                "y": {"field": "b", "type": "quantitative"},
                "x": {"field": "a", "type": "quantitative"}
            }
        }
    }));

    assert_eq!(
        data[0],
        json!({
            "name": "column_domain",
            "source": "dataName",
            "transform": [{
                "type": "aggregate",
                "groupby": ["c"],
                "fields": ["a"],
                "ops": ["min"],
                "as": ["a"]
            }]
        })
    );

    assert_eq!(
        data[1],
        json!({
            "name": "row_domain",
            "source": "dataName",
            "transform": [{
                "type": "aggregate",
                "groupby": ["r"],
                "fields": ["b"],
                "ops": ["median"],
                "as": ["median_b"]
            }]
        })
    );
}

#[test]
fn should_assemble_lookup_datasets_for_crossed_custom_facet_sort_metadata() {
    let data = assemble_facet(&json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
        "data": {"name": "a"},
        "facet": {
            "row": {"field": "r", "type": "nominal", "sort": {"op": "median", "field": "b"}},
            "column": {"field": "c", "type": "nominal", "sort": ["c1", "c2"]}
        },
        "spec": {
            "mark": "rect",
            "encoding": {
                "y": {"field": "b", "type": "quantitative"},
                "x": {"field": "a", "type": "quantitative"}
            }
        }
    }));

    assert!(data.contains(&json!({
        "name": "row_lookup_domain",
        "source": "row_domain",
        "transform": [{
            "type": "formula",
            "expr": "join([isValid(datum[\"r\"]) ? length(toString(datum[\"r\"])) + ':' + toString(datum[\"r\"]) : '-1:'], '|')",
            "as": "row_facet_key"
        }]
    })));

    assert!(data.contains(&json!({
        "name": "column_lookup_domain",
        "source": "column_domain",
        "transform": [{
            "type": "formula",
            "expr": "join([isValid(datum[\"c\"]) ? length(toString(datum[\"c\"])) + ':' + toString(datum[\"c\"]) : '-1:'], '|')",
            "as": "column_facet_key"
        }]
    })));

    assert!(data
        .iter()
        .find(|d| d.get("name") == Some(&json!("crossed_facet_domain")))
        .is_none());
}

#[test]
fn dependent_fields_should_return_proper_dependent_fields() {
    let model = parse_facet_model_with_scale(&json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v2.json",
        "data": {"name": "a"},
        "facet": {
            "row": {"field": "r", "type": "nominal", "sort": {"op": "median", "field": "b"}},
            "column": {"field": "c", "type": "nominal", "sort": [1, 2, 3]}
        },
        "spec": {
            "mark": "rect",
            "encoding": {
                "y": {"field": "b", "type": "quantitative"},
                "x": {"field": "a", "type": "quantitative"}
            }
        }
    }));

    let facet = FacetNode::new(None, &model, "facetName", "dataName");
    let mut deps: Vec<String> = dependent_fields(&facet).into_iter().collect();
    deps.sort();
    let mut expected = vec![
        "r".to_string(),
        "c".to_string(),
        "b".to_string(),
        "column_c_sort_index".to_string(),
    ];
    expected.sort();
    assert_eq!(deps, expected);
}

#[test]
fn sort_index_fields_should_return_the_sort_index_fields_of_array_sorted_channels() {
    let model = parse_facet_model_with_scale(&json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
        "data": {"name": "a"},
        "facet": {
            "row": {"field": "r", "type": "nominal", "sort": {"op": "median", "field": "b"}},
            "column": {"field": "c", "type": "nominal", "sort": [1, 2, 3]}
        },
        "spec": {
            "mark": "rect",
            "encoding": {
                "y": {"field": "b", "type": "quantitative"},
                "x": {"field": "a", "type": "quantitative"}
            }
        }
    }));

    let facet = FacetNode::new(None, &model, "facetName", "dataName");
    assert_eq!(
        FacetNode::node_sort_index_fields(&facet),
        vec!["column_c_sort_index".to_string()]
    );
}

fn facet_node_for_facet(facet: Value) -> vega_lite::compile::data::NodeRef {
    let model = parse_facet_model_with_scale(&json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
        "data": {"name": "a"},
        "facet": facet,
        "spec": {
            "mark": "rect",
            "encoding": {
                "y": {"field": "b", "type": "quantitative"},
                "x": {"field": "a", "type": "quantitative"}
            }
        }
    }));
    FacetNode::new(None, &model, "facetName", "dataName")
}

#[test]
fn can_move_below_aggregate_should_be_true_without_a_custom_sort() {
    let facet = facet_node_for_facet(json!({
        "row": {"field": "r", "type": "nominal"},
        "column": {"field": "c", "type": "nominal", "sort": "descending"}
    }));
    assert!(FacetNode::node_can_move_below_aggregate(&facet));
}

#[test]
fn can_move_below_aggregate_should_be_true_with_sort_arrays_on_unbinned_fields() {
    let facet = facet_node_for_facet(json!({
        "row": {"field": "r", "type": "nominal", "sort": ["r1", "r2"]},
        "column": {"field": "c", "type": "nominal", "sort": [1, 2, 3]}
    }));
    assert!(FacetNode::node_can_move_below_aggregate(&facet));
}

#[test]
fn can_move_below_aggregate_should_be_false_with_a_sort_field_definition() {
    let facet = facet_node_for_facet(json!({
        "row": {"field": "r", "type": "nominal", "sort": {"op": "median", "field": "b"}},
        "column": {"field": "c", "type": "nominal", "sort": [1, 2, 3]}
    }));
    assert!(!FacetNode::node_can_move_below_aggregate(&facet));
}

#[test]
fn can_move_below_aggregate_should_be_false_with_a_sort_array_on_a_binned_field() {
    let facet = facet_node_for_facet(json!({
        "row": {"bin": true, "field": "r", "type": "quantitative", "sort": [1, 2, 3]}
    }));
    assert!(!FacetNode::node_can_move_below_aggregate(&facet));
}

#[test]
fn hash_should_generate_the_correct_hash() {
    let model = parse_facet_model_with_scale(&json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v2.json",
        "data": {"name": "a"},
        "facet": {
            "row": {"field": "r", "type": "nominal", "sort": {"op": "median", "field": "b"}},
            "column": {"field": "c", "type": "nominal", "sort": {"op": "median", "field": "a"}}
        },
        "spec": {
            "mark": "rect",
            "encoding": {
                "y": {"field": "b", "type": "quantitative"},
                "x": {"field": "a", "type": "quantitative"}
            }
        }
    }));

    let facet = FacetNode::new(None, &model, "facetName", "dataName");
    assert_eq!(
        hash(&facet),
        json!("Facet r:{\"fields\":[\"r\"],\"name\":\"row_domain\",\"sortField\":{\"field\":\"b\",\"op\":\"median\"}} c:{\"fields\":[\"c\"],\"name\":\"column_domain\",\"sortField\":{\"field\":\"a\",\"op\":\"median\"}}")
    );
}

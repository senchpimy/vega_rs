//! Port of `test/compile/data/assemble.test.ts`.

use indexmap::{IndexMap, IndexSet};
use serde_json::json;

use vega_lite::compile::data::aggregate::{AggregateNode, Measure, Measures};
use vega_lite::compile::data::assemble::assemble_root_data;
use vega_lite::compile::data::dataflow::{OutputNode, OutputNodeType};
use vega_lite::compile::data::index::DataComponent;
use vega_lite::compile::data::source::SourceNode;
use vega_lite::compile::data::window::WindowTransformNode;

#[test]
fn should_assemble_named_data_source() {
    let src = SourceNode::new(Some(&json!({"name": "foo"})));
    let main = OutputNode::new(Some(&src), "mainOut", OutputNodeType::Main);

    assert_eq!(OutputNode::get_source(&main), "mainOut");

    let mut data_component = DataComponent {
        sources: vec![src.clone()],
        ..Default::default()
    };
    data_component
        .output_nodes
        .insert("out".to_string(), main.clone());

    let data = assemble_root_data(&data_component, &IndexMap::new());
    assert_eq!(data.len(), 1);
    assert_eq!(data[0]["name"], json!("foo"));
}

#[test]
fn should_assemble_raw_and_main_output() {
    let src = SourceNode::new(Some(&json!({"url": "foo.csv"})));
    let raw = OutputNode::new(Some(&src), "rawOut", OutputNodeType::Raw);

    let mut measures: Measures = IndexMap::new();
    let mut ops = IndexMap::new();
    ops.insert(
        "count".to_string(),
        Measure {
            aliases: IndexSet::from(["count_*".to_string()]),
            aggregate_param: None,
        },
    );
    measures.insert("b".to_string(), ops);
    let agg = AggregateNode::new(
        Some(&raw),
        IndexSet::from(["a".to_string()]),
        measures,
    );
    let main = OutputNode::new(Some(&agg), "mainOut", OutputNodeType::Main);

    assert_eq!(OutputNode::get_source(&raw), "rawOut");
    assert_eq!(OutputNode::get_source(&main), "mainOut");

    let mut data_component = DataComponent {
        sources: vec![src],
        ..Default::default()
    };
    data_component.output_nodes.insert("out".to_string(), main);

    let data = assemble_root_data(&data_component, &IndexMap::new());

    assert_eq!(
        serde_json::Value::Array(data),
        json!([
            {"name": "source_0", "url": "foo.csv", "format": {"type": "csv"}},
            {
                "name": "data_0",
                "source": "source_0",
                "transform": [{
                    "type": "aggregate",
                    "groupby": ["a"],
                    "ops": ["count"],
                    "fields": ["b"],
                    "as": ["count_*"]
                }]
            }
        ])
    );
}

#[test]
fn should_assemble_window_transform_node() {
    let src = SourceNode::new(Some(&json!({"url": "foo.csv"})));
    let raw = OutputNode::new(Some(&src), "rawOut", OutputNodeType::Raw);

    let transform = json!({
        "window": [{"op": "row_number", "as": "ordered_row_number"}],
        "ignorePeers": false,
        "sort": [{"field": "f", "order": "ascending"}],
        "groupby": ["f"],
        "frame": [null, 0]
    });
    let agg = WindowTransformNode::new(Some(&raw), &transform);
    let main = OutputNode::new(Some(&agg), "mainOut", OutputNodeType::Main);

    assert_eq!(OutputNode::get_source(&raw), "rawOut");
    assert_eq!(OutputNode::get_source(&main), "mainOut");

    let mut data_component = DataComponent {
        sources: vec![src],
        ..Default::default()
    };
    data_component.output_nodes.insert("out".to_string(), main);

    let data = assemble_root_data(&data_component, &IndexMap::new());

    assert_eq!(
        serde_json::Value::Array(data),
        json!([
            {"name": "source_0", "url": "foo.csv", "format": {"type": "csv"}},
            {
                "name": "data_0",
                "source": "source_0",
                "transform": [{
                    "type": "window",
                    "ops": ["row_number"],
                    "fields": [null],
                    "params": [null],
                    "sort": {"field": ["f"], "order": ["ascending"]},
                    "ignorePeers": false,
                    "as": ["ordered_row_number"],
                    "frame": [null, 0],
                    "groupby": ["f"]
                }]
            }
        ])
    );
}

#[test]
fn should_assemble_named_datasets_with_datastore() {
    let src = SourceNode::new(Some(&json!({"name": "foo"})));
    let main = OutputNode::new(Some(&src), "mainOut", OutputNodeType::Main);

    let mut data_component = DataComponent {
        sources: vec![src],
        ..Default::default()
    };
    data_component.output_nodes.insert("out".to_string(), main);

    let mut datasets = IndexMap::new();
    datasets.insert("foo".to_string(), json!([1, 2, 3]));

    let data = assemble_root_data(&data_component, &datasets);
    assert_eq!(
        serde_json::Value::Array(data),
        json!([{"name": "foo", "values": [1, 2, 3]}])
    );
}

#[test]
fn should_fix_lookup_from_references_and_move_sources_without_transforms_first() {
    use vega_lite::compile::data::lookup::LookupNode;

    let lookup_src = SourceNode::new(Some(&json!({"name": "lookup"})));
    let lookup_out = OutputNode::new(Some(&lookup_src), "lookupOut", OutputNodeType::Lookup);

    let main_src = SourceNode::new(Some(&json!({"url": "main.csv"})));
    let lookup = LookupNode::new(
        Some(&main_src),
        &json!({"lookup": "key", "from": {"fields": ["key"]}, "as": "val"}),
        Some("lookupOut"),
    );
    let main = OutputNode::new(Some(&lookup), "mainOut", OutputNodeType::Main);

    let mut data_component = DataComponent {
        sources: vec![lookup_src, main_src],
        ..Default::default()
    };
    data_component
        .output_nodes
        .insert("lookupOut".to_string(), lookup_out);
    data_component
        .output_nodes
        .insert("mainOut".to_string(), main);

    let data = assemble_root_data(&data_component, &IndexMap::new());
    // sources without transforms are moved to the beginning
    assert_eq!(data[0]["name"], json!("lookup"));
    assert!(data[0].get("transform").is_none());
    assert_eq!(data[1]["transform"][0]["from"], json!("lookup"));
}

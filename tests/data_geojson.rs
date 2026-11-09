//! Port of `test/compile/data/geojson.test.ts`.

use serde_json::json;

use vega_lite::compile::data::dataflow::{assemble, dependent_fields, produced_fields};
use vega_lite::compile::data::geojson::GeoJSONNode;
use vega_lite::compile::data::PlaceholderDataFlowNode;
use vega_lite::compile::model::Model;
use vega_lite::compile::support::init_config;

fn parse_unit_model_with_scale_and_layout_size(spec: &serde_json::Value) -> Model {
    let config = init_config(spec.get("config"));
    let mut model = Model::new_unit(spec, None, "", None, &config);
    model.parse_scale();
    model.parse_layout_size();
    model
}

#[test]
fn should_make_transform_and_assemble_correctly() {
    let model = parse_unit_model_with_scale_and_layout_size(&json!({
        "data": {"url": "data/zipcodes.csv", "format": {"type": "csv"}},
        "mark": "circle",
        "encoding": {
            "longitude": {"field": "longitude", "type": "quantitative"},
            "latitude": {"field": "latitude", "type": "quantitative"}
        }
    }));

    let root = PlaceholderDataFlowNode::new(None, "root");
    GeoJSONNode::parse_all(&root, &model);

    let node = vega_lite::compile::data::children(&root)[0].clone();
    assert!(GeoJSONNode::is_instance(&node));
    let transforms = assemble(&node);
    let arr = transforms.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    let transform = &arr[0];
    assert_eq!(transform["type"], json!("geojson"));
    let fields = transform["fields"].as_array().unwrap();
    assert!(fields.contains(&json!("longitude")));
    assert!(fields.contains(&json!("latitude")));
    assert!(transform.get("geojson").is_none());
}

#[test]
fn should_add_filter_when_shape_channel_is_used() {
    let model = parse_unit_model_with_scale_and_layout_size(&json!({
        "data": {"values": []},
        "mark": "geoshape",
        "encoding": {
            "shape": {"field": "geo", "type": "geojson"},
            "color": {"field": "state", "type": "nominal"}
        }
    }));

    let root = PlaceholderDataFlowNode::new(None, "root");
    GeoJSONNode::parse_all(&root, &model);

    let node = vega_lite::compile::data::children(&root)[0].clone();
    assert!(GeoJSONNode::is_instance(&node));
    let transforms = assemble(&node);
    let arr = transforms.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(
        arr[0],
        json!({"type": "filter", "expr": "isValid(datum[\"geo\"])"})
    );
    assert_eq!(arr[1]["type"], json!("geojson"));
    assert_eq!(arr[1]["geojson"], json!("geo"));
}

#[test]
fn should_skip_geojson_when_there_is_a_custom_projection() {
    let model = parse_unit_model_with_scale_and_layout_size(&json!({
        "data": {"url": "data/zipcodes.csv", "format": {"type": "csv"}},
        "projection": {"type": "mercator", "scale": 1000},
        "mark": "circle",
        "encoding": {
            "longitude": {"field": "longitude", "type": "quantitative"},
            "latitude": {"field": "latitude", "type": "quantitative"}
        }
    }));

    let root = PlaceholderDataFlowNode::new(None, "root");
    let retval = GeoJSONNode::parse_all(&root, &model);

    assert!(std::rc::Rc::ptr_eq(&retval, &root));
    assert_eq!(vega_lite::compile::data::num_children(&root), 0);
}

#[test]
fn dependent_fields_should_return_fields() {
    let node = GeoJSONNode::new(
        None,
        Some(vec![json!("foo"), json!({"expr": "s"})]),
        None,
        None,
    );
    let deps: Vec<String> = dependent_fields(&node).into_iter().collect();
    assert_eq!(deps, vec!["foo".to_string()]);
}

#[test]
fn dependent_fields_should_return_geojson() {
    let node = GeoJSONNode::new(None, None, Some("geo"), None);
    let deps: Vec<String> = dependent_fields(&node).into_iter().collect();
    assert_eq!(deps, vec!["geo".to_string()]);
}

#[test]
fn produced_fields_should_return_empty_set() {
    let node = GeoJSONNode::new(None, None, None, None);
    assert!(produced_fields(&node).unwrap().is_empty());
}

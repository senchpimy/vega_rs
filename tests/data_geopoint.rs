//! Port of `test/compile/data/geopoint.test.ts`.

use serde_json::json;

use vega_lite::compile::data::dataflow::{assemble, dependent_fields, produced_fields};
use vega_lite::compile::data::geopoint::GeoPointNode;
use vega_lite::compile::data::{children, PlaceholderDataFlowNode};
use vega_lite::compile::model::Model;
use vega_lite::compile::support::init_config;

#[test]
fn should_make_transform_and_assemble_correctly() {
    let spec = json!({
        "data": {"url": "data/zipcodes.csv", "format": {"type": "csv"}},
        "mark": "circle",
        "encoding": {
            "longitude": {"field": "longitude", "type": "quantitative"},
            "latitude": {"field": "latitude", "type": "quantitative"}
        }
    });
    let config = init_config(spec.get("config"));
    let mut model = Model::new_unit(&spec, None, "", None, &config);
    model.parse();

    let root = PlaceholderDataFlowNode::new(None, "root");
    GeoPointNode::parse_all(&root, &model);

    let mut node = children(&root).first().cloned();
    while let Some(n) = node {
        assert!(GeoPointNode::is_instance(&n));
        let transform = assemble(&n);
        assert_eq!(transform["type"], json!("geopoint"));
        let fields = transform["fields"].as_array().unwrap();
        assert!(fields.contains(&json!("longitude")));
        assert!(fields.contains(&json!("latitude")));
        let as_ = transform["as"].as_array().unwrap();
        assert!(as_.contains(&json!(model.get_name("x"))));
        assert!(as_.contains(&json!(model.get_name("y"))));
        assert!(transform.get("projection").is_some());
        assert!(children(&n).len() <= 1);
        node = children(&n).first().cloned();
    }
}

#[test]
fn dependent_fields_should_return_fields() {
    let gp = GeoPointNode::new(
        None,
        "mercator",
        [json!("foo"), json!("bar")],
        ["f1".to_string(), "f2".to_string()],
    );
    let deps: Vec<String> = dependent_fields(&gp).into_iter().collect();
    assert_eq!(deps, vec!["foo".to_string(), "bar".to_string()]);
}

#[test]
fn produced_fields_should_return_fields() {
    let gp = GeoPointNode::new(
        None,
        "mercator",
        [json!("foo"), json!("bar")],
        ["f1".to_string(), "f2".to_string()],
    );
    let produced: Vec<String> = produced_fields(&gp).unwrap().into_iter().collect();
    assert_eq!(produced, vec!["f1".to_string(), "f2".to_string()]);
}

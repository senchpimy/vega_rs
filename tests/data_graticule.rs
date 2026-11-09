//! Port of `test/compile/data/graticule.test.ts`.

use indexmap::IndexMap;
use serde_json::json;

use vega_lite::compile::data::assemble::assemble_root_data;
use vega_lite::compile::data::dataflow::{assemble, dependent_fields, produced_fields};
use vega_lite::compile::data::graticule::GraticuleNode;
use vega_lite::compile::data::parse::parse_data;
use vega_lite::compile::model::Model;
use vega_lite::compile::support::init_config;

#[test]
fn should_return_a_proper_vg_transform_with_object_param() {
    let graticule = GraticuleNode::new(None, &json!({"stepMinor": [15, 10], "precision": 2}));
    assert_eq!(
        assemble(&graticule),
        json!({"type": "graticule", "stepMinor": [15, 10], "precision": 2})
    );
}

#[test]
fn should_return_a_proper_vg_transform_with_true_param() {
    let graticule = GraticuleNode::new(None, &json!(true));
    assert_eq!(assemble(&graticule), json!({"type": "graticule"}));
}

#[test]
fn dependent_fields_should_return_empty_set() {
    let graticule = GraticuleNode::new(None, &json!(true));
    assert!(dependent_fields(&graticule).is_empty());
}

#[test]
fn produced_fields_should_return_undefined() {
    let graticule = GraticuleNode::new(None, &json!(true));
    assert!(produced_fields(&graticule).is_none());
}

#[test]
fn should_parse_and_add_generator_transform_correctly() {
    let spec = json!({
        "data": {"graticule": true},
        "mark": "geoshape"
    });
    let config = init_config(spec.get("config"));
    let mut model = Model::new_unit(&spec, None, "", None, &config);
    model.parse_scale();
    model.parse_layout_size();

    let data_component = parse_data(&model);
    let data = assemble_root_data(&data_component, &IndexMap::new());
    assert_eq!(data[0]["transform"][0], json!({"type": "graticule"}));
}

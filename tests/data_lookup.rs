//! Port of `test/compile/data/lookup.test.ts` (node-level tests).
//!
//! The two tests that go through `parseTransformArray` are skipped because the
//! parse layer and the selection component registry are not ported yet.

use serde_json::json;
use vega_lite::compile::data::dataflow::{
    assemble, clone_node, dependent_fields, hash, parent, produced_fields,
    PlaceholderDataFlowNode,
};
use vega_lite::compile::data::lookup::LookupNode;

fn nested_transform() -> serde_json::Value {
    json!({
        "lookup": "person",
        "from": {
            "data": {"url": "data/lookup_people.csv"},
            "key": "name"
        }
    })
}

#[test]
fn should_create_node_for_flat_lookup() {
    let lookup = LookupNode::new(
        None,
        &json!({
            "lookup": "person",
            "from": {
                "data": {"url": "data/lookup_people.csv"},
                "key": "name",
                "fields": ["age", "height"]
            }
        }),
        Some("lookup_0"),
    );
    assert_eq!(
        assemble(&lookup),
        json!({
            "type": "lookup",
            "from": "lookup_0",
            "key": "name",
            "fields": ["person"],
            "values": ["age", "height"]
        })
    );
}

#[test]
fn should_create_node_for_nested_lookup() {
    let mut transform = nested_transform();
    transform.as_object_mut().unwrap().insert("as".into(), json!("foo"));
    let lookup = LookupNode::new(None, &transform, Some("lookup_0"));
    assert_eq!(
        assemble(&lookup),
        json!({
            "type": "lookup",
            "from": "lookup_0",
            "key": "name",
            "fields": ["person"],
            "as": ["foo"]
        })
    );
}

#[test]
fn should_warn_if_fields_are_not_specified_and_as_is_missing() {
    let (result, logger) = vega_lite::log::capture(|| {
        let lookup = LookupNode::new(None, &nested_transform(), Some("lookup_0"));
        assemble(&lookup)
    });
    assert_eq!(
        result,
        json!({
            "type": "lookup",
            "from": "lookup_0",
            "key": "name",
            "fields": ["person"],
            "as": ["_lookup"]
        })
    );
    assert_eq!(
        logger.warns[0],
        json!("If \"from.fields\" is not specified, \"as\" has to be a string that specifies the key to be used for the data from the secondary source.")
    );
}

#[test]
fn dependent_fields_should_return_proper_dependent_fields() {
    let lookup = LookupNode::new(None, &nested_transform(), Some("lookup_0"));
    let fields: Vec<String> = dependent_fields(&lookup).into_iter().collect();
    assert_eq!(fields, vec!["person".to_string()]);
}

#[test]
fn produced_fields_should_return_empty_set() {
    let lookup = LookupNode::new(None, &nested_transform(), Some("lookup_0"));
    assert!(produced_fields(&lookup).unwrap().is_empty());
}

#[test]
fn produced_fields_should_return_fields_when_specified() {
    let mut transform = nested_transform();
    transform
        .as_object_mut()
        .unwrap()
        .get_mut("from")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert("fields".into(), json!(["foo", "bar"]));
    let lookup = LookupNode::new(None, &transform, Some("lookup_0"));
    let fields: Vec<String> = produced_fields(&lookup).unwrap().into_iter().collect();
    assert_eq!(fields, vec!["foo".to_string(), "bar".to_string()]);
}

#[test]
fn produced_fields_should_return_as_fields_when_fields_and_as_are_specified() {
    let mut transform = nested_transform();
    let from = transform.get_mut("from").unwrap().as_object_mut().unwrap();
    from.insert("fields".into(), json!(["foo", "bar"]));
    transform.as_object_mut().unwrap().insert("as".into(), json!(["f1", "f2"]));
    let lookup = LookupNode::new(None, &transform, Some("lookup_0"));
    let fields: Vec<String> = produced_fields(&lookup).unwrap().into_iter().collect();
    assert_eq!(fields, vec!["f1".to_string(), "f2".to_string()]);
}

#[test]
fn hash_should_generate_the_correct_hash() {
    let mut transform = nested_transform();
    transform.as_object_mut().unwrap().insert("as".into(), json!("lookedup"));
    let lookup = LookupNode::new(None, &transform, Some("lookup_0"));
    let _ = assemble(&lookup);
    assert_eq!(
        hash(&lookup),
        json!("Lookup {\"secondary\":\"lookup_0\",\"transform\":{\"as\":\"lookedup\",\"from\":{\"data\":{\"url\":\"data/lookup_people.csv\"},\"key\":\"name\"},\"lookup\":\"person\"}}")
    );
}

#[test]
fn clone_should_never_clone_parent() {
    let parent_node = PlaceholderDataFlowNode::new(None, "parent");
    let lookup = LookupNode::new(Some(&parent_node), &nested_transform(), None);
    assert!(parent(&clone_node(&lookup)).is_none());
}

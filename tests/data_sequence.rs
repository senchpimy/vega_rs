//! Port of `test/compile/data/sequence.test.ts` (node-level tests; the model
//! integration case in the last upstream test needs `parseUnitModel`).

use serde_json::json;
use vega_lite::compile::data::dataflow::{assemble, dependent_fields, produced_fields};
use vega_lite::compile::data::sequence::SequenceNode;

#[test]
fn assemble_should_return_a_proper_vg_transform() {
    let params = json!({"start": 1, "stop": 10, "step": 2});
    let sequence = SequenceNode::new(None, &params);
    assert_eq!(
        assemble(&sequence),
        json!({"type": "sequence", "start": 1, "stop": 10, "step": 2})
    );
}

#[test]
fn dependent_fields_should_return_empty_set() {
    let sequence = SequenceNode::new(None, &json!({"start": 1, "stop": 10, "step": 2}));
    assert!(dependent_fields(&sequence).is_empty());
}

#[test]
fn produced_fields_should_return_correct_default() {
    let sequence = SequenceNode::new(None, &json!({"start": 1, "stop": 10, "step": 2}));
    let fields: Vec<String> = produced_fields(&sequence).unwrap().into_iter().collect();
    assert_eq!(fields, vec!["data".to_string()]);
}

#[test]
fn produced_fields_should_return_specified_field() {
    let sequence = SequenceNode::new(None, &json!({"start": 1, "stop": 10, "step": 2, "as": "foo"}));
    let fields: Vec<String> = produced_fields(&sequence).unwrap().into_iter().collect();
    assert_eq!(fields, vec!["foo".to_string()]);
}

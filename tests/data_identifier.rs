//! Port of the identifier node tests (upstream coverage lives in the
//! selection tests, which are out of scope).

use serde_json::json;
use vega_lite::compile::data::dataflow::{
    assemble, dependent_fields, hash, produced_fields,
};
use vega_lite::compile::data::identifier::{IdentifierNode, SELECTION_ID};

#[test]
fn identifier_node_fields_and_transform() {
    let node = IdentifierNode::new(None);
    assert!(dependent_fields(&node).is_empty());
    let fields: Vec<String> = produced_fields(&node).unwrap().into_iter().collect();
    assert_eq!(fields, vec![SELECTION_ID.to_string()]);
    assert_eq!(hash(&node), json!("Identifier"));
    assert_eq!(
        assemble(&node),
        json!({"type": "identifier", "as": "_vgsid_"})
    );
}

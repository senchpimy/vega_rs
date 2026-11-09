//! Port of `src/compile/data/identifier.ts`.

use std::any::Any;

use serde_json::{json, Map, Value};

use super::dataflow::{DataFlowNode, FieldSet, NodeBehavior, NodeRef};

pub const SELECTION_ID: &str = "_vgsid_";

#[derive(Debug, Clone, Default)]
pub struct IdentifierNode;

impl IdentifierNode {
    pub fn new(parent: Option<&NodeRef>) -> NodeRef {
        DataFlowNode::new(Box::new(IdentifierNode), parent, None)
    }
}

impl NodeBehavior for IdentifierNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        Value::String("Identifier".to_string())
    }

    fn dependent_fields(&self) -> FieldSet {
        FieldSet::new()
    }

    fn produced_fields(&self) -> Option<FieldSet> {
        Some(FieldSet::from([SELECTION_ID.to_string()]))
    }

    fn assemble(&self) -> Value {
        let mut result = Map::new();
        result.insert("type".into(), json!("identifier"));
        result.insert("as".into(), json!(SELECTION_ID));
        Value::Object(result)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

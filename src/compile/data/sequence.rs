//! Port of `src/compile/data/sequence.ts`.

use std::any::Any;

use serde_json::{json, Map, Value};

use crate::util::hash;

use super::dataflow::{DataFlowNode, FieldSet, NodeBehavior, NodeRef};
use super::support::hash_str;

#[derive(Debug, Clone)]
pub struct SequenceNode {
    pub params: Value,
}

impl SequenceNode {
    pub fn new(parent: Option<&NodeRef>, params: &Value) -> NodeRef {
        DataFlowNode::new(
            Box::new(SequenceNode {
                params: params.clone(),
            }),
            parent,
            None,
        )
    }
}

impl NodeBehavior for SequenceNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        Value::String(format!("Hash {}", hash_str(&hash(&self.params))))
    }

    fn dependent_fields(&self) -> FieldSet {
        FieldSet::new()
    }

    fn produced_fields(&self) -> Option<FieldSet> {
        let as_ = self
            .params
            .get("as")
            .and_then(Value::as_str)
            .unwrap_or("data");
        Some(FieldSet::from([as_.to_string()]))
    }

    fn assemble(&self) -> Value {
        let mut result = Map::new();
        result.insert("type".into(), json!("sequence"));
        if let Some(obj) = self.params.as_object() {
            for (k, v) in obj {
                result.insert(k.clone(), v.clone());
            }
        }
        Value::Object(result)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

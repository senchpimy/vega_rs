//! Port of `src/compile/data/sample.ts`.

use std::any::Any;

use serde_json::{json, Map, Value};

use crate::util::{duplicate, hash};

use super::dataflow::{DataFlowNode, FieldSet, NodeBehavior, NodeRef};
use super::support::hash_str;

#[derive(Debug, Clone)]
pub struct SampleTransformNode {
    pub transform: Value,
}

impl SampleTransformNode {
    pub fn new(parent: Option<&NodeRef>, transform: &Value) -> NodeRef {
        DataFlowNode::new(
            Box::new(SampleTransformNode {
                transform: duplicate(transform),
            }),
            parent,
            None,
        )
    }
}

impl NodeBehavior for SampleTransformNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        Value::String(format!("SampleTransform {}", hash_str(&hash(&self.transform))))
    }

    fn dependent_fields(&self) -> FieldSet {
        FieldSet::new()
    }

    fn produced_fields(&self) -> Option<FieldSet> {
        Some(FieldSet::new())
    }

    fn assemble(&self) -> Value {
        let mut result = Map::new();
        result.insert("type".into(), json!("sample"));
        result.insert(
            "size".into(),
            self.transform.get("sample").cloned().unwrap_or(Value::Null),
        );
        Value::Object(result)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

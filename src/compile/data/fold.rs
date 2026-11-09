//! Port of `src/compile/data/fold.ts`.

use std::any::Any;

use serde_json::{json, Map, Value};

use crate::util::{duplicate, hash};

use super::dataflow::{DataFlowNode, FieldSet, NodeBehavior, NodeRef};
use super::support::hash_str;

#[derive(Debug, Clone)]
pub struct FoldTransformNode {
    pub transform: Value,
}

impl FoldTransformNode {
    pub fn new(parent: Option<&NodeRef>, transform: &Value) -> NodeRef {
        let mut transform = duplicate(transform);
        let specified_as = transform
            .get("as")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let first = specified_as
            .first()
            .and_then(Value::as_str)
            .unwrap_or("key");
        let second = specified_as
            .get(1)
            .and_then(Value::as_str)
            .unwrap_or("value");
        if let Some(obj) = transform.as_object_mut() {
            obj.insert("as".into(), json!([first, second]));
        }
        DataFlowNode::new(Box::new(FoldTransformNode { transform }), parent, None)
    }
}

impl NodeBehavior for FoldTransformNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        Value::String(format!("FoldTransform {}", hash_str(&hash(&self.transform))))
    }

    fn dependent_fields(&self) -> FieldSet {
        self.transform
            .get("fold")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default()
    }

    fn produced_fields(&self) -> Option<FieldSet> {
        Some(
            self.transform
                .get("as")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
                .unwrap_or_default(),
        )
    }

    fn assemble(&self) -> Value {
        let mut result = Map::new();
        result.insert("type".into(), json!("fold"));
        result.insert(
            "fields".into(),
            self.transform.get("fold").cloned().unwrap_or_else(|| json!([])),
        );
        result.insert(
            "as".into(),
            self.transform.get("as").cloned().unwrap_or_else(|| json!([])),
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

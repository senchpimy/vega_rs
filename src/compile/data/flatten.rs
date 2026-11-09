//! Port of `src/compile/data/flatten.ts`.

use std::any::Any;

use serde_json::{json, Map, Value};

use crate::util::{duplicate, hash};

use super::dataflow::{DataFlowNode, FieldSet, NodeBehavior, NodeRef};
use super::support::hash_str;

/// A class for flatten transform nodes.
#[derive(Debug, Clone)]
pub struct FlattenTransformNode {
    pub transform: Value,
}

impl FlattenTransformNode {
    pub fn new(parent: Option<&NodeRef>, transform: &Value) -> NodeRef {
        let mut transform = duplicate(transform);
        let flatten = transform
            .get("flatten")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let specified_as = transform
            .get("as")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let as_: Vec<Value> = flatten
            .iter()
            .enumerate()
            .map(|(i, f)| {
                specified_as
                    .get(i)
                    .filter(|v| !v.is_null())
                    .cloned()
                    .unwrap_or_else(|| f.clone())
            })
            .collect();
        if let Some(obj) = transform.as_object_mut() {
            obj.insert("as".into(), Value::Array(as_));
        }

        DataFlowNode::new(Box::new(FlattenTransformNode { transform }), parent, None)
    }
}

impl NodeBehavior for FlattenTransformNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        Value::String(format!("FlattenTransform {}", hash_str(&hash(&self.transform))))
    }

    fn dependent_fields(&self) -> FieldSet {
        self.transform
            .get("flatten")
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
        result.insert("type".into(), json!("flatten"));
        result.insert(
            "fields".into(),
            self.transform
                .get("flatten")
                .cloned()
                .unwrap_or_else(|| json!([])),
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

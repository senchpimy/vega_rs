//! Port of `src/compile/data/density.ts`.

use std::any::Any;

use serde_json::{json, Map, Value};

use crate::util::{duplicate, hash};

use super::dataflow::{DataFlowNode, FieldSet, NodeBehavior, NodeRef};
use super::support::hash_str;

/// A class for density transform nodes.
#[derive(Debug, Clone)]
pub struct DensityTransformNode {
    pub transform: Value,
}

impl DensityTransformNode {
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
            .unwrap_or("value");
        let second = specified_as
            .get(1)
            .and_then(Value::as_str)
            .unwrap_or("density");
        let resolve = transform
            .get("resolve")
            .and_then(Value::as_str)
            .unwrap_or("shared")
            .to_string();
        if let Some(obj) = transform.as_object_mut() {
            obj.insert("as".into(), json!([first, second]));
            obj.insert("resolve".into(), json!(resolve));
        }
        DataFlowNode::new(Box::new(DensityTransformNode { transform }), parent, None)
    }
}

impl NodeBehavior for DensityTransformNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        Value::String(format!("DensityTransform {}", hash_str(&hash(&self.transform))))
    }

    fn dependent_fields(&self) -> FieldSet {
        let mut out = FieldSet::new();
        if let Some(f) = self.transform.get("density").and_then(Value::as_str) {
            out.insert(f.to_string());
        }
        if let Some(groupby) = self.transform.get("groupby").and_then(Value::as_array) {
            for f in groupby.iter().filter_map(Value::as_str) {
                out.insert(f.to_string());
            }
        }
        out
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
        result.insert("type".into(), json!("kde"));
        result.insert(
            "field".into(),
            self.transform.get("density").cloned().unwrap_or(Value::Null),
        );
        if let Some(obj) = self.transform.as_object() {
            for (k, v) in obj {
                if k != "density" {
                    result.insert(k.clone(), v.clone());
                }
            }
        }
        result.insert(
            "resolve".into(),
            self.transform.get("resolve").cloned().unwrap_or(Value::Null),
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

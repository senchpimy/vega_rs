//! Port of `src/compile/data/pivot.ts`.

use std::any::Any;

use serde_json::{json, Map, Value};

use crate::util::{duplicate, hash};

use super::dataflow::{DataFlowNode, FieldSet, NodeBehavior, NodeRef};
use super::support::hash_str;

#[derive(Debug, Clone)]
pub struct PivotTransformNode {
    pub transform: Value,
}

impl PivotTransformNode {
    pub fn new(parent: Option<&NodeRef>, transform: &Value) -> NodeRef {
        DataFlowNode::new(
            Box::new(PivotTransformNode {
                transform: duplicate(transform),
            }),
            parent,
            None,
        )
    }

    pub fn add_dimensions(node: &NodeRef, fields: &[String]) {
        super::dataflow::with_node_mut::<PivotTransformNode, _>(node, |n| {
            let mut groupby: Vec<Value> = n
                .transform
                .get("groupby")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for f in fields {
                let v = json!(f);
                if !groupby.contains(&v) {
                    groupby.push(v);
                }
            }
            if let Some(obj) = n.transform.as_object_mut() {
                obj.insert("groupby".into(), Value::Array(groupby));
            }
        });
    }
}

impl NodeBehavior for PivotTransformNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        Value::String(format!("PivotTransform {}", hash_str(&hash(&self.transform))))
    }

    fn dependent_fields(&self) -> FieldSet {
        let mut out = FieldSet::new();
        if let Some(p) = self.transform.get("pivot").and_then(Value::as_str) {
            out.insert(p.to_string());
        }
        if let Some(v) = self.transform.get("value").and_then(Value::as_str) {
            out.insert(v.to_string());
        }
        if let Some(groupby) = self.transform.get("groupby").and_then(Value::as_array) {
            for g in groupby.iter().filter_map(Value::as_str) {
                out.insert(g.to_string());
            }
        }
        out
    }

    fn produced_fields(&self) -> Option<FieldSet> {
        None
    }

    fn assemble(&self) -> Value {
        let mut result = Map::new();
        result.insert("type".into(), json!("pivot"));
        result.insert(
            "field".into(),
            self.transform.get("pivot").cloned().unwrap_or(Value::Null),
        );
        result.insert(
            "value".into(),
            self.transform.get("value").cloned().unwrap_or(Value::Null),
        );
        if let Some(limit) = self.transform.get("limit").filter(|v| !v.is_null()) {
            result.insert("limit".into(), limit.clone());
        }
        if let Some(op) = self.transform.get("op").filter(|v| !v.is_null()) {
            result.insert("op".into(), op.clone());
        }
        if let Some(groupby) = self.transform.get("groupby").filter(|v| !v.is_null()) {
            result.insert("groupby".into(), groupby.clone());
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

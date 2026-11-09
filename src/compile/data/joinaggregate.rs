//! Port of `src/compile/data/joinaggregate.ts`.

use std::any::Any;

use serde_json::{json, Map, Value};

use crate::util::{duplicate, hash};

use super::dataflow::{DataFlowNode, FieldSet, NodeBehavior, NodeRef};
use super::support::{default_name, hash_str};

#[derive(Debug, Clone)]
pub struct JoinAggregateTransformNode {
    pub transform: Value,
}

impl JoinAggregateTransformNode {
    pub fn new(parent: Option<&NodeRef>, transform: &Value) -> NodeRef {
        DataFlowNode::new(
            Box::new(JoinAggregateTransformNode {
                transform: duplicate(transform),
            }),
            parent,
            None,
        )
    }

    pub fn add_dimensions(node: &NodeRef, fields: &[String]) {
        super::dataflow::with_node_mut::<JoinAggregateTransformNode, _>(node, |n| {
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

impl NodeBehavior for JoinAggregateTransformNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        Value::String(format!(
            "JoinAggregateTransform {}",
            hash_str(&hash(&self.transform))
        ))
    }

    fn dependent_fields(&self) -> FieldSet {
        let mut out = FieldSet::new();
        if let Some(groupby) = self.transform.get("groupby").and_then(Value::as_array) {
            for f in groupby.iter().filter_map(Value::as_str) {
                out.insert(f.to_string());
            }
        }
        if let Some(joinaggregate) = self.transform.get("joinaggregate").and_then(Value::as_array) {
            for w in joinaggregate {
                if let Some(f) = w.get("field").and_then(Value::as_str) {
                    out.insert(f.to_string());
                }
            }
        }
        out
    }

    fn produced_fields(&self) -> Option<FieldSet> {
        Some(
            self.transform
                .get("joinaggregate")
                .and_then(Value::as_array)
                .map(|a| a.iter().map(default_name).collect())
                .unwrap_or_default(),
        )
    }

    fn assemble(&self) -> Value {
        let mut fields: Vec<Value> = Vec::new();
        let mut ops: Vec<Value> = Vec::new();
        let mut as_: Vec<Value> = Vec::new();

        if let Some(joinaggregate) = self.transform.get("joinaggregate").and_then(Value::as_array) {
            for w in joinaggregate {
                ops.push(w.get("op").cloned().unwrap_or(Value::Null));
                as_.push(json!(default_name(w)));
                fields.push(w.get("field").filter(|v| !v.is_null()).cloned().unwrap_or(Value::Null));
            }
        }

        let mut result = Map::new();
        result.insert("type".into(), json!("joinaggregate"));
        result.insert("as".into(), Value::Array(as_));
        result.insert("ops".into(), Value::Array(ops));
        result.insert("fields".into(), Value::Array(fields));
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

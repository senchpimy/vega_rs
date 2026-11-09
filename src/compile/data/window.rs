//! Port of `src/compile/data/window.ts`.

use std::any::Any;

use serde_json::{json, Map, Value};

use crate::util::{duplicate, hash};

use super::dataflow::{DataFlowNode, FieldSet, NodeBehavior, NodeRef};
use super::support::{default_name, hash_str, is_aggregate_op};

#[derive(Debug, Clone)]
pub struct WindowTransformNode {
    pub transform: Value,
}

impl WindowTransformNode {
    pub fn new(parent: Option<&NodeRef>, transform: &Value) -> NodeRef {
        DataFlowNode::new(
            Box::new(WindowTransformNode {
                transform: duplicate(transform),
            }),
            parent,
            None,
        )
    }

    pub fn add_dimensions(node: &NodeRef, fields: &[String]) {
        super::dataflow::with_node_mut::<WindowTransformNode, _>(node, |n| {
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

impl NodeBehavior for WindowTransformNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        Value::String(format!("WindowTransform {}", hash_str(&hash(&self.transform))))
    }

    fn dependent_fields(&self) -> FieldSet {
        let mut out = FieldSet::new();
        if let Some(groupby) = self.transform.get("groupby").and_then(Value::as_array) {
            for f in groupby.iter().filter_map(Value::as_str) {
                out.insert(f.to_string());
            }
        }
        if let Some(sort) = self.transform.get("sort").and_then(Value::as_array) {
            for m in sort {
                if let Some(f) = m.get("field").and_then(Value::as_str) {
                    out.insert(f.to_string());
                }
            }
        }
        if let Some(window) = self.transform.get("window").and_then(Value::as_array) {
            for w in window {
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
                .get("window")
                .and_then(Value::as_array)
                .map(|a| a.iter().map(default_name).collect())
                .unwrap_or_default(),
        )
    }

    fn assemble(&self) -> Value {
        let mut fields: Vec<Value> = Vec::new();
        let mut ops: Vec<Value> = Vec::new();
        let mut as_: Vec<Value> = Vec::new();
        let mut params: Vec<Value> = Vec::new();

        if let Some(window) = self.transform.get("window").and_then(Value::as_array) {
            for w in window {
                ops.push(w.get("op").cloned().unwrap_or(Value::Null));
                as_.push(json!(default_name(w)));
                params.push(w.get("param").filter(|v| !v.is_null()).cloned().unwrap_or(Value::Null));
                fields.push(w.get("field").filter(|v| !v.is_null()).cloned().unwrap_or(Value::Null));
            }
        }

        let frame = self.transform.get("frame").filter(|v| !v.is_null()).cloned();
        let groupby = self
            .transform
            .get("groupby")
            .filter(|v| !v.is_null())
            .cloned();

        if let Some(Value::Array(f)) = &frame {
            if f.len() == 2 && f[0].is_null() && f[1].is_null() && ops.iter().all(is_aggregate_op) {
                let mut result = Map::new();
                result.insert("type".into(), json!("joinaggregate"));
                result.insert("as".into(), Value::Array(as_));
                result.insert("ops".into(), Value::Array(ops));
                result.insert("fields".into(), Value::Array(fields));
                if let Some(groupby) = groupby {
                    result.insert("groupby".into(), groupby);
                }
                return Value::Object(result);
            }
        }

        let mut sort_fields: Vec<Value> = Vec::new();
        let mut sort_order: Vec<Value> = Vec::new();
        if let Some(sort) = self.transform.get("sort").and_then(Value::as_array) {
            for sort_field in sort {
                sort_fields.push(sort_field.get("field").cloned().unwrap_or(Value::Null));
                sort_order.push(
                    sort_field
                        .get("order")
                        .filter(|v| !v.is_null())
                        .cloned()
                        .unwrap_or_else(|| json!("ascending")),
                );
            }
        }

        let mut result = Map::new();
        result.insert("type".into(), json!("window"));
        result.insert("params".into(), Value::Array(params));
        result.insert("as".into(), Value::Array(as_));
        result.insert("ops".into(), Value::Array(ops));
        result.insert("fields".into(), Value::Array(fields));
        result.insert(
            "sort".into(),
            json!({"field": sort_fields, "order": sort_order}),
        );
        if let Some(ignore_peers) = self.transform.get("ignorePeers").filter(|v| !v.is_null()) {
            result.insert("ignorePeers".into(), ignore_peers.clone());
        }
        if let Some(groupby) = groupby {
            result.insert("groupby".into(), groupby);
        }
        if let Some(frame) = frame {
            result.insert("frame".into(), frame);
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

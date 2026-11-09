//! Port of `src/compile/data/regression.ts`.

use std::any::Any;

use serde_json::{json, Map, Value};

use crate::util::{duplicate, hash};

use super::dataflow::{DataFlowNode, FieldSet, NodeBehavior, NodeRef};
use super::support::hash_str;

/// A class for regression transform nodes.
#[derive(Debug, Clone)]
pub struct RegressionTransformNode {
    pub transform: Value,
}

impl RegressionTransformNode {
    pub fn new(parent: Option<&NodeRef>, transform: &Value) -> NodeRef {
        let mut transform = duplicate(transform);
        let specified_as = transform
            .get("as")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let on = transform.get("on").cloned().unwrap_or(Value::Null);
        let regression = transform.get("regression").cloned().unwrap_or(Value::Null);
        let first = specified_as
            .first()
            .filter(|v| !v.is_null())
            .cloned()
            .unwrap_or(on);
        let second = specified_as
            .get(1)
            .filter(|v| !v.is_null())
            .cloned()
            .unwrap_or(regression);
        if let Some(obj) = transform.as_object_mut() {
            obj.insert("as".into(), json!([first, second]));
        }
        DataFlowNode::new(Box::new(RegressionTransformNode { transform }), parent, None)
    }
}

impl NodeBehavior for RegressionTransformNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        Value::String(format!(
            "RegressionTransform {}",
            hash_str(&hash(&self.transform))
        ))
    }

    fn dependent_fields(&self) -> FieldSet {
        let mut out = FieldSet::new();
        for key in ["regression", "on"] {
            if let Some(f) = self.transform.get(key).and_then(Value::as_str) {
                out.insert(f.to_string());
            }
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
        result.insert("type".into(), json!("regression"));
        result.insert(
            "x".into(),
            self.transform.get("on").cloned().unwrap_or(Value::Null),
        );
        result.insert(
            "y".into(),
            self.transform
                .get("regression")
                .cloned()
                .unwrap_or(Value::Null),
        );
        if let Some(obj) = self.transform.as_object() {
            for (k, v) in obj {
                if k != "regression" && k != "on" {
                    result.insert(k.clone(), v.clone());
                }
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

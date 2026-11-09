//! Port of `src/compile/data/extent.ts`.

use std::any::Any;

use serde_json::{json, Map, Value};

use crate::util::{duplicate, hash};

use super::dataflow::{DataFlowNode, FieldSet, NodeBehavior, NodeRef};
use super::support::hash_str;

/// A class for extent transform nodes.
#[derive(Debug, Clone)]
pub struct ExtentTransformNode {
    pub transform: Value,
}

/// Port of `isExtent` from `src/transform.ts`.
pub fn is_extent(transform: &Value) -> bool {
    let Some(o) = transform.as_object() else {
        return false;
    };
    let has_extent = o.get("extent").map(|v| !v.is_null()).unwrap_or(false);
    has_extent
        && o.get("param").map(|v| !v.is_null()).unwrap_or(false)
        && (o.get("density").is_none() || o.get("density") == Some(&Value::Null))
        && (o.get("regression").is_none() || o.get("regression") == Some(&Value::Null))
}

impl ExtentTransformNode {
    pub fn new(parent: Option<&NodeRef>, transform: &Value) -> NodeRef {
        DataFlowNode::new(
            Box::new(ExtentTransformNode {
                transform: duplicate(transform),
            }),
            parent,
            None,
        )
    }
}

impl NodeBehavior for ExtentTransformNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        Value::String(format!("ExtentTransform {}", hash_str(&hash(&self.transform))))
    }

    fn dependent_fields(&self) -> FieldSet {
        self.transform
            .get("extent")
            .and_then(Value::as_str)
            .map(|f| FieldSet::from([f.to_string()]))
            .unwrap_or_default()
    }

    fn produced_fields(&self) -> Option<FieldSet> {
        Some(FieldSet::new())
    }

    fn assemble(&self) -> Value {
        let mut result = Map::new();
        result.insert("type".into(), json!("extent"));
        result.insert(
            "field".into(),
            self.transform.get("extent").cloned().unwrap_or(Value::Null),
        );
        result.insert(
            "signal".into(),
            self.transform.get("param").cloned().unwrap_or(Value::Null),
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

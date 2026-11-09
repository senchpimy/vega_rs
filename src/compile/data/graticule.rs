//! Port of `src/compile/data/graticule.ts`.

use std::any::Any;

use serde_json::{json, Map, Value};

use crate::util::hash;

use super::dataflow::{DataFlowNode, FieldSet, NodeBehavior, NodeRef};
use super::support::hash_str;

#[derive(Debug, Clone)]
pub struct GraticuleNode {
    /// `true` or a `GraticuleParams` object.
    pub params: Value,
}

impl GraticuleNode {
    pub fn new(parent: Option<&NodeRef>, params: &Value) -> NodeRef {
        DataFlowNode::new(
            Box::new(GraticuleNode {
                params: params.clone(),
            }),
            parent,
            None,
        )
    }
}

impl NodeBehavior for GraticuleNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        Value::String(format!("Graticule {}", hash_str(&hash(&self.params))))
    }

    fn dependent_fields(&self) -> FieldSet {
        FieldSet::new()
    }

    fn produced_fields(&self) -> Option<FieldSet> {
        None // there should never be a node before graticule
    }

    fn assemble(&self) -> Value {
        let mut result = Map::new();
        result.insert("type".into(), json!("graticule"));
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

/// Port of `isGraticuleGenerator`.
pub fn is_graticule_generator(data: &Value) -> bool {
    data.get("graticule")
        .map(|v| !v.is_null())
        .unwrap_or(false)
}

//! Port of `src/compile/data/calculate.ts` (dataflow node only; the
//! sort-array-index helper lives in `calculate::sort_array_index_field`).

use std::any::Any;

use serde_json::{json, Map, Value};

use crate::util::{duplicate, hash};

use super::dataflow::{DataFlowNode, FieldSet, NodeBehavior, NodeRef};
use super::support::{get_dependent_fields, hash_str, vg_field};

#[derive(Debug, Clone)]
pub struct CalculateNode {
    pub transform: Value,
    pub dependent_fields: FieldSet,
}

impl CalculateNode {
    pub fn new(parent: Option<&NodeRef>, transform: &Value) -> NodeRef {
        let transform = duplicate(transform);
        let expr = transform
            .get("calculate")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let dependent_fields = get_dependent_fields(&expr);
        DataFlowNode::new(
            Box::new(CalculateNode {
                transform,
                dependent_fields,
            }),
            parent,
            None,
        )
    }
}

impl NodeBehavior for CalculateNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        Value::String(format!("Calculate {}", hash_str(&hash(&self.transform))))
    }

    fn dependent_fields(&self) -> FieldSet {
        self.dependent_fields.clone()
    }

    fn produced_fields(&self) -> Option<FieldSet> {
        Some(
            self.transform
                .get("as")
                .and_then(Value::as_str)
                .map(|s| FieldSet::from([s.to_string()]))
                .unwrap_or_default(),
        )
    }

    fn assemble(&self) -> Value {
        let mut result = Map::new();
        result.insert("type".into(), json!("formula"));
        result.insert(
            "expr".into(),
            self.transform
                .get("calculate")
                .cloned()
                .unwrap_or(Value::Null),
        );
        result.insert(
            "as".into(),
            self.transform.get("as").cloned().unwrap_or(Value::Null),
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

/// Port of `sortArrayIndexField`.
pub fn sort_array_index_field(
    field: Option<&str>,
    channel: Option<&str>,
    fn_name: Option<&str>,
    for_as: bool,
) -> String {
    let field = vg_field(field, fn_name, Some("sort_index"), for_as);
    match channel {
        Some(channel) => format!("{channel}_{field}"),
        None => field,
    }
}

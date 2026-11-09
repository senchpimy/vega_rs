//! Port of `src/compile/data/geopoint.ts`.

use std::any::Any;

use serde_json::{json, Map, Value};

use crate::channel::{LATITUDE, LATITUDE2, LONGITUDE, LONGITUDE2};
use crate::channeldef::{get_field_or_datum_def, is_datum_def, is_field_def, is_value_def};
use crate::util::hash;

use super::dataflow::{with_node, DataFlowNode, FieldSet, NodeBehavior, NodeRef};
use super::support::hash_str;
use crate::compile::model::Model;

#[derive(Debug, Clone)]
pub struct GeoPointNode {
    pub projection: String,
    pub fields: [Value; 2],
    pub as_: [String; 2],
}

impl GeoPointNode {
    pub fn new(
        parent: Option<&NodeRef>,
        projection: &str,
        fields: [Value; 2],
        as_: [String; 2],
    ) -> NodeRef {
        DataFlowNode::new(
            Box::new(GeoPointNode {
                projection: projection.to_string(),
                fields,
                as_,
            }),
            parent,
            None,
        )
    }

    pub fn is_instance(node: &NodeRef) -> bool {
        with_node::<GeoPointNode, _>(node, |_| ()).is_some()
    }

    /// Port of `GeoPointNode.parseAll`.
    pub fn parse_all(parent: &NodeRef, model: &Model) -> NodeRef {
        if !model.has_projection() {
            return parent.clone();
        }
        let projection = match model.projection_name(false) {
            Some(name) => name,
            None => return parent.clone(),
        };
        let mut head = parent.clone();

        for coordinates in [[LONGITUDE, LATITUDE], [LONGITUDE2, LATITUDE2]] {
            let pair: Vec<Value> = coordinates
                .iter()
                .map(|channel| {
                    let def = model
                        .encoding()
                        .map(|e| get_field_or_datum_def(&e[*channel]))
                        .unwrap_or(None);
                    match def {
                        Some(def) if is_field_def(&def) => def
                            .get("field")
                            .cloned()
                            .unwrap_or(Value::Null),
                        Some(def) if is_datum_def(&def) => {
                            let datum = def.get("datum").cloned().unwrap_or(Value::Null);
                            json!({"expr": value_to_expr(&datum)})
                        }
                        Some(def) if is_value_def(&def) => {
                            let value = def.get("value").cloned().unwrap_or(Value::Null);
                            json!({"expr": value_to_expr(&value)})
                        }
                        _ => Value::Null,
                    }
                })
                .collect();

            let suffix = if coordinates[0] == LONGITUDE2 { "2" } else { "" };
            if pair[0].is_null() && pair[1].is_null() {
                continue;
            }
            let fields = [pair[0].clone(), pair[1].clone()];
            let as_ = [
                model.get_name(&format!("x{suffix}")),
                model.get_name(&format!("y{suffix}")),
            ];
            head = GeoPointNode::new(Some(&head), &projection, fields, as_);
        }

        head
    }
}

fn value_to_expr(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => super::support::js_value_string(other),
    }
}

impl NodeBehavior for GeoPointNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        Value::String(format!(
            "Geopoint {} {} {}",
            self.projection,
            hash_str(&hash(&Value::Array(self.fields.to_vec()))),
            hash_str(&hash(&json!(self.as_)))
        ))
    }

    fn dependent_fields(&self) -> FieldSet {
        self.fields
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect()
    }

    fn produced_fields(&self) -> Option<FieldSet> {
        Some(self.as_.iter().map(|s| s.to_string()).collect())
    }

    fn assemble(&self) -> Value {
        let mut result = Map::new();
        result.insert("type".into(), json!("geopoint"));
        result.insert("projection".into(), json!(self.projection));
        result.insert("fields".into(), Value::Array(self.fields.to_vec()));
        result.insert("as".into(), json!(self.as_));
        Value::Object(result)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

//! Port of `src/compile/data/impute.ts`.

use std::any::Any;

use serde_json::{json, Map, Value};

use crate::channeldef::is_field_def;
use crate::encoding::path_grouping_fields;
use crate::util::{duplicate, hash};

use super::dataflow::{DataFlowNode, FieldSet, NodeBehavior, NodeRef};
use super::support::{hash_str, js_value_string};
use crate::compile::model::Model;

#[derive(Debug, Clone)]
pub struct ImputeNode {
    pub transform: Value,
}

/// Port of `isImputeSequence`.
pub fn is_impute_sequence(v: &Value) -> bool {
    v.as_object()
        .map(|o| o.contains_key("stop") && !o.contains_key("value"))
        .unwrap_or(false)
}

impl ImputeNode {
    pub fn new(parent: Option<&NodeRef>, transform: &Value) -> NodeRef {
        DataFlowNode::new(
            Box::new(ImputeNode {
                transform: duplicate(transform),
            }),
            parent,
            None,
        )
    }

    pub fn make_from_transform(parent: Option<&NodeRef>, impute_transform: &Value) -> NodeRef {
        ImputeNode::new(parent, impute_transform)
    }

    /// Port of `ImputeNode.makeFromEncoding`.
    pub fn make_from_encoding(parent: Option<&NodeRef>, model: &Model) -> Option<NodeRef> {
        let encoding = model.encoding()?;
        let x_def = encoding.get("x").cloned().unwrap_or(Value::Null);
        let y_def = encoding.get("y").cloned().unwrap_or(Value::Null);

        if !is_field_def(&x_def) || !is_field_def(&y_def) {
            return None;
        }

        let x_impute = x_def.get("impute").map(|v| !v.is_null()).unwrap_or(false);
        let y_impute = y_def.get("impute").map(|v| !v.is_null()).unwrap_or(false);
        let (imputed_channel, key_channel) = if x_impute {
            (&x_def, &y_def)
        } else if y_impute {
            (&y_def, &x_def)
        } else {
            return None;
        };

        let impute = imputed_channel.get("impute").cloned().unwrap_or(Value::Null);
        let method = impute.get("method").cloned().unwrap_or(Value::Null);
        let value = impute.get("value").cloned().unwrap_or(Value::Null);
        let frame = impute.get("frame").cloned().unwrap_or(Value::Null);
        let keyvals = impute.get("keyvals").cloned().unwrap_or(Value::Null);

        let mark = model.mark_type();
        let groupby_fields = path_grouping_fields(mark.as_str(), encoding);

        let mut transform = Map::new();
        transform.insert(
            "impute".into(),
            imputed_channel.get("field").cloned().unwrap_or(Value::Null),
        );
        transform.insert(
            "key".into(),
            key_channel.get("field").cloned().unwrap_or(Value::Null),
        );
        if !method.is_null() && !method.is_boolean() || method.as_str().map(|s| !s.is_empty()).unwrap_or(false) {
            transform.insert("method".into(), method);
        }
        if !value.is_null() {
            transform.insert("value".into(), value);
        }
        if !frame.is_null() {
            transform.insert("frame".into(), frame);
        }
        if !keyvals.is_null() {
            transform.insert("keyvals".into(), keyvals);
        }
        if !groupby_fields.is_empty() {
            transform.insert("groupby".into(), json!(groupby_fields));
        }

        Some(ImputeNode::new(parent, &Value::Object(transform)))
    }

    fn process_sequence(keyvals: &Value) -> Value {
        let start = keyvals.get("start").cloned().unwrap_or(json!(0));
        let stop = keyvals.get("stop").cloned().unwrap_or(Value::Null);
        let step = keyvals.get("step").cloned().unwrap_or(Value::Null);
        let mut parts = vec![js_value_string(&start), js_value_string(&stop)];
        if !step.is_null() {
            parts.push(js_value_string(&step));
        }
        json!({"signal": format!("sequence({})", parts.join(","))})
    }
}

impl NodeBehavior for ImputeNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        Value::String(format!("Impute {}", hash_str(&hash(&self.transform))))
    }

    fn dependent_fields(&self) -> FieldSet {
        let mut out = FieldSet::new();
        for key in ["impute", "key"] {
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
                .get("impute")
                .and_then(Value::as_str)
                .map(|f| FieldSet::from([f.to_string()]))
                .unwrap_or_default(),
        )
    }

    fn assemble(&self) -> Value {
        let impute = self.transform.get("impute").cloned().unwrap_or(Value::Null);
        let key = self.transform.get("key").cloned().unwrap_or(Value::Null);
        let keyvals = self.transform.get("keyvals").cloned().unwrap_or(Value::Null);
        let method = self.transform.get("method").cloned().unwrap_or(Value::Null);
        let groupby = self.transform.get("groupby").cloned().unwrap_or(Value::Null);
        let value = self.transform.get("value").cloned().unwrap_or(Value::Null);
        let frame = self
            .transform
            .get("frame")
            .cloned()
            .unwrap_or_else(|| json!([null, null]));

        let mut impute_transform = Map::new();
        impute_transform.insert("type".into(), json!("impute"));
        impute_transform.insert("field".into(), impute.clone());
        impute_transform.insert("key".into(), key);
        if !keyvals.is_null() {
            let kv = if is_impute_sequence(&keyvals) {
                ImputeNode::process_sequence(&keyvals)
            } else {
                keyvals.clone()
            };
            impute_transform.insert("keyvals".into(), kv);
        }
        impute_transform.insert("method".into(), json!("value"));
        if !groupby.is_null() {
            impute_transform.insert("groupby".into(), groupby.clone());
        }
        let method_str = method.as_str().unwrap_or("");
        if method_str.is_empty() || method_str == "value" {
            if !value.is_null() {
                impute_transform.insert("value".into(), value);
            }
        } else {
            impute_transform.insert("value".into(), Value::Null);
        }

        if !method_str.is_empty() && method_str != "value" {
            let impute_name = impute.as_str().unwrap_or("");
            let mut window = Map::new();
            window.insert("type".into(), json!("window"));
            window.insert(
                "as".into(),
                json!([format!("imputed_{impute_name}_value")]),
            );
            window.insert("ops".into(), json!([method_str]));
            window.insert("fields".into(), json!([impute.clone()]));
            window.insert("frame".into(), frame);
            window.insert("ignorePeers".into(), json!(false));
            if !groupby.is_null() {
                window.insert("groupby".into(), groupby);
            }
            let formula = json!({
                "type": "formula",
                "expr": format!("datum.{impute_name} === null ? datum.imputed_{impute_name}_value : datum.{impute_name}"),
                "as": impute_name,
            });
            Value::Array(vec![
                Value::Object(impute_transform),
                Value::Object(window),
                formula,
            ])
        } else {
            Value::Array(vec![Value::Object(impute_transform)])
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

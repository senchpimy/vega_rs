//! Port of `src/compile/data/geojson.ts`.

use std::any::Any;

use serde_json::{json, Map, Value};

use crate::channel::{LATITUDE, LATITUDE2, LONGITUDE, LONGITUDE2, SHAPE};
use crate::channeldef::{get_field_or_datum_def, is_datum_def, is_field_def, is_value_def};
use crate::util::{duplicate, hash};

use super::dataflow::{with_node, DataFlowNode, FieldSet, NodeBehavior, NodeRef};
use super::support::hash_str;
use crate::compile::model::Model;

#[derive(Debug, Clone)]
pub struct GeoJSONNode {
    pub fields: Option<Vec<Value>>,
    pub geojson: Option<String>,
    pub signal: Option<String>,
}

impl GeoJSONNode {
    pub fn new(
        parent: Option<&NodeRef>,
        fields: Option<Vec<Value>>,
        geojson: Option<&str>,
        signal: Option<&str>,
    ) -> NodeRef {
        DataFlowNode::new(
            Box::new(GeoJSONNode {
                fields: fields.map(|f| duplicate(&f)),
                geojson: geojson.map(str::to_string),
                signal: signal.map(str::to_string),
            }),
            parent,
            None,
        )
    }

    pub fn is_instance(node: &NodeRef) -> bool {
        with_node::<GeoJSONNode, _>(node, |_| ()).is_some()
    }

    /// Port of `GeoJSONNode.parseAll`.
    pub fn parse_all(parent: &NodeRef, model: &Model) -> NodeRef {
        if has_custom_projection(model) {
            return parent.clone();
        }

        let mut geo_json_counter = 0;
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

            if !pair[0].is_null() || !pair[1].is_null() {
                let signal = model.get_name(&format!("geojson_{geo_json_counter}"));
                geo_json_counter += 1;
                head = GeoJSONNode::new(Some(&head), Some(pair), None, Some(&signal));
            }
        }

        if model.channel_has_field(SHAPE) {
            if let Some(field_def) = model.typed_field_def(SHAPE) {
                if field_def.get("type").and_then(Value::as_str) == Some("geojson") {
                    let geojson = field_def
                        .get("field")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    let signal = model.get_name(&format!("geojson_{geo_json_counter}"));
                    head = GeoJSONNode::new(
                        Some(&head),
                        None,
                        geojson.as_deref(),
                        Some(&signal),
                    );
                }
            }
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

/// Upstream checks `model.component.projection && !projection.isFit`.  Since
/// the projection component is not materialised in this port, use the
/// equivalent condition on the specified projection: a projection that
/// specifies `scale` or `translate` is not fit to the data.
fn has_custom_projection(model: &Model) -> bool {
    if let Some(projection) = &model.component.projection {
        return !projection.is_fit();
    }
    if !model.has_projection() {
        return false;
    }
    if let Value::Object(o) = &model.unit.specified_projection {
        let has = |key: &str| {
            o.get(key)
                .map(|v| !v.is_null())
                .unwrap_or(false)
        };
        return has("scale") || has("translate");
    }
    false
}

impl NodeBehavior for GeoJSONNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        Value::String(format!(
            "GeoJSON {} {} {}",
            self.geojson.clone().unwrap_or_default(),
            self.signal.clone().unwrap_or_default(),
            hash_str(&hash(&self.fields.clone().map(Value::Array).unwrap_or(Value::Null)))
        ))
    }

    fn dependent_fields(&self) -> FieldSet {
        let mut out = FieldSet::new();
        if let Some(g) = &self.geojson {
            out.insert(g.clone());
        }
        if let Some(fields) = &self.fields {
            for f in fields.iter().filter_map(Value::as_str) {
                out.insert(f.to_string());
            }
        }
        out
    }

    fn produced_fields(&self) -> Option<FieldSet> {
        Some(FieldSet::new())
    }

    fn assemble(&self) -> Value {
        let mut out = Vec::new();
        if let Some(geojson) = &self.geojson {
            out.push(json!({
                "type": "filter",
                "expr": format!("isValid(datum[\"{geojson}\"])"),
            }));
        }
        let mut transform = Map::new();
        transform.insert("type".into(), json!("geojson"));
        if let Some(fields) = &self.fields {
            transform.insert("fields".into(), Value::Array(fields.clone()));
        }
        if let Some(geojson) = &self.geojson {
            transform.insert("geojson".into(), json!(geojson));
        }
        if let Some(signal) = &self.signal {
            transform.insert("signal".into(), json!(signal));
        }
        out.push(Value::Object(transform));
        Value::Array(out)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

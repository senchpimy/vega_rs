//! Port of `src/compile/data/source.ts`.

use std::any::Any;

use serde_json::{json, Map, Value};

use crate::util::omit;

use super::dataflow::{with_node, with_node_mut, DataFlowNode, FieldSet, NodeBehavior, NodeRef};

#[derive(Debug, Clone)]
pub struct SourceNode {
    pub data: Value,
    pub name: Option<String>,
    pub generator: bool,
}

// ---------------------------------------------------------------------------
// `src/data.ts` type guards
// ---------------------------------------------------------------------------

pub fn is_inline_data(data: &Value) -> bool {
    data.get("values").is_some()
}

pub fn is_url_data(data: &Value) -> bool {
    data.get("url").map(|v| !v.is_null()).unwrap_or(false)
}

pub fn is_named_data(data: &Value) -> bool {
    data.get("name").map(|v| !v.is_null()).unwrap_or(false)
}

pub fn is_sequence_generator(data: &Value) -> bool {
    data.get("sequence").map(|v| !v.is_null()).unwrap_or(false)
}

pub fn is_graticule_generator(data: &Value) -> bool {
    data.get("graticule").map(|v| !v.is_null()).unwrap_or(false)
}

pub fn is_sphere_generator(data: &Value) -> bool {
    data.get("sphere").map(|v| !v.is_null()).unwrap_or(false)
}

pub fn is_generator(data: &Value) -> bool {
    is_sequence_generator(data) || is_graticule_generator(data) || is_sphere_generator(data)
}

/// Port of `contains(['json', 'csv', 'tsv', 'dsv', 'topojson'], ext)`.
fn is_known_extension(ext: &str) -> bool {
    matches!(ext, "json" | "csv" | "tsv" | "dsv" | "topojson")
}

impl SourceNode {
    /// `new SourceNode(data)`.  Sources cannot have parents.
    pub fn new(data: Option<&Value>) -> NodeRef {
        let placeholder = json!({"name": "source"});
        let data = data.unwrap_or(&placeholder);

        let mut format = if !is_generator(data) {
            match data.get("format") {
                Some(Value::Object(o)) => Value::Object(omit(o, &["parse"])),
                _ => json!({}),
            }
        } else {
            json!({})
        };

        let mut source_data = json!({});
        if is_inline_data(data) {
            source_data = json!({"values": data.get("values").cloned().unwrap_or(Value::Null)});
        } else if is_url_data(data) {
            let url = data.get("url").cloned().unwrap_or(Value::Null);
            source_data = json!({"url": url});
            let has_type = format
                .get("type")
                .map(|v| !v.is_null() && !v.as_str().map(str::is_empty).unwrap_or(false))
                .unwrap_or(false);
            if !has_type {
                let url = data.get("url").and_then(Value::as_str).unwrap_or("");
                let ext = url.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
                let default_extension = if is_known_extension(ext) { ext } else { "json" };
                if let Some(o) = format.as_object_mut() {
                    o.insert("type".into(), json!(default_extension));
                }
            }
        } else if is_sphere_generator(data) {
            source_data = json!({"values": [{"type": "Sphere"}]});
        } else if is_named_data(data) || is_generator(data) {
            source_data = json!({});
        } else {
            // `Partial<VgData>` never assigned (data without values/url/name).
            source_data = json!({});
        }

        let generator = is_generator(data);
        let name = data
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string);

        let format_empty = format
            .as_object()
            .map(|o| o.is_empty())
            .unwrap_or(true);
        if !format_empty {
            if let Some(o) = source_data.as_object_mut() {
                o.insert("format".into(), format);
            }
        }

        DataFlowNode::new(
            Box::new(SourceNode {
                data: source_data,
                name,
                generator,
            }),
            None,
            None,
        )
    }

    pub fn has_name(node: &NodeRef) -> bool {
        with_node::<SourceNode, _>(node, |n| n.name.is_some()).unwrap_or(false)
    }

    pub fn data_name(node: &NodeRef) -> Option<String> {
        with_node::<SourceNode, _>(node, |n| n.name.clone()).unwrap_or(None)
    }

    pub fn set_data_name(node: &NodeRef, name: &str) {
        with_node_mut::<SourceNode, _>(node, |n| n.name = Some(name.to_string()));
    }

    pub fn is_generator_node(node: &NodeRef) -> bool {
        with_node::<SourceNode, _>(node, |n| n.generator).unwrap_or(false)
    }

    pub fn data(node: &NodeRef) -> Value {
        with_node::<SourceNode, _>(node, |n| n.data.clone()).unwrap_or(Value::Null)
    }

    pub fn is_source(node: &NodeRef) -> bool {
        with_node::<SourceNode, _>(node, |_| ()).is_some()
    }

    pub fn assemble_source(node: &NodeRef) -> Value {
        with_node::<SourceNode, _>(node, |n| {
            let mut result = Map::new();
            if let Some(name) = &n.name {
                result.insert("name".into(), json!(name));
            }
            if let Some(o) = n.data.as_object() {
                for (k, v) in o {
                    result.insert(k.clone(), v.clone());
                }
            }
            result.insert("transform".into(), json!([]));
            Value::Object(result)
        })
        .unwrap_or(Value::Null)
    }
}

impl NodeBehavior for SourceNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        // Upstream throws: "Cannot hash sources".
        Value::Null
    }

    fn dependent_fields(&self) -> FieldSet {
        FieldSet::new()
    }

    fn produced_fields(&self) -> Option<FieldSet> {
        None // we don't know what this source produces
    }

    fn assemble(&self) -> Value {
        let mut result = Map::new();
        if let Some(name) = &self.name {
            result.insert("name".into(), json!(name));
        }
        if let Some(o) = self.data.as_object() {
            for (k, v) in o {
                result.insert(k.clone(), v.clone());
            }
        }
        result.insert("transform".into(), json!([]));
        Value::Object(result)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

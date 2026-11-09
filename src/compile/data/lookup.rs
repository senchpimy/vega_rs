//! Port of `src/compile/data/lookup.ts` (node only; the model-driven
//! `make` constructor is not ported since it needs the full data component).

use std::any::Any;

use serde_json::{json, Map, Value};

use crate::log;
use crate::util::{duplicate, hash};

use super::dataflow::{
    DataFlowNode, FieldSet, NodeBehavior, NodeRef, OutputNode, OutputNodeType,
};
use super::index::DataComponent;
use super::parse::find_source;
use super::source::SourceNode;
use super::support::hash_str;
use crate::compile::model::Model;

#[derive(Debug, Clone)]
pub struct LookupNode {
    pub transform: Value,
    pub secondary: Option<String>,
}

impl LookupNode {
    pub fn new(parent: Option<&NodeRef>, transform: &Value, secondary: Option<&str>) -> NodeRef {
        DataFlowNode::new(
            Box::new(LookupNode {
                transform: duplicate(transform),
                secondary: secondary.map(str::to_string),
            }),
            parent,
            None,
        )
    }
}

impl LookupNode {
    /// Port of `LookupNode.make`.
    ///
    /// Registers the secondary source (creating it when it does not exist yet)
    /// and a `lookup_N` output node, then references that output node through
    /// the lookup's `secondary` name.  Requesting the output name through
    /// `OutputNode::get_source` marks the output as required, mirroring the
    /// reference counting of the TypeScript implementation.
    pub fn make(
        parent: Option<&NodeRef>,
        model: &Model,
        transform: &Value,
        counter: usize,
        component: &mut DataComponent,
    ) -> NodeRef {
        let from = transform.get("from").cloned().unwrap_or(Value::Null);
        let mut transform = transform.clone();
        let secondary: Option<String>;

        if let Some(data) = from.get("data").filter(|v| !v.is_null()) {
            let from_source = match find_source(data, &component.sources) {
                Some(source) => source,
                None => {
                    let source = SourceNode::new(Some(data));
                    component.sources.push(source.clone());
                    source
                }
            };

            let name = model.get_name(&format!("lookup_{counter}"));
            let output = OutputNode::new(Some(&from_source), &name, OutputNodeType::Lookup);
            if let Some(count) = component.output_node_ref_counts.get(&name) {
                if *count > 0 {
                    OutputNode::set_ref_count(&output, *count as u64);
                }
            }
            component.output_nodes.insert(name.clone(), output.clone());
            secondary = Some(OutputNode::get_source(&output));
        } else if let Some(param) = from.get("param").and_then(Value::as_str) {
            // Selection lookup: reference the materialized output node of the
            // selection.  The node belongs to the unit that declared the
            // selection, which may be a sibling (e.g. a layer child), so fall
            // back to any `lookup_<param>` registered in the shared component.
            let name = model.get_name(&format!("lookup_{param}"));
            let output = component.output_nodes.get(&name).cloned().or_else(|| {
                let suffix = format!("lookup_{param}");
                component
                    .output_nodes
                    .iter()
                    .find(|(key, _)| *key == &suffix || key.ends_with(&format!("_{suffix}")))
                    .map(|(_, node)| node.clone())
            });
            match output {
                Some(output) => secondary = Some(OutputNode::get_source(&output)),
                None => secondary = Some(name),
            }
            let mut map = serde_json::Map::new();
            map.insert("as".into(), json!(param));
            for (k, v) in transform.as_object().cloned().unwrap_or_default() {
                map.insert(k, v);
            }
            transform = Value::Object(map);
        } else {
            secondary = Some(format!("lookup_{counter}"));
        }

        LookupNode::new(parent, &transform, secondary.as_deref())
    }
}

fn array(value: &Value) -> Vec<Value> {
    match value {
        Value::Array(a) => a.clone(),
        Value::Null => Vec::new(),
        other => vec![other.clone()],
    }
}

impl NodeBehavior for LookupNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        let obj = json!({
            "transform": self.transform,
            "secondary": self.secondary,
        });
        Value::String(format!("Lookup {}", hash_str(&hash(&obj))))
    }

    fn dependent_fields(&self) -> FieldSet {
        let mut out = FieldSet::new();
        if let Some(lookup) = self.transform.get("lookup").and_then(Value::as_str) {
            out.insert(lookup.to_string());
        }
        out
    }

    fn produced_fields(&self) -> Option<FieldSet> {
        let mut out = FieldSet::new();
        if let Some(as_) = self.transform.get("as").filter(|v| !v.is_null()) {
            for v in array(as_) {
                if let Some(s) = v.as_str() {
                    out.insert(s.to_string());
                }
            }
        } else if let Some(fields) = self
            .transform
            .get("from")
            .and_then(|f| f.get("fields"))
            .and_then(Value::as_array)
        {
            for f in fields.iter().filter_map(Value::as_str) {
                out.insert(f.to_string());
            }
        }
        Some(out)
    }

    fn assemble(&self) -> Value {
        let mut result = Map::new();
        result.insert("type".into(), json!("lookup"));
        result.insert(
            "from".into(),
            self.secondary.clone().map(Value::String).unwrap_or(Value::Null),
        );
        result.insert(
            "key".into(),
            self.transform
                .get("from")
                .and_then(|f| f.get("key"))
                .cloned()
                .unwrap_or(Value::Null),
        );
        result.insert(
            "fields".into(),
            json!([self.transform.get("lookup").cloned().unwrap_or(Value::Null)]),
        );

        let from_fields = self
            .transform
            .get("from")
            .and_then(|f| f.get("fields"))
            .filter(|v| !v.is_null());

        if let Some(fields) = from_fields {
            result.insert("values".into(), fields.clone());
            if let Some(as_) = self.transform.get("as").filter(|v| !v.is_null()) {
                result.insert("as".into(), Value::Array(array(as_)));
            }
        } else {
            let mut as_name = self
                .transform
                .get("as")
                .and_then(Value::as_str)
                .map(str::to_string);
            if as_name.is_none() {
                log::warn(
                    "If \"from.fields\" is not specified, \"as\" has to be a string that specifies the key to be used for the data from the secondary source.",
                );
                as_name = Some("_lookup".to_string());
            }
            result.insert("as".into(), json!([as_name.unwrap()]));
        }

        if let Some(default) = self.transform.get("default").filter(|v| !v.is_null()) {
            result.insert("default".into(), default.clone());
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


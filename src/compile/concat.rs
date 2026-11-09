//! Port of `src/compile/concat.ts`.

use serde_json::Value;

use crate::log;

use super::buildmodel::build_model;
use super::model::{Model, SpecType};
use super::support;

impl Model {
    /// `new ConcatModel(spec, parent, parentGivenName, config)`
    pub fn new_concat(
        spec: &Value,
        parent: Option<&Model>,
        parent_given_name: &str,
        config: &Value,
    ) -> Model {
        let mut model = Model::new_common(
            spec,
            SpecType::Concat,
            parent,
            parent_given_name,
            config,
            None,
        );

        if model.component.resolve.get_scale("x").as_deref() == Some("shared")
            && spec
                .get("resolve")
                .and_then(|r| r.get("axis"))
                .and_then(|a| a.get("x"))
                .and_then(Value::as_str)
                == Some("shared")
            || spec
                .get("resolve")
                .and_then(|r| r.get("axis"))
                .and_then(|a| a.get("y"))
                .and_then(Value::as_str)
                == Some("shared")
        {
            log::warn(support::CONCAT_CANNOT_SHARE_AXIS);
        }

        let children_specs = get_children(spec);
        let mut children = Vec::new();
        for (i, child_spec) in children_specs.iter().enumerate() {
            let child_name = model.get_name(&format!("concat_{i}"));
            children.push(build_model(
                child_spec,
                Some(&model),
                &child_name,
                None,
                config,
            ));
        }
        model.children = children;
        model
    }
}

fn get_children(spec: &Value) -> Vec<Value> {
    if let Some(v) = spec.get("vconcat").and_then(Value::as_array) {
        v.clone()
    } else if let Some(h) = spec.get("hconcat").and_then(Value::as_array) {
        h.clone()
    } else {
        spec.get("concat")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    }
}

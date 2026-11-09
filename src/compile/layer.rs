//! Port of `src/compile/layer.ts`.

use serde_json::{Map, Value};

use super::buildmodel::build_model;
use super::model::{Model, SpecType};
use super::support;

impl Model {
    /// `new LayerModel(spec, parent, parentGivenName, parentGivenSize, config)`
    pub fn new_layer(
        spec: &Value,
        parent: Option<&Model>,
        parent_given_name: &str,
        parent_given_size: Option<&Value>,
        config: &Value,
    ) -> Model {
        let view = spec.get("view").cloned();
        let mut model = Model::new_common(
            spec,
            SpecType::Layer,
            parent,
            parent_given_name,
            config,
            view.map(|v| support::replace_expr_ref(&v)),
        );

        let mut layout_size = parent_given_size
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_else(Map::new);
        if let Some(width) = spec.get("width") {
            if crate::channeldef_support::truthy(width) {
                layout_size.insert("width".to_string(), width.clone());
            }
        }
        if let Some(height) = spec.get("height") {
            if crate::channeldef_support::truthy(height) {
                layout_size.insert("height".to_string(), height.clone());
            }
        }
        let layout_size_value = Value::Object(layout_size);

        let layers = spec
            .get("layer")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut children = Vec::new();
        for (i, layer) in layers.iter().enumerate() {
            let child_name = model.get_name(&format!("layer_{i}"));
            children.push(build_model(
                layer,
                Some(&model),
                &child_name,
                Some(&layout_size_value),
                config,
            ));
        }
        model.children = children;
        model
    }
}

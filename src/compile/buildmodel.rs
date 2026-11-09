//! Port of `src/compile/buildmodel.ts`.

use serde_json::Value;

use crate::log;

use super::model::Model;
use super::support::{self, is_any_concat_spec};

/// Port of `buildModel`.
pub fn build_model(
    spec: &Value,
    parent: Option<&Model>,
    parent_given_name: &str,
    unit_size: Option<&Value>,
    config: &Value,
) -> Model {
    if support::is_facet_spec(spec) {
        Model::new_facet(spec, parent, parent_given_name, config)
    } else if support::is_layer_spec(spec) {
        Model::new_layer(spec, parent, parent_given_name, unit_size, config)
    } else if support::is_unit_spec(spec) {
        Model::new_unit(spec, parent, parent_given_name, unit_size, config)
    } else if is_any_concat_spec(spec) {
        Model::new_concat(spec, parent, parent_given_name, config)
    } else {
        panic!("{}", log::message::invalid_spec(spec))
    }
}

/// Convenience helper mirroring the test util `parseModel`.
pub fn parse_model(spec: &Value, config: &Value) -> Model {
    let normalized = support::normalize(spec, config);
    build_model(&normalized, None, "", None, config)
}

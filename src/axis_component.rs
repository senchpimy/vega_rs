//! Port of `src/compile/axis/component.ts` (runtime pieces).

use serde_json::{Map, Value};

use crate::axis::COMMON_AXIS_PROPERTIES;

/// `AXIS_COMPONENT_PROPERTIES`: disable, gridScale, scale, then common props,
/// then labelExpr and encode.
pub fn axis_component_properties() -> Vec<String> {
    let mut props = vec!["disable".to_string(), "gridScale".to_string(), "scale".to_string()];
    props.extend(COMMON_AXIS_PROPERTIES.iter().map(|s| s.to_string()));
    props.push("labelExpr".to_string());
    props.push("encode".to_string());
    props
}

#[derive(Debug, Clone, Default)]
pub struct AxisComponent {
    pub explicit: Map<String, Value>,
    pub implicit: Map<String, Value>,
    pub main_extracted: bool,
}

impl AxisComponent {
    pub fn new(explicit: Map<String, Value>, implicit: Map<String, Value>, main_extracted: bool) -> Self {
        Self {
            explicit,
            implicit,
            main_extracted,
        }
    }

    pub fn get(&self, prop: &str) -> Option<&Value> {
        self.explicit.get(prop).or_else(|| self.implicit.get(prop))
    }

    pub fn has_axis_part(&self, part: &str) -> bool {
        if part == "axis" {
            return true;
        }
        if part == "grid" || part == "title" {
            return self.get(part).map(|v| !v.is_null()).unwrap_or(false);
        }
        !matches!(self.get(part), Some(Value::Bool(false)) | Some(Value::Null))
    }

    pub fn has_orient_signal_ref(&self) -> bool {
        self.explicit
            .get("orient")
            .map(|v| v.get("signal").is_some())
            .unwrap_or(false)
    }
}

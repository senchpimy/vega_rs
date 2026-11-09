//! Port of `src/compositemark/base.ts`.

use serde_json::Value;

use crate::mark;
use crate::normalize_support::{get, is_unit_spec};

/// A composite mark normalizer is identified by the mark type it handles.
#[derive(Debug, Clone)]
pub struct CompositeMarkNormalizer {
    pub name: String,
}

impl CompositeMarkNormalizer {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    /// Port of `CompositeMarkNormalizer.hasMatchingType`.
    pub fn has_matching_type(&self, spec: &Value) -> bool {
        is_unit_spec(spec)
            && mark::get_mark_type(get(spec, "mark").unwrap_or(&Value::Null))
                == Some(self.name.as_str())
    }
}

/// Port of `getMarkType(spec.mark) === name` for a unit spec.
pub fn is_composite_unit_spec(spec: &Value, name: &str) -> bool {
    CompositeMarkNormalizer::new(name).has_matching_type(spec)
}

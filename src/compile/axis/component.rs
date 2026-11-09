//! Port of `src/compile/axis/component.ts`.
//!
//! The upstream `AxisComponent` extends the generic `Split` helper.  Here the
//! split storage is inlined (explicit / implicit maps) so the component can be
//! serialized into `model.component.axes` while staying easy to inspect.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::axis_component::axis_component_properties;
use crate::compile::split::{Explicit, Split};
use crate::util::{deep_equal, has_property};

/// `AxisComponentIndex`
pub type AxisComponentIndex = BTreeMap<String, Vec<AxisComponent>>;

/// `AxisComponentProps` — all values are kept as JSON.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AxisComponent {
    pub explicit: BTreeMap<String, Value>,
    pub implicit: BTreeMap<String, Value>,
    pub main_extracted: bool,
}

fn is_false_or_null(v: &Value) -> bool {
    matches!(v, Value::Bool(false) | Value::Null)
}

impl AxisComponent {
    /// `new AxisComponent(explicit, implicit, mainExtracted)`
    pub fn new(
        explicit: BTreeMap<String, Value>,
        implicit: BTreeMap<String, Value>,
        main_extracted: bool,
    ) -> Self {
        Self {
            explicit,
            implicit,
            main_extracted,
        }
    }

    pub fn from_json_map(
        explicit: Map<String, Value>,
        implicit: Map<String, Value>,
        main_extracted: bool,
    ) -> Self {
        Self {
            explicit: explicit.into_iter().collect(),
            implicit: implicit.into_iter().collect(),
            main_extracted,
        }
    }

    pub fn clone_component(&self) -> Self {
        self.clone()
    }

    /// `combine()`: explicit properties come first, then implicit ones.
    pub fn combine(&self) -> BTreeMap<String, Value> {
        let split = Split::new(self.explicit.clone(), self.implicit.clone());
        split.combine()
    }

    /// `get(key)`: explicit has higher precedence.
    pub fn get(&self, key: &str) -> Option<Value> {
        self.explicit
            .get(key)
            .or_else(|| self.implicit.get(key))
            .cloned()
    }

    pub fn get_ref(&self, key: &str) -> Option<&Value> {
        self.explicit.get(key).or_else(|| self.implicit.get(key))
    }

    /// `getWithExplicit(key)`.
    pub fn get_with_explicit(&self, key: &str) -> Explicit {
        if let Some(v) = self.explicit.get(key) {
            Explicit {
                explicit: true,
                value: Some(v.clone()),
            }
        } else if let Some(v) = self.implicit.get(key) {
            Explicit {
                explicit: false,
                value: Some(v.clone()),
            }
        } else {
            Explicit::undefined()
        }
    }

    /// `setWithExplicit(key, value)`
    pub fn set_with_explicit(&mut self, key: &str, value: Explicit) {
        if let Some(v) = value.value {
            self.set(key, v, value.explicit);
        }
    }

    /// `set(key, value, explicit)`
    pub fn set(&mut self, key: &str, value: Value, explicit: bool) {
        if explicit {
            self.implicit.remove(key);
            self.explicit.insert(key.to_string(), value);
        } else {
            self.explicit.remove(key);
            self.implicit.insert(key.to_string(), value);
        }
    }

    /// `hasAxisPart(part)`
    pub fn has_axis_part(&self, part: &str) -> bool {
        if part == "axis" {
            return true;
        }
        if part == "grid" || part == "title" {
            return self.get(part).map(|v| !v.is_null()).unwrap_or(false);
        }
        !self
            .get(part)
            .map(|v| is_false_or_null(&v))
            .unwrap_or(false)
    }

    /// `hasOrientSignalRef()`
    pub fn has_orient_signal_ref(&self) -> bool {
        self.explicit
            .get("orient")
            .map(|v| has_property(v, "signal"))
            .unwrap_or(false)
    }

    /// Serialize into the shape stored in `model.component.axes`.
    pub fn to_value(&self) -> Value {
        let explicit: Map<String, Value> = self
            .explicit
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let implicit: Map<String, Value> = self
            .implicit
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        serde_json::json!({
            "explicit": Value::Object(explicit),
            "implicit": Value::Object(implicit),
            "mainExtracted": self.main_extracted,
        })
    }

    /// Deserialize from the shape stored in `model.component.axes`.
    pub fn from_value(value: &Value) -> Self {
        let explicit = value
            .get("explicit")
            .and_then(Value::as_object)
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        let implicit = value
            .get("implicit")
            .and_then(Value::as_object)
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        AxisComponent {
            explicit,
            implicit,
            main_extracted: value
                .get("mainExtracted")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }
    }

    /// Compare two components (used by tests).
    pub fn equals(&self, other: &AxisComponent) -> bool {
        self.main_extracted == other.main_extracted
            && self.explicit.len() == other.explicit.len()
            && self.implicit.len() == other.implicit.len()
            && self
                .explicit
                .iter()
                .all(|(k, v)| other.explicit.get(k).map(|o| deep_equal(v, o)).unwrap_or(false))
            && self
                .implicit
                .iter()
                .all(|(k, v)| other.implicit.get(k).map(|o| deep_equal(v, o)).unwrap_or(false))
    }
}

/// `AXIS_COMPONENT_PROPERTIES`
pub fn axis_component_properties_list() -> Vec<String> {
    axis_component_properties()
}

/// Convert an [`AxisComponentIndex`] into the JSON stored in
/// `model.component.axes`.
pub fn axis_index_to_value(index: &AxisComponentIndex) -> Value {
    let mut out = Map::new();
    for (channel, components) in index {
        out.insert(
            channel.clone(),
            Value::Array(components.iter().map(AxisComponent::to_value).collect()),
        );
    }
    Value::Object(out)
}

/// Convert `model.component.axes` back into an [`AxisComponentIndex`].
pub fn axis_index_from_value(value: &Value) -> AxisComponentIndex {
    let mut index = AxisComponentIndex::new();
    if let Some(map) = value.as_object() {
        for (channel, components) in map {
            if let Some(array) = components.as_array() {
                index.insert(
                    channel.clone(),
                    array.iter().map(AxisComponent::from_value).collect(),
                );
            }
        }
    }
    index
}

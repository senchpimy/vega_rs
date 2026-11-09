//! Port of `src/compile/scale/component.ts`.

use std::collections::BTreeMap;

use indexmap::IndexMap;

use serde_json::Value;

use super::super::split::{Explicit, Split};
use crate::scale::ScaleType;

/// Port of `ScaleComponentProps`; stored as a [`Split`] of JSON values.
pub type ScaleComponentProps = Split;

/// Port of `ScaleComponent`.
#[derive(Debug, Clone, PartialEq)]
pub struct ScaleComponent {
    /// Explicit properties.
    pub explicit: BTreeMap<String, Value>,
    /// Implicit properties.
    pub implicit: BTreeMap<String, Value>,
    pub merged: bool,
}

impl ScaleComponent {
    /// `new ScaleComponent(name, typeWithExplicit)`
    pub fn new(name: &str, type_with_explicit: Explicit) -> Self {
        let mut component = ScaleComponent {
            explicit: BTreeMap::new(),
            implicit: BTreeMap::new(),
            merged: false,
        };
        component.implicit.insert("name".to_string(), Value::String(name.to_string()));
        component.set_with_explicit("type", type_with_explicit);
        component
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        self.get_ref(key).cloned()
    }

    /// Borrowing variant of [`ScaleComponent::get`].
    pub fn get_ref(&self, key: &str) -> Option<&Value> {
        self.explicit
            .get(key)
            .or_else(|| self.implicit.get(key))
    }

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

    pub fn set(&mut self, key: &str, value: Value, explicit: bool) {
        if explicit {
            self.implicit.remove(key);
            self.explicit.insert(key.to_string(), value);
        } else {
            self.explicit.remove(key);
            self.implicit.insert(key.to_string(), value);
        }
    }

    pub fn set_with_explicit(&mut self, key: &str, value: Explicit) {
        if let Some(v) = value.value {
            self.set(key, v, value.explicit);
        }
    }

    pub fn copy_key_from_object(&mut self, key: &str, object: &Value) {
        if let Some(v) = object.get(key) {
            if !v.is_null() {
                self.set(key, v.clone(), true);
            }
        }
    }

    pub fn combine(&self) -> BTreeMap<String, Value> {
        let mut out = self.explicit.clone();
        for (k, v) in &self.implicit {
            out.insert(k.clone(), v.clone());
        }
        out
    }

    pub fn get_type(&self) -> Option<ScaleType> {
        self.get_ref("type")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
    }

    /// Port of `domainHasZero`.
    pub fn domain_has_zero(&self) -> &'static str {
        use crate::scale::ScaleType::*;
        let scale_type = match self.get_type() {
            Some(t) => t,
            None => return "maybe",
        };
        if matches!(scale_type, Log | Time | Utc) {
            return "definitely-not";
        }

        let scale_zero = self.get("zero");
        if scale_zero.as_ref().and_then(Value::as_bool) == Some(true)
            || (scale_zero.is_none() && matches!(scale_type, Linear | Sqrt | Pow))
        {
            return "definitely";
        }

        let domains = self.get("domains").unwrap_or(Value::Array(Vec::new()));
        let domains = domains.as_array().cloned().unwrap_or_default();
        if !domains.is_empty() {
            let mut has_zero = false;
            let mut without_zero = false;
            let mut based_on_field = false;
            for d in &domains {
                if let Some(arr) = d.as_array() {
                    let first = arr.first();
                    let last = arr.last();
                    if let (Some(Value::Number(a)), Some(Value::Number(b))) = (first, last) {
                        let a = a.as_f64().unwrap_or(0.0);
                        let b = b.as_f64().unwrap_or(0.0);
                        if a <= 0.0 && b >= 0.0 {
                            has_zero = true;
                        } else {
                            without_zero = true;
                        }
                        continue;
                    }
                }
                based_on_field = true;
            }
            if has_zero {
                return "definitely";
            } else if without_zero && !based_on_field {
                return "definitely-not";
            }
        }
        "maybe"
    }
}

/// Port of `ScaleComponentIndex`.
pub type ScaleComponentIndex = IndexMap<String, ScaleComponent>;

/// Port of `ScaleIndex` (specified scales).
pub type ScaleIndex = BTreeMap<String, Value>;

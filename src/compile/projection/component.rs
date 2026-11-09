//! Port of `src/compile/projection/component.ts`.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::compile::split::Explicit;

/// Port of `ProjectionComponent`.
#[derive(Debug, Clone, Default)]
pub struct ProjectionComponent {
    pub explicit: BTreeMap<String, Value>,
    pub implicit: BTreeMap<String, Value>,
    pub size: Option<Vec<Value>>,
    pub data: Option<Vec<Value>>,
    pub merged: bool,
}

impl ProjectionComponent {
    pub fn new(
        name: &str,
        specified_projection: &Value,
        size: Option<Vec<Value>>,
        data: Option<Vec<Value>>,
    ) -> Self {
        let mut explicit = BTreeMap::new();
        if let Some(map) = specified_projection.as_object() {
            for (k, v) in map {
                if !v.is_null() {
                    explicit.insert(k.clone(), v.clone());
                }
            }
        }
        let mut implicit = BTreeMap::new();
        implicit.insert("name".to_string(), Value::String(name.to_string()));
        ProjectionComponent {
            explicit,
            implicit,
            size,
            data,
            merged: false,
        }
    }

    pub fn is_fit(&self) -> bool {
        self.data.is_some()
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        self.explicit
            .get(key)
            .or_else(|| self.implicit.get(key))
            .cloned()
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

    /// `Split.combine()`
    pub fn combine(&self) -> BTreeMap<String, Value> {
        let mut out = self.explicit.clone();
        for (k, v) in &self.implicit {
            out.insert(k.clone(), v.clone());
        }
        out
    }
}

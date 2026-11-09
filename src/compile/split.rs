//! Port of `src/compile/split.ts`.
//!
//! `Split` stores properties that were explicitly specified by the user apart
//! from the implicit defaults determined by the compiler.  The TypeScript
//! version is generic; here property values are [`serde_json::Value`].

use std::collections::BTreeMap;

use serde_json::Value;

use crate::log;
use crate::util::deep_equal;

use super::support;

/// Port of the `Explicit<T>` interface.
#[derive(Debug, Clone, PartialEq)]
pub struct Explicit {
    pub explicit: bool,
    pub value: Option<Value>,
}

impl Explicit {
    pub fn undefined() -> Self {
        Explicit {
            explicit: false,
            value: None,
        }
    }

    pub fn value(&self) -> Option<&Value> {
        self.value.as_ref()
    }
}

/// Port of `makeExplicit`.
pub fn make_explicit(value: Value) -> Explicit {
    Explicit {
        explicit: true,
        value: Some(value),
    }
}

/// Port of `makeImplicit`.
pub fn make_implicit(value: Value) -> Explicit {
    Explicit {
        explicit: false,
        value: Some(value),
    }
}

/// Port of `Split<T>`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Split {
    pub explicit: BTreeMap<String, Value>,
    pub implicit: BTreeMap<String, Value>,
}

impl Split {
    pub fn new(
        explicit: BTreeMap<String, Value>,
        implicit: BTreeMap<String, Value>,
    ) -> Self {
        Self { explicit, implicit }
    }

    pub fn clone_split(&self) -> Self {
        self.clone()
    }

    pub fn combine(&self) -> BTreeMap<String, Value> {
        let mut out = self.explicit.clone();
        for (k, v) in &self.implicit {
            out.insert(k.clone(), v.clone());
        }
        out
    }

    /// Port of `get`: explicit has higher precedence.
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

    pub fn set_with_explicit(&mut self, key: &str, value: Explicit) {
        if let Some(v) = value.value {
            self.set(key, v, value.explicit);
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

    pub fn copy_key_from_split(&mut self, key: &str, other: &Split) {
        if let Some(v) = other.explicit.get(key) {
            self.set(key, v.clone(), true);
        } else if let Some(v) = other.implicit.get(key) {
            self.set(key, v.clone(), false);
        }
    }

    pub fn copy_key_from_object(&mut self, key: &str, object: &Value) {
        if let Some(v) = object.get(key) {
            if !v.is_null() {
                self.set(key, v.clone(), true);
            }
        }
    }

    pub fn copy_all(&mut self, other: &Split) {
        for key in other.combine().keys() {
            let value = other.get_with_explicit(key);
            self.set_with_explicit(key, value);
        }
    }
}

/// Port of `tieBreakByComparing` for property values.
pub fn tie_break_by_comparing<F>(
    compare: F,
) -> impl Fn(&Explicit, &Explicit, &str, &str) -> Explicit
where
    F: Fn(&Value, &Value) -> i64,
{
    move |v1, v2, property, property_of| {
        let diff = match (v1.value(), v2.value()) {
            (Some(a), Some(b)) => compare(a, b),
            _ => 0,
        };
        if diff > 0 {
            v1.clone()
        } else if diff < 0 {
            v2.clone()
        } else {
            default_tie_breaker(v1, v2, property, property_of)
        }
    }
}

/// Port of `defaultTieBreaker`.
pub fn default_tie_breaker(
    v1: &Explicit,
    v2: &Explicit,
    property: &str,
    property_of: &str,
) -> Explicit {
    if v1.explicit && v2.explicit {
        log::warn(support::merge_conflicting_property(
            property,
            property_of,
            v1.value.as_ref().unwrap_or(&Value::Null),
            v2.value.as_ref().unwrap_or(&Value::Null),
        ));
    }
    v1.clone()
}

/// Port of `mergeValuesWithExplicit`.
pub fn merge_values_with_explicit<F>(
    v1: Option<Explicit>,
    v2: Explicit,
    property: &str,
    property_of: &str,
    tie_breaker: F,
) -> Explicit
where
    F: Fn(&Explicit, &Explicit, &str, &str) -> Explicit,
{
    let v1 = match v1 {
        Some(v) if v.value.is_some() => v,
        _ => return v2,
    };

    if v1.explicit && !v2.explicit {
        return v1;
    } else if v2.explicit && !v1.explicit {
        return v2;
    } else if let (Some(a), Some(b)) = (&v1.value, &v2.value) {
        if deep_equal(a, b) {
            return v1;
        }
    }

    tie_breaker(&v1, &v2, property, property_of)
}

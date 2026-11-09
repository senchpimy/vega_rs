//! Port of `src/normalize/base.ts`.
//!
//! The TypeScript interfaces are represented by [`NormalizerParams`]. Shared
//! mutable state (`selections`, `emptySelections`, `selectionPredicates`) is
//! kept in `Rc<RefCell<..>>` cells, mirroring the object mutation performed by
//! the upstream normalizers.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RepeaterValue {
    pub row: Option<String>,
    pub column: Option<String>,
    pub repeat: Option<String>,
    pub layer: Option<String>,
}

impl RepeaterValue {
    pub fn has(&self, key: &str) -> bool {
        match key {
            "row" => self.row.is_some(),
            "column" => self.column.is_some(),
            "repeat" => self.repeat.is_some(),
            "layer" => self.layer.is_some(),
            _ => false,
        }
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        match key {
            "row" => self.row.as_ref(),
            "column" => self.column.as_ref(),
            "repeat" => self.repeat.as_ref(),
            "layer" => self.layer.as_ref(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NormalizerParams {
    pub config: Rc<Value>,
    pub parent_encoding: Option<Value>,
    pub parent_projection: Option<Value>,
    pub repeater: Option<RepeaterValue>,
    pub repeater_prefix: Option<String>,
    pub selections: Rc<RefCell<Vec<Value>>>,
    pub empty_selections: Rc<RefCell<BTreeMap<String, Value>>>,
    pub selection_predicates: Rc<RefCell<BTreeMap<String, Value>>>,
    pub path: Vec<Value>,
}

impl Default for NormalizerParams {
    fn default() -> Self {
        Self {
            config: Rc::new(Value::Object(Default::default())),
            parent_encoding: None,
            parent_projection: None,
            repeater: None,
            repeater_prefix: None,
            selections: Rc::new(RefCell::new(Vec::new())),
            empty_selections: Rc::new(RefCell::new(BTreeMap::new())),
            selection_predicates: Rc::new(RefCell::new(BTreeMap::new())),
            path: Vec::new(),
        }
    }
}

impl NormalizerParams {
    pub fn with_config(config: impl Into<Rc<Value>>) -> Self {
        Self {
            config: config.into(),
            ..Default::default()
        }
    }

    pub fn with_config_value(config: Value) -> Self {
        Self::with_config(Rc::new(config))
    }

    /// Clone the parameter object, sharing the mutable selection state.
    pub fn fork(&self) -> Self {
        self.clone()
    }
}

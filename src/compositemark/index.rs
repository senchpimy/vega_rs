//! Port of `src/compositemark/index.ts`.
//!
//! The registry mirrors upstream's `compositeMarkRegistry`. Normalization in
//! this crate dispatches directly, but the registry is kept for parity and for
//! callers that need the list of composite marks.

use serde_json::{Map, Value};

use crate::compositemark::boxplot::{BOXPLOT, BOXPLOT_PARTS};
use crate::compositemark::errorband::{ERRORBAND, ERRORBAND_PARTS};
use crate::compositemark::errorbar::{ERRORBAR, ERRORBAR_PARTS};

#[derive(Debug, Clone)]
pub struct RegistryEntry {
    pub mark: String,
    pub parts: Vec<String>,
}

/// Port of `getAllCompositeMarks`.
pub fn get_all_composite_marks() -> Vec<String> {
    vec![
        BOXPLOT.to_string(),
        ERRORBAR.to_string(),
        ERRORBAND.to_string(),
    ]
}

/// Port of the registry's `parts` lookup.
pub fn get_composite_mark_parts(mark: &str) -> Option<Vec<String>> {
    match mark {
        BOXPLOT => Some(BOXPLOT_PARTS.iter().map(|p| p.to_string()).collect()),
        ERRORBAR => Some(ERRORBAR_PARTS.iter().map(|p| p.to_string()).collect()),
        ERRORBAND => Some(ERRORBAND_PARTS.iter().map(|p| p.to_string()).collect()),
        _ => None,
    }
}

/// Port of `add`/`remove` (a small dynamic registry for parity with upstream).
#[derive(Debug, Default)]
pub struct CompositeMarkRegistry {
    entries: Map<String, Value>,
}

impl CompositeMarkRegistry {
    pub fn new() -> Self {
        let mut entries = Map::new();
        for mark in get_all_composite_marks() {
            let parts = get_composite_mark_parts(&mark).unwrap();
            entries.insert(
                mark.clone(),
                serde_json::json!({"normalizer": {"name": mark}, "parts": parts}),
            );
        }
        Self { entries }
    }

    pub fn add(&mut self, mark: &str, parts: &[&str]) {
        self.entries.insert(
            mark.to_string(),
            serde_json::json!({"normalizer": {"name": mark}, "parts": parts}),
        );
    }

    pub fn remove(&mut self, mark: &str) {
        self.entries.remove(mark);
    }

    pub fn get_all(&self) -> Vec<String> {
        self.entries.keys().cloned().collect()
    }
}

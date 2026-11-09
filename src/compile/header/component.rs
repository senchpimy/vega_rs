//! Port of `src/compile/header/component.ts`.

use serde_json::{json, Map, Value};

/// `HeaderChannel`
pub const HEADER_CHANNELS: [&str; 2] = ["row", "column"];

/// `HeaderType`
pub const HEADER_TYPES: [&str; 2] = ["header", "footer"];

/// Convert an [`HeaderComponent`] JSON value into the shape used by the rest of
/// the compiler.
pub fn make_header_component(labels: bool, size_signal: Option<Value>) -> Value {
    let mut out = Map::new();
    out.insert("labels".to_string(), json!(labels));
    if let Some(size_signal) = size_signal {
        out.insert("sizeSignal".to_string(), size_signal);
    }
    out.insert("axes".to_string(), Value::Array(Vec::new()));
    Value::Object(out)
}

//! Port of `src/selection.ts` (runtime helpers and constants).
//!
//! The TypeScript interfaces are represented by [`serde_json::Value`]; only the
//! runtime helpers and the default selection configuration are ported here.

use serde_json::{json, Value};

/// The internal field name used to identify data tuples.
pub const SELECTION_ID: &str = "_vgsid_";

/// Default selection config. Mirrors `defaultConfig` in `src/selection.ts`.
pub fn default_config() -> Value {
    json!({
        "point": {
            "on": "click",
            "fields": [SELECTION_ID],
            "toggle": "event.shiftKey",
            "resolve": "global",
            "clear": "dblclick",
        },
        "interval": {
            "on": "[pointerdown, window:pointerup] > window:pointermove!",
            "encodings": ["x", "y"],
            "translate": "[pointerdown, window:pointerup] > window:pointermove!",
            "zoom": "wheel!",
            "mark": {"fill": "#333", "fillOpacity": 0.125, "stroke": "white"},
            "resolve": "global",
            "clear": "dblclick",
        },
    })
}

pub fn is_legend_binding(bind: &Value) -> bool {
    bind == &Value::String("legend".to_string())
        || bind
            .as_object()
            .map(|o| o.get("legend").map(|v| !v.is_null()).unwrap_or(false))
            .unwrap_or(false)
}

pub fn is_legend_stream_binding(bind: &Value) -> bool {
    is_legend_binding(bind) && bind.is_object()
}

pub fn is_selection_parameter(param: &Value) -> bool {
    param
        .as_object()
        .map(|o| o.get("select").map(|v| !v.is_null()).unwrap_or(false))
        .unwrap_or(false)
}

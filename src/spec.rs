//! Port of the runtime helpers in `src/spec/base.ts` and `src/spec/toplevel.ts`.

use serde_json::{Map, Value};

use crate::util::{has_property, keys};

pub const DEFAULT_SPACING: f64 = 20.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutosizeType {
    Pad,
    None,
    Fit,
    FitX,
    FitY,
}

impl AutosizeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AutosizeType::Pad => "pad",
            AutosizeType::None => "none",
            AutosizeType::Fit => "fit",
            AutosizeType::FitX => "fit-x",
            AutosizeType::FitY => "fit-y",
        }
    }
}

pub fn is_fit_type(auto_size_type: &str) -> bool {
    matches!(auto_size_type, "fit" | "fit-x" | "fit-y")
}

/// `getFitType(sizeType)`
pub fn get_fit_type(size_type: Option<&str>) -> String {
    match size_type {
        Some("width") => "fit-x".to_string(),
        Some("height") => "fit-y".to_string(),
        _ => "fit".to_string(),
    }
}

pub fn is_step(size: &Value) -> bool {
    has_property(size, "step")
}

pub fn get_step_for(step: &Value, offset_is_discrete: bool) -> String {
    if offset_is_discrete {
        step.get("for")
            .and_then(|v| v.as_str())
            .unwrap_or("offset")
            .to_string()
    } else {
        "position".to_string()
    }
}

pub fn is_frame_mixins(o: &Value) -> bool {
    has_property(o, "view") || has_property(o, "width") || has_property(o, "height")
}

const TOP_LEVEL_PROPERTIES: [&str; 2] = ["background", "padding"];

/// `extractTopLevelProperties(t, includeParams)`
pub fn extract_top_level_properties(t: &Value, include_params: bool) -> Map<String, Value> {
    let mut o = Map::new();
    for p in TOP_LEVEL_PROPERTIES {
        if let Some(v) = t.get(p) {
            if !v.is_null() {
                o.insert(p.to_string(), signal_ref_or_value(v));
            }
        }
    }
    if include_params {
        if let Some(params) = t.get("params") {
            o.insert("params".to_string(), params.clone());
        }
    }
    o
}

/// `signalRefOrValue` from `compile/common.ts` (only the ExprRef case matters here).
pub fn signal_ref_or_value(v: &Value) -> Value {
    if has_property(v, "expr") {
        serde_json::json!({"signal": v["expr"].clone()})
    } else {
        v.clone()
    }
}

const COMPOSITION_LAYOUT_PROPERTIES: [&str; 5] = ["align", "bounds", "center", "columns", "spacing"];

/// `extractCompositionLayout(...)`
pub fn extract_composition_layout(
    spec: &Value,
    spec_type: &str,
    config: &Value,
) -> Map<String, Value> {
    let composition_config = config.get(spec_type).cloned().unwrap_or(Value::Null);
    let mut layout = Map::new();

    let spacing_config = composition_config.get("spacing").cloned();
    let columns = composition_config.get("columns").cloned();

    if let Some(spacing) = spacing_config.clone() {
        if !spacing.is_null() {
            layout.insert("spacing".to_string(), spacing);
        }
    }

    if let Some(columns) = columns {
        if !columns.is_null() {
            // Matches the upstream conditions: facet without mapping or concat.
            if (spec.get("facet").is_some() && spec["facet"].is_object() && !is_facet_mapping(&spec["facet"]))
                || spec.get("concat").is_some()
                || spec.get("hconcat").is_some()
                || spec.get("vconcat").is_some()
            {
                layout.insert("columns".to_string(), columns);
            }
        }
    }

    if spec.get("vconcat").is_some() {
        layout.insert("columns".to_string(), Value::from(1));
    }

    for prop in COMPOSITION_LAYOUT_PROPERTIES {
        if let Some(value) = spec.get(prop) {
            if value.is_null() {
                continue;
            }
            if prop == "spacing" {
                let spacing = if value.is_number() {
                    value.clone()
                } else {
                    let row = value
                        .get("row")
                        .cloned()
                        .or(spacing_config.clone())
                        .unwrap_or(Value::Null);
                    let column = value
                        .get("column")
                        .cloned()
                        .or(spacing_config.clone())
                        .unwrap_or(Value::Null);
                    serde_json::json!({"row": row, "column": column})
                };
                layout.insert(prop.to_string(), spacing);
            } else {
                layout.insert(prop.to_string(), value.clone());
            }
        }
    }

    layout
}

fn is_facet_mapping(facet: &Value) -> bool {
    ["row", "column", "facet"].iter().any(|k| has_property(facet, k))
}

#[allow(dead_code)]
fn _keys(v: &Value) -> Vec<String> {
    keys(v)
}

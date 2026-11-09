//! Port of `src/normalize/index.ts`.

pub mod base;
pub mod core;
pub mod pathoverlay;
pub mod repeater;
pub mod ruleforrangedline;
pub mod selectioncompat;
pub mod toplevelselection;

pub use base::NormalizerParams;

use serde_json::{Map, Value};

use crate::normalize_support::*;

/// Decompose extended unit specs into composition of pure unit specs and push
/// top-level selection definitions down to unit specs.
pub fn normalize(spec: &Value, config: Option<&Value>) -> Result<Value, String> {
    let config = match config {
        Some(config) => config.clone(),
        None => init_config(get(spec, "config")),
    };

    let normalized_spec = normalize_generic_spec(spec, &config)?;

    let autosize = normalize_auto_size(&normalized_spec, spec, &config);

    let mut out = normalized_spec.as_object().cloned().unwrap_or_default();
    if let Some(autosize) = autosize {
        out.insert("autosize".to_string(), autosize);
    }
    Ok(Value::Object(out))
}

fn normalize_generic_spec(spec: &Value, config: &Value) -> Result<Value, String> {
    let norm_params = NormalizerParams::with_config_value(config.clone());
    let mut spec = selectioncompat::map(spec, &norm_params)?;
    selectioncompat::finalize_predicates(&mut spec, &norm_params.empty_selections.borrow());
    let spec = core::map(&spec, &norm_params)?;
    Ok(toplevelselection::map(&spec, &norm_params))
}

fn normalize_auto_size_value(autosize: Option<&Value>) -> Value {
    match autosize {
        Some(Value::String(s)) => serde_json::json!({"type": s}),
        Some(other) if other.is_object() => other.clone(),
        _ => empty_object(),
    }
}

/// Normalize autosize and deal with width or height == "container".
pub fn normalize_auto_size(
    spec: &Value,
    size_info: &Value,
    config: &Value,
) -> Option<Value> {
    let width = get(size_info, "width").cloned();
    let height = get(size_info, "height").cloned();

    let is_fit_compatible = is_unit_spec(spec) || is_layer_spec(spec);
    let mut autosize_default = Map::new();

    let width_is_container = width.as_ref().and_then(|w| w.as_str()) == Some("container");
    let height_is_container = height.as_ref().and_then(|h| h.as_str()) == Some("container");

    if !is_fit_compatible {
        if width_is_container {
            crate::log::warn("Width \"container\" only works for single views and layered views.");
        }
        if height_is_container {
            crate::log::warn("Height \"container\" only works for single views and layered views.");
        }
    } else if width_is_container && height_is_container {
        autosize_default.insert("type".to_string(), Value::String("fit".to_string()));
        autosize_default.insert("contains".to_string(), Value::String("padding".to_string()));
    } else if width_is_container {
        autosize_default.insert("type".to_string(), Value::String("fit-x".to_string()));
        autosize_default.insert("contains".to_string(), Value::String("padding".to_string()));
    } else if height_is_container {
        autosize_default.insert("type".to_string(), Value::String("fit-y".to_string()));
        autosize_default.insert("contains".to_string(), Value::String("padding".to_string()));
    }

    let mut autosize = Map::new();
    autosize.insert("type".to_string(), Value::String("pad".to_string()));
    for (k, v) in autosize_default {
        autosize.insert(k, v);
    }
    let config_autosize = normalize_auto_size_value(get(config, "autosize"));
    if let Value::Object(m) = config_autosize {
        for (k, v) in m {
            autosize.insert(k, v);
        }
    }
    let spec_autosize = normalize_auto_size_value(get(size_info, "autosize"));
    if let Value::Object(m) = spec_autosize {
        for (k, v) in m {
            autosize.insert(k, v);
        }
    }

    let is_fit = autosize.get("type").and_then(|t| t.as_str()) == Some("fit");
    if is_fit && !is_fit_compatible {
        crate::log::warn("Autosize \"fit\" only works for single views and layered views.");
        autosize.insert("type".to_string(), Value::String("pad".to_string()));
    }

    let autosize_type = autosize
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();
    if width_is_container && autosize_type != "fit" && autosize_type != "fit-x" {
        crate::log::warn(
            "Width \"container\" only works well with autosize \"fit\" or \"fit-x\".",
        );
    }
    if height_is_container && autosize_type != "fit" && autosize_type != "fit-y" {
        crate::log::warn(
            "Height \"container\" only works well with autosize \"fit\" or \"fit-y\".",
        );
    }

    let autosize = Value::Object(autosize);

    // Delete autosize property if it's Vega's default
    if deep_equal(&autosize, &serde_json::json!({"type": "pad"})) {
        return None;
    }
    Some(autosize)
}

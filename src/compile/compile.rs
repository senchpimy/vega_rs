//! Top-level `compile()` entry point (port of `src/compile/compile.ts`).

use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use crate::config::{init_config, strip_and_redirect_config};
use crate::normalize::normalize;
use crate::parameter::assemble_parameter_signals;
use crate::spec::{extract_top_level_properties, get_fit_type, is_fit_type};
use crate::util::keys;

use super::buildmodel::build_model;
use super::data::assemble::assemble_root_data;
use super::data::optimize::optimize_dataflow;
use super::model::Model;

pub struct CompileResult {
    pub spec: Value,
    pub normalized: Value,
}

/// Compile a Vega-Lite specification into a Vega specification.
pub fn compile(input_spec: &Value) -> Result<CompileResult, String> {
    if !input_spec.is_object() {
        return Err(crate::log::message::invalid_spec(input_spec));
    }
    if !input_spec.get("mark").is_some()
        && !input_spec.get("layer").is_some()
        && !input_spec.get("facet").is_some()
        && !input_spec.get("concat").is_some()
        && !input_spec.get("hconcat").is_some()
        && !input_spec.get("vconcat").is_some()
        && !input_spec.get("repeat").is_some()
    {
        return Err(crate::log::message::invalid_spec(input_spec));
    }

    let config = init_config(input_spec.get("config").unwrap_or(&json!({})));
    let normalized = normalize(input_spec, Some(&config))?;
    let mut model = build_model(&normalized, None, "", None, &config);

    model.parse();

    if let Some(mut data) = model.data_flow.take() {
        optimize_dataflow(&mut data, Some(&model));
        model.data_flow = Some(data);
    }
    // Upstream gives every model its own data component whose output-node
    // objects are shared with the root dataflow. This port keeps a single
    // component on the root; expose it to unit/layer descendants so that
    // `lookupDataSource` resolves output names during scale assembly.
    model.propagate_data_flow();

    let top_level_properties = get_top_level_properties(input_spec, &normalized, &config, &model);
    let mut spec = assemble_top_level_model(&model, top_level_properties, input_spec, &config);
    canonicalize_numbers(&mut spec);

    Ok(CompileResult { spec, normalized })
}

fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(_) => true,
    }
}

/// Port of `getTopLevelProperties`.
fn get_top_level_properties(
    input_spec: &Value,
    normalized: &Value,
    config: &Value,
    model: &Model,
) -> Map<String, Value> {
    let width = model.component.layout_size.get("width").unwrap_or(Value::Null);
    let height = model
        .component
        .layout_size
        .get("height")
        .unwrap_or(Value::Null);

    let mut autosize = normalized.get("autosize").cloned().unwrap_or(Value::Null);
    let autosize_is_undefined = autosize.is_null();

    if autosize_is_undefined {
        autosize = json!({"type": "pad"});
        if model.has_axis_orient_signal_ref() {
            autosize["resize"] = Value::Bool(true);
        }
    } else if autosize.is_string() {
        autosize = json!({"type": autosize});
    }

    if is_truthy(&width) && is_truthy(&height) {
        let auto_type = autosize
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if is_fit_type(&auto_type) {
            let width_is_step = width.as_str() == Some("step");
            let height_is_step = height.as_str() == Some("step");
            if width_is_step && height_is_step {
                crate::log::warn(crate::log::message::dropping_fit(None));
                autosize["type"] = Value::String("pad".to_string());
            } else if width_is_step || height_is_step {
                let size_type = if width_is_step { "width" } else { "height" };
                let channel = crate::channel::get_position_scale_channel(size_type);
                crate::log::warn(crate::log::message::dropping_fit(Some(channel)));
                let inverse_size_type = if size_type == "width" { "height" } else { "width" };
                autosize["type"] = Value::String(get_fit_type(Some(inverse_size_type)));
            }
        }
    }

    let mut properties = Map::new();
    let auto_keys = keys(&autosize);
    let auto_type = autosize.get("type").and_then(Value::as_str).unwrap_or("");
    if auto_keys.len() == 1 && !auto_type.is_empty() {
        if auto_type != "pad" {
            properties.insert("autosize".to_string(), Value::String(auto_type.to_string()));
        }
    } else {
        properties.insert("autosize".to_string(), autosize.clone());
    }

    for (k, v) in extract_top_level_properties(config, false) {
        properties.insert(k, v);
    }
    for (k, v) in extract_top_level_properties(input_spec, true) {
        properties.insert(k, v);
    }

    properties
}

/// Port of `assembleTopLevelModel`.
fn assemble_top_level_model(
    model: &Model,
    mut top_level_properties: Map<String, Value>,
    input_spec: &Value,
    config: &Value,
) -> Value {
    let vg_config = strip_and_redirect_config(config);

    let datasets: IndexMap<String, Value> = input_spec
        .get("datasets")
        .and_then(Value::as_object)
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();

    let root_data = match &model.data_flow {
        Some(component) => assemble_root_data(component, &datasets),
        None => Vec::new(),
    };
    let data = model.assemble_selection_data(root_data);

    let projections = model.assemble_projections();
    let title = model.assemble_title();
    let style = model.assemble_group_style();
    let encode_entry = model.assemble_group_encode_entry(true);

    let mut layout_signals = model.assemble_layout_signals();

    // move width and height signals with values to the top level
    let mut moved_width: Option<Value> = None;
    let mut moved_height: Option<Value> = None;
    layout_signals.retain(|signal| {
        let name = signal.get("name").and_then(Value::as_str).unwrap_or("");
        if (name == "width" || name == "height")
            && signal.get("value").map(|v| !v.is_null()).unwrap_or(false)
        {
            let value = signal.get("value").cloned().unwrap_or(Value::Null);
            let value = match &value {
                Value::String(s) => s
                    .parse::<f64>()
                    .map(crate::compile::data::support::number_value)
                    .unwrap_or(value.clone()),
                _ => value,
            };
            if name == "width" {
                moved_width = Some(value);
            } else {
                moved_height = Some(value);
            }
            false
        } else {
            true
        }
    });
    if let Some(value) = moved_width {
        top_level_properties.insert("width".to_string(), value);
    }
    if let Some(value) = moved_height {
        top_level_properties.insert("height".to_string(), value);
    }

    let params = top_level_properties.remove("params");
    let params: Vec<Value> = params
        .and_then(|p| p.as_array().cloned())
        .unwrap_or_default();

    let mut group_signals = layout_signals;
    group_signals.extend(model.assemble_selection_top_level_signals(Vec::new()));
    group_signals.extend(assemble_parameter_signals(&params));

    let mut spec = Map::new();
    spec.insert(
        "$schema".to_string(),
        Value::String("https://vega.github.io/schema/vega/v6.json".to_string()),
    );
    if let Some(description) = &model.description {
        if !description.is_empty() {
            spec.insert(
                "description".to_string(),
                Value::String(description.clone()),
            );
        }
    }
    for (k, v) in top_level_properties {
        spec.insert(k, v);
    }
    if let Some(title) = title {
        spec.insert("title".to_string(), title);
    }
    if let Some(style) = style {
        spec.insert("style".to_string(), style);
    }
    if let Some(encode_entry) = encode_entry {
        spec.insert("encode".to_string(), json!({"update": encode_entry}));
    }
    spec.insert("data".to_string(), Value::Array(data));
    if !projections.is_empty() {
        spec.insert("projections".to_string(), Value::Array(projections));
    }

    if let Value::Object(group) = model.assemble_group(group_signals) {
        for (k, v) in group {
            spec.insert(k, v);
        }
    }

    if let Some(vg_config) = vg_config {
        spec.insert("config".to_string(), vg_config);
    }
    if let Some(usermeta) = input_spec.get("usermeta") {
        if !usermeta.is_null() {
            spec.insert("usermeta".to_string(), usermeta.clone());
        }
    }

    Value::Object(spec)
}

/// Vega-Lite is compiled through JavaScript, where `JSON.stringify` prints
/// integral floats without a trailing `.0` (e.g. `112.0` becomes `112`).
/// Numeric literals inside the input data/transforms keep their parsed JSON
/// representation in this port, so canonicalize the assembled spec once at the
/// end to match the upstream JSON output exactly.
fn canonicalize_numbers(value: &mut Value) {
    match value {
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                if f.is_finite() && f.fract() == 0.0 && f >= i64::MIN as f64 && f <= i64::MAX as f64 {
                    *n = serde_json::Number::from(f as i64);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                canonicalize_numbers(item);
            }
        }
        Value::Object(map) => {
            for (_, v) in map.iter_mut() {
                canonicalize_numbers(v);
            }
        }
        _ => {}
    }
}

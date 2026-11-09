//! Port of `src/normalize/toplevelselection.ts`.

use serde_json::Value;

use crate::normalize::base::NormalizerParams;
use crate::normalize_support::*;

pub fn map(spec: &Value, norm_params: &NormalizerParams) -> Value {
    let mut spec = spec.clone();
    if get(&spec, "params").and_then(|p| p.as_array()).is_some() && !is_unit_spec(&spec) {
        let params_arr = get(&spec, "params")
            .and_then(|p| p.as_array())
            .cloned()
            .unwrap_or_default();
        let mut variables: Vec<Value> = Vec::new();
        for param in params_arr {
            if is_selection_parameter(&param) {
                norm_params.selections.borrow_mut().push(param);
            } else {
                variables.push(param);
            }
        }
        if let Some(m) = spec.as_object_mut() {
            m.insert("params".to_string(), Value::Array(variables));
        }
    }

    map_dispatch(&spec, norm_params)
}

fn is_selection_parameter(param: &Value) -> bool {
    truthy(get(param, "select").unwrap_or(&Value::Null))
}

fn add_spec_name_to_params(spec: &Value, params: &NormalizerParams) -> NormalizerParams {
    if let Some(name) = get(spec, "name") {
        if truthy(name) {
            let mut new_params = params.clone();
            new_params.path.push(name.clone());
            return new_params;
        }
    }
    params.clone()
}

fn map_dispatch(spec: &Value, params: &NormalizerParams) -> Value {
    if is_unit_spec(spec) {
        map_unit(spec, params)
    } else if has_prop(spec, "layer") {
        map_layer(spec, params)
    } else if has_prop(spec, "facet") {
        map_facet(spec, params)
    } else if has_prop(spec, "repeat") {
        map_repeat(spec, params)
    } else if has_prop(spec, "hconcat") {
        map_hconcat(spec, params)
    } else if has_prop(spec, "vconcat") {
        map_vconcat(spec, params)
    } else if has_prop(spec, "concat") {
        map_concat(spec, params)
    } else {
        spec.clone()
    }
}

fn map_layer(spec: &Value, params: &NormalizerParams) -> Value {
    let params = add_spec_name_to_params(spec, params);
    let mut out = spec.as_object().cloned().unwrap_or_default();
    let children: Vec<Value> = get(spec, "layer")
        .and_then(|a| a.as_array())
        .map(|a| a.iter().map(|s| map_layer_or_unit(s, &params)).collect())
        .unwrap_or_default();
    out.insert("layer".to_string(), Value::Array(children));
    Value::Object(out)
}

fn map_layer_or_unit(spec: &Value, params: &NormalizerParams) -> Value {
    if has_prop(spec, "layer") {
        map_layer(spec, params)
    } else if is_unit_spec(spec) {
        map_unit(spec, params)
    } else {
        spec.clone()
    }
}

fn map_facet(spec: &Value, params: &NormalizerParams) -> Value {
    let params = add_spec_name_to_params(spec, params);
    let mut out = spec.as_object().cloned().unwrap_or_default();
    out.insert(
        "spec".to_string(),
        map(get(spec, "spec").unwrap_or(&Value::Null), &params),
    );
    Value::Object(out)
}

fn map_repeat(spec: &Value, params: &NormalizerParams) -> Value {
    let params = add_spec_name_to_params(spec, params);
    let mut out = spec.as_object().cloned().unwrap_or_default();
    out.insert(
        "spec".to_string(),
        map(get(spec, "spec").unwrap_or(&Value::Null), &params),
    );
    Value::Object(out)
}

fn map_hconcat(spec: &Value, params: &NormalizerParams) -> Value {
    let params = add_spec_name_to_params(spec, params);
    let mut out = spec.as_object().cloned().unwrap_or_default();
    let children: Vec<Value> = get(spec, "hconcat")
        .and_then(|a| a.as_array())
        .map(|a| a.iter().map(|s| map(s, &params)).collect())
        .unwrap_or_default();
    out.insert("hconcat".to_string(), Value::Array(children));
    Value::Object(out)
}

fn map_vconcat(spec: &Value, params: &NormalizerParams) -> Value {
    let params = add_spec_name_to_params(spec, params);
    let mut out = spec.as_object().cloned().unwrap_or_default();
    let children: Vec<Value> = get(spec, "vconcat")
        .and_then(|a| a.as_array())
        .map(|a| a.iter().map(|s| map(s, &params)).collect())
        .unwrap_or_default();
    out.insert("vconcat".to_string(), Value::Array(children));
    Value::Object(out)
}

fn map_concat(spec: &Value, params: &NormalizerParams) -> Value {
    let mut out = omit(spec, &["concat"]);
    let children: Vec<Value> = get(spec, "concat")
        .and_then(|a| a.as_array())
        .map(|a| a.iter().map(|s| map(s, params)).collect())
        .unwrap_or_default();
    if let Some(m) = out.as_object_mut() {
        m.insert("concat".to_string(), Value::Array(children));
    }
    out
}

pub fn map_unit(spec: &Value, norm_params: &NormalizerParams) -> Value {
    let selections = norm_params.selections.borrow().clone();
    if selections.is_empty() {
        return spec.clone();
    }

    let mut path = norm_params.path.clone();
    path.push(
        get(spec, "name")
            .cloned()
            .unwrap_or(Value::Null),
    );
    let mut params: Vec<Value> = Vec::new();

    for selection in &selections {
        let views = get(selection, "views").and_then(|v| v.as_array()).cloned();
        match views {
            None => params.push(selection.clone()),
            Some(views) if views.is_empty() => params.push(selection.clone()),
            Some(views) => {
                for view in &views {
                    if let Some(view_str) = view.as_str() {
                        let name_matches = get(spec, "name").and_then(|n| n.as_str()) == Some(view_str);
                        let path_matches = path.iter().any(|p| p.as_str() == Some(view_str));
                        if name_matches || path_matches {
                            params.push(selection.clone());
                        }
                    } else if let Some(view_arr) = view.as_array() {
                        let indices: Vec<i64> = view_arr
                            .iter()
                            .map(|v| {
                                path.iter()
                                    .position(|p| p == v)
                                    .map(|i| i as i64)
                                    .unwrap_or(-1)
                            })
                            .collect();
                        let mut ok = true;
                        for (i, idx) in indices.iter().enumerate() {
                            if *idx == -1 || (i > 0 && *idx <= indices[i - 1]) {
                                ok = false;
                                break;
                            }
                        }
                        if ok {
                            params.push(selection.clone());
                        }
                    }
                }
            }
        }
    }

    let mut out = spec.as_object().cloned().unwrap_or_default();
    if !params.is_empty() {
        out.insert("params".to_string(), Value::Array(params));
    }
    Value::Object(out)
}


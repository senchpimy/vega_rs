//! Port of `src/normalize/selectioncompat.ts`.

use serde_json::{json, Map, Value};

use crate::logical;
use crate::normalize::base::NormalizerParams;
use crate::normalize_support::*;
use crate::util::duplicate;

pub fn map(spec: &Value, norm_params: &NormalizerParams) -> Result<Value, String> {
    // `emptySelections ??= {}` and `selectionPredicates ??= {}`
    let spec = normalize_transforms(spec, norm_params);
    map_dispatch(&spec, norm_params)
}

fn map_dispatch(spec: &Value, params: &NormalizerParams) -> Result<Value, String> {
    if has_prop(spec, "facet") {
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
        map_layer_or_unit(spec, params)
    }
}

fn map_facet(spec: &Value, params: &NormalizerParams) -> Result<Value, String> {
    let mut out = spec.as_object().cloned().unwrap_or_default();
    let child = map(get(spec, "spec").unwrap_or(&Value::Null), params)?;
    out.insert("spec".to_string(), child);
    Ok(Value::Object(out))
}

fn map_repeat(spec: &Value, params: &NormalizerParams) -> Result<Value, String> {
    let mut out = spec.as_object().cloned().unwrap_or_default();
    let child = map(get(spec, "spec").unwrap_or(&Value::Null), params)?;
    out.insert("spec".to_string(), child);
    Ok(Value::Object(out))
}

fn map_hconcat(spec: &Value, params: &NormalizerParams) -> Result<Value, String> {
    let mut out = spec.as_object().cloned().unwrap_or_default();
    let children: Vec<Value> = get(spec, "hconcat")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .map(|s| map(s, params))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    out.insert("hconcat".to_string(), Value::Array(children));
    Ok(Value::Object(out))
}

fn map_vconcat(spec: &Value, params: &NormalizerParams) -> Result<Value, String> {
    let mut out = spec.as_object().cloned().unwrap_or_default();
    let children: Vec<Value> = get(spec, "vconcat")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .map(|s| map(s, params))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    out.insert("vconcat".to_string(), Value::Array(children));
    Ok(Value::Object(out))
}

fn map_concat(spec: &Value, params: &NormalizerParams) -> Result<Value, String> {
    let mut out = omit(spec, &["concat"]);
    let children: Vec<Value> = get(spec, "concat")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .map(|s| map(s, params))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    if let Some(m) = out.as_object_mut() {
        m.insert("concat".to_string(), Value::Array(children));
    }
    Ok(out)
}

pub fn map_layer_or_unit(spec: &Value, params: &NormalizerParams) -> Result<Value, String> {
    let spec = normalize_transforms(spec, params);

    let spec = if let Some(encoding) = get(&spec, "encoding") {
        let mut normalized = Map::new();
        for (channel, enc) in entries(encoding) {
            normalized.insert(channel, normalize_channel_def(&enc, params));
        }
        let mut out = spec.as_object().cloned().unwrap_or_default();
        out.insert("encoding".to_string(), Value::Object(normalized));
        Value::Object(out)
    } else {
        spec
    };

    if has_prop(&spec, "layer") {
        let mut out = spec.as_object().cloned().unwrap_or_default();
        let children: Vec<Value> = get(&spec, "layer")
            .and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .map(|s| map_layer_or_unit(s, params))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default();
        out.insert("layer".to_string(), Value::Array(children));
        Ok(Value::Object(out))
    } else if has_prop(&spec, "mark") {
        map_unit(&spec, params)
    } else {
        Err(crate::log::message::invalid_spec(&spec))
    }
}

pub fn map_unit(spec: &Value, norm_params: &NormalizerParams) -> Result<Value, String> {
    let selection = get(spec, "selection").cloned();
    if let Some(selection) = selection {
        let mut rest = spec.as_object().cloned().unwrap_or_default();
        rest.remove("selection");
        let mut params: Vec<Value> = Vec::new();
        for (name, sel_def) in entries(&selection) {
            let value = get(&sel_def, "init").cloned();
            let bind = get(&sel_def, "bind").cloned();
            let empty = get(&sel_def, "empty").cloned();
            let mut select = omit(&sel_def, &["init", "bind", "empty"]);
            let select_type = get(&select, "type").and_then(|t| t.as_str()).map(|s| s.to_string());
            if select_type.as_deref() == Some("single") {
                if let Some(m) = select.as_object_mut() {
                    m.insert("type".to_string(), Value::String("point".to_string()));
                    m.insert("toggle".to_string(), Value::Bool(false));
                }
            } else if select_type.as_deref() == Some("multi") {
                if let Some(m) = select.as_object_mut() {
                    m.insert("type".to_string(), Value::String("point".to_string()));
                }
            }

            let is_empty = empty.as_ref().and_then(|e| e.as_str()) != Some("none");
            norm_params
                .empty_selections
                .borrow_mut()
                .insert(name.clone(), Value::Bool(is_empty));
            {
                let mut predicates = norm_params.selection_predicates.borrow_mut();
                if let Some(Value::Array(preds)) = predicates.get_mut(&name) {
                    for pred in preds.iter_mut() {
                        if let Some(m) = pred.as_object_mut() {
                            m.insert("empty".to_string(), Value::Bool(is_empty));
                        }
                    }
                }
            }

            let mut param = Map::new();
            param.insert("name".to_string(), Value::String(name));
            if let Some(value) = value {
                param.insert("value".to_string(), value);
            }
            param.insert("select".to_string(), select);
            if let Some(bind) = bind {
                param.insert("bind".to_string(), bind);
            }
            params.push(Value::Object(param));
        }
        rest.insert("params".to_string(), Value::Array(params));
        Ok(Value::Object(rest))
    } else {
        Ok(spec.clone())
    }
}

fn normalize_transforms(spec: &Value, norm_params: &NormalizerParams) -> Value {
    let Some(tx) = get(spec, "transform").and_then(|t| t.as_array()) else {
        return spec.clone();
    };

    let transform: Vec<Value> = tx
        .iter()
        .map(|t| {
            if has_prop(t, "filter") {
                json!({"filter": normalize_predicate(t, norm_params)})
            } else if has_prop(t, "bin") && is_bin_params(get(t, "bin").unwrap()) {
                let mut m = t.as_object().cloned().unwrap_or_default();
                m.insert(
                    "bin".to_string(),
                    normalize_bin_extent(get(t, "bin").unwrap()),
                );
                Value::Object(m)
            } else if has_prop(t, "lookup") {
                if let Some(from) = get(t, "from") {
                    if let Some(param) = get(from, "selection") {
                        let mut new_from = Map::new();
                        new_from.insert("param".to_string(), param.clone());
                        if let Value::Object(rest) = omit(from, &["selection"]) {
                            for (k, v) in rest {
                                new_from.insert(k, v);
                            }
                        }
                        let mut m = t.as_object().cloned().unwrap_or_default();
                        m.insert("from".to_string(), Value::Object(new_from));
                        return Value::Object(m);
                    }
                }
                t.clone()
            } else {
                t.clone()
            }
        })
        .collect();

    let mut out = omit(spec, &["transform"]);
    if let Some(m) = out.as_object_mut() {
        m.insert("transform".to_string(), Value::Array(transform));
    }
    out
}

fn normalize_channel_def(obj: &Value, norm_params: &NormalizerParams) -> Value {
    let mut enc = duplicate(obj);

    if is_field_def(&enc) {
        if let Some(bin) = get(&enc, "bin").cloned() {
            if is_bin_params(&bin) {
                if let Some(m) = enc.as_object_mut() {
                    m.insert("bin".to_string(), normalize_bin_extent(&bin));
                }
            }
        }
    }

    if is_scale_field_def(&enc) {
        if let Some(scale) = get(&enc, "scale").cloned() {
            if let Some(domain) = get(&scale, "domain").cloned() {
                if let Some(param) = get(&domain, "selection").cloned() {
                    let mut new_domain = Map::new();
                    if let Value::Object(rest) = omit(&domain, &["selection"]) {
                        for (k, v) in rest {
                            new_domain.insert(k, v);
                        }
                    }
                    if truthy(&param) {
                        new_domain.insert("param".to_string(), param);
                    }
                    let mut new_scale = scale.as_object().cloned().unwrap_or_default();
                    new_scale.insert("domain".to_string(), Value::Object(new_domain));
                    if let Some(m) = enc.as_object_mut() {
                        m.insert("scale".to_string(), Value::Object(new_scale));
                    }
                }
            }
        }
    }

    if is_conditional_def(&enc) {
        if let Some(Value::Array(arr)) = get(&enc, "condition").cloned() {
            let mapped: Vec<Value> = arr
                .iter()
                .map(|c| {
                    let param = get(c, "param").cloned();
                    if truthy(param.as_ref().unwrap_or(&Value::Null)) {
                        c.clone()
                    } else {
                        let mut cond = omit(c, &["selection", "param", "test"]);
                        if let Some(m) = cond.as_object_mut() {
                            m.insert("test".to_string(), normalize_predicate(c, norm_params));
                        }
                        cond
                    }
                })
                .collect();
            if let Some(m) = enc.as_object_mut() {
                m.insert("condition".to_string(), Value::Array(mapped));
            }
        } else if let Some(condition) = get(&enc, "condition").cloned() {
            let normalized_cond = normalize_channel_def(&condition, norm_params);
            let param = get(&normalized_cond, "param").cloned();
            if truthy(param.as_ref().unwrap_or(&Value::Null)) {
                // keep original condition
            } else {
                let mut cond = omit(&normalized_cond, &["selection", "param", "test"]);
                if let Some(m) = cond.as_object_mut() {
                    m.insert(
                        "test".to_string(),
                        normalize_predicate(&condition, norm_params),
                    );
                }
                if let Some(m) = enc.as_object_mut() {
                    m.insert("condition".to_string(), cond);
                }
            }
        }
    }

    enc
}

/// Final credits: predicates are created before the `selection` definitions are
/// converted, so (like the mutable objects upstream) their `empty` flag is
/// resolved once every selection has been processed.  A private marker records
/// which objects were generated by `normalizePredicate`.
const PREDICATE_MARKER: &str = "__vl_predicate_marker";

pub fn finalize_predicates(spec: &mut Value, empty_selections: &std::collections::BTreeMap<String, Value>) {
    match spec {
        Value::Object(m) => {
            if m.remove(PREDICATE_MARKER).is_some() {
                if let Some(name) = m.get("param").and_then(|p| p.as_str()).map(|s| s.to_string()) {
                    let empty = empty_selections
                        .get(&name)
                        .cloned()
                        .unwrap_or(Value::Bool(true));
                    m.insert("empty".to_string(), empty);
                }
            }
            for value in m.values_mut() {
                finalize_predicates(value, empty_selections);
            }
        }
        Value::Array(a) => {
            for value in a.iter_mut() {
                finalize_predicates(value, empty_selections);
            }
        }
        _ => {}
    }
}

fn normalize_selection_composition(o: &Value, norm_params: &NormalizerParams) -> Value {
    let mut normalizer = |param: &Value| -> Value {
        let name = param.as_str().unwrap_or("").to_string();
        let empty = norm_params
            .empty_selections
            .borrow()
            .get(&name)
            .cloned()
            .unwrap_or(Value::Bool(true));
        let pred = json!({
            "param": name,
            "empty": empty,
            "__vl_predicate_marker": true
        });
        {
            let mut predicates = norm_params.selection_predicates.borrow_mut();
            let entry = predicates
                .entry(name.clone())
                .or_insert_with(|| Value::Array(Vec::new()));
            if let Value::Array(arr) = entry {
                arr.push(pred.clone());
            }
        }
        pred
    };
    logical::normalize_logical_composition(o, &mut normalizer)
}

fn normalize_predicate(op: &Value, norm_params: &NormalizerParams) -> Value {
    if let Some(selection) = get(op, "selection") {
        if truthy(selection) {
            return normalize_selection_composition(selection, norm_params);
        }
    }
    let test_op = get(op, "test")
        .or_else(|| get(op, "filter"))
        .cloned()
        .unwrap_or(Value::Null);
    let mut normalizer = |o: &Value| -> Value {
        if let Some(selection) = get(o, "selection") {
            if truthy(selection) {
                return normalize_selection_composition(selection, norm_params);
            }
        }
        o.clone()
    };
    logical::normalize_logical_composition(&test_op, &mut normalizer)
}

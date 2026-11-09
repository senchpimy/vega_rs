//! Port of `src/normalize/pathoverlay.ts`.

use std::rc::Rc;
use serde_json::{json, Map, Value};

use crate::mark;
use crate::normalize::base::NormalizerParams;
use crate::normalize::core;
use crate::normalize_support::*;

const DEFAULT_REDUCED_OPACITY: f64 = 0.7;

fn drop_line_and_point(mark_def: &Value) -> Value {
    let mark = omit(mark_def, &["point", "line"]);
    if keys(&mark).len() > 1 {
        mark
    } else {
        get(&mark, "type").cloned().unwrap_or(Value::Null)
    }
}

fn drop_line_and_point_from_config(config: &Value) -> Value {
    let mut config = config.clone();
    for mark_type in ["line", "area", "rule", "trail"] {
        if truthy(get(&config, mark_type).unwrap_or(&Value::Null)) {
            let updated = omit(get(&config, mark_type).unwrap(), &["point", "line"]);
            if let Some(m) = config.as_object_mut() {
                m.insert(mark_type.to_string(), updated);
            }
        }
    }
    config
}

fn get_point_overlay(
    mark_def: &Value,
    mark_config: &Value,
    encoding: &Value,
) -> Option<Value> {
    let point = get(mark_def, "point").cloned();
    if point.as_ref().and_then(|p| p.as_str()) == Some("transparent") {
        return Some(json!({"opacity": 0}));
    } else if point.as_ref().map(truthy).unwrap_or(false) {
        let p = point.unwrap();
        return Some(if is_object(&p) { p } else { empty_object() });
    } else if point.is_some() {
        // false or null: explicitly disabled
        return None;
    }

    let config_point = get(mark_config, "point").cloned();
    if config_point.as_ref().map(truthy).unwrap_or(false)
        || truthy(get(encoding, "shape").unwrap_or(&Value::Null))
    {
        let p = config_point.unwrap_or(Value::Null);
        return Some(if is_object(&p) { p } else { empty_object() });
    }
    None
}

fn get_line_overlay(mark_def: &Value, mark_config: &Value) -> Option<Value> {
    let line = get(mark_def, "line").cloned();
    if line.as_ref().map(truthy).unwrap_or(false) {
        let l = line.unwrap();
        return Some(if l == Value::Bool(true) { empty_object() } else { l });
    } else if line.is_some() {
        return None;
    }

    let config_line = get(mark_config, "line").cloned();
    if config_line.as_ref().map(truthy).unwrap_or(false) {
        let l = config_line.unwrap();
        return Some(if l == Value::Bool(true) { empty_object() } else { l });
    }
    None
}

pub fn has_matching_type(spec: &Value, config: &Value) -> bool {
    if is_unit_spec(spec) {
        let mark_value = get(spec, "mark").cloned().unwrap_or(Value::Null);
        let encoding = get(spec, "encoding").cloned().unwrap_or(empty_object());
        let mark_def = if mark::is_mark_def(&mark_value) {
            mark_value.clone()
        } else {
            json!({"type": mark_value.clone()})
        };
        let mark_type = get(&mark_def, "type").and_then(|t| t.as_str()).unwrap_or("");
        let mark_config = get(config, mark_type).cloned().unwrap_or(empty_object());
        match mark_type {
            "line" | "rule" | "trail" => get_point_overlay(&mark_def, &mark_config, &encoding).is_some(),
            "area" => {
                get_point_overlay(&mark_def, &mark_config, &encoding).is_some()
                    || get_line_overlay(&mark_def, &mark_config).is_some()
            }
            _ => false,
        }
    } else {
        false
    }
}

pub fn run(spec: &Value, norm_params: &NormalizerParams) -> Result<Value, String> {
    let config = &norm_params.config;
    let mark_value = get(spec, "mark").cloned().unwrap_or(Value::Null);
    let mark_def = if mark::is_mark_def(&mark_value) {
        mark_value.clone()
    } else {
        json!({"type": mark_value.clone()})
    };
    let mark_type = get(&mark_def, "type").and_then(|t| t.as_str()).unwrap_or("").to_string();
    let mark_config = get(config, &mark_type).cloned().unwrap_or(empty_object());

    let Some(encoding_original) = get(spec, "encoding") else {
        return Err("path overlay spec without encoding".to_string());
    };
    let encoding = normalize_encoding(encoding_original, config);

    let point_overlay = get_point_overlay(&mark_def, &mark_config, &encoding);
    let line_overlay = if mark_type == "area" {
        get_line_overlay(&mark_def, &mark_config)
    } else {
        None
    };

    let mut outer_spec = spec.as_object().cloned().unwrap_or_default();
    outer_spec.remove("params");
    outer_spec.remove("projection");
    outer_spec.remove("mark");
    outer_spec.remove("name");
    outer_spec.remove("encoding");

    let mut first_mark = Map::new();
    let reduced_opacity = mark_type == "area"
        && get_mark_prop_or_config("opacity", &mark_def, config)
            .map(|v| v.is_null())
            .unwrap_or(true)
        && get_mark_prop_or_config("fillOpacity", &mark_def, config)
            .map(|v| v.is_null())
            .unwrap_or(true);
    if mark_type == "area" && reduced_opacity {
        first_mark.insert(
            "opacity".to_string(),
            json!(DEFAULT_REDUCED_OPACITY),
        );
    }
    if let Value::Object(m) = &mark_def {
        for (k, v) in m {
            first_mark.insert(k.clone(), v.clone());
        }
    }
    let mut first_layer = Map::new();
    if let Some(name) = get(spec, "name") {
        if !name.is_null() {
            first_layer.insert("name".to_string(), name.clone());
        }
    }
    if let Some(params) = get(spec, "params") {
        if truthy(params) {
            first_layer.insert("params".to_string(), params.clone());
        }
    }
    first_layer.insert("mark".to_string(), drop_line_and_point(&Value::Object(first_mark)));
    first_layer.insert("encoding".to_string(), omit(&encoding, &["shape"]));

    let mut layer = vec![Value::Object(first_layer)];

    let stack_props = stack(&init_markdef(&mark_def, &encoding, config), &encoding);

    let mut overlay_encoding = encoding.clone();
    if let Some(stack_props) = stack_props {
        let mut enc_obj = overlay_encoding.as_object().cloned().unwrap_or_default();
        let field_channel = stack_props.field_channel.clone();
        let mut field_def = get(&overlay_encoding, &field_channel)
            .cloned()
            .unwrap_or_else(empty_object);
        if let Some(m) = field_def.as_object_mut() {
            m.insert("stack".to_string(), Value::String(stack_props.offset.clone()));
        }
        enc_obj.insert(field_channel, field_def);
        overlay_encoding = Value::Object(enc_obj);
    }
    overlay_encoding = omit(&overlay_encoding, &["y2", "x2"]);

    if let Some(line_overlay) = line_overlay {
        let mut line_mark = Map::new();
        line_mark.insert("type".to_string(), Value::String("line".to_string()));
        if let Value::Object(picked) = pick(&mark_def, &["clip", "interpolate", "tension", "tooltip"]) {
            for (k, v) in picked {
                line_mark.insert(k, v);
            }
        }
        if let Value::Object(over) = line_overlay {
            for (k, v) in over {
                line_mark.insert(k, v);
            }
        }
        let mut layer_spec = Map::new();
        if let Some(projection) = get(spec, "projection") {
            if truthy(projection) {
                layer_spec.insert("projection".to_string(), projection.clone());
            }
        }
        layer_spec.insert("mark".to_string(), Value::Object(line_mark));
        layer_spec.insert("encoding".to_string(), overlay_encoding.clone());
        layer.push(Value::Object(layer_spec));
    }

    if let Some(point_overlay) = point_overlay {
        let mut point_mark = Map::new();
        point_mark.insert("type".to_string(), Value::String("point".to_string()));
        point_mark.insert("opacity".to_string(), json!(1));
        point_mark.insert("filled".to_string(), Value::Bool(true));
        if let Value::Object(picked) = pick(&mark_def, &["clip", "tooltip"]) {
            for (k, v) in picked {
                point_mark.insert(k, v);
            }
        }
        if let Value::Object(over) = point_overlay {
            for (k, v) in over {
                point_mark.insert(k, v);
            }
        }
        let mut layer_spec = Map::new();
        if let Some(projection) = get(spec, "projection") {
            if truthy(projection) {
                layer_spec.insert("projection".to_string(), projection.clone());
            }
        }
        layer_spec.insert("mark".to_string(), Value::Object(point_mark));
        layer_spec.insert("encoding".to_string(), overlay_encoding.clone());
        layer.push(Value::Object(layer_spec));
    }

    outer_spec.insert("layer".to_string(), Value::Array(layer));

    let mut new_params = norm_params.clone();
    new_params.config = Rc::new(drop_line_and_point_from_config(config));
    core::map_layer_or_unit(&Value::Object(outer_spec), &new_params)
}

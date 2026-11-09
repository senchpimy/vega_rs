//! Port of `src/compile/mark/encode/tooltip.ts`.

use serde_json::{json, Map, Value};

use crate::channeldef::{
    default_title, get_field_def, get_format_mixins, has_conditional_field_def, is_count,
    is_field_def, is_typed_field_def, vg_field,
};
use crate::channeldef_support::{is_binned, truthy};
use crate::compile::model::Model;
use crate::compile_common::signal_or_value_ref;
use crate::encoding::for_each;
use crate::mark::get_mark_prop_or_config;
use crate::predicate::{
    field_filter_expression_with_options, is_field_predicate, is_field_valid_predicate,
};
use crate::timeunit::normalize_time_unit;
use crate::util::{array, has_property, logical_expr, stringify};

use super::conditional::wrap_condition;
use super::format::{bin_format_expression, format_signal_ref};
use super::text::text_ref;

#[derive(Debug, Clone)]
pub struct TooltipTuple {
    pub channel: String,
    pub key: String,
    pub value: String,
    pub test: Option<String>,
}

pub fn tooltip(model: &Model) -> Value {
    tooltip_with_options(model, false)
}

pub fn tooltip_with_options(model: &Model, reactive_geom: bool) -> Value {
    let encoding = &model.unit.encoding;
    let mark_def = &model.unit.mark_def;
    let config = &model.config;
    let stack = super::super::mark::stack_of(model);

    let channel_def = encoding.get("tooltip").cloned().unwrap_or(Value::Null);
    let has_tooltip_key = encoding.get("tooltip").is_some();

    if channel_def.is_array() {
        let channel_def = channel_def;
        let reference = tooltip_ref_for_encoding(
            &json!({"tooltip": channel_def}),
            stack.as_ref(),
            config,
            reactive_geom,
        );
        let mut map = Map::new();
        if let Some(reference) = reference {
            map.insert("tooltip".to_string(), reference);
        }
        return Value::Object(map);
    }

    let datum = if reactive_geom { "datum.datum" } else { "datum" };
    let main_ref_fn = |c_def: &Value| -> Option<Value> {
        let tooltip_ref_from_channel_def = add_line_breaks_to_tooltip(c_def, config, datum);
        if let Some(reference) = tooltip_ref_from_channel_def {
            return Some(reference);
        }

        if c_def.is_null() && has_tooltip_key {
            return None;
        }

        let mut mark_tooltip = get_mark_prop_or_config("tooltip", mark_def, config, None, false);

        if mark_tooltip.as_ref().and_then(Value::as_bool) == Some(true) {
            mark_tooltip = Some(json!({"content": "encoding"}));
        }

        if let Some(Value::String(s)) = &mark_tooltip {
            return Some(json!({"value": s}));
        } else if let Some(mt) = &mark_tooltip {
            if mt.is_object() {
                if mt.get("signal").is_some() {
                    return Some(mt.clone());
                } else if mt.get("content").and_then(Value::as_str) == Some("encoding") {
                    return tooltip_ref_for_encoding(encoding, stack.as_ref(), config, reactive_geom);
                } else {
                    return Some(json!({"signal": datum}));
                }
            }
        }
        None
    };

    wrap_condition(
        model,
        Some(&channel_def),
        "tooltip",
        None,
        main_ref_fn,
    )
}

fn is_xor_y(channel: &str) -> bool {
    channel == "x" || channel == "y"
}

pub fn tooltip_data_tuples(
    encoding: &Value,
    stack: Option<&Value>,
    config: &Value,
    reactive_geom: bool,
) -> Vec<TooltipTuple> {
    // `format_config` is the config with the top-level `tooltipFormat` entries
    // promoted; avoid deep-cloning the config when there is nothing to promote
    // (the common case).
    let format_config: std::borrow::Cow<'_, Value> =
        match config.get("tooltipFormat").and_then(Value::as_object) {
            Some(tooltip_format) => {
                let mut merged = config.as_object().cloned().unwrap_or_default();
                for (k, v) in tooltip_format {
                    merged.insert(k.clone(), v.clone());
                }
                std::borrow::Cow::Owned(Value::Object(merged))
            }
            None => std::borrow::Cow::Borrowed(config),
        };
    let format_config = format_config.as_ref();
    let mut to_skip: Vec<String> = Vec::new();
    let expr = if reactive_geom { "datum.datum" } else { "datum" };
    let mut tuples: Vec<TooltipTuple> = Vec::new();

    let add = |f_def: &Value, channel: &str, tuples: &mut Vec<TooltipTuple>, to_skip: &mut Vec<String>| {
        let main_channel = crate::channel::get_main_range_channel(channel);

        let field_def = if is_typed_field_def(f_def) {
            f_def.clone()
        } else {
            let mut fd = f_def.clone();
            if let Some(main) = encoding.get(main_channel) {
                if let Some(t) = main.get("type") {
                    fd["type"] = t.clone();
                }
            }
            fd
        };

        let title = field_def
            .get("title")
            .filter(|v| crate::channeldef_support::truthy(v))
            .cloned()
            .unwrap_or_else(|| Value::String(default_title(&field_def, format_config)));
        let key = array(&title)
            .iter()
            .map(|v| match v {
                Value::String(s) => s.clone(),
                other => stringify(other),
            })
            .collect::<Vec<_>>()
            .join(", ")
            .replace('"', "\\\"");

        let mut value: Option<String> = None;

        if is_xor_y(channel) {
            let channel2 = if channel == "x" { "x2" } else { "y2" };
            let field_def2 = get_field_def(encoding.get(channel2).unwrap_or(&Value::Null));

            if is_binned(&field_def["bin"]) {
                if let Some(field_def2) = field_def2 {
                    let start_field = crate::channeldef::vg_field_with(
                        &field_def,
                        &crate::channeldef::FieldRefOption {
                            expr: Some(expr.to_string()),
                            ..Default::default()
                        },
                    );
                    let end_field = crate::channeldef::vg_field_with(
                        &field_def2,
                        &crate::channeldef::FieldRefOption {
                            expr: Some(expr.to_string()),
                            ..Default::default()
                        },
                    );
                    let mixins = get_format_mixins(&field_def);
                    value = Some(bin_format_expression(
                        &start_field,
                        &end_field,
                        mixins.get("format"),
                        mixins.get("formatType").and_then(Value::as_str),
                        format_config,
                    ));
                    to_skip.push(channel2.to_string());
                    let _ = start_field;
                }
            }
        }

        if field_def.get("tooltip").and_then(Value::as_bool) == Some(false) {
            return;
        }

        let test = tooltip_filter_expression(&field_def, channel, expr).ok();

        let stack_matches = stack
            .and_then(|s| s.get("fieldChannel"))
            .and_then(Value::as_str)
            == Some(channel)
            && stack
                .and_then(|s| s.get("offset"))
                .and_then(Value::as_str)
                == Some("normalize");

        if (is_xor_y(channel) || channel == "theta" || channel == "radius")
            && stack.is_some()
            && stack_matches
        {
            let mixins = get_format_mixins(&field_def);
            if let Some(Value::String(signal)) = format_signal_ref(
                &field_def,
                mixins.get("format"),
                mixins.get("formatType").and_then(Value::as_str),
                Some(expr),
                true,
                format_config,
            )
            .and_then(|v| v.get("signal").cloned())
            {
                value = Some(signal);
            }
        }

        if value.is_none() {
            value = add_line_breaks_to_tooltip(&field_def, format_config, expr)
                .and_then(|v| v.get("signal").and_then(Value::as_str).map(str::to_string));
        }

        tuples.push(TooltipTuple {
            channel: channel.to_string(),
            key,
            value: value.unwrap_or_default(),
            test,
        });
    };

    let mut channels: Vec<(String, Value)> = Vec::new();
    for_each(encoding, |channel_def, channel| {
        if is_field_def(channel_def) {
            channels.push((channel.to_string(), channel_def.clone()));
        } else if has_conditional_field_def(channel_def) {
            channels.push((channel.to_string(), channel_def["condition"].clone()));
        }
    });

    for (channel, channel_def) in channels {
        add(&channel_def, &channel, &mut tuples, &mut to_skip);
    }

    let mut out = Vec::new();
    let mut keys: Vec<String> = Vec::new();
    for tuple in tuples {
        if !to_skip.contains(&tuple.channel) && !keys.contains(&tuple.key) {
            keys.push(tuple.key.clone());
            out.push(tuple);
        }
    }
    out
}

fn tooltip_filter_expression(
    field_def: &Value,
    channel: &str,
    expr: &str,
) -> Result<String, ()> {
    if channel != "tooltip" || !has_property(field_def, "filter") {
        return Err(());
    }

    let filter = &field_def["filter"];

    let aggregate_value = field_def.get("aggregate").cloned().unwrap_or(Value::Null);
    let aggregate = aggregate_value.as_str().unwrap_or("");
    if aggregate == "argmin"
        || aggregate == "argmax"
        || aggregate_value.get("argmin").is_some()
        || aggregate_value.get("argmax").is_some()
        || (field_def.get("field").map(|f| f.is_null()).unwrap_or(true) && !is_count(field_def))
    {
        crate::log::warn(
            "Ignoring tooltip filter because it requires a field (argmin and argmax fields are not supported).",
        );
        return Err(());
    }

    let time_unit = normalize_time_unit(&field_def["timeUnit"]);
    let mut bound_field_def = json!({"field": vg_field(field_def)});
    if let Some(time_unit) = time_unit {
        let mut tu = Map::new();
        if let Some(unit) = time_unit.unit {
            tu.insert("unit".to_string(), Value::String(unit));
        }
        tu.insert("binned".to_string(), Value::Bool(true));
        bound_field_def["timeUnit"] = Value::Object(tu);
    }

    let mut valid = true;
    let mut leaf_count = 0;
    let test = logical_expr(filter, &mut |predicate: &Value| {
        leaf_count += 1;
        if predicate.is_object() {
            let mut field_predicate = predicate.as_object().cloned().unwrap_or_default();
            if let Some(bound) = bound_field_def.as_object() {
                for (k, v) in bound {
                    field_predicate.insert(k.clone(), v.clone());
                }
            }
            let field_predicate = Value::Object(field_predicate);
            if is_field_predicate(&field_predicate) || is_field_valid_predicate(&field_predicate) {
                return field_filter_expression_with_options(&field_predicate, true, expr);
            }
        }
        valid = false;
        String::new()
    });

    if valid && leaf_count > 0 {
        Ok(test)
    } else {
        crate::log::warn(format!(
            "Ignoring an invalid tooltip filter: {}.",
            stringify(filter)
        ));
        Err(())
    }
}

pub fn tooltip_ref_for_encoding(
    encoding: &Value,
    stack: Option<&Value>,
    config: &Value,
    reactive_geom: bool,
) -> Option<Value> {
    let tuples = tooltip_data_tuples(encoding, stack, config, reactive_geom);
    if tuples.is_empty() {
        return None;
    }

    let object_expr = |t: &TooltipTuple| format!("{{\"{}\": {}}}", t.key, t.value);
    if tuples.iter().any(|t| t.test.is_some()) {
        if tuples.len() == 1 {
            let t = &tuples[0];
            return Some(json!({"signal": format!(
                "({}) ? {} : null",
                t.test.as_deref().unwrap_or(""),
                object_expr(t)
            )}));
        }

        let key_values: Vec<String> = tuples
            .iter()
            .map(|t| match &t.test {
                Some(test) => format!("({test}) ? {} : {{}}", object_expr(t)),
                None => object_expr(t),
            })
            .collect();
        let value = format!("merge({})", key_values.join(", "));

        if tuples.iter().all(|t| t.test.is_some()) {
            let any_visible = tuples
                .iter()
                .map(|t| format!("({})", t.test.as_deref().unwrap_or("")))
                .collect::<Vec<_>>()
                .join(" || ");
            return Some(json!({"signal": format!("({any_visible}) ? {value} : null")}));
        }
        return Some(json!({"signal": value}));
    }

    let key_values: Vec<String> = tuples
        .iter()
        .map(|t| format!("\"{}\": {}", t.key, t.value))
        .collect();
    Some(json!({"signal": format!("{{{}}}", key_values.join(", "))}))
}

fn add_line_breaks_to_tooltip(channel_def: &Value, config: &Value, expr: &str) -> Option<Value> {
    let discrete_type = matches!(
        channel_def.get("type").and_then(Value::as_str),
        Some("nominal") | Some("ordinal")
    );
    if is_field_def(channel_def)
        && discrete_type
        && channel_def.get("timeUnit").map(|v| v.is_null()).unwrap_or(true)
        && get_format_mixins(channel_def).get("format").map(|v| v.is_null()).unwrap_or(true)
        && get_format_mixins(channel_def)
            .get("formatType")
            .map(|v| v.is_null())
            .unwrap_or(true)
    {
        let field_string = format!("{expr}[\"{}\"]", channel_def["field"].as_str().unwrap_or(""));
        return Some(json!({"signal": format!(
            "isValid({field_string}) ? isArray({field_string}) ? join({field_string}, '\\n') : {field_string} : \"\"+{field_string}"
        )}));
    }

    text_ref(channel_def, config, expr)
}

#[allow(dead_code)]
fn _unused(v: &Value) -> Value {
    if truthy(v) {
        signal_or_value_ref(v)
    } else {
        Value::Null
    }
}

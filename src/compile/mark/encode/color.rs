//! Port of `src/compile/mark/encode/color.ts`.

use serde_json::{json, Map, Value};

use crate::channeldef_support::truthy;
use crate::compile::model::Model;
use crate::compile_common::signal_or_value_ref;
use crate::mark::get_mark_style_config;
use crate::util::contains;

use super::nonposition::{non_position, NonPositionOpt};

pub fn color(model: &Model, filled_override: Option<bool>) -> Value {
    let mark_def = &model.unit.mark_def;
    let encoding = &model.unit.encoding;
    let config = &model.config;
    let mark_type = mark_def.get("type").and_then(Value::as_str).unwrap_or("");

    let filled = filled_override.unwrap_or_else(|| {
        get_mark_prop_or_config_fixed("filled", mark_def, config, None)
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    });

    let transparent_if_needed = if contains(
        &["bar", "point", "circle", "square", "geoshape"],
        &mark_type,
    ) {
        Some("transparent")
    } else {
        None
    };

    let default_fill = if filled {
        get_mark_prop_or_config_fixed("color", mark_def, config, Some("fill"))
            .or_else(|| config.get("mark").and_then(|m| m.get("color")).cloned())
    } else {
        None
    }
    .or_else(|| transparent_if_needed.map(|s| Value::String(s.to_string())));

    let default_stroke = if !filled {
        get_mark_prop_or_config_fixed("color", mark_def, config, Some("stroke"))
            .or_else(|| config.get("mark").and_then(|m| m.get("color")).cloned())
    } else {
        None
    };

    let color_vg_channel = if filled { "fill" } else { "stroke" };

    let mut out = Map::new();
    if let Some(default_fill) = &default_fill {
        if truthy(default_fill) {
            out.insert("fill".to_string(), signal_or_value_ref(default_fill));
        }
    }
    if let Some(default_stroke) = &default_stroke {
        if truthy(default_stroke) {
            out.insert("stroke".to_string(), signal_or_value_ref(default_stroke));
        }
    }

    if mark_def.get("color").map(truthy).unwrap_or(false)
        && (if filled {
            mark_def.get("fill").map(truthy).unwrap_or(false)
        } else {
            mark_def.get("stroke").map(truthy).unwrap_or(false)
        })
    {
        crate::log::warn(crate::encoding::dropping_color(
            "property",
            mark_def.get("fill").is_some(),
            mark_def.get("stroke").is_some(),
        ));
    }

    merge(
        &mut out,
        non_position(
            "color",
            model,
            NonPositionOpt {
                vg_channel: Some(color_vg_channel.to_string()),
                default_value: if filled {
                    default_fill.clone()
                } else {
                    default_stroke.clone()
                },
                ..Default::default()
            },
        ),
    );
    merge(
        &mut out,
        non_position(
            "fill",
            model,
            NonPositionOpt {
                default_value: if encoding.get("fill").is_some() {
                    default_fill.clone()
                } else {
                    None
                },
                ..Default::default()
            },
        ),
    );
    merge(
        &mut out,
        non_position(
            "stroke",
            model,
            NonPositionOpt {
                default_value: if encoding.get("stroke").is_some() {
                    default_stroke.clone()
                } else {
                    None
                },
                ..Default::default()
            },
        ),
    );

    Value::Object(out)
}

fn get_mark_prop_or_config_fixed(
    channel: &str,
    mark: &Value,
    config: &Value,
    vg_channel: Option<&str>,
) -> Option<Value> {
    if let Some(vg) = vg_channel {
        if let Some(v) = mark.get(vg).filter(|v| !v.is_null()) {
            return Some(v.clone());
        }
    }
    if let Some(v) = mark.get(channel).filter(|v| !v.is_null()) {
        return Some(v.clone());
    }

    let style = config.get("style").unwrap_or(&Value::Null);
    let style_value = get_mark_style_config(channel, mark, style);
    let mark_type = mark.get("type").and_then(Value::as_str).unwrap_or("");
    let mark_specific = config.get(mark_type).unwrap_or(&Value::Null);
    let general = config.get("mark").unwrap_or(&Value::Null);
    let vg = vg_channel.unwrap_or(channel);

    [
        style_value,
        mark_specific.get(vg).filter(|v| !v.is_null()).cloned(),
        mark_specific.get(channel).filter(|v| !v.is_null()).cloned(),
        if vg_channel.is_some() {
            general.get(vg).filter(|v| !v.is_null()).cloned()
        } else {
            None
        },
        if vg_channel.is_some() {
            None
        } else {
            general.get(channel).filter(|v| !v.is_null()).cloned()
        },
    ]
    .into_iter()
    .find_map(|v| v)
}

fn merge(dest: &mut Map<String, Value>, src: Value) {
    if let Value::Object(src) = src {
        for (k, v) in src {
            dest.insert(k, v);
        }
    }
}

#[allow(dead_code)]
fn _unused() -> Value {
    json!(null)
}

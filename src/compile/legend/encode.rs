//! Port of `src/compile/legend/encode.ts`.

use serde_json::{json, Map, Value};

use crate::channel::OPACITY;
use crate::channeldef::{
    has_conditional_value_def, is_field_def, is_value_def, vg_field,
};
use crate::mark::{get_mark_config, FILL_STROKE_CONFIG};
use crate::util::{array, get_first_defined, has_property, string_value};

use super::component::LegendComponent;

/// Port of `LegendEncodeParams`.
pub struct LegendEncodeParams<'a> {
    pub field_or_datum_def: &'a Value,
    pub model: &'a crate::compile::model::Model,
    pub channel: &'a str,
    pub legend_cmpt: &'a LegendComponent,
    pub legend_type: &'a str,
}

/// Port of `legendEncodeRules`; returns `None` when the rule returns
/// `undefined`.
pub fn legend_encode_rule(
    part: &str,
    spec: &Map<String, Value>,
    params: &LegendEncodeParams,
) -> Option<Map<String, Value>> {
    match part {
        "symbols" => symbols(spec, params),
        "gradient" => gradient(spec, params),
        "labels" => labels(spec, params),
        "entries" => Some(entries(spec, params)),
        _ => Some(spec.clone()),
    }
}

fn legend_config(model: &crate::compile::model::Model) -> &Value {
    model.config.get("legend").unwrap_or(&Value::Null)
}

/// Port of `applyMarkConfig` (limited to the fill/stroke config list).
fn apply_mark_config(
    mut entry: Map<String, Value>,
    model: &crate::compile::model::Model,
    props: &[&str],
) -> Map<String, Value> {
    let mark_def = model.mark_def().cloned().unwrap_or(Value::Null);
    for property in props {
        if let Some(value) = get_mark_config(property, &mark_def, &model.config, None) {
            entry.insert(
                (*property).to_string(),
                crate::compile_common::signal_or_value_ref(&value),
            );
        }
    }
    entry
}

fn empty_map() -> Map<String, Value> {
    Map::new()
}

fn is_empty_map(map: &Map<String, Value>) -> bool {
    map.is_empty()
}

/// Port of `color` from `src/compile/mark/encode/color.ts`, restricted to the
/// properties needed by legend symbols.
pub fn color(model: &crate::compile::model::Model, filled_opt: Option<bool>) -> Map<String, Value> {
    let null_value = Value::Null;
    let mark_def = model.mark_def().unwrap_or(&null_value);
    let encoding = model.encoding().unwrap_or(&null_value);
    let config = &model.config;
    let mark_type = mark_def
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let filled = filled_opt.or_else(|| {
        crate::mark::get_mark_prop_or_config("filled", mark_def, config, None, false)
            .and_then(|v| v.as_bool())
    });

    let transparent_if_needed = if ["bar", "point", "circle", "square", "geoshape"]
        .contains(&mark_type.as_str())
    {
        Some(Value::String("transparent".to_string()))
    } else {
        None
    };

    let config_mark = config.get("mark").unwrap_or(&null_value);

    let mut default_fill = if filled == Some(true) {
        crate::mark::get_mark_prop_or_config("color", mark_def, config, Some("fill"), false)
    } else {
        mark_def
            .get("fill")
            .cloned()
            .or_else(|| config.get(&mark_type).and_then(|c| c.get("fill")).cloned())
            .or_else(|| config_mark.get("fill").cloned())
    };
    if default_fill.is_none() && filled == Some(true) {
        default_fill = config_mark.get("color").cloned();
    }
    if default_fill.is_none() {
        default_fill = transparent_if_needed;
    }

    let default_stroke = if filled == Some(false) {
        crate::mark::get_mark_prop_or_config("color", mark_def, config, Some("stroke"), false)
    } else {
        mark_def
            .get("stroke")
            .cloned()
            .or_else(|| config.get(&mark_type).and_then(|c| c.get("stroke")).cloned())
            .or_else(|| config_mark.get("stroke").cloned())
    };
    let default_stroke = if default_stroke.is_none() && filled == Some(false) {
        config_mark.get("color").cloned()
    } else {
        default_stroke
    };

    let color_vg_channel = if filled == Some(true) { "fill" } else { "stroke" };

    let mut out = Map::new();
    if let Some(default_fill) = &default_fill {
        out.insert(
            "fill".to_string(),
            crate::compile_common::signal_or_value_ref(default_fill),
        );
    }
    if let Some(default_stroke) = &default_stroke {
        out.insert(
            "stroke".to_string(),
            crate::compile_common::signal_or_value_ref(default_stroke),
        );
    }

    if let Some(entry) = non_position(
        "color",
        model,
        color_vg_channel,
        if filled == Some(true) {
            default_fill.clone()
        } else {
            default_stroke.clone()
        },
    ) {
        for (key, value) in entry {
            out.insert(key, value);
        }
    }
    if let Some(entry) = non_position(
        "fill",
        model,
        "fill",
        if encoding.get("fill").is_some() {
            default_fill.clone()
        } else {
            None
        },
    ) {
        for (key, value) in entry {
            out.insert(key, value);
        }
    }
    if let Some(entry) = non_position(
        "stroke",
        model,
        "stroke",
        if encoding.get("stroke").is_some() {
            default_stroke.clone()
        } else {
            None
        },
    ) {
        for (key, value) in entry {
            out.insert(key, value);
        }
    }

    out
}

/// Port of `nonPosition` for a single channel (legend encoding only).
fn non_position(
    channel: &str,
    model: &crate::compile::model::Model,
    vg_channel: &str,
    default_value: Option<Value>,
) -> Option<Map<String, Value>> {
    let null_value = Value::Null;
    let encoding = model.encoding().unwrap_or(&null_value);
    let channel_def = encoding.get(channel);

    let default_ref = default_value
        .as_ref()
        .map(crate::compile_common::signal_or_value_ref);

    let reference = |def: &Value| -> Option<Value> {
        if let Some(field_or_datum_def) = crate::channeldef::get_field_or_datum_def_ref(def) {
            let mut reference = Map::new();
            if let Some(scale_name) = model.scale_name(channel, false) {
                reference.insert("scale".to_string(), Value::String(scale_name));
            }
            if is_field_def(field_or_datum_def) {
                reference.insert(
                    "field".to_string(),
                    Value::String(vg_field(field_or_datum_def)),
                );
            } else if let Some(datum) = field_or_datum_def.get("datum") {
                reference.insert("value".to_string(), datum.clone());
            }
            return Some(Value::Object(reference));
        }
        if is_value_def(def) {
            return Some(crate::compile_common::signal_or_value_ref(
                def.get("value").unwrap_or(&Value::Null),
            ));
        }
        default_ref.clone()
    };

    let wrapped = crate::compile::mark::encode::conditional::wrap_condition(
        model,
        channel_def,
        vg_channel,
        None,
        reference,
    );
    match wrapped {
        Value::Object(map) if map.is_empty() => None,
        Value::Object(map) => Some(map),
        _ => None,
    }
}

/// Port of `symbols`.
pub fn symbols(
    symbols_spec: &Map<String, Value>,
    params: &LegendEncodeParams,
) -> Option<Map<String, Value>> {
    if params.legend_type != "symbol" {
        return None;
    }

    let model = params.model;
    let mark_def = model.mark_def().cloned().unwrap_or(Value::Null);
    let encoding = model.encoding().cloned().unwrap_or(Value::Null);
    let mark = model.mark_type();

    let filled = mark_def.get("filled").and_then(Value::as_bool).unwrap_or(false)
        && mark != "trail";

    let mut out = apply_mark_config(empty_map(), model, &FILL_STROKE_CONFIG);
    for (key, value) in color(model, Some(filled)) {
        out.insert(key, value);
    }

    let legend_config = legend_config(model);
    let symbol_opacity = params
        .legend_cmpt
        .get("symbolOpacity")
        .or_else(|| legend_config.get("symbolOpacity").cloned());
    let symbol_fill_color = params
        .legend_cmpt
        .get("symbolFillColor")
        .or_else(|| legend_config.get("symbolFillColor").cloned());
    let symbol_stroke_color = params
        .legend_cmpt
        .get("symbolStrokeColor")
        .or_else(|| legend_config.get("symbolStrokeColor").cloned());

    let opacity: Option<Value> = if symbol_opacity.is_none() {
        get_max_value(encoding.get("opacity").unwrap_or(&Value::Null))
            .or_else(|| mark_def.get("opacity").cloned())
    } else {
        None
    };

    if out.get("fill").is_some() {
        let fill = out.get("fill").cloned().unwrap_or(Value::Null);
        if params.channel == "fill" || (filled && params.channel == "color") {
            out.remove("fill");
        } else if has_property(&fill, "field") {
            if symbol_fill_color.is_some() {
                out.remove("fill");
            } else {
                let base_fill = legend_config
                    .get("symbolBaseFillColor")
                    .cloned()
                    .unwrap_or_else(|| Value::String("black".to_string()));
                out.insert(
                    "fill".to_string(),
                    crate::compile_common::signal_or_value_ref(&base_fill),
                );
                let fill_opacity = opacity.clone().unwrap_or(json!(1));
                out.insert(
                    "fillOpacity".to_string(),
                    crate::compile_common::signal_or_value_ref(&fill_opacity),
                );
            }
        } else if fill.is_array() {
            let fill = get_first_condition_value(
                encoding.get("fill").unwrap_or(&encoding["color"]),
            )
            .or_else(|| mark_def.get("fill").cloned())
            .or_else(|| {
                if filled {
                    mark_def.get("color").cloned()
                } else {
                    None
                }
            });
            if let Some(fill) = fill {
                out.insert(
                    "fill".to_string(),
                    crate::compile_common::signal_or_value_ref(&fill),
                );
            }
        }
    }

    if out.get("stroke").is_some() {
        let stroke = out.get("stroke").cloned().unwrap_or(Value::Null);
        if params.channel == "stroke"
            || (!filled && params.channel == "color")
            || has_property(&stroke, "field")
            || symbol_stroke_color.is_some()
        {
            out.remove("stroke");
        } else if stroke.is_array() {
            let stroke = get_first_condition_value(
                encoding.get("stroke").unwrap_or(&encoding["color"]),
            )
            .or_else(|| mark_def.get("stroke").cloned())
            .or_else(|| {
                if filled {
                    mark_def.get("color").cloned()
                } else {
                    None
                }
            });
            if let Some(stroke) = stroke {
                out.insert(
                    "stroke".to_string(),
                    crate::compile_common::signal_or_value_ref(&stroke),
                );
            }
        }
    }

    if params.channel != OPACITY {
        let condition = if is_field_def(params.field_or_datum_def) {
            selected_condition(model, params.legend_cmpt, params.field_or_datum_def)
        } else {
            None
        };

        if let Some(condition) = condition {
            let mut first = Map::new();
            first.insert("test".to_string(), Value::String(condition));
            if let Value::Object(refs) =
                crate::compile_common::signal_or_value_ref(&opacity.clone().unwrap_or(json!(1)))
            {
                for (k, v) in refs {
                    first.insert(k, v);
                }
            }
            let second = crate::compile_common::signal_or_value_ref(
                legend_config
                    .get("unselectedOpacity")
                    .unwrap_or(&Value::Null),
            );
            out.insert(
                "opacity".to_string(),
                Value::Array(vec![Value::Object(first), second]),
            );
        } else if let Some(opacity) = &opacity {
            if crate::channeldef_support::truthy(opacity) {
                out.insert(
                    "opacity".to_string(),
                    crate::compile_common::signal_or_value_ref(opacity),
                );
            }
        }
    }

    for (key, value) in symbols_spec {
        out.insert(key.clone(), value.clone());
    }

    if is_empty_map(&out) {
        None
    } else {
        Some(out)
    }
}

/// Port of `gradient`.
pub fn gradient(
    gradient_spec: &Map<String, Value>,
    params: &LegendEncodeParams,
) -> Option<Map<String, Value>> {
    if params.legend_type != "gradient" {
        return None;
    }

    let model = params.model;
    let mark_def = model.mark_def().cloned().unwrap_or(Value::Null);
    let encoding = model.encoding().cloned().unwrap_or(Value::Null);
    let legend_config = legend_config(model);

    let mut out = Map::new();
    let gradient_opacity = params
        .legend_cmpt
        .get("gradientOpacity")
        .or_else(|| legend_config.get("gradientOpacity").cloned());
    let opacity = if gradient_opacity.is_none() {
        let max_value = get_max_value(encoding.get("opacity").unwrap_or(&Value::Null));
        match max_value {
            Some(v) if crate::channeldef_support::truthy(&v) => Some(v),
            _ => mark_def.get("opacity").cloned(),
        }
    } else {
        None
    };
    if let Some(opacity) = opacity {
        if crate::channeldef_support::truthy(&opacity) {
            out.insert(
                "opacity".to_string(),
                crate::compile_common::signal_or_value_ref(&opacity),
            );
        }
    }

    for (key, value) in gradient_spec {
        out.insert(key.clone(), value.clone());
    }

    if is_empty_map(&out) {
        None
    } else {
        Some(out)
    }
}

/// Port of `labels`.
pub fn labels(
    specified_labels_spec: &Map<String, Value>,
    params: &LegendEncodeParams,
) -> Option<Map<String, Value>> {
    let model = params.model;
    let legend = super::parse::model_legend(model, params.channel).unwrap_or(Value::Null);
    let config = &model.config;

    let condition = if is_field_def(params.field_or_datum_def) {
        selected_condition(model, params.legend_cmpt, params.field_or_datum_def)
    } else {
        None
    };
    let opacity = condition.map(|condition| {
        let unselected = config
            .get("legend")
            .and_then(|l| l.get("unselectedOpacity"))
            .cloned()
            .unwrap_or(Value::Null);
        json!([
            {"test": condition, "value": 1},
            {"value": unselected}
        ])
    });

    let format = legend.get("format").filter(|v| !v.is_null());
    let format_type = legend.get("formatType").filter(|v| !v.is_null());

    let mut text: Option<Value> = None;

    if let Some(ft) = format_type.and_then(Value::as_str) {
        if super::format::is_custom_format_type(ft) {
            text = Some(super::format::format_custom_type(
                params.field_or_datum_def,
                format,
                ft,
                config,
                Some("datum.value"),
            ));
        }
    } else if format.is_none()
        && config.get("customFormatTypes").and_then(Value::as_bool) == Some(true)
    {
        let type_ = params
            .field_or_datum_def
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("");
        if type_ == "quantitative" {
            if let Some(number_format_type) =
                config.get("numberFormatType").and_then(Value::as_str)
            {
                text = Some(super::format::format_custom_type(
                    params.field_or_datum_def,
                    config.get("numberFormat"),
                    number_format_type,
                    config,
                    Some("datum.value"),
                ));
            }
        } else if type_ == "temporal"
            && config.get("timeFormatType").is_some()
            && is_field_def(params.field_or_datum_def)
            && params.field_or_datum_def.get("timeUnit").is_none()
        {
            if let Some(time_format_type) = config.get("timeFormatType").and_then(Value::as_str) {
                text = Some(super::format::format_custom_type(
                    params.field_or_datum_def,
                    config.get("timeFormat"),
                    time_format_type,
                    config,
                    Some("datum.value"),
                ));
            }
        }
    }

    let mut labels_spec = Map::new();
    if let Some(opacity) = opacity {
        labels_spec.insert("opacity".to_string(), opacity);
    }
    if let Some(text) = text {
        labels_spec.insert("text".to_string(), text);
    }
    for (key, value) in specified_labels_spec {
        labels_spec.insert(key.clone(), value.clone());
    }

    if is_empty_map(&labels_spec) {
        None
    } else {
        Some(labels_spec)
    }
}

/// Port of `entries`.
pub fn entries(
    entries_spec: &Map<String, Value>,
    params: &LegendEncodeParams,
) -> Map<String, Value> {
    let selections = params.legend_cmpt.get("selections");
    let has_selections = selections
        .and_then(|s| s.as_array().map(|a| !a.is_empty()))
        .unwrap_or(false);
    let mut out = entries_spec.clone();
    if has_selections {
        out.insert("fill".to_string(), json!({"value": "transparent"}));
    }
    out
}

fn get_max_value(channel_def: &Value) -> Option<Value> {
    get_condition_value(channel_def, |v, conditional_def| {
        let a = v.and_then(Value::as_f64).unwrap_or(f64::NEG_INFINITY);
        let b = conditional_def
            .get("value")
            .and_then(Value::as_f64)
            .unwrap_or(f64::NEG_INFINITY);
        Some(if b >= a {
            conditional_def.get("value").cloned().unwrap_or(Value::Null)
        } else {
            v.cloned().unwrap_or(Value::Null)
        })
    })
}

/// Port of `getFirstConditionValue`.
pub fn get_first_condition_value(channel_def: &Value) -> Option<Value> {
    get_condition_value(channel_def, |v, conditional_def| {
        get_first_defined(&[v, conditional_def.get("value")]).cloned()
    })
}

fn get_condition_value<F>(channel_def: &Value, reducer: F) -> Option<Value>
where
    F: Fn(Option<&Value>, &Value) -> Option<Value> + Copy,
{
    if has_conditional_value_def(channel_def) {
        let mut acc = channel_def.get("value").cloned();
        let condition = channel_def.get("condition").cloned().unwrap_or(Value::Null);
        for conditional_def in array(&condition) {
            acc = reducer(acc.as_ref(), &conditional_def);
        }
        acc
    } else if is_value_def(channel_def) {
        channel_def.get("value").cloned()
    } else {
        None
    }
}

fn selected_condition(
    _model: &crate::compile::model::Model,
    legend_cmpt: &LegendComponent,
    field_def: &Value,
) -> Option<String> {
    let selections = legend_cmpt.get("selections")?;
    let selections = selections.as_array()?;
    if selections.is_empty() {
        return None;
    }

    let field = string_value(field_def.get("field").unwrap_or(&Value::Null));
    let conditions: Vec<String> = selections
        .iter()
        .filter_map(Value::as_str)
        .map(|name| {
            let store = string_value(&Value::String(format!(
                "{}{}",
                crate::util::var_name(name),
                crate::compile::selection::STORE
            )));
            format!(
                "(!length(data({store})) || ({name}[{field}] && indexof({name}[{field}], datum.value) >= 0))"
            )
        })
        .collect();
    Some(conditions.join(" || "))
}

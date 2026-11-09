//! Port of `src/compile/legend/properties.ts`.

use serde_json::{json, Value};

use crate::channel::is_color_channel;
use crate::channeldef::{default_title, is_field_def, value_array};
use crate::scale::{is_continuous_to_continuous, ScaleType};
use crate::util::{contains, get_first_defined, has_property};

use super::encode::get_first_condition_value;
use super::format::{guide_format, guide_format_type};

/// Port of `LegendRuleParams`.
pub struct LegendRuleParams<'a> {
    pub legend: &'a Value,
    pub channel: &'a str,
    pub model: &'a crate::compile::model::Model,
    pub mark_def: &'a Value,
    pub encoding: &'a Value,
    pub field_or_datum_def: &'a Value,
    pub legend_config: &'a Value,
    pub config: &'a Value,
    pub scale_type: Option<ScaleType>,
    pub orient: Option<String>,
    pub legend_type: &'a str,
    pub direction: Option<String>,
}

fn prop<'a>(object: &'a Value, key: &str) -> Option<&'a Value> {
    object.get(key).filter(|v| !v.is_null())
}

fn string_prop(object: &Value, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Port of `legendRules`. Returns `None` when the property is not handled by a
/// rule (the caller then falls back to the specified legend property).
pub fn legend_rules(property: &str, params: &LegendRuleParams) -> Option<Value> {
    match property {
        "direction" => params.direction.clone().map(Value::String),
        "format" => {
            let format = prop(params.legend, "format");
            let format_type = prop(params.legend, "formatType");
            guide_format(
                params.field_or_datum_def,
                params
                    .field_or_datum_def
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
                format,
                format_type,
                params.config,
                false,
            )
        }
        "formatType" => {
            let format_type = prop(params.legend, "formatType");
            guide_format_type(
                format_type,
                params.field_or_datum_def,
                params.scale_type,
            )
        }
        "gradientLength" => prop(params.legend, "gradientLength")
            .or_else(|| prop(params.legend_config, "gradientLength"))
            .cloned()
            .or_else(|| default_gradient_length(params)),
        "labelOverlap" => prop(params.legend, "labelOverlap")
            .or_else(|| prop(params.legend_config, "labelOverlap"))
            .cloned()
            .or_else(|| {
                params.scale_type.and_then(default_label_overlap)
            }),
        "symbolType" => prop(params.legend, "symbolType").cloned().or_else(|| {
            default_symbol_type(
                params
                    .mark_def
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
                params.channel,
                params.encoding.get("shape"),
                params.mark_def.get("shape"),
            )
        }),
        "title" => legend_title(params.field_or_datum_def, params.config),
        "type" => {
            let legend_type = params.legend_type;
            if is_color_channel(params.channel)
                && params
                    .scale_type
                    .map(is_continuous_to_continuous)
                    .unwrap_or(false)
            {
                if legend_type == "gradient" {
                    return None;
                }
            } else if legend_type == "symbol" {
                return None;
            }
            Some(Value::String(legend_type.to_string()))
        }
        "values" => values(params.legend, params.field_or_datum_def),
        _ => None,
    }
}

/// Port of `title` from `channeldef.ts` for a legend guide.
fn legend_title(field_or_datum_def: &Value, config: &Value) -> Option<Value> {
    let legend = field_or_datum_def
        .get("legend")
        .filter(|legend| crate::channeldef_support::truthy(legend));
    let guide_title = legend.and_then(|legend| legend.get("title"));

    if !is_field_def(field_or_datum_def) {
        return guide_title
            .cloned()
            .or_else(|| field_or_datum_def.get("title").cloned());
    }

    let field_title = field_or_datum_def.get("title");
    let default = Value::String(default_title(field_or_datum_def, config));

    get_first_defined(&[guide_title, field_title, Some(&default)]).cloned()
}

/// Port of `values`.
pub fn values(legend: &Value, field_or_datum_def: &Value) -> Option<Value> {
    let vals = legend.get("values").filter(|v| !v.is_null())?;
    if let Value::Array(arr) = vals {
        Some(Value::Array(value_array(field_or_datum_def, arr)))
    } else if has_property(vals, "signal") {
        Some(vals.clone())
    } else {
        None
    }
}

/// Port of `defaultSymbolType`.
pub fn default_symbol_type(
    mark: &str,
    channel: &str,
    shape_channel_def: Option<&Value>,
    mark_shape: Option<&Value>,
) -> Option<Value> {
    if channel != "shape" {
        let shape = shape_channel_def
            .and_then(get_first_condition_value)
            .or_else(|| mark_shape.cloned());
        if let Some(shape) = shape {
            return Some(shape);
        }
    }

    let symbol = match mark {
        "bar" | "rect" | "image" | "square" => "square",
        "line" | "trail" | "rule" => "stroke",
        "arc" | "point" | "circle" | "tick" | "geoshape" | "area" | "text" => "circle",
        _ => return None,
    };
    Some(Value::String(symbol.to_string()))
}

/// Port of `clipHeight`.
pub fn clip_height(legend_type: &str) -> Option<Value> {
    if legend_type == "gradient" {
        Some(json!(20))
    } else {
        None
    }
}

/// Port of `getLegendType`.
pub fn get_legend_type(
    legend: &Value,
    channel: &str,
    time_unit: Option<&str>,
    scale_type: Option<ScaleType>,
) -> String {
    get_first_defined(&[
        legend.get("type").filter(|v| !v.is_null()),
        Some(&Value::String(default_type(channel, time_unit, scale_type))),
    ])
    .and_then(Value::as_str)
    .unwrap_or("symbol")
    .to_string()
}

/// Port of `defaultType`.
pub fn default_type(
    channel: &str,
    time_unit: Option<&str>,
    scale_type: Option<ScaleType>,
) -> String {
    if is_color_channel(channel) {
        if let Some(time_unit) = time_unit {
            if contains(&["quarter", "month", "day"], &time_unit) {
                return "symbol".to_string();
            }
        }
        if scale_type.map(is_continuous_to_continuous).unwrap_or(false) {
            return "gradient".to_string();
        }
    }
    "symbol".to_string()
}

/// Port of `getDirection`.
pub fn get_direction(
    legend_config: &Value,
    legend_type: &str,
    orient: Option<&str>,
    legend: &Value,
) -> Option<String> {
    prop(legend, "direction")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            // Upstream reads `legendType ? 'gradientDirection' : 'symbolDirection'`;
            // a non-empty legend type is always truthy so this is always the
            // gradient direction.
            let _ = legend_type;
            string_prop(legend_config, "gradientDirection")
        })
        .or_else(|| default_direction(orient, legend_type))
}

/// Port of `defaultDirection`.
pub fn default_direction(orient: Option<&str>, legend_type: &str) -> Option<String> {
    match orient {
        Some("top") | Some("bottom") => Some("horizontal".to_string()),
        Some("left") | Some("right") | Some("none") | None => None,
        // top-left / ... inner legend uses a compact layout.
        Some(_) => {
            if legend_type == "gradient" {
                Some("horizontal".to_string())
            } else {
                None
            }
        }
    }
}

/// Port of `defaultGradientLength`.
pub fn default_gradient_length(params: &LegendRuleParams) -> Option<Value> {
    let legend_config = params.legend_config;
    let gradient_horizontal_max_length = legend_config.get("gradientHorizontalMaxLength");
    let gradient_horizontal_min_length = legend_config.get("gradientHorizontalMinLength");
    let gradient_vertical_max_length = legend_config.get("gradientVerticalMaxLength");
    let gradient_vertical_min_length = legend_config.get("gradientVerticalMinLength");

    if !params.scale_type.map(is_continuous_to_continuous).unwrap_or(false) {
        return None;
    }

    if params.direction.as_deref() == Some("horizontal") {
        if params.orient.as_deref() == Some("top") || params.orient.as_deref() == Some("bottom") {
            gradient_length_signal(
                params.model,
                "width",
                gradient_horizontal_min_length,
                gradient_horizontal_max_length,
            )
        } else {
            gradient_horizontal_min_length.cloned()
        }
    } else {
        // vertical / undefined (Vega uses vertical by default)
        gradient_length_signal(
            params.model,
            "height",
            gradient_vertical_min_length,
            gradient_vertical_max_length,
        )
    }
}

fn gradient_length_signal(
    model: &crate::compile::model::Model,
    size_type: &str,
    min: Option<&Value>,
    max: Option<&Value>,
) -> Option<Value> {
    let size_signal = model
        .get_size_signal_ref(size_type)
        .get("signal")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let min = min.cloned().unwrap_or(Value::Null);
    let max = max.cloned().unwrap_or(Value::Null);
    Some(json!({"signal": format!(
        "clamp({}, {}, {})",
        size_signal,
        crate::util::stringify(&min),
        crate::util::stringify(&max)
    )}))
}

/// Port of `defaultLabelOverlap`.
pub fn default_label_overlap(scale_type: ScaleType) -> Option<Value> {
    if contains(
        &[
            ScaleType::Quantile,
            ScaleType::Threshold,
            ScaleType::Log,
            ScaleType::Symlog,
        ],
        &scale_type,
    ) {
        Some(Value::String("greedy".to_string()))
    } else {
        None
    }
}

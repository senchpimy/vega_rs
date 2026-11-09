//! Port of `src/compile/mark/encode/valueref.ts`.

use serde_json::{json, Map, Value};

use crate::channel::{get_main_range_channel, X, X2, Y2};
use crate::channeldef::{
    bin_requires_range, get_band_position_pub, is_datum_def, is_field_def, is_field_or_datum_def,
    is_typed_field_def, is_value_def, vg_field_with, FieldRefOption,
};
use crate::channeldef_support::{has_discrete_domain, is_binned, is_binning, truthy};
use crate::compile::scale::component::ScaleComponent;
use crate::datetime::{date_time_to_expr, is_date_time};
use crate::expr::is_expr_ref;
use crate::util::{contains, stringify};

use super::invalid::get_conditional_value_ref_for_including_invalid_value;

pub fn is_signal_ref(v: &Value) -> bool {
    v.as_object()
        .map(|m| m.get("signal").map(|s| !s.is_null()).unwrap_or(false))
        .unwrap_or(false)
}

/// Port of the `Offset` used by `valueRefForFieldOrDatumDef`.
#[derive(Debug, Clone, Default)]
pub struct ValueRefEncode {
    pub offset: Option<Value>,
    pub band: Option<Value>,
}

impl ValueRefEncode {
    pub fn offset(offset: Option<Value>) -> Self {
        ValueRefEncode {
            offset,
            band: None,
        }
    }
}

pub fn datum_def_to_expr(datum_def: &Value) -> String {
    let datum = &datum_def["datum"];
    if is_date_time(datum) {
        date_time_to_expr(datum)
    } else {
        stringify(datum)
    }
}

pub fn value_ref_for_field_or_datum_def(
    field_def: &Value,
    scale_name: &str,
    opt: &FieldRefOption,
    encode: &ValueRefEncode,
) -> Value {
    let mut map = Map::new();

    if !scale_name.is_empty() {
        map.insert("scale".to_string(), Value::String(scale_name.to_string()));
    }

    if is_datum_def(field_def) {
        let datum = &field_def["datum"];
        if is_date_time(datum) {
            map.insert(
                "signal".to_string(),
                Value::String(date_time_to_expr(datum)),
            );
        } else if is_signal_ref(datum) {
            map.insert("signal".to_string(), datum["signal"].clone());
        } else if is_expr_ref(datum) {
            map.insert("signal".to_string(), datum["expr"].clone());
        } else {
            map.insert("value".to_string(), datum.clone());
        }
    } else {
        map.insert(
            "field".to_string(),
            Value::String(vg_field_with(field_def, opt)),
        );
    }

    if let Some(offset) = &encode.offset {
        if truthy(offset) {
            map.insert("offset".to_string(), offset.clone());
        }
    }
    if let Some(band) = &encode.band {
        if truthy(band) {
            map.insert("band".to_string(), band.clone());
        }
    }
    Value::Object(map)
}

pub fn interpolated_signal_ref(
    scale_name: &str,
    field_or_datum_def: &Value,
    field_or_datum_def2: Option<&Value>,
    offset: Option<&Value>,
    start_suffix: Option<&str>,
    end_suffix: &str,
    band_position: &Value,
) -> Value {
    let expr = if !is_signal_ref(band_position) {
        match band_position.as_f64() {
            Some(b) if b > 0.0 && b < 1.0 => Some("datum"),
            _ => None,
        }
    } else {
        None
    };

    let start = vg_field_with(
        field_or_datum_def,
        &FieldRefOption {
            expr: expr.map(str::to_string),
            suffix: start_suffix.map(str::to_string),
            ..Default::default()
        },
    );
    let end = match field_or_datum_def2 {
        Some(def2) => vg_field_with(
            def2,
            &FieldRefOption {
                expr: expr.map(str::to_string),
                ..Default::default()
            },
        ),
        None => vg_field_with(
            field_or_datum_def,
            &FieldRefOption {
                suffix: Some(end_suffix.to_string()),
                expr: expr.map(str::to_string),
                ..Default::default()
            },
        ),
    };

    let mut map = Map::new();
    let bp = band_position.as_f64();
    if bp == Some(0.0) || bp == Some(1.0) {
        map.insert("scale".to_string(), Value::String(scale_name.to_string()));
        let field = if bp == Some(0.0) { start } else { end };
        map.insert("field".to_string(), Value::String(field));
    } else {
        let datum = if is_signal_ref(band_position) {
            let signal = band_position["signal"].as_str().unwrap_or("");
            format!("(1-{signal}) * {start} + {signal} * {end}")
        } else {
            let b = bp.unwrap_or(0.5);
            format!("{} * {} + {} * {}", 1.0 - b, start, b, end)
        };
        map.insert(
            "signal".to_string(),
            Value::String(format!("scale(\"{scale_name}\", {datum})")),
        );
    }

    if let Some(offset) = offset {
        if truthy(offset) {
            map.insert("offset".to_string(), offset.clone());
        }
    }
    Value::Object(map)
}

pub fn bin_size_expr(scale_name: &str, field_def: &Value) -> String {
    let start = vg_field_with(
        field_def,
        &FieldRefOption {
            expr: Some("datum".to_string()),
            ..Default::default()
        },
    );
    let end = vg_field_with(
        field_def,
        &FieldRefOption {
            expr: Some("datum".to_string()),
            suffix: Some("end".to_string()),
            ..Default::default()
        },
    );
    format!("abs(scale(\"{scale_name}\", {end}) - scale(\"{scale_name}\", {start}))")
}

pub struct MidPointParams<'a> {
    pub channel: &'a str,
    pub channel_def: Option<&'a Value>,
    pub channel2_def: Option<&'a Value>,
    pub mark_def: &'a Value,
    pub config: &'a Value,
    pub scale_name: &'a str,
    pub scale: Option<&'a ScaleComponent>,
    pub stack: Option<&'a Value>,
    pub offset: Option<Value>,
    pub default_ref: Option<Value>,
    pub band_position: Option<Value>,
}

pub fn mid_point_ref_with_position_invalid_test(params: MidPointParams) -> Value {
    let scale_channel = get_main_range_channel(params.channel).to_string();
    let main_ref = mid_point(&params);

    let value_ref_for_including_invalid = get_conditional_value_ref_for_including_invalid_value(
        &scale_channel,
        params.channel_def,
        params.scale,
        params.scale_name,
        params.mark_def,
        params.config,
    );

    match value_ref_for_including_invalid {
        Some(invalid) => json!([invalid, main_ref]),
        None => main_ref,
    }
}

fn scale_type_of(scale: Option<&ScaleComponent>) -> Option<String> {
    scale
        .and_then(|s| s.get("type"))
        .and_then(|v| v.as_str().map(str::to_string))
}

/// Port of `midPoint`.
pub fn mid_point(params: &MidPointParams) -> Value {
    if let Some(channel_def) = params.channel_def {
        if is_field_or_datum_def(channel_def) {
            let scale_type = scale_type_of(params.scale);
            if is_typed_field_def(channel_def) {
                let band_position = params
                    .band_position
                    .clone()
                    .or_else(|| channel_def.get("bandPosition").cloned())
                    .or_else(|| {
                        get_band_position_pub(
                            channel_def,
                            params.channel2_def,
                            params.mark_def,
                            params.config,
                        )
                    });

                let bin = &channel_def["bin"];
                let time_unit = channel_def.get("timeUnit").cloned().unwrap_or(Value::Null);
                let type_ = channel_def.get("type").and_then(Value::as_str).unwrap_or("");

                if is_binning(bin)
                    || (band_position.as_ref().map(truthy).unwrap_or(false)
                        && truthy(&time_unit)
                        && type_ == "temporal")
                {
                    if params
                        .stack
                        .and_then(|s| s.get("impute"))
                        .map(truthy)
                        .unwrap_or(false)
                    {
                        return value_ref_for_field_or_datum_def(
                            channel_def,
                            params.scale_name,
                            &FieldRefOption {
                                bin_suffix: Some("mid".to_string()),
                                ..Default::default()
                            },
                            &ValueRefEncode::offset(params.offset.clone()),
                        );
                    }

                    if band_position.as_ref().map(truthy).unwrap_or(false)
                        && !scale_type
                            .as_deref()
                            .map(has_discrete_domain)
                            .unwrap_or(false)
                    {
                        return interpolated_signal_ref(
                            params.scale_name,
                            channel_def,
                            None,
                            params.offset.as_ref(),
                            None,
                            "end",
                            band_position.as_ref().unwrap(),
                        );
                    }
                    return value_ref_for_field_or_datum_def(
                        channel_def,
                        params.scale_name,
                        &FieldRefOption {
                            bin_suffix: if bin_requires_range(channel_def, params.channel) {
                                Some("range".to_string())
                            } else {
                                None
                            },
                            ..Default::default()
                        },
                        &ValueRefEncode::offset(params.offset.clone()),
                    );
                } else if is_binned(bin) {
                    if let Some(channel2) = params.channel2_def {
                        if is_field_def(channel2) {
                            return interpolated_signal_ref(
                                params.scale_name,
                                channel_def,
                                Some(channel2),
                                params.offset.as_ref(),
                                None,
                                "end",
                                band_position.as_ref().unwrap_or(&json!(0.5)),
                            );
                        }
                    }
                    let channel2 = if params.channel == X { X2 } else { Y2 };
                    crate::log::warn(format!(
                        "Channel {channel2} should be used with bin."
                    ));
                }
            }

            let opt = if scale_type
                .as_deref()
                .map(has_discrete_domain)
                .unwrap_or(false)
            {
                FieldRefOption {
                    bin_suffix: Some("range".to_string()),
                    ..Default::default()
                }
            } else {
                FieldRefOption::default()
            };

            let band = if scale_type.as_deref() == Some("band") {
                params
                    .band_position
                    .clone()
                    .or_else(|| channel_def.get("bandPosition").cloned())
                    .or(Some(json!(0.5)))
            } else {
                None
            };

            return value_ref_for_field_or_datum_def(
                channel_def,
                params.scale_name,
                &opt,
                &ValueRefEncode {
                    offset: params.offset.clone(),
                    band,
                },
            );
        } else if is_value_def(channel_def) {
            let value = &channel_def["value"];
            let mut reference =
                width_height_value_or_signal_ref(params.channel, value);
            if let Some(offset) = &params.offset {
                if truthy(offset) {
                    if let Some(map) = reference.as_object_mut() {
                        map.insert("offset".to_string(), offset.clone());
                    }
                }
            }
            return reference;
        }
    }

    let mut default_ref = params.default_ref.clone().unwrap_or(Value::Null);
    if !default_ref.is_null() {
        if let Some(offset) = &params.offset {
            if truthy(offset) {
                if let Some(map) = default_ref.as_object_mut() {
                    map.insert("offset".to_string(), offset.clone());
                }
            }
        }
        return default_ref;
    }
    default_ref
}

pub fn width_height_value_or_signal_ref(channel: &str, value: &Value) -> Value {
    if contains(&["x", "x2"][..], &channel) && value.as_str() == Some("width") {
        json!({"field": {"group": "width"}})
    } else if contains(&["y", "y2"][..], &channel) && value.as_str() == Some("height") {
        json!({"field": {"group": "height"}})
    } else {
        crate::compile_common::signal_or_value_ref(value)
    }
}

//! Port of `src/compositemark/errorband.ts`.

use serde_json::{Map, Value};

use crate::compositemark::common::{make_composite_aggregate_part, CompositePart};
use crate::compositemark::errorbar::error_bar_params;
use crate::normalize_support::{empty_object, get, normalize_encoding, truthy};

pub const ERRORBAND: &str = "errorband";
pub const ERRORBAND_PARTS: [&str; 2] = ["band", "borders"];

/// Port of `normalizeErrorBand`.
pub fn normalize_error_band(spec: &Value, config: &Value) -> Result<Value, String> {
    let mut spec = spec.clone();
    let encoding = normalize_encoding(get(&spec, "encoding").unwrap_or(&Value::Null), config);
    if let Some(map) = spec.as_object_mut() {
        map.insert("encoding".to_string(), encoding);
    }

    // NOTE: upstream passes `ERRORBAND` here, while `errorBarParams` also
    // consults `config.errorbar` for the default center.
    let params = error_bar_params(&spec, ERRORBAND, config)?;
    let error_band_def = params.mark_def.clone();

    let composite_mark_config = get(config, ERRORBAND).cloned().unwrap_or_else(empty_object);

    let spec_encoding = get(&spec, "encoding").cloned().unwrap_or_else(empty_object);
    let is_2d = get(&spec_encoding, "x").is_some() && get(&spec_encoding, "y").is_some();

    let mut band_mark = Map::new();
    band_mark.insert(
        "type".to_string(),
        Value::String(if is_2d { "area" } else { "rect" }.to_string()),
    );
    let mut borders_mark = Map::new();
    borders_mark.insert(
        "type".to_string(),
        Value::String(if is_2d { "line" } else { "rule" }.to_string()),
    );

    let mut interpolate = Map::new();
    if truthy(get(&error_band_def, "interpolate").unwrap_or(&Value::Null)) {
        interpolate.insert(
            "interpolate".to_string(),
            get(&error_band_def, "interpolate").unwrap().clone(),
        );
        if truthy(get(&error_band_def, "tension").unwrap_or(&Value::Null)) {
            interpolate.insert(
                "tension".to_string(),
                get(&error_band_def, "tension").unwrap().clone(),
            );
        }
    }

    if is_2d {
        for (key, value) in &interpolate {
            band_mark.insert(key.clone(), value.clone());
            borders_mark.insert(key.clone(), value.clone());
        }
        band_mark.insert(
            "ariaRoleDescription".to_string(),
            Value::String("errorband".to_string()),
        );
        borders_mark.insert("aria".to_string(), Value::Bool(false));
    } else if truthy(get(&error_band_def, "interpolate").unwrap_or(&Value::Null)) {
        crate::log::warn("1D error band does not support interpolate.".to_string());
    } else if truthy(get(&error_band_def, "tension").unwrap_or(&Value::Null)) {
        crate::log::warn("1D error band does not support tension.".to_string());
    }

    let band_mark = Value::Object(band_mark);
    let borders_mark = Value::Object(borders_mark);

    let mut layer: Vec<Value> = Vec::new();
    layer.extend(make_composite_aggregate_part(
        &error_band_def,
        &params.continuous_axis,
        &params.continuous_axis_channel_def,
        &params.encoding_without_continuous_axis,
        &composite_mark_config,
        CompositePart::new("band", band_mark, "lower")
            .end_position_prefix("upper")
            .extra_encoding(params.tooltip_encoding.clone()),
    ));
    layer.extend(make_composite_aggregate_part(
        &error_band_def,
        &params.continuous_axis,
        &params.continuous_axis_channel_def,
        &params.encoding_without_continuous_axis,
        &composite_mark_config,
        CompositePart::new("borders", borders_mark.clone(), "lower")
            .extra_encoding(params.tooltip_encoding.clone()),
    ));
    layer.extend(make_composite_aggregate_part(
        &error_band_def,
        &params.continuous_axis,
        &params.continuous_axis_channel_def,
        &params.encoding_without_continuous_axis,
        &composite_mark_config,
        CompositePart::new("borders", borders_mark, "upper")
            .extra_encoding(params.tooltip_encoding),
    ));

    let mut out = params.outer_spec.as_object().cloned().unwrap_or_default();
    out.insert("transform".to_string(), Value::Array(params.transform));
    out.insert("layer".to_string(), Value::Array(layer));
    Ok(Value::Object(out))
}

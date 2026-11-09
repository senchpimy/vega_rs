//! Port of `src/compositemark/errorbar.ts`.

use serde_json::{json, Map, Value};

use crate::compositemark::common::{
    aggregate_title, composite_mark_continuous_axis, composite_mark_orient,
    get_composite_mark_tooltip, make_composite_aggregate_part, CompositePart,
};
use crate::encoding::extract_transforms_from_encoding;
use crate::normalize_support::{
    empty_object, get, is_continuous_field_or_datum_def, is_field_or_datum_def, normalize_encoding,
    omit, title_text, truthy,
};
use crate::util::{access_with_datum_to_unescaped_path, replace_all, title_case};

pub const ERRORBAR: &str = "errorbar";
pub const ERRORBAR_PARTS: [&str; 2] = ["ticks", "rule"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorInputType {
    Raw,
    AggregatedUpperLower,
    AggregatedError,
}

impl ErrorInputType {
    #[allow(dead_code)]
    fn as_str(&self) -> &'static str {
        match self {
            ErrorInputType::Raw => "raw",
            ErrorInputType::AggregatedUpperLower => "aggregated-upper-lower",
            ErrorInputType::AggregatedError => "aggregated-error",
        }
    }
}

pub fn error_bar_is_input_type_raw(encoding: &Value) -> bool {
    (is_field_or_datum_def(get(encoding, "x").unwrap_or(&Value::Null))
        || is_field_or_datum_def(get(encoding, "y").unwrap_or(&Value::Null)))
        && !is_field_or_datum_def(get(encoding, "x2").unwrap_or(&Value::Null))
        && !is_field_or_datum_def(get(encoding, "y2").unwrap_or(&Value::Null))
        && !is_field_or_datum_def(get(encoding, "xError").unwrap_or(&Value::Null))
        && !is_field_or_datum_def(get(encoding, "xError2").unwrap_or(&Value::Null))
        && !is_field_or_datum_def(get(encoding, "yError").unwrap_or(&Value::Null))
        && !is_field_or_datum_def(get(encoding, "yError2").unwrap_or(&Value::Null))
}

pub fn error_bar_is_input_type_aggregated_upper_lower(encoding: &Value) -> bool {
    is_field_or_datum_def(get(encoding, "x2").unwrap_or(&Value::Null))
        || is_field_or_datum_def(get(encoding, "y2").unwrap_or(&Value::Null))
}

pub fn error_bar_is_input_type_aggregated_error(encoding: &Value) -> bool {
    ["xError", "xError2", "yError", "yError2"]
        .iter()
        .any(|channel| is_field_or_datum_def(get(encoding, channel).unwrap_or(&Value::Null)))
}

/// Port of `errorBarOrientAndInputType`.
pub fn error_bar_orient_and_input_type(
    spec: &Value,
    composite_mark: &str,
) -> Result<(String, ErrorInputType), String> {
    let encoding = get(spec, "encoding").cloned().unwrap_or_else(empty_object);

    if error_bar_is_input_type_raw(&encoding) {
        return Ok((
            composite_mark_orient(spec, composite_mark)?,
            ErrorInputType::Raw,
        ));
    }

    let is_upper_lower = error_bar_is_input_type_aggregated_upper_lower(&encoding);
    let is_error = error_bar_is_input_type_aggregated_error(&encoding);
    let x = get(&encoding, "x").cloned().unwrap_or(Value::Null);
    let y = get(&encoding, "y").cloned().unwrap_or(Value::Null);

    if is_upper_lower {
        if is_error {
            return Err(format!(
                "{} cannot be both type aggregated-upper-lower and aggregated-error",
                composite_mark
            ));
        }

        let x2 = get(&encoding, "x2").cloned().unwrap_or(Value::Null);
        let y2 = get(&encoding, "y2").cloned().unwrap_or(Value::Null);

        if is_field_or_datum_def(&x2) && is_field_or_datum_def(&y2) {
            return Err(format!("{} cannot have both x2 and y2", composite_mark));
        } else if is_field_or_datum_def(&x2) {
            if is_continuous_field_or_datum_def(&x) {
                return Ok((
                    "horizontal".to_string(),
                    ErrorInputType::AggregatedUpperLower,
                ));
            }
            return Err(format!(
                "Both x and x2 have to be quantitative in {}",
                composite_mark
            ));
        } else if is_field_or_datum_def(&y2) {
            if is_continuous_field_or_datum_def(&y) {
                return Ok(("vertical".to_string(), ErrorInputType::AggregatedUpperLower));
            }
            return Err(format!(
                "Both y and y2 have to be quantitative in {}",
                composite_mark
            ));
        }
        return Err("No ranged axis".to_string());
    }

    let x_error = get(&encoding, "xError").cloned().unwrap_or(Value::Null);
    let x_error2 = get(&encoding, "xError2").cloned().unwrap_or(Value::Null);
    let y_error = get(&encoding, "yError").cloned().unwrap_or(Value::Null);
    let y_error2 = get(&encoding, "yError2").cloned().unwrap_or(Value::Null);

    if is_field_or_datum_def(&x_error2) && !is_field_or_datum_def(&x_error) {
        return Err(format!(
            "{} cannot have xError2 without xError",
            composite_mark
        ));
    }
    if is_field_or_datum_def(&y_error2) && !is_field_or_datum_def(&y_error) {
        return Err(format!(
            "{} cannot have yError2 without yError",
            composite_mark
        ));
    }

    if is_field_or_datum_def(&x_error) && is_field_or_datum_def(&y_error) {
        return Err(format!(
            "{} cannot have both xError and yError with both are quantiative",
            composite_mark
        ));
    } else if is_field_or_datum_def(&x_error) {
        if is_continuous_field_or_datum_def(&x) {
            return Ok(("horizontal".to_string(), ErrorInputType::AggregatedError));
        }
        return Err("All x, xError, and xError2 (if exist) have to be quantitative".to_string());
    } else if is_field_or_datum_def(&y_error) {
        if is_continuous_field_or_datum_def(&y) {
            return Ok(("vertical".to_string(), ErrorInputType::AggregatedError));
        }
        return Err("All y, yError, and yError2 (if exist) have to be quantitative".to_string());
    }

    Err("No ranged axis".to_string())
}

#[derive(Debug, Clone)]
pub struct ErrorBarCalculation {
    pub post_aggregate_calculates: Vec<Value>,
    pub error_bar_specific_aggregate: Vec<Value>,
    pub tooltip_summary: Vec<Value>,
    pub tooltip_title_with_field_name: bool,
}

/// Port of `errorBarAggregationAndCalculation`.
#[allow(clippy::too_many_arguments)]
pub fn error_bar_aggregation_and_calculation(
    mark_def: &Value,
    continuous_axis_channel_def: &Value,
    continuous_axis_channel_def2: &Value,
    continuous_axis_channel_def_error: &Value,
    continuous_axis_channel_def_error2: &Value,
    input_type: ErrorInputType,
    composite_mark: &str,
    config: &Value,
) -> ErrorBarCalculation {
    let continuous_field_name = get(continuous_axis_channel_def, "field")
        .map(title_text)
        .unwrap_or_default();

    let mut error_bar_specific_aggregate: Vec<Value> = Vec::new();
    let mut post_aggregate_calculates: Vec<Value> = Vec::new();
    let mut tooltip_summary: Vec<Value> = Vec::new();
    let mut tooltip_title_with_field_name = false;

    let errorbar_config = get(config, "errorbar")
        .cloned()
        .unwrap_or_else(empty_object);

    if input_type == ErrorInputType::Raw {
        let mark_center = get(mark_def, "center").and_then(|c| c.as_str());
        let mark_extent = get(mark_def, "extent").and_then(|e| e.as_str());

        let center = if let Some(center) = mark_center {
            center.to_string()
        } else if let Some(extent) = mark_extent {
            if extent == "iqr" {
                "median".to_string()
            } else {
                "mean".to_string()
            }
        } else {
            get(&errorbar_config, "center")
                .and_then(|c| c.as_str())
                .unwrap_or("mean")
                .to_string()
        };

        let extent = if let Some(extent) = mark_extent {
            extent.to_string()
        } else if center == "mean" {
            "stderr".to_string()
        } else {
            "iqr".to_string()
        };

        if (center == "median") != (extent == "iqr") {
            crate::log::warn(format!(
                "{} is not usually used with {} for {}s.",
                center, extent, composite_mark
            ));
        }

        if extent == "stderr" || extent == "stdev" {
            error_bar_specific_aggregate = vec![
                json!({"op": extent, "field": continuous_field_name, "as": format!("extent_{}", continuous_field_name)}),
                json!({"op": center, "field": continuous_field_name, "as": format!("center_{}", continuous_field_name)}),
            ];
            post_aggregate_calculates = vec![
                json!({
                    "calculate": format!(
                        "{} + {}",
                        access_with_datum_to_unescaped_path(&format!("center_{}", continuous_field_name)),
                        access_with_datum_to_unescaped_path(&format!("extent_{}", continuous_field_name))
                    ),
                    "as": format!("upper_{}", continuous_field_name)
                }),
                json!({
                    "calculate": format!(
                        "{} - {}",
                        access_with_datum_to_unescaped_path(&format!("center_{}", continuous_field_name)),
                        access_with_datum_to_unescaped_path(&format!("extent_{}", continuous_field_name))
                    ),
                    "as": format!("lower_{}", continuous_field_name)
                }),
            ];
            tooltip_summary = vec![
                json!({"fieldPrefix": "center_", "titlePrefix": title_case(&center)}),
                json!({"fieldPrefix": "upper_", "titlePrefix": format!("{} + {}", title_case(&center), extent)}),
                json!({"fieldPrefix": "lower_", "titlePrefix": format!("{} - {}", title_case(&center), extent)}),
            ];
            tooltip_title_with_field_name = true;
        } else {
            let (center_op, lower_extent_op, upper_extent_op) = if extent == "ci" {
                ("mean", "ci0", "ci1")
            } else {
                ("median", "q1", "q3")
            };

            error_bar_specific_aggregate = vec![
                json!({"op": lower_extent_op, "field": continuous_field_name, "as": format!("lower_{}", continuous_field_name)}),
                json!({"op": upper_extent_op, "field": continuous_field_name, "as": format!("upper_{}", continuous_field_name)}),
                json!({"op": center_op, "field": continuous_field_name, "as": format!("center_{}", continuous_field_name)}),
            ];

            tooltip_summary = vec![
                json!({"fieldPrefix": "upper_", "titlePrefix": aggregate_title(&continuous_field_name, upper_extent_op)}),
                json!({"fieldPrefix": "lower_", "titlePrefix": aggregate_title(&continuous_field_name, lower_extent_op)}),
                json!({"fieldPrefix": "center_", "titlePrefix": aggregate_title(&continuous_field_name, center_op)}),
            ];
        }
    } else {
        if truthy(get(mark_def, "center").unwrap_or(&Value::Null))
            || truthy(get(mark_def, "extent").unwrap_or(&Value::Null))
        {
            let center = get(mark_def, "center").cloned();
            let extent = get(mark_def, "extent").cloned();
            crate::log::warn(error_bar_center_and_extent_are_not_needed(
                center.as_ref(),
                extent.as_ref(),
            ));
        }

        if input_type == ErrorInputType::AggregatedUpperLower {
            let upper_field = get(continuous_axis_channel_def2, "field")
                .map(title_text)
                .unwrap_or_default();
            post_aggregate_calculates = vec![
                json!({
                    "calculate": access_with_datum_to_unescaped_path(&upper_field),
                    "as": format!("upper_{}", continuous_field_name)
                }),
                json!({
                    "calculate": access_with_datum_to_unescaped_path(&continuous_field_name),
                    "as": format!("lower_{}", continuous_field_name)
                }),
            ];
        } else if input_type == ErrorInputType::AggregatedError {
            let error_field = get(continuous_axis_channel_def_error, "field")
                .map(title_text)
                .unwrap_or_default();

            tooltip_summary =
                vec![json!({"fieldPrefix": "", "titlePrefix": continuous_field_name})];

            post_aggregate_calculates = vec![json!({
                "calculate": format!(
                    "{} + {}",
                    access_with_datum_to_unescaped_path(&continuous_field_name),
                    access_with_datum_to_unescaped_path(&error_field)
                ),
                "as": format!("upper_{}", continuous_field_name)
            })];

            if truthy(continuous_axis_channel_def_error2) {
                let error2_field = get(continuous_axis_channel_def_error2, "field")
                    .map(title_text)
                    .unwrap_or_default();
                post_aggregate_calculates.push(json!({
                    "calculate": format!(
                        "{} + {}",
                        access_with_datum_to_unescaped_path(&continuous_field_name),
                        access_with_datum_to_unescaped_path(&error2_field)
                    ),
                    "as": format!("lower_{}", continuous_field_name)
                }));
            } else {
                post_aggregate_calculates.push(json!({
                    "calculate": format!(
                        "{} - {}",
                        access_with_datum_to_unescaped_path(&continuous_field_name),
                        access_with_datum_to_unescaped_path(&error_field)
                    ),
                    "as": format!("lower_{}", continuous_field_name)
                }));
            }
        }

        for calculate in &post_aggregate_calculates {
            let calculate_str = get(calculate, "calculate")
                .and_then(|c| c.as_str())
                .unwrap_or("");
            let as_str = get(calculate, "as").and_then(|a| a.as_str()).unwrap_or("");
            let prefix = &as_str[..as_str.len().min(6)];
            tooltip_summary.push(json!({
                "fieldPrefix": prefix,
                "titlePrefix": replace_all(&replace_all(calculate_str, "datum['", ""), "']", "")
            }));
        }
    }

    ErrorBarCalculation {
        post_aggregate_calculates,
        error_bar_specific_aggregate,
        tooltip_summary,
        tooltip_title_with_field_name,
    }
}

fn error_bar_center_and_extent_are_not_needed(
    center: Option<&Value>,
    extent: Option<&Value>,
) -> String {
    let center = center.and_then(|c| c.as_str());
    let extent = extent.and_then(|e| e.as_str());
    let prefix = format!(
        "{}{}{}",
        if extent.is_some() { "extent " } else { "" },
        if extent.is_some() && center.is_some() {
            "and "
        } else {
            ""
        },
        if center.is_some() { "center " } else { "" },
    );
    format!(
        "{}{}not needed when data are aggregated.",
        prefix,
        if extent.is_some() && center.is_some() {
            "are "
        } else {
            "is "
        }
    )
}

/// Port of `errorBarParams`'s result.
#[derive(Debug, Clone)]
pub struct ErrorBarParams {
    pub transform: Vec<Value>,
    pub groupby: Vec<String>,
    pub continuous_axis_channel_def: Value,
    pub continuous_axis: String,
    pub encoding_without_continuous_axis: Value,
    pub ticks_orient: String,
    pub mark_def: Value,
    pub outer_spec: Value,
    pub tooltip_encoding: Value,
}

/// Port of `errorBarParams`.
pub fn error_bar_params(
    spec: &Value,
    composite_mark: &str,
    config: &Value,
) -> Result<ErrorBarParams, String> {
    let mark = get(spec, "mark").cloned().unwrap_or(Value::Null);
    let encoding = get(spec, "encoding").cloned().unwrap_or_else(empty_object);
    let mark_def = if crate::mark::is_mark_def(&mark) {
        mark.clone()
    } else {
        json!({"type": mark})
    };
    let outer_spec = omit(spec, &["mark", "encoding", "params", "projection"]);

    if truthy(get(spec, "params").unwrap_or(&Value::Null)) {
        crate::log::warn(format!(
            "Selection not supported for {} yet.",
            composite_mark
        ));
    }

    let (orient, input_type) = error_bar_orient_and_input_type(spec, composite_mark)?;
    let axis = composite_mark_continuous_axis(spec, &orient, composite_mark);

    let calculation = error_bar_aggregation_and_calculation(
        &mark_def,
        &axis.continuous_axis_channel_def,
        &axis.continuous_axis_channel_def2,
        &axis.continuous_axis_channel_def_error,
        &axis.continuous_axis_channel_def_error2,
        input_type,
        composite_mark,
        config,
    );

    let continuous_axis = axis.continuous_axis.clone();

    let encoding_without_continuous_axis = omit(
        &encoding,
        &[
            continuous_axis.as_str(),
            format!("{}2", continuous_axis).as_str(),
            format!("{}Error", continuous_axis).as_str(),
            format!("{}Error2", continuous_axis).as_str(),
        ],
    );

    let extracted = extract_transforms_from_encoding(&encoding_without_continuous_axis, config);
    let mut aggregate = extracted.aggregate.clone();
    aggregate.extend(calculation.error_bar_specific_aggregate.clone());
    let groupby = if input_type != ErrorInputType::Raw {
        Vec::new()
    } else {
        extracted.groupby.clone()
    };

    let tooltip_encoding = get_composite_mark_tooltip(
        &calculation.tooltip_summary,
        &axis.continuous_axis_channel_def,
        &extracted.encoding,
        calculation.tooltip_title_with_field_name,
    );

    let mut transform: Vec<Value> = get(spec, "transform")
        .and_then(|t| t.as_array().cloned())
        .unwrap_or_default();
    transform.extend(extracted.bins.clone());
    transform.extend(extracted.time_units.clone());
    if !aggregate.is_empty() {
        transform.push(json!({"aggregate": aggregate, "groupby": groupby}));
    }
    transform.extend(calculation.post_aggregate_calculates.clone());

    Ok(ErrorBarParams {
        transform,
        groupby,
        continuous_axis_channel_def: axis.continuous_axis_channel_def,
        continuous_axis,
        encoding_without_continuous_axis: extracted.encoding,
        ticks_orient: if orient == "vertical" {
            "horizontal".to_string()
        } else {
            "vertical".to_string()
        },
        mark_def,
        outer_spec,
        tooltip_encoding,
    })
}

/// Port of `normalizeErrorBar`.
pub fn normalize_error_bar(spec: &Value, config: &Value) -> Result<Value, String> {
    let mut spec = spec.clone();
    let encoding = normalize_encoding(get(&spec, "encoding").unwrap_or(&Value::Null), config);
    if let Some(map) = spec.as_object_mut() {
        map.insert("encoding".to_string(), encoding);
    }

    let params = error_bar_params(&spec, ERRORBAR, config)?;
    let mut encoding_without_continuous_axis = params.encoding_without_continuous_axis.clone();
    if let Some(map) = encoding_without_continuous_axis.as_object_mut() {
        map.remove("size");
    }

    let composite_mark_config = get(config, ERRORBAR).cloned().unwrap_or_else(empty_object);
    let thickness = get(&params.mark_def, "thickness").cloned();
    let size = get(&params.mark_def, "size").cloned();

    let mut tick = Map::new();
    tick.insert("type".to_string(), Value::String("tick".to_string()));
    tick.insert(
        "orient".to_string(),
        Value::String(params.ticks_orient.clone()),
    );
    tick.insert("aria".to_string(), Value::Bool(false));
    if let Some(thickness) = &thickness {
        tick.insert("thickness".to_string(), thickness.clone());
    }
    if let Some(size) = &size {
        tick.insert("size".to_string(), size.clone());
    }
    let tick = Value::Object(tick);

    let mut rule_mark = Map::new();
    rule_mark.insert("type".to_string(), Value::String("rule".to_string()));
    rule_mark.insert(
        "ariaRoleDescription".to_string(),
        Value::String("errorbar".to_string()),
    );
    if let Some(thickness) = &thickness {
        rule_mark.insert("size".to_string(), thickness.clone());
    }
    let rule_mark = Value::Object(rule_mark);

    let mut layer: Vec<Value> = Vec::new();
    layer.extend(make_composite_aggregate_part(
        &params.mark_def,
        &params.continuous_axis,
        &params.continuous_axis_channel_def,
        &encoding_without_continuous_axis,
        &composite_mark_config,
        CompositePart::new("ticks", tick.clone(), "lower")
            .extra_encoding(params.tooltip_encoding.clone()),
    ));
    layer.extend(make_composite_aggregate_part(
        &params.mark_def,
        &params.continuous_axis,
        &params.continuous_axis_channel_def,
        &encoding_without_continuous_axis,
        &composite_mark_config,
        CompositePart::new("ticks", tick, "upper").extra_encoding(params.tooltip_encoding.clone()),
    ));
    layer.extend(make_composite_aggregate_part(
        &params.mark_def,
        &params.continuous_axis,
        &params.continuous_axis_channel_def,
        &encoding_without_continuous_axis,
        &composite_mark_config,
        CompositePart::new("rule", rule_mark, "lower")
            .end_position_prefix("upper")
            .extra_encoding(params.tooltip_encoding.clone()),
    ));

    let mut out = params.outer_spec.as_object().cloned().unwrap_or_default();
    out.insert(
        "transform".to_string(),
        Value::Array(params.transform.clone()),
    );
    if layer.len() > 1 {
        out.insert("layer".to_string(), Value::Array(layer));
    } else if let Some(Value::Object(first)) = layer.into_iter().next() {
        for (key, value) in first {
            out.insert(key, value);
        }
    }
    Ok(Value::Object(out))
}

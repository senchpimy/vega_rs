//! Port of `src/compile/axis/properties.ts`.

use serde_json::{json, Number, Value};

use crate::channel::{get_secondary_range_channel, X};
use crate::channeldef::{
    is_discrete, is_field_def, to_field_def_base, value_array,
};
use crate::channeldef_support::{is_binned, is_binning, truthy};
use crate::compile::model::Model;
use crate::compile_common::{is_signal_ref, merge_title, merge_title_field_defs};
use crate::scale::{has_discrete_domain, ScaleType};
use crate::timeunit::{duration_expr, normalize_time_unit};
use crate::util::{contains, js_number_to_string, normalize_angle};

use super::config::{get_axis_config, AxisConfigs};

/// `AxisRuleParams`
#[derive(Debug, Clone)]
pub struct AxisRuleParams<'a> {
    pub field_or_datum_def: Option<Value>,
    pub axis: Value,
    pub channel: String,
    pub model: &'a Model,
    pub mark: String,
    pub scale_type: Option<ScaleType>,
    pub orient: Value,
    pub label_angle: Option<Value>,
    pub format: Option<Value>,
    pub format_type: Option<Value>,
    pub config: Value,
}

fn json_number(f: f64) -> Value {
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 9e15 {
        Value::Number(Number::from(f as i64))
    } else {
        Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

/// `axis.prop ?? default` (nullish coalescing).
fn axis_nullish(axis: &Value, prop: &str) -> Option<Value> {
    axis.get(prop).filter(|v| !v.is_null()).cloned()
}

/// Properties for which a rule is defined.
pub fn is_axis_rule(property: &str) -> bool {
    matches!(
        property,
        "scale"
            | "format"
            | "formatType"
            | "grid"
            | "gridScale"
            | "labelAlign"
            | "labelAngle"
            | "labelBaseline"
            | "labelFlush"
            | "labelOverlap"
            | "bandPosition"
            | "orient"
            | "tickCount"
            | "tickMinStep"
            | "title"
            | "values"
            | "zindex"
    )
}

/// Evaluate the rule for `property`, mirroring `axisRules[property](params)`.
pub fn axis_rule(property: &str, p: &AxisRuleParams) -> Option<Value> {
    match property {
        "scale" => p
            .model
            .scale_name(&p.channel, false)
            .map(Value::String),
        "format" => p.format.clone(),
        "formatType" => p.format_type.clone(),
        "grid" => axis_nullish(&p.axis, "grid").or_else(|| {
            p.field_or_datum_def
                .as_ref()
                .map(|d| Value::Bool(default_grid(p.scale_type, d)))
        }),
        "gridScale" => grid_scale(p.model, &p.channel).map(Value::String),
        "labelAlign" => {
            let explicit = p.axis.get("labelAlign").filter(|v| truthy(v)).cloned();
            explicit.or_else(|| default_label_align(p.label_angle.as_ref(), Some(&p.orient), &p.channel))
        }
        "labelAngle" => p.label_angle.clone(),
        "labelBaseline" => {
            let explicit = p.axis.get("labelBaseline").filter(|v| truthy(v)).cloned();
            explicit.or_else(|| {
                default_label_baseline(p.label_angle.as_ref(), Some(&p.orient), &p.channel, false)
            })
        }
        "labelFlush" => axis_nullish(&p.axis, "labelFlush").or_else(|| {
            p.field_or_datum_def
                .as_ref()
                .map(|d| match default_label_flush(d.get("type").and_then(Value::as_str), &p.channel) {
                    Some(v) => v,
                    None => Value::Null,
                })
                .filter(|v| !v.is_null())
        }),
        "labelOverlap" => axis_nullish(&p.axis, "labelOverlap").or_else(|| {
            let def = p.field_or_datum_def.as_ref()?;
            let has_time_unit = is_field_def(def) && truthy(&def["timeUnit"]);
            let sort = is_field_def(def).then(|| def.get("sort").cloned()).flatten();
            default_label_overlap(
                def.get("type").and_then(Value::as_str),
                p.scale_type,
                has_time_unit,
                sort.as_ref(),
            )
        }),
        "bandPosition" => axis_nullish(&p.axis, "bandPosition")
            .or_else(|| default_band_position(p.model, &p.channel)),
        "orient" => Some(p.orient.clone()),
        "tickCount" => axis_nullish(&p.axis, "tickCount").or_else(|| {
            let size_type = if p.channel == "x" {
                Some("width")
            } else if p.channel == "y" {
                Some("height")
            } else {
                None
            };
            let size = size_type.map(|t| p.model.get_size_signal_ref(t));
            let values = p.axis.get("values").cloned();
            default_tick_count(
                p.field_or_datum_def.as_ref(),
                p.scale_type,
                size.as_ref(),
                values.as_ref(),
            )
        }),
        "tickMinStep" => axis_nullish(&p.axis, "tickMinStep").or_else(|| {
            default_tick_min_step(p.format.as_ref(), p.field_or_datum_def.as_ref())
        }),
        "title" => {
            if let Some(title) = p.axis.get("title") {
                return Some(title.clone());
            }
            if let Some(field_def_title) = get_field_def_title(p.model, &p.channel) {
                return Some(field_def_title);
            }
            let field_def = p.model.typed_field_def(&p.channel);
            let channel2 = if p.channel == "x" { "x2" } else { "y2" };
            let field_def2 = p.model.field_def(channel2);
            Some(Value::Array(merge_title_field_defs(
                &field_def
                    .as_ref()
                    .map(to_field_def_base)
                    .into_iter()
                    .collect::<Vec<_>>(),
                &field_def2
                    .as_ref()
                    .filter(|f| is_field_def(f))
                    .map(to_field_def_base)
                    .into_iter()
                    .collect::<Vec<_>>(),
            )))
        }
        "values" => values(
            &p.axis,
            p.field_or_datum_def.as_ref().unwrap_or(&Value::Null),
        ),
        "zindex" => axis_nullish(&p.axis, "zindex").or_else(|| {
            p.field_or_datum_def
                .as_ref()
                .map(|d| json!(default_zindex(&p.mark, d)))
        }),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Default rules
// ---------------------------------------------------------------------------

/// `defaultGrid`
pub fn default_grid(scale_type: Option<ScaleType>, field_def: &Value) -> bool {
    match scale_type {
        Some(t) => {
            !has_discrete_domain(t)
                && is_field_def(field_def)
                && !is_binning(&field_def["bin"])
                && !is_binned(&field_def["bin"])
        }
        None => false,
    }
}

/// `defaultBandPosition`
pub fn default_band_position(model: &Model, channel: &str) -> Option<Value> {
    if is_ranged_offset(model, channel) {
        return Some(json!(if channel == "x" { 0 } else { 1 }));
    }
    None
}

/// `UnitModel.isRangedOffset`
pub fn is_ranged_offset(model: &Model, channel: &str) -> bool {
    let mark = model.mark_type();
    if mark != "bar" && mark != "area" {
        return false;
    }
    let Some(encoding) = model.encoding() else {
        return false;
    };
    if let Some(secondary) = get_secondary_range_channel(channel) {
        if encoding.get(secondary).map(truthy).unwrap_or(false) {
            return false;
        }
    }
    if !crate::encoding::channel_has_quantitative_offset(encoding, channel) {
        return false;
    }
    let channel_def = encoding.get(channel);
    match channel_def {
        None | Some(Value::Null) => true,
        Some(channel_def) => {
            crate::channeldef::is_field_or_datum_def(channel_def)
                && model
                    .get_scale_component(channel)
                    .and_then(|c| c.get_type())
                    .map(has_discrete_domain)
                    .unwrap_or(false)
        }
    }
}

/// `gridScale`
pub fn grid_scale(model: &Model, channel: &str) -> Option<String> {
    let grid_channel = if channel == "x" { "y" } else { "x" };
    if model.get_scale_component(grid_channel).is_some() {
        return model.scale_name(grid_channel, false);
    }
    None
}

/// `getLabelAngle`
pub fn get_label_angle(
    field_or_datum_def: Option<&Value>,
    axis: &Value,
    channel: &str,
    style_config: &Value,
    axis_configs: Option<&AxisConfigs>,
) -> Option<Value> {
    if let Some(label_angle) = axis.get("labelAngle") {
        if is_signal_ref(label_angle) {
            return Some(label_angle.clone());
        }
        return Some(json_number(normalize_angle(label_angle.as_f64()).unwrap_or(0.0)));
    }

    let config = get_axis_config(
        "labelAngle",
        style_config,
        axis.get("style"),
        axis_configs,
    );
    if let Some(angle) = config.config_value {
        if is_signal_ref(&angle) {
            return Some(angle);
        }
        return Some(json_number(normalize_angle(angle.as_f64()).unwrap_or(0.0)));
    }

    if channel == X {
        let def = field_or_datum_def?;
        if let Some(type_) = def.get("type").and_then(Value::as_str) {
            if matches!(type_, "nominal" | "ordinal")
                && !(is_field_def(def) && truthy(&def["timeUnit"]))
            {
                return Some(json!(270));
            }
        }
    }

    None
}

/// `normalizeAngleExpr`
pub fn normalize_angle_expr(angle: &Value) -> String {
    format!(
        "((({} % 360) + 360) % 360)",
        angle.get("signal").and_then(Value::as_str).unwrap_or_default()
    )
}

fn num_string(f: f64) -> String {
    js_number_to_string(f)
}

/// `defaultLabelBaseline`
pub fn default_label_baseline(
    angle: Option<&Value>,
    orient: Option<&Value>,
    channel: &str,
    always_include_middle: bool,
) -> Option<Value> {
    let angle = angle?;

    if channel == "x" {
        if is_signal_ref(angle) {
            let a = normalize_angle_expr(angle);
            let orient_is_top = match orient {
                Some(o) if is_signal_ref(o) => {
                    format!("({} === \"top\")", o.get("signal").and_then(Value::as_str).unwrap_or_default())
                }
                Some(o) => (o.as_str() == Some("top")).to_string(),
                None => "false".to_string(),
            };
            return Some(json!({
                "signal": format!(
                    "(45 < {a} && {a} < 135) || (225 < {a} && {a} < 315) ? \"middle\" :({a} <= 45 || 315 <= {a}) === {orient_is_top} ? \"bottom\" : \"top\""
                )
            }));
        }

        let a = angle.as_f64()?;
        if (45.0 < a && a < 135.0) || (225.0 < a && a < 315.0) {
            return Some(json!("middle"));
        }
        if let Some(o) = orient {
            if is_signal_ref(o) {
                let op = if a <= 45.0 || 315.0 <= a { "===" } else { "!==" };
                return Some(json!({
                    "signal": format!(
                        "{} {op} \"top\" ? \"bottom\" : \"top\"",
                        o.get("signal").and_then(Value::as_str).unwrap_or_default()
                    )
                }));
            }
            let is_top = o.as_str() == Some("top");
            return Some(json!(if (a <= 45.0 || 315.0 <= a) == is_top {
                "bottom"
            } else {
                "top"
            }));
        }
        return Some(json!(if a <= 45.0 || 315.0 <= a { "bottom" } else { "top" }));
    }

    // channel === 'y'
    if is_signal_ref(angle) {
        let a = normalize_angle_expr(angle);
        let orient_is_left = match orient {
            Some(o) if is_signal_ref(o) => {
                format!("({} === \"left\")", o.get("signal").and_then(Value::as_str).unwrap_or_default())
            }
            Some(o) => (o.as_str() == Some("left")).to_string(),
            None => "false".to_string(),
        };
        let middle = if always_include_middle {
            "\"middle\""
        } else {
            "null"
        };
        return Some(json!({
            "signal": format!(
                "{a} <= 45 || 315 <= {a} || (135 <= {a} && {a} <= 225) ? {middle} : (45 <= {a} && {a} <= 135) === {orient_is_left} ? \"top\" : \"bottom\""
            )
        }));
    }

    let a = angle.as_f64()?;
    if a <= 45.0 || 315.0 <= a || (135.0..=225.0).contains(&a) {
        return Some(if always_include_middle {
            json!("middle")
        } else {
            Value::Null
        });
    }
    if let Some(o) = orient {
        if is_signal_ref(o) {
            let op = if (45.0..=135.0).contains(&a) { "===" } else { "!==" };
            return Some(json!({
                "signal": format!(
                    "{} {op} \"left\" ? \"top\" : \"bottom\"",
                    o.get("signal").and_then(Value::as_str).unwrap_or_default()
                )
            }));
        }
        let is_left = o.as_str() == Some("left");
        return Some(json!(if (45.0..=135.0).contains(&a) == is_left {
            "top"
        } else {
            "bottom"
        }));
    }
    Some(json!(if (45.0..=135.0).contains(&a) { "top" } else { "bottom" }))
}

/// `defaultLabelAlign`
pub fn default_label_align(
    angle: Option<&Value>,
    orient: Option<&Value>,
    channel: &str,
) -> Option<Value> {
    let angle = angle?;
    let is_x = channel == "x";
    let start_angle = if is_x { 0.0 } else { 90.0 };
    let main_orient = if is_x { "bottom" } else { "left" };

    if is_signal_ref(angle) {
        let a = normalize_angle_expr(angle);
        let orient_is_main = match orient {
            Some(o) if is_signal_ref(o) => format!(
                "({} === \"{main_orient}\")",
                o.get("signal").and_then(Value::as_str).unwrap_or_default()
            ),
            Some(o) => (o.as_str() == Some(main_orient)).to_string(),
            None => "false".to_string(),
        };
        let start_expr = if start_angle != 0.0 {
            format!("({a} + 90)")
        } else {
            a.clone()
        };
        let zero_case = if is_x { "null" } else { "\"center\"" };
        return Some(json!({
            "signal": format!(
                "({start_expr} % 180 === 0) ? {zero_case} :({} < {a} && {a} < {}) === {orient_is_main} ? \"left\" : \"right\"",
                num_string(start_angle),
                num_string(180.0 + start_angle)
            )
        }));
    }

    let a = angle.as_f64()?;
    if ((a + start_angle) % 180.0) == 0.0 {
        return Some(if is_x { Value::Null } else { json!("center") });
    }

    if let Some(o) = orient {
        if is_signal_ref(o) {
            let op = if start_angle < a && a < 180.0 + start_angle {
                "==="
            } else {
                "!=="
            };
            let orient_is_main = format!(
                "{} {op} \"{main_orient}\"",
                o.get("signal").and_then(Value::as_str).unwrap_or_default()
            );
            return Some(json!({
                "signal": format!("{orient_is_main} ? \"left\" : \"right\"")
            }));
        }
        let is_main = o.as_str() == Some(main_orient);
        return Some(json!(
            if (start_angle < a && a < 180.0 + start_angle) == is_main {
                "left"
            } else {
                "right"
            }
        ));
    }

    Some(json!(
        if start_angle < a && a < 180.0 + start_angle {
            "left"
        } else {
            "right"
        }
    ))
}

/// `defaultLabelFlush`
pub fn default_label_flush(type_: Option<&str>, channel: &str) -> Option<Value> {
    if channel == "x" && matches!(type_, Some("quantitative") | Some("temporal")) {
        return Some(json!(true));
    }
    None
}

/// `defaultLabelOverlap`
pub fn default_label_overlap(
    type_: Option<&str>,
    scale_type: Option<ScaleType>,
    has_time_unit: bool,
    sort: Option<&Value>,
) -> Option<Value> {
    let sort_is_object = sort
        .map(|s| s.is_object() || s.is_array())
        .unwrap_or(false);
    if (has_time_unit && !sort_is_object)
        || !matches!(type_, Some("nominal") | Some("ordinal"))
    {
        if matches!(scale_type, Some(ScaleType::Log) | Some(ScaleType::Symlog)) {
            return Some(json!("greedy"));
        }
        return Some(json!(true));
    }
    None
}

/// `defaultOrient`
pub fn default_orient(channel: &str) -> Value {
    json!(if channel == "x" { "bottom" } else { "left" })
}

/// `defaultTickCount`
pub fn default_tick_count(
    field_or_datum_def: Option<&Value>,
    scale_type: Option<ScaleType>,
    size: Option<&Value>,
    values: Option<&Value>,
) -> Option<Value> {
    let has_values = values.map(truthy).unwrap_or(false);
    let discrete = scale_type.map(has_discrete_domain).unwrap_or(false);
    let is_log = scale_type == Some(ScaleType::Log);

    if !has_values && !discrete && !is_log {
        let size_signal = size
            .and_then(|s| s.get("signal"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        if let Some(def) = field_or_datum_def {
            if is_field_def(def) {
                if is_binning(&def["bin"]) {
                    return Some(json!({ "signal": format!("ceil({size_signal}/10)") }));
                }
                if truthy(&def["timeUnit"]) {
                    let unit = normalize_time_unit(&def["timeUnit"]).and_then(|t| t.unit);
                    if contains(
                        &["month", "hours", "day", "quarter"],
                        &unit.as_deref().unwrap_or(""),
                    ) {
                        return None;
                    }
                }
            }
        }
        return Some(json!({ "signal": format!("ceil({size_signal}/40)") }));
    }

    None
}

/// `defaultTickMinStep`
pub fn default_tick_min_step(
    format: Option<&Value>,
    field_or_datum_def: Option<&Value>,
) -> Option<Value> {
    if format.and_then(Value::as_str) == Some("d") {
        return Some(json!(1));
    }
    if let Some(def) = field_or_datum_def {
        if is_field_def(def)
            && truthy(&def["timeUnit"]) {
                if let Some(signal) = duration_expr(&def["timeUnit"]) {
                    return Some(json!({ "signal": signal }));
                }
            }
    }
    None
}

/// `getFieldDefTitle`
pub fn get_field_def_title(model: &Model, channel: &str) -> Option<Value> {
    let channel2 = if channel == "x" { "x2" } else { "y2" };
    let field_def = model.field_def(channel);
    let field_def2 = model.field_def(channel2);

    let title1 = field_def.as_ref().and_then(|f| f.get("title")).cloned();
    let title2 = field_def2.as_ref().and_then(|f| f.get("title")).cloned();

    match (&title1, &title2) {
        (Some(t1), Some(t2)) if truthy(t1) && truthy(t2) => Some(merge_title(t1, t2)),
        (Some(t1), _) if truthy(t1) => Some(t1.clone()),
        (_, Some(t2)) if truthy(t2) => Some(t2.clone()),
        (Some(t1), _) => Some(t1.clone()),
        (_, Some(t2)) => Some(t2.clone()),
        _ => None,
    }
}

/// `values`
pub fn values(axis: &Value, field_or_datum_def: &Value) -> Option<Value> {
    match axis.get("values") {
        Some(Value::Array(vals)) => Some(Value::Array(value_array(field_or_datum_def, vals))),
        Some(v) if is_signal_ref(v) => Some(v.clone()),
        _ => None,
    }
}

/// `defaultZindex`
pub fn default_zindex(mark: &str, field_def: &Value) -> i64 {
    if mark == "rect" && is_discrete(field_def) {
        return 1;
    }
    0
}

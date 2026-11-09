//! Port of `src/channeldef.ts`.
//!
//! The flexible TypeScript interfaces (`FieldDef`, `TypedFieldDef`,
//! `DatumDef`, ...) are represented as [`serde_json::Value`] objects, while the
//! small helper options are ordinary Rust structs.  Every public function from
//! the TypeScript module is provided with a snake-case name.

use std::cell::RefCell;

use serde_json::{json, Map, Value};

use crate::channeldef_support as support;
use crate::channeldef_support::truthy;
use crate::log;
use crate::type_::get_full_name;
use crate::util::{
    flat_access_with_datum, get_first_defined, has_property, internal_field, is_primitive, omit,
    remove_path_from_field, replace_path_in_field, stringify, title_case,
};

// ---------------------------------------------------------------------------
// Type aliases and option structs
// ---------------------------------------------------------------------------

/// A primitive value (`number | string | boolean | null`).
pub type PrimitiveValue = Value;

/// A constant value definition: `{value: ...}`.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueDef {
    pub value: Value,
}

/// Reference to a repeated value: `{repeat: 'row' | 'column' | ...}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepeatRef {
    pub repeat: String,
}

/// Port of `FieldRefOption`.
#[derive(Debug, Clone, Default)]
pub struct FieldRefOption {
    /// Exclude bin, aggregate, timeUnit.
    pub nofn: bool,
    /// Wrap the field with `datum`, `parent`, or `datum.datum`.
    pub expr: Option<String>,
    /// Prepend fn with custom function prefix.
    pub prefix: Option<String>,
    /// Append suffix to the field ref for bin (default = `start`).
    pub bin_suffix: Option<String>,
    /// Append suffix to the field ref (general).
    pub suffix: Option<String>,
    /// Use the field name for `as` in a transform.
    pub for_as: bool,
}

/// Options accepted by [`init_channel_def`].
#[derive(Debug, Clone, Default)]
pub struct InitOpt {
    pub composite_mark: bool,
}

/// Options accepted by [`title`].
#[derive(Debug, Clone)]
pub struct TitleOpt {
    pub allow_disabling: bool,
    pub include_default: bool,
}

impl Default for TitleOpt {
    fn default() -> Self {
        Self {
            allow_disabling: false,
            include_default: true,
        }
    }
}

/// Options accepted by [`value_expr`].
#[derive(Debug, Clone, Default)]
pub struct ValueExprOpt {
    pub time_unit: Option<Value>,
    pub type_: Option<String>,
    pub wrap_time: bool,
    pub undefined_if_expr_not_required: bool,
}

/// Result of [`channel_compatibility`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelCompatibility {
    pub compatible: bool,
    pub warning: Option<String>,
}

impl ChannelCompatibility {
    fn yes() -> Self {
        Self {
            compatible: true,
            warning: None,
        }
    }

    fn no(warning: impl Into<String>) -> Self {
        Self {
            compatible: false,
            warning: Some(warning.into()),
        }
    }
}

// ---------------------------------------------------------------------------
// Basic type guards
// ---------------------------------------------------------------------------

/// Port of `isRepeatRef`.
pub fn is_repeat_ref(field: &Value) -> bool {
    !field.is_string() && has_property(field, "repeat")
}

/// Port of `toFieldDefBase`.
pub fn to_field_def_base(field_def: &Value) -> Value {
    let mut out = Map::new();
    if truthy(&field_def["timeUnit"]) {
        out.insert("timeUnit".to_string(), field_def["timeUnit"].clone());
    }
    if truthy(&field_def["bin"]) {
        out.insert("bin".to_string(), field_def["bin"].clone());
    }
    if truthy(&field_def["aggregate"]) {
        out.insert("aggregate".to_string(), field_def["aggregate"].clone());
    }
    out.insert(
        "field".to_string(),
        field_def.get("field").cloned().unwrap_or(Value::Null),
    );
    Value::Object(out)
}

/// Port of `isSortableFieldDef`.
pub fn is_sortable_field_def(field_def: &Value) -> bool {
    has_property(field_def, "sort")
}

/// Port of `isOrderOnlyDef`.
pub fn is_order_only_def(order_def: &Value) -> bool {
    has_property(order_def, "sort") && !has_property(order_def, "field")
}

/// Port of `isConditionalParameter`.
pub fn is_conditional_parameter(c: &Value) -> bool {
    has_property(c, "param")
}

/// Port of `isConditionalDef`.
pub fn is_conditional_def(channel_def: &Value) -> bool {
    has_property(channel_def, "condition")
}

/// Port of `hasConditionalFieldDef`.
pub fn has_conditional_field_def(channel_def: &Value) -> bool {
    let condition = &channel_def["condition"];
    truthy(condition) && !condition.is_array() && is_field_def(condition)
}

/// Port of `hasConditionalFieldOrDatumDef`.
pub fn has_conditional_field_or_datum_def(channel_def: &Value) -> bool {
    let condition = &channel_def["condition"];
    truthy(condition) && !condition.is_array() && is_field_or_datum_def(condition)
}

/// Port of `hasConditionalValueDef`.
pub fn has_conditional_value_def(channel_def: &Value) -> bool {
    let condition = &channel_def["condition"];
    truthy(condition) && (condition.is_array() || is_value_def(condition))
}

/// Port of `isFieldDef`.
pub fn is_field_def(channel_def: &Value) -> bool {
    has_property(channel_def, "field") || channel_def.get("aggregate").and_then(|a| a.as_str()) == Some("count")
}

/// Port of `channelDefType`.
pub fn channel_def_type(channel_def: &Value) -> Option<String> {
    channel_def
        .get("type")
        .and_then(|t| t.as_str())
        .map(str::to_string)
}

/// Port of `isDatumDef`.
pub fn is_datum_def(channel_def: &Value) -> bool {
    has_property(channel_def, "datum")
}

/// Port of `isTypedFieldDef`.
pub fn is_typed_field_def(channel_def: &Value) -> bool {
    truthy(channel_def)
        && (has_property(channel_def, "field")
            || channel_def.get("aggregate").and_then(|a| a.as_str()) == Some("count"))
        && has_property(channel_def, "type")
}

/// Port of `isValueDef`.
pub fn is_value_def(channel_def: &Value) -> bool {
    has_property(channel_def, "value")
}

/// Port of `isFieldOrDatumDef`.
pub fn is_field_or_datum_def(channel_def: &Value) -> bool {
    is_field_def(channel_def) || is_datum_def(channel_def)
}

/// Port of `isScaleFieldDef`.
pub fn is_scale_field_def(channel_def: &Value) -> bool {
    has_property(channel_def, "scale") || has_property(channel_def, "sort")
}

/// Port of `isPositionFieldOrDatumDef`.
pub fn is_position_field_or_datum_def(channel_def: &Value) -> bool {
    has_property(channel_def, "axis")
        || has_property(channel_def, "stack")
        || has_property(channel_def, "impute")
}

/// Port of `isMarkPropFieldOrDatumDef`.
pub fn is_mark_prop_field_or_datum_def(channel_def: &Value) -> bool {
    has_property(channel_def, "legend")
}

/// Port of `isStringFieldOrDatumDef`.
pub fn is_string_field_or_datum_def(channel_def: &Value) -> bool {
    has_property(channel_def, "format") || has_property(channel_def, "formatType")
}

/// Port of `isFacetFieldDef`.
pub fn is_facet_field_def(channel_def: &Value) -> bool {
    has_property(channel_def, "header")
}

/// Port of `isNumericDataDef`.
pub fn is_numeric_data_def(channel_def: &Value) -> bool {
    is_datum_def(channel_def) && channel_def["datum"].is_number()
}

/// Port of `isContinuousFieldOrDatumDef`.
pub fn is_continuous_field_or_datum_def(channel_def: &Value) -> bool {
    (is_typed_field_def(channel_def) && !is_discrete(channel_def)) || is_numeric_data_def(channel_def)
}

/// Port of `isUnbinnedQuantitativeFieldOrDatumDef`.
pub fn is_unbinned_quantitative_field_or_datum_def(channel_def: &Value) -> bool {
    (is_typed_field_def(channel_def)
        && channel_def.get("type").and_then(|t| t.as_str()) == Some("quantitative")
        && !truthy(&channel_def["bin"]))
        || is_numeric_data_def(channel_def)
}

/// Port of `toStringFieldDef`.
pub fn to_string_field_def(field_def: &Value) -> Value {
    let omitted = ["legend", "axis", "header", "scale"];
    match field_def.as_object() {
        Some(map) => Value::Object(omit(map, &omitted)),
        None => field_def.clone(),
    }
}

// ---------------------------------------------------------------------------
// vgField
// ---------------------------------------------------------------------------

fn is_op_field_def(field_def: &Value) -> bool {
    has_property(field_def, "op")
}

/// Port of `isCount`.
pub fn is_count(field_def: &Value) -> bool {
    field_def.get("aggregate").and_then(|a| a.as_str()) == Some("count")
}

/// Port of `vgField` with the default options.
pub fn vg_field(field_def: &Value) -> String {
    vg_field_with(field_def, &FieldRefOption::default())
}

/// Port of `vgField(fieldDef, opt)`.
pub fn vg_field_with(field_def: &Value, opt: &FieldRefOption) -> String {
    let mut field = field_def
        .get("field")
        .map(support::js_to_string)
        .unwrap_or_default();
    let mut suffix = opt.suffix.clone();
    let mut arg_accessor = String::new();

    if is_count(field_def) {
        field = internal_field("count");
    } else {
        let mut f: Option<String> = None;

        if !opt.nofn {
            if is_op_field_def(field_def) {
                f = support::get_aggregate_op(&field_def["op"]);
            } else {
                let bin = &field_def["bin"];
                let aggregate = field_def.get("aggregate").cloned().unwrap_or(Value::Null);
                let time_unit = field_def.get("timeUnit").cloned().unwrap_or(Value::Null);

                if support::is_binning(bin) {
                    f = Some(support::bin_to_string(bin));
                    suffix = Some(
                        opt.bin_suffix.clone().unwrap_or_default() + &opt.suffix.clone().unwrap_or_default(),
                    );
                } else if truthy(&aggregate) {
                    if support::is_argmax_def(&aggregate) {
                        arg_accessor = format!("[\"{field}\"]");
                        field = format!("argmax_{}", support::js_to_string(&aggregate["argmax"]));
                    } else if support::is_argmin_def(&aggregate) {
                        arg_accessor = format!("[\"{field}\"]");
                        field = format!("argmin_{}", support::js_to_string(&aggregate["argmin"]));
                    } else {
                        f = support::get_aggregate_op(&aggregate);
                    }
                } else if truthy(&time_unit) && !support::is_binned_time_unit(&time_unit) {
                    f = Some(support::time_unit_to_string(&time_unit));
                    let bin_suffix = opt.bin_suffix.clone().unwrap_or_default();
                    suffix = Some(
                        (if bin_suffix != "range" && bin_suffix != "mid" {
                            bin_suffix
                        } else {
                            String::new()
                        }) + &opt.suffix.clone().unwrap_or_default(),
                    );
                }
            }
        }

        if let Some(f) = f {
            field = if !field.is_empty() {
                format!("{f}_{field}")
            } else {
                f
            };
        }
    }

    if let Some(suffix) = suffix {
        if !suffix.is_empty() {
            field = format!("{field}_{suffix}");
        }
    }

    if let Some(prefix) = &opt.prefix {
        field = format!("{prefix}_{field}");
    }

    if opt.for_as {
        remove_path_from_field(&field)
    } else if let Some(expr) = &opt.expr {
        flat_access_with_datum(&field, expr) + &arg_accessor
    } else {
        replace_path_in_field(&field) + &arg_accessor
    }
}

// ---------------------------------------------------------------------------
// isDiscrete / isDiscretizing
// ---------------------------------------------------------------------------

/// Port of `isDiscrete`. Panics for an invalid field type, like the TS version
/// throws.
pub fn is_discrete(def: &Value) -> bool {
    let type_str = def.get("type").and_then(|t| t.as_str()).unwrap_or("undefined");
    match type_str {
        "nominal" | "ordinal" | "geojson" => true,
        "quantitative" => is_field_def(def) && truthy(&def["bin"]),
        "temporal" => false,
        _ => panic!("{}", log::message::invalid_field_type(type_str)),
    }
}

/// Port of `isDiscretizing`.
pub fn is_discretizing(def: &Value) -> bool {
    if !is_scale_field_def(def) {
        return false;
    }
    let scale_type = def.get("scale").and_then(|s| s.get("type")).and_then(|t| t.as_str());
    scale_type.map(support::is_continuous_to_discrete).unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Titles
// ---------------------------------------------------------------------------

/// Port of `verbalTitleFormatter`.
pub fn verbal_title_formatter(field_def: &Value, config: &Value) -> String {
    let field = field_def
        .get("field")
        .map(support::js_to_string)
        .unwrap_or_default();
    let bin = &field_def["bin"];
    let time_unit = field_def.get("timeUnit").cloned().unwrap_or(Value::Null);
    let aggregate = field_def.get("aggregate").cloned().unwrap_or(Value::Null);

    if aggregate.as_str() == Some("count") {
        return config
            .get("countTitle")
            .and_then(|v| v.as_str())
            .unwrap_or("Count of Records")
            .to_string();
    } else if support::is_binning(bin) {
        return format!("{field} (binned)");
    } else if truthy(&time_unit) && !support::is_binned_time_unit(&time_unit) {
        let normalized = support::normalize_time_unit(&time_unit);
        if let Some(unit) = normalized.get("unit").and_then(|u| u.as_str()) {
            return format!("{field} ({})", support::get_time_unit_parts(unit).join("-"));
        }
    } else if truthy(&aggregate) {
        if support::is_argmax_def(&aggregate) {
            return format!("{field} for max {}", support::js_to_string(&aggregate["argmax"]));
        } else if support::is_argmin_def(&aggregate) {
            return format!("{field} for min {}", support::js_to_string(&aggregate["argmin"]));
        } else {
            let op = support::get_aggregate_op(&aggregate).unwrap_or_default();
            return format!("{} of {field}", title_case(&op));
        }
    }
    field
}

/// Port of `functionalTitleFormatter`.
pub fn functional_title_formatter(field_def: &Value) -> String {
    let aggregate = field_def.get("aggregate").cloned().unwrap_or(Value::Null);
    let bin = &field_def["bin"];
    let time_unit = field_def.get("timeUnit").cloned().unwrap_or(Value::Null);
    let field = field_def
        .get("field")
        .map(support::js_to_string)
        .unwrap_or_default();

    if support::is_argmax_def(&aggregate) {
        return format!("{field} for argmax({})", support::js_to_string(&aggregate["argmax"]));
    } else if support::is_argmin_def(&aggregate) {
        return format!("{field} for argmin({})", support::js_to_string(&aggregate["argmin"]));
    }

    let time_unit_params = if truthy(&time_unit) && !support::is_binned_time_unit(&time_unit) {
        Some(support::normalize_time_unit(&time_unit))
    } else {
        None
    };

    let fn_name = support::get_aggregate_op(&aggregate)
        .or_else(|| {
            time_unit_params
                .as_ref()
                .and_then(|p| p.get("unit"))
                .and_then(|u| u.as_str())
                .map(str::to_string)
        })
        .or_else(|| {
            let has_maxbins = time_unit_params
                .as_ref()
                .map(|p| truthy(&p["maxbins"]))
                .unwrap_or(false);
            if has_maxbins {
                Some("timeunit".to_string())
            } else {
                None
            }
        })
        .or_else(|| {
            if support::is_binning(bin) {
                Some("bin".to_string())
            } else {
                None
            }
        });

    match fn_name {
        Some(f) => format!("{}({field})", f.to_uppercase()),
        None => field,
    }
}

/// Port of `defaultTitleFormatter`.
pub fn default_title_formatter(field_def: &Value, config: &Value) -> String {
    match config.get("fieldTitle").and_then(|v| v.as_str()) {
        Some("plain") => field_def
            .get("field")
            .map(support::js_to_string)
            .unwrap_or_default(),
        Some("functional") => functional_title_formatter(field_def),
        _ => verbal_title_formatter(field_def, config),
    }
}

/// Signature of a field title formatter.
pub type FieldTitleFormatter = fn(&Value, &Value) -> String;

thread_local! {
    static TITLE_FORMATTER: RefCell<Option<FieldTitleFormatter>> = const { RefCell::new(None) };
}

/// Port of `setTitleFormatter`.
pub fn set_title_formatter(formatter: FieldTitleFormatter) {
    TITLE_FORMATTER.with(|f| *f.borrow_mut() = Some(formatter));
}

/// Port of `resetTitleFormatter`.
pub fn reset_title_formatter() {
    TITLE_FORMATTER.with(|f| *f.borrow_mut() = None);
}

fn current_title_formatter() -> FieldTitleFormatter {
    TITLE_FORMATTER
        .with(|f| *f.borrow())
        .unwrap_or(default_title_formatter as FieldTitleFormatter)
}

/// Port of `defaultTitle`.
pub fn default_title(field_def: &Value, config: &Value) -> String {
    current_title_formatter()(field_def, config)
}

/// Port of `getGuide`. Returns a reference to the guide definition, if any.
pub fn get_guide(field_def: &Value) -> Option<&Value> {
    if is_position_field_or_datum_def(field_def) && truthy(&field_def["axis"]) {
        Some(&field_def["axis"])
    } else if is_mark_prop_field_or_datum_def(field_def) && truthy(&field_def["legend"]) {
        Some(&field_def["legend"])
    } else if is_facet_field_def(field_def) && truthy(&field_def["header"]) {
        Some(&field_def["header"])
    } else {
        None
    }
}

/// Port of `title`.
pub fn title(field_or_datum_def: &Value, config: &Value, opt: &TitleOpt) -> Value {
    let guide_title = get_guide(field_or_datum_def)
        .and_then(|g| g.get("title"))
        .filter(|t| !t.is_null())
        .cloned();

    if !is_field_def(field_or_datum_def) {
        return guide_title
            .or_else(|| {
                field_or_datum_def
                    .get("title")
                    .filter(|t| !t.is_null())
                    .cloned()
            })
            .unwrap_or(Value::Null);
    }
    let field_def = field_or_datum_def;
    let def = if opt.include_default {
        Some(Value::String(default_title(field_def, config)))
    } else {
        None
    };
    let field_title = field_def.get("title").filter(|t| !t.is_null()).cloned();

    if opt.allow_disabling {
        get_first_defined(&[guide_title.as_ref(), field_title.as_ref(), def.as_ref()])
            .cloned()
            .unwrap_or(Value::Null)
    } else {
        guide_title.or(field_title).or(def).unwrap_or(Value::Null)
    }
}

/// Port of `getFormatMixins`.
pub fn get_format_mixins(field_def: &Value) -> Value {
    let (format, format_type) = if is_string_field_or_datum_def(field_def) {
        (
            field_def.get("format").cloned(),
            field_def.get("formatType").cloned(),
        )
    } else {
        let guide = get_guide(field_def).cloned().unwrap_or_else(|| json!({}));
        (
            guide.get("format").cloned(),
            guide.get("formatType").cloned(),
        )
    };
    let mut out = Map::new();
    if let Some(f) = format {
        if !f.is_null() {
            out.insert("format".to_string(), f);
        }
    }
    if let Some(f) = format_type {
        if !f.is_null() {
            out.insert("formatType".to_string(), f);
        }
    }
    Value::Object(out)
}

// ---------------------------------------------------------------------------
// defaultType
// ---------------------------------------------------------------------------

/// Port of `defaultType`.
pub fn default_type(field_def: &Value, channel: &str) -> String {
    match channel {
        "latitude" | "longitude" => return "quantitative".to_string(),
        "row" | "column" | "facet" | "shape" | "strokeDash" => return "nominal".to_string(),
        "order" => return "ordinal".to_string(),
        _ => {}
    }

    if is_sortable_field_def(field_def) && field_def["sort"].is_array() {
        return "ordinal".to_string();
    }

    let aggregate = field_def.get("aggregate").cloned().unwrap_or(Value::Null);
    let bin = &field_def["bin"];
    let time_unit = field_def.get("timeUnit").cloned().unwrap_or(Value::Null);

    if truthy(&time_unit) {
        return "temporal".to_string();
    }

    if truthy(bin)
        || (truthy(&aggregate)
            && !support::is_argmax_def(&aggregate)
            && !support::is_argmin_def(&aggregate))
    {
        return "quantitative".to_string();
    }

    if is_scale_field_def(field_def) {
        if let Some(scale_type) = field_def
            .get("scale")
            .and_then(|s| s.get("type"))
            .and_then(|t| t.as_str())
        {
            match support::scale_category(scale_type) {
                Some("numeric") | Some("discretizing") => return "quantitative".to_string(),
                Some("time") => return "temporal".to_string(),
                _ => {}
            }
        }
    }

    "nominal".to_string()
}

// ---------------------------------------------------------------------------
// getFieldDef / getFieldOrDatumDef
// ---------------------------------------------------------------------------

/// Port of `getFieldDef`.
pub fn get_field_def(channel_def: &Value) -> Option<Value> {
    get_field_def_ref(channel_def).cloned()
}

/// Borrowing variant of [`get_field_def`].
pub fn get_field_def_ref(channel_def: &Value) -> Option<&Value> {
    if is_field_def(channel_def) {
        Some(channel_def)
    } else if has_conditional_field_def(channel_def) {
        Some(&channel_def["condition"])
    } else {
        None
    }
}

/// Port of `getFieldOrDatumDef`.
pub fn get_field_or_datum_def(channel_def: &Value) -> Option<Value> {
    get_field_or_datum_def_ref(channel_def).cloned()
}

/// Borrowing variant of [`get_field_or_datum_def`].
pub fn get_field_or_datum_def_ref(channel_def: &Value) -> Option<&Value> {
    if is_field_or_datum_def(channel_def) {
        Some(channel_def)
    } else if has_conditional_field_or_datum_def(channel_def) {
        Some(&channel_def["condition"])
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// initChannelDef
// ---------------------------------------------------------------------------

/// Port of `initChannelDef` with default options.
pub fn init_channel_def(channel_def: &Value, channel: &str, config: &Value) -> Value {
    init_channel_def_with(channel_def, channel, config, &InitOpt::default())
}

/// Port of `initChannelDef(channelDef, channel, config, opt)`.
pub fn init_channel_def_with(
    channel_def: &Value,
    channel: &str,
    config: &Value,
    opt: &InitOpt,
) -> Value {
    if is_primitive(channel_def) {
        let primitive_type = if channel_def.is_string() {
            "string"
        } else if channel_def.is_number() {
            "number"
        } else {
            "boolean"
        };
        log::warn(support::primitive_channel_def(channel, primitive_type, channel_def));
        return json!({"value": channel_def});
    }

    if is_field_or_datum_def(channel_def) {
        init_field_or_datum_def(channel_def, channel, config, opt)
    } else if has_conditional_field_or_datum_def(channel_def) {
        let mut out = channel_def.clone();
        out["condition"] = init_field_or_datum_def(&channel_def["condition"], channel, config, opt);
        out
    } else {
        channel_def.clone()
    }
}

/// Port of `initFieldOrDatumDef`.
pub fn init_field_or_datum_def(
    fd: &Value,
    channel: &str,
    config: &Value,
    opt: &InitOpt,
) -> Value {
    if is_string_field_or_datum_def(fd) {
        let format_type = fd.get("formatType").and_then(|f| f.as_str());
        let custom_format_types = truthy(config.get("customFormatTypes").unwrap_or(&Value::Null));
        if support::is_custom_format_type(format_type) && !custom_format_types {
            log::warn(support::custom_format_type_not_allowed(channel));
            let mut rest = fd.clone();
            if let Some(map) = rest.as_object_mut() {
                map.remove("format");
                map.remove("formatType");
            }
            return init_field_or_datum_def(&rest, channel, config, opt);
        }
    } else {
        let guide_type = if is_position_field_or_datum_def(fd) {
            Some("axis")
        } else if is_mark_prop_field_or_datum_def(fd) {
            Some("legend")
        } else if is_facet_field_def(fd) {
            Some("header")
        } else {
            None
        };
        if let Some(guide_type) = guide_type {
            if truthy(&fd[guide_type]) {
                let guide = &fd[guide_type];
                let format_type = guide.get("formatType").and_then(|f| f.as_str());
                let custom_format_types =
                    truthy(config.get("customFormatTypes").unwrap_or(&Value::Null));
                if support::is_custom_format_type(format_type) && !custom_format_types {
                    log::warn(support::custom_format_type_not_allowed(channel));
                    let mut new_guide = guide.clone();
                    if let Some(map) = new_guide.as_object_mut() {
                        map.remove("format");
                        map.remove("formatType");
                    }
                    let mut new_fd = fd.clone();
                    new_fd[guide_type] = new_guide;
                    return init_field_or_datum_def(&new_fd, channel, config, opt);
                }
            }
        }
    }

    if is_field_def(fd) {
        init_field_def(fd, channel, opt)
    } else {
        init_datum_def(fd)
    }
}

/// Port of `initDatumDef` (private in TypeScript).
pub fn init_datum_def(datum_def: &Value) -> Value {
    if truthy(&datum_def["type"]) {
        return datum_def.clone();
    }
    let datum = &datum_def["datum"];
    let type_ = if datum.is_number() {
        Some("quantitative")
    } else if datum.is_string() {
        Some("nominal")
    } else if support::is_date_time(datum) {
        Some("temporal")
    } else {
        None
    };

    let mut out = datum_def.clone();
    match type_ {
        Some(t) => out["type"] = Value::String(t.to_string()),
        None => {
            if let Some(map) = out.as_object_mut() {
                map.remove("type");
            }
        }
    }
    out
}

/// Port of `initFieldDef`.
pub fn init_field_def(fd: &Value, channel: &str, opt: &InitOpt) -> Value {
    let aggregate = fd.get("aggregate").cloned().unwrap_or(Value::Null);
    let time_unit = fd.get("timeUnit").cloned().unwrap_or(Value::Null);
    let bin = fd.get("bin").cloned().unwrap_or(Value::Null);
    let field = fd.get("field").cloned().unwrap_or(Value::Null);
    let mut field_def = fd.clone();

    // Drop invalid aggregate
    if !opt.composite_mark
        && truthy(&aggregate)
        && !support::is_aggregate_op(&aggregate)
        && !support::is_argmax_def(&aggregate)
        && !support::is_argmin_def(&aggregate)
        && !support::is_parameterized_aggregate_def(&aggregate)
    {
        log::warn(log::message::invalid_aggregate(&support::js_to_string(&aggregate)));
        if let Some(map) = field_def.as_object_mut() {
            map.remove("aggregate");
        }
    }

    // Normalize Time Unit
    if truthy(&time_unit) {
        field_def["timeUnit"] = support::normalize_time_unit(&time_unit);
    }

    if truthy(&field) {
        field_def["field"] = Value::String(support::js_to_string(&field));
    }

    // Normalize bin
    if support::is_binning(&bin) {
        field_def["bin"] = normalize_bin(&bin, Some(channel));
    }

    if support::is_binned(&bin) && !support::is_xor_y(channel) {
        log::warn(support::channel_should_not_be_used_for_binned(channel));
    }

    // Normalize Type
    if is_typed_field_def(&field_def) {
        let type_str = field_def
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("undefined")
            .to_string();
        if let Some(full_type) = get_full_name(&type_str) {
            if type_str != full_type.as_str() {
                field_def["type"] = Value::String(full_type.as_str().to_string());
            }
        } else if let Some(map) = field_def.as_object_mut() {
            map.remove("type");
        }
        if type_str != "quantitative" && support::is_counting_aggregate_op(&aggregate) {
            log::warn(log::message::invalid_field_type_for_count_aggregate(
                &type_str,
                &support::js_to_string(&aggregate),
            ));
            field_def["type"] = Value::String("quantitative".to_string());
        }
    } else if !support::is_secondary_range_channel(channel) {
        // If type is empty / invalid, then augment with default type
        let new_type = default_type(&field_def, channel);
        field_def["type"] = Value::String(new_type);
    }

    if is_typed_field_def(&field_def) {
        let compatibility = channel_compatibility(&field_def, channel);
        if !compatibility.compatible {
            if let Some(warning) = compatibility.warning {
                log::warn(warning);
            }
        }
    }

    if is_sortable_field_def(&field_def) {
        if let Some(sort) = field_def.get("sort").and_then(|s| s.as_str()) {
            let sort = sort.to_string();
            if support::is_sort_by_channel(&sort) {
                field_def["sort"] = json!({"encoding": sort});
                return field_def;
            }
            if let Some(sub) = sort.strip_prefix('-') {
                if support::is_sort_by_channel(sub) {
                    field_def["sort"] = json!({"encoding": sub, "order": "descending"});
                    return field_def;
                }
            }
        }
    }

    if is_facet_field_def(&field_def)
        && truthy(&field_def["header"]) {
            let header = field_def["header"].clone();
            let orient = header.get("orient").cloned().unwrap_or(Value::Null);
            if truthy(&orient) {
                let mut rest = header.clone();
                if let Some(map) = rest.as_object_mut() {
                    map.remove("orient");
                }
                let label_orient = header
                    .get("labelOrient")
                    .filter(|v| truthy(v))
                    .cloned()
                    .unwrap_or_else(|| orient.clone());
                let title_orient = header
                    .get("titleOrient")
                    .filter(|v| truthy(v))
                    .cloned()
                    .unwrap_or_else(|| orient.clone());
                rest["labelOrient"] = label_orient;
                rest["titleOrient"] = title_orient;
                field_def["header"] = rest;
                return field_def;
            }
        }

    field_def
}

/// Port of `normalizeBin`.
pub fn normalize_bin(bin: &Value, channel: Option<&str>) -> Value {
    if bin.is_boolean() {
        json!({"maxbins": support::auto_max_bins(channel)})
    } else if bin.as_str() == Some("binned") {
        json!({"binned": true})
    } else if !truthy(&bin["maxbins"]) && !truthy(&bin["step"]) {
        let mut out = bin.clone();
        out["maxbins"] = json!(support::auto_max_bins(channel));
        out
    } else {
        bin.clone()
    }
}

// ---------------------------------------------------------------------------
// channelCompatibility
// ---------------------------------------------------------------------------

/// Port of `channelCompatibility`.
pub fn channel_compatibility(field_def: &Value, channel: &str) -> ChannelCompatibility {
    let type_str = field_def
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("undefined");

    if type_str == "geojson" && channel != "shape" {
        return ChannelCompatibility::no(format!(
            "Channel {channel} should not be used with a geojson data."
        ));
    }

    match channel {
        "row" | "column" | "facet" => {
            if !is_discrete(field_def) {
                return ChannelCompatibility::no(log::message::channel_should_be_discrete(channel));
            }
            ChannelCompatibility::yes()
        }

        "x" | "y" | "xOffset" | "yOffset" | "color" | "fill" | "stroke" | "text" | "detail"
        | "key" | "tooltip" | "href" | "url" | "angle" | "theta" | "radius" | "description" => {
            ChannelCompatibility::yes()
        }

        "longitude" | "longitude2" | "latitude" | "latitude2" => {
            if type_str != "quantitative" {
                return ChannelCompatibility::no(format!(
                    "Channel {channel} should be used with a quantitative field only, not {type_str} field."
                ));
            }
            ChannelCompatibility::yes()
        }

        "opacity" | "fillOpacity" | "strokeOpacity" | "strokeWidth" | "size" | "theta2"
        | "radius2" | "x2" | "y2" | "time" => {
            if type_str == "nominal" && !truthy(&field_def["sort"]) {
                return ChannelCompatibility::no(format!(
                    "Channel {channel} should not be used with an unsorted discrete field."
                ));
            }
            ChannelCompatibility::yes()
        }

        "shape" | "strokeDash" => {
            if !is_discrete(field_def) && !is_discretizing(field_def) {
                return ChannelCompatibility::no(
                    support::channel_should_be_discrete_or_discretizing(channel),
                );
            }
            ChannelCompatibility::yes()
        }

        "order" => {
            if type_str == "nominal" && !field_def.as_object().map(|m| m.contains_key("sort")).unwrap_or(false)
            {
                return ChannelCompatibility::no(
                    "Channel order is inappropriate for nominal field, which has no inherent order."
                        .to_string(),
                );
            }
            ChannelCompatibility::yes()
        }

        _ => ChannelCompatibility::yes(),
    }
}

// ---------------------------------------------------------------------------
// Time formats / value expressions
// ---------------------------------------------------------------------------

/// Port of `isFieldOrDatumDefForTimeFormat`.
pub fn is_field_or_datum_def_for_time_format(field_or_datum_def: &Value) -> bool {
    let mixins = get_format_mixins(field_or_datum_def);
    let format_type = mixins.get("formatType").and_then(|f| f.as_str());
    format_type == Some("time")
        || (format_type.is_none() && is_temporal_field_def(field_or_datum_def))
}

/// Port of `isTemporalFieldDef`.
pub fn is_temporal_field_def(def: &Value) -> bool {
    truthy(def)
        && (def.get("type").and_then(|t| t.as_str()) == Some("temporal")
            || (is_field_def(def) && truthy(&def["timeUnit"])))
}

/// Port of `valueExpr`. Returns `None` where TypeScript returns `undefined`.
pub fn value_expr(v: &Value, opt: &ValueExprOpt) -> Option<String> {
    let time_unit = opt.time_unit.clone().unwrap_or(Value::Null);
    let unit = if truthy(&time_unit) {
        support::normalize_time_unit(&time_unit)
            .get("unit")
            .and_then(|u| u.as_str())
            .map(str::to_string)
    } else {
        None
    };
    let mut is_time = unit.is_some() || opt.type_.as_deref() == Some("temporal");

    let mut expr: Option<String> = None;
    if support::is_expr_ref(v) {
        expr = v.get("expr").and_then(|e| e.as_str()).map(str::to_string);
    } else if support::is_signal_ref(v) {
        expr = v.get("signal").and_then(|s| s.as_str()).map(str::to_string);
    } else if support::is_date_time(v) {
        is_time = true;
        expr = Some(support::date_time_to_expr(v));
    } else if (v.is_string() || v.is_number())
        && is_time {
            let mut e = format!("datetime({})", stringify(v));
            if let Some(unit) = &unit {
                if support::is_local_single_time_unit(unit) {
                    let needs_conversion = (v.is_number()
                        && v.as_f64().map(|n| n < 10000.0).unwrap_or(false))
                        || (v.is_string()
                            && support::date_parse_is_nan(v.as_str().unwrap_or_default()));
                    if needs_conversion {
                        e = support::date_time_to_expr(&json!({unit.clone(): v.clone()}));
                    }
                }
            }
            expr = Some(e);
        }

    if let Some(expr) = expr {
        return Some(if opt.wrap_time && is_time {
            format!("time({expr})")
        } else {
            expr
        });
    }
    if opt.undefined_if_expr_not_required {
        None
    } else {
        Some(stringify(v))
    }
}

/// Port of `valueArray`.
pub fn value_array(field_or_datum_def: &Value, values: &[Value]) -> Vec<Value> {
    let type_ = field_or_datum_def
        .get("type")
        .and_then(|t| t.as_str())
        .map(str::to_string);

    values
        .iter()
        .map(|v| {
            let time_unit = if is_field_def(field_or_datum_def) {
                let tu = field_or_datum_def.get("timeUnit").cloned().unwrap_or(Value::Null);
                if !support::is_binned_time_unit(&tu) && truthy(&tu) {
                    Some(tu)
                } else {
                    None
                }
            } else {
                None
            };
            let expr = value_expr(
                v,
                &ValueExprOpt {
                    time_unit,
                    type_: type_.clone(),
                    wrap_time: false,
                    undefined_if_expr_not_required: true,
                },
            );
            match expr {
                Some(expr) => json!({"signal": expr}),
                None => v.clone(),
            }
        })
        .collect()
}

/// Port of `binRequiresRange`.
pub fn bin_requires_range(field_def: &Value, channel: &str) -> bool {
    if !support::is_binning(&field_def["bin"]) {
        log::warn("Only call this method for binned field defs.");
        return false;
    }
    let type_str = field_def.get("type").and_then(|t| t.as_str()).unwrap_or("");
    support::is_scale_channel(channel) && (type_str == "ordinal" || type_str == "nominal")
}

// ---------------------------------------------------------------------------
// Band helpers
// ---------------------------------------------------------------------------

fn get_band_position(
    field_def: &Value,
    field_def2: Option<&Value>,
    mark_def: &Value,
    config: &Value,
) -> Option<Value> {
    if is_field_or_datum_def(field_def) {
        if let Some(bp) = field_def.get("bandPosition") {
            if !bp.is_null() {
                return Some(bp.clone());
            }
        }
    }
    if is_field_def(field_def) {
        let time_unit = field_def.get("timeUnit").cloned().unwrap_or(Value::Null);
        let bin = &field_def["bin"];
        if truthy(&time_unit) && field_def2.is_none() {
            return support::get_mark_config("timeUnitBandPosition", mark_def, config);
        } else if support::is_binning(bin) {
            return Some(json!(0.5));
        }
    }
    None
}

/// Port of `getBandPosition`.
pub fn get_band_position_pub(
    field_def: &Value,
    field_def2: Option<&Value>,
    mark_def: &Value,
    config: &Value,
) -> Option<Value> {
    get_band_position(field_def, field_def2, mark_def, config)
}

/// Port of `getBandSize`.
pub fn get_band_size(
    channel: &str,
    field_def: &Value,
    field_def2: Option<&Value>,
    mark_def: &Value,
    config: &Value,
    scale_type: Option<&str>,
    use_vl_size_channel: bool,
) -> Value {
    let size_channel = support::get_size_channel(channel);
    let prop = if use_vl_size_channel {
        "size"
    } else {
        size_channel.unwrap_or("")
    };
    let size = support::get_mark_prop_or_config(prop, mark_def, config, size_channel, false);
    if let Some(size) = size {
        return size;
    }

    if is_field_def(field_def) {
        let time_unit = field_def.get("timeUnit").cloned().unwrap_or(Value::Null);
        let bin = &field_def["bin"];
        if truthy(&time_unit) && field_def2.is_none() {
            let band = support::get_mark_config("timeUnitBandSize", mark_def, config)
                .unwrap_or(Value::Null);
            return json!({"band": band});
        } else if support::is_binning(bin)
            && !scale_type.map(support::has_discrete_domain).unwrap_or(false)
        {
            return json!({"band": 1});
        }
    }

    let mark_type = mark_def.get("type").and_then(|t| t.as_str()).unwrap_or("");
    if support::is_rect_based_mark(mark_type) {
        let mark_config = config.get(mark_type);
        if let Some(scale_type) = scale_type {
            if support::has_discrete_domain(scale_type) {
                return mark_config
                    .and_then(|m| m.get("discreteBandSize"))
                    .filter(|v| truthy(v))
                    .cloned()
                    .unwrap_or_else(|| json!({"band": 1}));
            } else {
                return mark_config
                    .and_then(|m| m.get("continuousBandSize"))
                    .cloned()
                    .unwrap_or(Value::Null);
            }
        }
        return mark_config
            .and_then(|m| m.get("discreteBandSize"))
            .cloned()
            .unwrap_or(Value::Null);
    }

    Value::Null
}

/// Port of `hasBandEnd`.
pub fn has_band_end(
    field_def: &Value,
    field_def2: Option<&Value>,
    mark_def: &Value,
    config: &Value,
) -> bool {
    let time_unit = field_def.get("timeUnit").cloned().unwrap_or(Value::Null);
    if support::is_binning(&field_def["bin"])
        || (truthy(&time_unit)
            && is_typed_field_def(field_def)
            && field_def.get("type").and_then(|t| t.as_str()) == Some("temporal"))
    {
        return get_band_position(field_def, field_def2, mark_def, config).is_some();
    }
    false
}

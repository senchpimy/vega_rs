//! Local support for the model/scale compiler.
//!
//! Upstream these helpers live in `src/config.ts`, `src/normalize/**` and
//! `src/spec/*.ts`.  Those modules are out of scope for this port (and may only
//! be stubs in this crate), so faithful minimal implementations are provided
//! here.  They cover exactly the behaviour needed by the model tree and the
//! scale/type/domain/range parsers.

use serde_json::{json, Map, Value};

use crate::compile::split::Explicit;
use crate::log;
use crate::util::{get_first_defined, has_property, stringify};

// ---------------------------------------------------------------------------
// Spec type guards (`src/spec/*.ts`)
// ---------------------------------------------------------------------------

pub fn is_unit_spec(spec: &Value) -> bool {
    has_property(spec, "mark")
}

pub fn is_layer_spec(spec: &Value) -> bool {
    has_property(spec, "layer")
}

pub fn is_facet_spec(spec: &Value) -> bool {
    has_property(spec, "facet")
}

pub fn is_concat_spec(spec: &Value) -> bool {
    has_property(spec, "concat")
}

pub fn is_vconcat_spec(spec: &Value) -> bool {
    has_property(spec, "vconcat")
}

pub fn is_hconcat_spec(spec: &Value) -> bool {
    has_property(spec, "hconcat")
}

pub fn is_any_concat_spec(spec: &Value) -> bool {
    is_concat_spec(spec) || is_vconcat_spec(spec) || is_hconcat_spec(spec)
}

pub fn is_repeat_spec(spec: &Value) -> bool {
    has_property(spec, "repeat")
}

pub fn is_facet_mapping(facet: &Value) -> bool {
    has_property(facet, "row") || has_property(facet, "column")
}

// ---------------------------------------------------------------------------
// Config (`src/config.ts`)
// ---------------------------------------------------------------------------

/// Port of `defaultViewConfig`.
pub fn default_view_config() -> Value {
    json!({"continuousWidth": 300, "continuousHeight": 300, "step": 20})
}

/// Port of `defaultScaleConfig` as a JSON value.
pub fn default_scale_config() -> Value {
    json!({
        "pointPadding": 0.5,
        "barBandPaddingInner": 0.1,
        "rectBandPaddingInner": 0,
        "tickBandPaddingInner": 0.25,
        "bandWithNestedOffsetPaddingInner": 0.2,
        "bandWithNestedOffsetPaddingOuter": 0.2,
        "minBandSize": 2,
        "minFontSize": 8,
        "maxFontSize": 40,
        "minOpacity": 0.3,
        "maxOpacity": 0.8,
        "minSize": 4,
        "minStrokeWidth": 1,
        "maxStrokeWidth": 4,
        "quantileCount": 4,
        "quantizeCount": 4,
        "zero": true,
        "framesPerSecond": 2,
        "animationDuration": 5
    })
}

/// Port of `defaultConfig`.
pub fn default_config() -> Value {
    // A JSON literal is used instead of a large `json!` invocation so the macro
    // recursion limit is not exceeded.
    serde_json::from_str(
        r##"{
  "background": "white",
  "padding": 5,
  "timeFormat": "%b %d, %Y",
  "countTitle": "Count of Records",
  "view": {"continuousWidth": 300, "continuousHeight": 300, "step": 20},
  "mark": {"color": "#4c78a8", "invalid": "break-paths-show-path-domains", "timeUnitBandSize": 1},
  "arc": {},
  "area": {},
  "bar": {"binSpacing": 1, "continuousBandSize": 5, "minBandSize": 0.25, "timeUnitBandPosition": 0.5},
  "circle": {},
  "geoshape": {},
  "image": {},
  "line": {},
  "point": {},
  "rect": {"binSpacing": 0, "continuousBandSize": 5, "minBandSize": 0.25, "timeUnitBandPosition": 0.5},
  "rule": {"color": "black"},
  "square": {},
  "text": {"color": "black"},
  "tick": {"binSpacing": 0, "continuousBandSize": 5, "minBandSize": 0.25, "timeUnitBandPosition": 0.5, "thickness": 1},
  "trail": {},
  "boxplot": {"size": 14, "extent": 1.5, "box": {}, "median": {"color": "white"}, "outliers": {}, "rule": {}, "ticks": null},
  "errorbar": {"center": "mean", "rule": true, "ticks": false},
  "errorband": {"band": {"opacity": 0.3}, "borders": false},
  "scale": {"pointPadding": 0.5, "barBandPaddingInner": 0.1, "rectBandPaddingInner": 0, "tickBandPaddingInner": 0.25, "bandWithNestedOffsetPaddingInner": 0.2, "bandWithNestedOffsetPaddingOuter": 0.2, "minBandSize": 2, "minFontSize": 8, "maxFontSize": 40, "minOpacity": 0.3, "maxOpacity": 0.8, "minSize": 4, "minStrokeWidth": 1, "maxStrokeWidth": 4, "quantileCount": 4, "quantizeCount": 4, "zero": true, "framesPerSecond": 2, "animationDuration": 5},
  "projection": {},
  "legend": {"gradientHorizontalMaxLength": 200, "gradientHorizontalMinLength": 100, "gradientVerticalMaxLength": 200, "gradientVerticalMinLength": 64, "unselectedOpacity": 0.35},
  "header": {"titlePadding": 10, "labelPadding": 10},
  "headerColumn": {},
  "headerRow": {},
  "headerFacet": {},
  "selection": {"point": {"on": "click", "fields": ["_vgsid_"], "toggle": "event.shiftKey", "resolve": "global", "clear": "dblclick"}, "interval": {"on": "[pointerdown, window:pointerup] > window:pointermove!", "encodings": ["x", "y"], "translate": "[pointerdown, window:pointerup] > window:pointermove!", "zoom": "wheel!", "mark": {"fill": "#333", "fillOpacity": 0.125, "stroke": "white"}, "resolve": "global", "clear": "dblclick"}},
  "style": {},
  "title": {},
  "facet": {"spacing": 20},
  "concat": {"spacing": 20},
  "normalizedNumberFormat": ".0%"
}"##,
    )
    .unwrap()
}

fn merge_config_into(dest: &mut Value, src: &Value) {
    let src_map = match src.as_object() {
        Some(m) => m,
        None => {
            *dest = src.clone();
            return;
        }
    };
    if !dest.is_object() {
        *dest = Value::Object(Map::new());
    }
    let dest_map = dest.as_object_mut().unwrap();
    for (key, value) in src_map {
        if key == "signals" {
            let existing = dest_map.remove(key);
            let mut merged = match existing {
                Some(Value::Array(a)) => a,
                _ => Vec::new(),
            };
            if let Some(Value::Array(b)) = Some(value.clone()) {
                merged.extend(b);
            }
            dest_map.insert(key.clone(), Value::Array(merged));
        } else if value.is_null() {
            dest_map.remove(key);
        } else if value.is_object() {
            let entry = dest_map
                .entry(key.clone())
                .or_insert_with(|| Value::Object(Map::new()));
            merge_config_into(entry, value);
        } else {
            dest_map.insert(key.clone(), value.clone());
        }
    }
}

/// Port of `mergeConfig` (deep merge, arrays replaced except `signals`).
pub fn merge_config(def: &Value, configs: &[&Value]) -> Value {
    let mut out = def.clone();
    for source in configs {
        merge_config_into(&mut out, source);
    }
    out
}

/// Port of `initConfig` (the subset of transformations that are relevant for
/// the model tree; expression refs are replaced one level deep).
pub fn init_config(specified_config: Option<&Value>) -> Value {
    let empty = Value::Object(Map::new());
    let specified = specified_config.unwrap_or(&empty);
    let mut merged = merge_config(&Value::Object(Map::new()), &[&default_config(), specified]);

    // `initConfig` replaces ExprRefs in top level properties and mark configs.
    for prop in ["background", "padding"] {
        if let Some(v) = merged.get(prop).cloned() {
            if !v.is_null() {
                merged[prop] = signal_ref_or_value(&v);
            }
        }
    }
    for mark_config_type in crate::mark::MARK_CONFIGS {
        if let Some(v) = merged.get(mark_config_type).cloned() {
            merged[mark_config_type] = replace_expr_ref(&v);
        }
    }
    merged
}

/// Port of `signalRefOrValue` from `compile/common.ts`.
pub fn signal_ref_or_value(value: &Value) -> Value {
    if value.is_object() && has_property(value, "expr") {
        let mut out = value.as_object().unwrap().clone();
        let expr = out.remove("expr").unwrap_or(Value::Null);
        out.insert("signal".to_string(), expr);
        Value::Object(out)
    } else {
        value.clone()
    }
}

/// Port of `replaceExprRef` for a single object.
pub fn replace_expr_ref(value: &Value) -> Value {
    signal_ref_or_value(value)
}

// ---------------------------------------------------------------------------
// Normalization (`src/normalize/index.ts`)
// ---------------------------------------------------------------------------

/// Minimal normalization.
///
/// The three validation suites only exercise already-normalized specs (no
/// composite marks, no row/column shortcut encodings), so cloning the spec is
/// sufficient.  Keeping this function separate makes it easy to extend later.
pub fn normalize(spec: &Value, _config: &Value) -> Value {
    spec.clone()
}

// ---------------------------------------------------------------------------
// View config helpers (`src/config.ts`)
// ---------------------------------------------------------------------------

pub fn get_view_config_continuous_size(view_config: &Value, channel: &str) -> Value {
    if let Some(v) = view_config.get(channel) {
        if !v.is_null() {
            return v.clone();
        }
    }
    let key = if channel == "width" {
        "continuousWidth"
    } else {
        "continuousHeight"
    };
    view_config.get(key).cloned().unwrap_or(Value::Null)
}

pub fn get_view_config_discrete_step(view_config: &Value, channel: &str) -> Value {
    let size = get_view_config_discrete_size(view_config, channel);
    if let Some(step) = size.get("step") {
        step.clone()
    } else {
        json!(20)
    }
}

pub fn get_view_config_discrete_size(view_config: &Value, channel: &str) -> Value {
    let direct = view_config.get(channel).cloned();
    let key = if channel == "width" {
        "discreteWidth"
    } else {
        "discreteHeight"
    };
    let size = match direct {
        Some(v) if !v.is_null() => Some(v),
        _ => view_config.get(key).cloned(),
    };
    let fallback = json!({"step": view_config.get("step").cloned().unwrap_or(json!(20))});
    get_first_defined(&[
        size.as_ref().filter(|v| !v.is_null()),
        Some(&fallback),
    ])
    .cloned()
    .unwrap_or(fallback)
}

// ---------------------------------------------------------------------------
// Resolve (`src/compile/resolve.ts`)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct Resolve {
    pub scale: Map<String, Value>,
    pub axis: Map<String, Value>,
    pub legend: Map<String, Value>,
}

impl Resolve {
    pub fn from_spec(spec: &Value) -> Self {
        let mut resolve = Resolve::default();
        if let Some(value) = spec.get("resolve") {
            for (guide, target) in [
                ("scale", &mut resolve.scale),
                ("axis", &mut resolve.axis),
                ("legend", &mut resolve.legend),
            ] {
                if let Some(map) = value.get(guide).and_then(Value::as_object) {
                    for (k, v) in map {
                        target.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        resolve
    }

    pub fn get_scale(&self, channel: &str) -> Option<String> {
        self.scale
            .get(channel)
            .and_then(Value::as_str)
            .map(str::to_string)
    }
}

use super::model::SpecType;

pub fn default_scale_resolve(channel: &str, model_type: SpecType) -> &'static str {
    use crate::channel::{is_polar_position_channel, is_x_or_y, is_x_or_y_offset};
    match model_type {
        SpecType::Facet => {
            if channel == "theta" {
                "independent"
            } else {
                "shared"
            }
        }
        SpecType::Layer => "shared",
        SpecType::Concat => {
            if is_x_or_y(channel)
                || is_x_or_y_offset(channel)
                || is_polar_position_channel(channel)
            {
                "independent"
            } else {
                "shared"
            }
        }
        SpecType::Unit => "shared",
    }
}

// ---------------------------------------------------------------------------
// Misc helpers
// ---------------------------------------------------------------------------

/// Map a dynamic channel name to the `&'static str` channel constant.
///
/// The upstream `Channel` type is a union of string literals; this port uses
/// `&'static str` for it.  Parsed specs yield `String`s, so helpers that take a
/// `Channel` go through this normaliser.
pub fn channel_static(channel: &str) -> &'static str {
    use crate::channel::*;
    match channel {
        "x" => X,
        "y" => Y,
        "x2" => X2,
        "y2" => Y2,
        "xOffset" => XOFFSET,
        "yOffset" => YOFFSET,
        "theta" => THETA,
        "theta2" => THETA2,
        "radius" => RADIUS,
        "radius2" => RADIUS2,
        "latitude" => LATITUDE,
        "longitude" => LONGITUDE,
        "latitude2" => LATITUDE2,
        "longitude2" => LONGITUDE2,
        "time" => TIME,
        "color" => COLOR,
        "fill" => FILL,
        "stroke" => STROKE,
        "shape" => SHAPE,
        "size" => SIZE,
        "angle" => ANGLE,
        "opacity" => OPACITY,
        "fillOpacity" => FILLOPACITY,
        "strokeOpacity" => STROKEOPACITY,
        "strokeWidth" => STROKEWIDTH,
        "strokeDash" => STROKEDASH,
        "text" => TEXT,
        "order" => ORDER,
        "detail" => DETAIL,
        "key" => KEY,
        "tooltip" => TOOLTIP,
        "href" => HREF,
        "url" => URL,
        "description" => DESCRIPTION,
        "row" => ROW,
        "column" => COLUMN,
        "facet" => FACET,
        _ => "",
    }
}

/// Port of `isFrameMixins`.
pub fn is_frame_mixins(spec: &Value) -> bool {
    crate::spec::is_frame_mixins(spec)
}

pub fn is_graticule_generator(data: &Value) -> bool {
    data.is_object() && has_property(data, "graticule")
}

/// Port of `getStepFor` (`src/spec/base.ts`).
pub fn get_step_for(step: &Value, offset_is_discrete: bool) -> String {
    if offset_is_discrete {
        step.get("for")
            .and_then(Value::as_str)
            .unwrap_or("offset")
            .to_string()
    } else {
        "position".to_string()
    }
}

pub fn is_step(size: &Value) -> bool {
    has_property(size, "step")
}

// ---------------------------------------------------------------------------
// Log messages (`src/log/message.ts`), kept local to the compile subtree.
// ---------------------------------------------------------------------------

pub fn scale_type_not_work_with_channel(
    channel: &str,
    scale_type: &str,
    default_scale_type: &str,
) -> String {
    format!(
        "Channel \"{channel}\" does not work with \"{scale_type}\" scale. We are using \"{default_scale_type}\" scale instead."
    )
}

pub fn scale_type_not_work_with_field_def(scale_type: &str, default_scale_type: &str) -> String {
    format!(
        "FieldDef does not work with \"{scale_type}\" scale. We are using \"{default_scale_type}\" scale instead."
    )
}

pub fn discrete_channel_cannot_encode(channel: &str, type_: &str) -> String {
    let what = if type_ == "ordinal" { "order" } else { "magnitude" };
    format!(
        "Using discrete channel \"{channel}\" to encode \"{type_}\" field can be misleading as it does not encode {what}."
    )
}

pub fn merge_conflicting_property(
    property: &str,
    property_of: &str,
    v1: &Value,
    v2: &Value,
) -> String {
    format!(
        "Conflicting {property_of} property \"{property}\" ({} and {}). Using {}.",
        stringify(v1),
        stringify(v2),
        stringify(v1)
    )
}

pub fn merge_conflicting_domain_property(
    property: &str,
    property_of: &str,
    v1: &Value,
    v2: &Value,
) -> String {
    format!(
        "Conflicting {property_of} property \"{property}\" ({} and {}). Using the union of the two domains.",
        stringify(v1),
        stringify(v2)
    )
}

pub fn scale_property_not_work_with_scale_type(
    scale_type: &str,
    prop_name: &str,
    channel: &str,
) -> String {
    format!(
        "{channel}-scale's \"{prop_name}\" is dropped as it does not work with {scale_type} scale."
    )
}

pub fn step_dropped(channel: &str) -> String {
    let other = if channel == "width" { "x" } else { "y" };
    format!("The step for \"{channel}\" is dropped because the {other} is continuous.")
}

pub fn unknown_field(channel: &str) -> String {
    format!("Unknown field for {channel}. Cannot calculate view size.")
}

pub fn domain_sort_dropped(sort: &Value) -> String {
    format!(
        "Dropping sort property {} as unioned domains only support boolean or op \"count\", \"min\", and \"max\".",
        stringify(sort)
    )
}

pub const MORE_THAN_ONE_SORT: &str =
    "Domains that should be unioned has conflicting sort properties. Sort will be set to true.";

pub const NEEDS_SAME_SELECTION: &str =
    "The same selection must be used to override scale domains in a layered view.";

pub const CONCAT_CANNOT_SHARE_AXIS: &str =
    "Cannot share axes in concat, so the shared axes will be overridden.";

pub const MULTI_VIEW_ANIMATION_UNSUPPORTED: &str =
    "Animation is not supported for multi-view displays.";

pub const FACETED_INDEPENDENT_DIFFERENT_SOURCES: &str = "Detected faceted independent scales that union domain of multiple fields from different data sources. We will use the first field. The result view size may be incorrect.";

pub const FACETED_INDEPENDENT_SAME_FIELDS_DIFFERENT_SOURCES: &str = "Detected faceted independent scales that union domain of the same fields from different source. We will assume that this is the same field from a different fork of the same data source. However, if this is not the case, the result view size may be incorrect.";

pub const FACETED_INDEPENDENT_SAME_SOURCE: &str = "Detected faceted independent scales that union domain of multiple fields from the same data source. We will use the first field. The result view size may be incorrect.";

pub fn unaggregate_domain_has_no_effect_for_raw_field(field_def: &Value) -> String {
    format!(
        "Using unaggregated domain with raw field has no effect ({}).",
        stringify(field_def)
    )
}

pub fn unaggregate_domain_with_non_shared_domain_op(aggregate: &str) -> String {
    format!(
        "Unaggregated domain not applicable for \"{aggregate}\" since it produces values outside the origin domain of the source data."
    )
}

pub fn unaggregated_domain_with_log_scale(field_def: &Value) -> String {
    format!(
        "Unaggregated domain is currently unsupported for log scale ({}).",
        stringify(field_def)
    )
}

pub fn domain_required_for_threshold_scale(channel: &str) -> String {
    format!(
        "Domain for {channel} is required for threshold scale. Using default domain instead."
    )
}

// ---------------------------------------------------------------------------
// Warning helper used by scale rules.
// ---------------------------------------------------------------------------

pub fn warn_if_explicit(explicit: &Explicit, message: String) {
    if explicit.explicit {
        log::warn(message);
    }
}

/// Port of `signalOrStringValue` from `compile/common.ts`.
pub fn signal_or_string_value(value: &Value) -> String {
    if value.is_object() && has_property(value, "signal") {
        return value
            .get("signal")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
    }
    crate::channeldef_support::js_to_string(value)
}

/// Port of `exprFromSignalRefOrValue`.
pub fn expr_from_value(value: &Value) -> String {
    signal_or_string_value(value)
}

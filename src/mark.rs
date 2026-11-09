//! Port of `src/mark.ts`.
//!
//! The TypeScript module declares a large number of interfaces for mark
//! configuration. Those interfaces are structurally typed and map directly to
//! JSON, so they are represented by [`serde_json::Value`] here. The runtime
//! values, constants and helper functions are ported faithfully.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::util::has_property;

/// All types of primitive marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mark {
    Arc,
    Area,
    Bar,
    Image,
    Line,
    Point,
    Rect,
    Rule,
    Text,
    Tick,
    Trail,
    Circle,
    Square,
    Geoshape,
}

pub const ARC: Mark = Mark::Arc;
pub const AREA: Mark = Mark::Area;
pub const BAR: Mark = Mark::Bar;
pub const IMAGE: Mark = Mark::Image;
pub const LINE: Mark = Mark::Line;
pub const POINT: Mark = Mark::Point;
pub const RECT: Mark = Mark::Rect;
pub const RULE: Mark = Mark::Rule;
pub const TEXT: Mark = Mark::Text;
pub const TICK: Mark = Mark::Tick;
pub const TRAIL: Mark = Mark::Trail;
pub const CIRCLE: Mark = Mark::Circle;
pub const SQUARE: Mark = Mark::Square;
pub const GEOSHAPE: Mark = Mark::Geoshape;

impl Mark {
    pub fn as_str(&self) -> &'static str {
        match self {
            Mark::Arc => "arc",
            Mark::Area => "area",
            Mark::Bar => "bar",
            Mark::Image => "image",
            Mark::Line => "line",
            Mark::Point => "point",
            Mark::Rect => "rect",
            Mark::Rule => "rule",
            Mark::Text => "text",
            Mark::Tick => "tick",
            Mark::Trail => "trail",
            Mark::Circle => "circle",
            Mark::Square => "square",
            Mark::Geoshape => "geoshape",
        }
    }

    /// All primitive marks, in the same order as the upstream `Mark` object.
    pub const ALL: [Mark; 14] = [
        Mark::Arc,
        Mark::Area,
        Mark::Bar,
        Mark::Image,
        Mark::Line,
        Mark::Point,
        Mark::Rect,
        Mark::Rule,
        Mark::Text,
        Mark::Tick,
        Mark::Trail,
        Mark::Circle,
        Mark::Square,
        Mark::Geoshape,
    ];
}

impl std::fmt::Display for Mark {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Mark {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "arc" => Ok(Mark::Arc),
            "area" => Ok(Mark::Area),
            "bar" => Ok(Mark::Bar),
            "image" => Ok(Mark::Image),
            "line" => Ok(Mark::Line),
            "point" => Ok(Mark::Point),
            "rect" => Ok(Mark::Rect),
            "rule" => Ok(Mark::Rule),
            "text" => Ok(Mark::Text),
            "tick" => Ok(Mark::Tick),
            "trail" => Ok(Mark::Trail),
            "circle" => Ok(Mark::Circle),
            "square" => Ok(Mark::Square),
            "geoshape" => Ok(Mark::Geoshape),
            _ => Err(()),
        }
    }
}

/// Membership index for all primitive marks (upstream `Mark` object keys).
pub const MARK_INDEX: [&str; 14] = [
    "arc", "area", "bar", "image", "line", "point", "rect", "rule", "text", "tick", "trail",
    "circle", "square", "geoshape",
];

pub fn is_mark(m: &str) -> bool {
    MARK_INDEX.contains(&m)
}

/// Composite mark names. Upstream these live in `compositemark/index.ts`; they
/// are exposed here for the mark predicates requested by this port.
pub const BOXPLOT: &str = "boxplot";
pub const ERRORBAR: &str = "errorbar";
pub const ERRORBAND: &str = "errorband";

pub const COMPOSITE_MARKS: [&str; 3] = [BOXPLOT, ERRORBAR, ERRORBAND];

pub fn is_composite_mark_name(m: &str) -> bool {
    COMPOSITE_MARKS.contains(&m)
}

/// Anything that can be interpreted as a mark type: a [`Mark`], a string, a
/// mark definition (JSON object with a `type` property), or [`MarkDef`].
pub trait MarkLike {
    fn mark_type(&self) -> Option<&str>;
}

impl MarkLike for Mark {
    fn mark_type(&self) -> Option<&str> {
        Some(self.as_str())
    }
}

impl MarkLike for str {
    fn mark_type(&self) -> Option<&str> {
        Some(self)
    }
}

impl MarkLike for String {
    fn mark_type(&self) -> Option<&str> {
        Some(self.as_str())
    }
}

impl MarkLike for Value {
    fn mark_type(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s.as_str()),
            Value::Object(map) => map.get("type").and_then(Value::as_str),
            _ => None,
        }
    }
}

/// A generic mark definition: `{type: "...", ...mark properties}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkDef {
    #[serde(rename = "type")]
    pub mark_type: String,
    #[serde(flatten)]
    pub props: Map<String, Value>,
}

impl MarkLike for MarkDef {
    fn mark_type(&self) -> Option<&str> {
        Some(self.mark_type.as_str())
    }
}

/// Port of `getMarkType`.
pub fn get_mark_type<M: MarkLike + ?Sized>(m: &M) -> Option<&str> {
    m.mark_type()
}

/// Port of `isMarkDef` (`hasProperty(mark, 'type')`).
pub fn is_mark_def(mark: &Value) -> bool {
    has_property(mark, "type")
}

pub const PATH_MARKS: [Mark; 3] = [Mark::Line, Mark::Area, Mark::Trail];

pub fn is_path_mark<M: MarkLike + ?Sized>(m: &M) -> bool {
    matches!(m.mark_type(), Some("line") | Some("area") | Some("trail"))
}

/* arc is rect/interval in polar coordinate */
pub const RECT_BASED_MARKS: [Mark; 5] = [Mark::Rect, Mark::Bar, Mark::Image, Mark::Arc, Mark::Tick];

pub fn is_rect_based_mark<M: MarkLike + ?Sized>(m: &M) -> bool {
    matches!(
        m.mark_type(),
        Some("rect") | Some("bar") | Some("image") | Some("arc") | Some("tick")
    )
}

/// Marks that span from a baseline to a value (e.g., for zero baselines and
/// stacking).
pub const BAR_AREA_MARKS: [Mark; 2] = [Mark::Bar, Mark::Area];

pub fn is_bar_or_area<M: MarkLike + ?Sized>(m: &M) -> bool {
    matches!(m.mark_type(), Some("bar") | Some("area"))
}

/// Upstream this set lives in `src/stack.ts`.
pub const STACKABLE_MARKS: [Mark; 10] = [
    Mark::Arc,
    Mark::Bar,
    Mark::Area,
    Mark::Rule,
    Mark::Point,
    Mark::Circle,
    Mark::Square,
    Mark::Line,
    Mark::Text,
    Mark::Tick,
];

/// Upstream this set lives in `src/stack.ts`.
pub const STACK_BY_DEFAULT_MARKS: [Mark; 3] = [Mark::Bar, Mark::Area, Mark::Arc];

pub fn is_stackable_mark<M: MarkLike + ?Sized>(m: &M) -> bool {
    matches!(
        m.mark_type(),
        Some("arc")
            | Some("bar")
            | Some("area")
            | Some("rule")
            | Some("point")
            | Some("circle")
            | Some("square")
            | Some("line")
            | Some("text")
            | Some("tick")
    )
}

pub fn is_stack_by_default_mark<M: MarkLike + ?Sized>(m: &M) -> bool {
    matches!(m.mark_type(), Some("bar") | Some("area") | Some("arc"))
}

/// Compatibility helper: marks that connect a continuous domain to a
/// continuous range (the path marks).
///
/// This predicate does not exist in the upstream `mark.ts`; it is provided
/// because it was requested for this port. It is an alias of [`is_path_mark`].
pub fn is_continuous_to_continuous_mark<M: MarkLike + ?Sized>(m: &M) -> bool {
    is_path_mark(m)
}

pub const PRIMITIVE_MARKS: [Mark; 14] = Mark::ALL;

pub fn is_primitive_mark<M: MarkLike + ?Sized>(mark: &M) -> bool {
    mark.mark_type().map(is_mark).unwrap_or(false)
}

pub fn is_composite_mark<M: MarkLike + ?Sized>(mark: &M) -> bool {
    mark.mark_type()
        .map(is_composite_mark_name)
        .unwrap_or(false)
}

pub const STROKE_CONFIG: [&str; 7] = [
    "stroke",
    "strokeWidth",
    "strokeDash",
    "strokeDashOffset",
    "strokeOpacity",
    "strokeJoin",
    "strokeMiterLimit",
];

pub const FILL_CONFIG: [&str; 2] = ["fill", "fillOpacity"];

pub const FILL_STROKE_CONFIG: [&str; 9] = [
    "stroke",
    "strokeWidth",
    "strokeDash",
    "strokeDashOffset",
    "strokeOpacity",
    "strokeJoin",
    "strokeMiterLimit",
    "fill",
    "fillOpacity",
];

pub const VL_ONLY_MARK_CONFIG_PROPERTIES: [&str; 8] = [
    "color",
    "filled",
    "invalid",
    "order",
    "radius2",
    "theta2",
    "timeUnitBandSize",
    "timeUnitBandPosition",
];

pub const VL_ONLY_RECT_CONFIG: [&str; 4] = [
    "binSpacing",
    "continuousBandSize",
    "discreteBandSize",
    "minBandSize",
];

/// Port of `VL_ONLY_MARK_SPECIFIC_CONFIG_PROPERTY_INDEX` as a lookup helper.
pub fn vl_only_mark_specific_config_properties(mark: &str) -> Vec<&'static str> {
    match mark {
        "area" => vec!["line", "point"],
        "bar" | "rect" => VL_ONLY_RECT_CONFIG.to_vec(),
        "line" => vec!["point"],
        "tick" => {
            let mut props = vec!["bandSize", "thickness"];
            props.extend_from_slice(&VL_ONLY_RECT_CONFIG);
            props
        }
        _ => Vec::new(),
    }
}

pub const MARK_CONFIGS: [&str; 15] = [
    "mark", "arc", "area", "bar", "circle", "image", "line", "point", "rect", "rule", "square",
    "text", "tick", "trail", "geoshape",
];

pub fn default_mark_config() -> Value {
    serde_json::json!({
        "color": "#4c78a8",
        "invalid": "break-paths-show-path-domains",
        "timeUnitBandSize": 1,
    })
}

const DEFAULT_RECT_BAND_SIZE: i64 = 5;

pub fn default_rect_config() -> Value {
    serde_json::json!({
        "binSpacing": 0,
        "continuousBandSize": DEFAULT_RECT_BAND_SIZE,
        "minBandSize": 0.25,
        "timeUnitBandPosition": 0.5,
    })
}

pub fn default_bar_config() -> Value {
    serde_json::json!({
        "binSpacing": 1,
        "continuousBandSize": DEFAULT_RECT_BAND_SIZE,
        "minBandSize": 0.25,
        "timeUnitBandPosition": 0.5,
    })
}

pub fn default_tick_config() -> Value {
    serde_json::json!({
        "binSpacing": 0,
        "continuousBandSize": DEFAULT_RECT_BAND_SIZE,
        "minBandSize": 0.25,
        "timeUnitBandPosition": 0.5,
        "thickness": 1,
    })
}

/// A relative band size such as `{band: 0.5}`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RelativeBandSize {
    pub band: f64,
}

/// Port of `isRelativeBandSize`.
pub fn is_relative_band_size(o: &Value) -> bool {
    has_property(o, "band")
}

/// Port of `getStyles`: the mark type followed by any explicit styles.
pub fn get_styles(mark: &Value) -> Vec<String> {
    let mut styles = Vec::new();
    if let Some(mark_type) = get_mark_type(mark) {
        styles.push(mark_type.to_string());
    }
    match mark.get("style") {
        Some(Value::String(s)) => styles.push(s.clone()),
        Some(Value::Array(arr)) => {
            for s in arr {
                if let Some(s) = s.as_str() {
                    styles.push(s.to_string());
                }
            }
        }
        _ => {}
    }
    styles
}

fn get_first_defined(values: &[Option<Value>]) -> Option<Value> {
    values.iter().find_map(|v| v.clone())
}

fn get_style_config(prop: &str, styles: &[String], style_config_index: &Value) -> Option<Value> {
    let mut value = None;
    for style in styles {
        if let Some(style_config) = style_config_index.get(style.as_str()) {
            if has_property(style_config, prop) {
                value = style_config.get(prop).cloned();
            }
        }
    }
    value
}

/// Port of `getMarkStyleConfig`.
pub fn get_mark_style_config(
    prop: &str,
    mark: &Value,
    style_config_index: &Value,
) -> Option<Value> {
    get_style_config(prop, &get_styles(mark), style_config_index)
}

/// Port of `getMarkConfig`.
pub fn get_mark_config(
    channel: &str,
    mark: &Value,
    config: &Value,
    vg_channel: Option<&str>,
) -> Option<Value> {
    let style = config.get("style").unwrap_or(&Value::Null);
    let style_config = get_mark_style_config(channel, mark, style);

    let mark_type = get_mark_type(mark).unwrap_or("");
    let mark_specific = config.get(mark_type).unwrap_or(&Value::Null);
    let general = config.get("mark").unwrap_or(&Value::Null);

    let vg = vg_channel.unwrap_or(channel);
    let mark_specific_value = mark_specific.get(vg).cloned();
    let general_value = general.get(vg).cloned();

    get_first_defined(&[style_config, mark_specific_value, general_value])
}

/// Port of `getMarkPropOrConfig`, with the optional `{vgChannel,
/// ignoreVgConfig}` argument expanded into two parameters.
pub fn get_mark_prop_or_config(
    channel: &str,
    mark: &Value,
    config: &Value,
    vg_channel: Option<&str>,
    ignore_vg_config: bool,
) -> Option<Value> {
    if let Some(vg) = vg_channel {
        if has_property(mark, vg) {
            return mark.get(vg).cloned();
        }
    }
    if has_property(mark, channel) {
        return mark.get(channel).cloned();
    }
    if ignore_vg_config && (vg_channel.is_none() || vg_channel == Some(channel)) {
        return None;
    }

    get_mark_config(channel, mark, config, vg_channel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_names_roundtrip() {
        for mark in Mark::ALL {
            assert_eq!(mark.as_str().parse::<Mark>(), Ok(mark));
            assert!(is_mark(mark.as_str()));
        }
    }
}

//! Port of `src/scale.ts`.
//!
//! Only the core, pure part of the upstream module is ported here: scale type
//! constants, categories, predicates, the (default) scale config and the
//! channel/scale type compatibility helpers. Types that come from `vega` or
//! from out-of-scope modules (`SignalRef`, `DateTime`, ...) are represented with
//! `serde_json::Value` / local structs.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::channel::{self, Channel};
use crate::type_::{Type, NOMINAL, ORDINAL, QUANTITATIVE, TEMPORAL};
use crate::util::{contains, has_property, is_boolean};

/// Scale types, mirroring the `ScaleType` object of the TypeScript source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScaleType {
    // Continuous - Quantitative
    Linear,
    Log,
    Pow,
    Sqrt,
    Symlog,

    Identity,
    Sequential,

    // Continuous - Time
    Time,
    Utc,

    // Discretizing scales
    Quantile,
    Quantize,
    Threshold,
    BinOrdinal,

    // Discrete scales
    Ordinal,
    Point,
    Band,
}

impl ScaleType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ScaleType::Linear => "linear",
            ScaleType::Log => "log",
            ScaleType::Pow => "pow",
            ScaleType::Sqrt => "sqrt",
            ScaleType::Symlog => "symlog",
            ScaleType::Identity => "identity",
            ScaleType::Sequential => "sequential",
            ScaleType::Time => "time",
            ScaleType::Utc => "utc",
            ScaleType::Quantile => "quantile",
            ScaleType::Quantize => "quantize",
            ScaleType::Threshold => "threshold",
            ScaleType::BinOrdinal => "bin-ordinal",
            ScaleType::Ordinal => "ordinal",
            ScaleType::Point => "point",
            ScaleType::Band => "band",
        }
    }
}

impl std::fmt::Display for ScaleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ScaleType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        const ALL: [ScaleType; 16] = [
            ScaleType::Linear,
            ScaleType::Log,
            ScaleType::Pow,
            ScaleType::Sqrt,
            ScaleType::Symlog,
            ScaleType::Identity,
            ScaleType::Sequential,
            ScaleType::Time,
            ScaleType::Utc,
            ScaleType::Quantile,
            ScaleType::Quantize,
            ScaleType::Threshold,
            ScaleType::BinOrdinal,
            ScaleType::Ordinal,
            ScaleType::Point,
            ScaleType::Band,
        ];
        ALL.iter().copied().find(|t| t.as_str() == s).ok_or(())
    }
}

/// Category of a scale type. Only scales in the same category can be merged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScaleCategory {
    Numeric,
    Time,
    Ordinal,
    BinOrdinal,
    OrdinalPosition,
    Discretizing,
}

impl ScaleCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            ScaleCategory::Numeric => "numeric",
            ScaleCategory::Time => "time",
            ScaleCategory::Ordinal => "ordinal",
            ScaleCategory::BinOrdinal => "bin-ordinal",
            ScaleCategory::OrdinalPosition => "ordinal-position",
            ScaleCategory::Discretizing => "discretizing",
        }
    }
}

pub fn scale_category(scale_type: ScaleType) -> ScaleCategory {
    match scale_type {
        ScaleType::Linear
        | ScaleType::Log
        | ScaleType::Pow
        | ScaleType::Sqrt
        | ScaleType::Symlog
        | ScaleType::Identity
        | ScaleType::Sequential => ScaleCategory::Numeric,
        ScaleType::Time | ScaleType::Utc => ScaleCategory::Time,
        ScaleType::Ordinal => ScaleCategory::Ordinal,
        ScaleType::BinOrdinal => ScaleCategory::BinOrdinal,
        ScaleType::Point | ScaleType::Band => ScaleCategory::OrdinalPosition,
        ScaleType::Quantile | ScaleType::Quantize | ScaleType::Threshold => {
            ScaleCategory::Discretizing
        }
    }
}

/// All scale types, in the insertion order of `SCALE_CATEGORY_INDEX`.
pub const SCALE_TYPES: [ScaleType; 16] = [
    ScaleType::Linear,
    ScaleType::Log,
    ScaleType::Pow,
    ScaleType::Sqrt,
    ScaleType::Symlog,
    ScaleType::Identity,
    ScaleType::Sequential,
    ScaleType::Time,
    ScaleType::Utc,
    ScaleType::Ordinal,
    ScaleType::BinOrdinal,
    ScaleType::Point,
    ScaleType::Band,
    ScaleType::Quantile,
    ScaleType::Quantize,
    ScaleType::Threshold,
];

/// Whether the two given scale types can be merged together.
pub fn scale_compatible(scale_type1: ScaleType, scale_type2: ScaleType) -> bool {
    let c1 = scale_category(scale_type1);
    let c2 = scale_category(scale_type2);
    c1 == c2
        || (c1 == ScaleCategory::OrdinalPosition && c2 == ScaleCategory::Time)
        || (c2 == ScaleCategory::OrdinalPosition && c1 == ScaleCategory::Time)
}

/// Scale precedence -- high score = higher priority for merging.
pub fn scale_type_precedence(scale_type: ScaleType) -> i64 {
    match scale_type {
        // numeric
        ScaleType::Linear => 0,
        ScaleType::Log
        | ScaleType::Pow
        | ScaleType::Sqrt
        | ScaleType::Symlog
        | ScaleType::Identity
        | ScaleType::Sequential => 1,
        // time
        ScaleType::Time | ScaleType::Utc => 0,
        // ordinal-position -- these have higher precedence than continuous scales
        // as they support more types of data
        ScaleType::Point => 10,
        ScaleType::Band => 11,
        // non grouped types
        ScaleType::Ordinal
        | ScaleType::BinOrdinal
        | ScaleType::Quantile
        | ScaleType::Quantize
        | ScaleType::Threshold => 0,
    }
}

pub const QUANTITATIVE_SCALES: [ScaleType; 5] = [
    ScaleType::Linear,
    ScaleType::Log,
    ScaleType::Pow,
    ScaleType::Sqrt,
    ScaleType::Symlog,
];

pub const CONTINUOUS_TO_CONTINUOUS_SCALES: [ScaleType; 7] = [
    ScaleType::Linear,
    ScaleType::Log,
    ScaleType::Pow,
    ScaleType::Sqrt,
    ScaleType::Symlog,
    ScaleType::Time,
    ScaleType::Utc,
];

pub fn is_quantitative(scale_type: ScaleType) -> bool {
    contains(&QUANTITATIVE_SCALES, &scale_type)
}

pub const CONTINUOUS_TO_DISCRETE_SCALES: [ScaleType; 3] = [
    ScaleType::Quantile,
    ScaleType::Quantize,
    ScaleType::Threshold,
];

pub const CONTINUOUS_DOMAIN_SCALES: [ScaleType; 12] = [
    ScaleType::Linear,
    ScaleType::Log,
    ScaleType::Pow,
    ScaleType::Sqrt,
    ScaleType::Symlog,
    ScaleType::Time,
    ScaleType::Utc,
    ScaleType::Quantile,
    ScaleType::Quantize,
    ScaleType::Threshold,
    ScaleType::Sequential,
    ScaleType::Identity,
];

pub const DISCRETE_DOMAIN_SCALES: [ScaleType; 4] = [
    ScaleType::Ordinal,
    ScaleType::BinOrdinal,
    ScaleType::Point,
    ScaleType::Band,
];

pub const TIME_SCALE_TYPES: [ScaleType; 2] = [ScaleType::Time, ScaleType::Utc];

pub fn has_discrete_domain(scale_type: ScaleType) -> bool {
    contains(&DISCRETE_DOMAIN_SCALES, &scale_type)
}

pub fn has_continuous_domain(scale_type: ScaleType) -> bool {
    contains(&CONTINUOUS_DOMAIN_SCALES, &scale_type)
}

pub fn is_continuous_to_continuous(scale_type: ScaleType) -> bool {
    contains(&CONTINUOUS_TO_CONTINUOUS_SCALES, &scale_type)
}

pub fn is_continuous_to_discrete(scale_type: ScaleType) -> bool {
    contains(&CONTINUOUS_TO_DISCRETE_SCALES, &scale_type)
}

// ---------------------------------------------------------------------------
// Scale config
// ---------------------------------------------------------------------------

/// Scale configuration. Mirrors `ScaleConfig<SignalRef>` with the signal-ref
/// variants dropped (plain values only).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScaleConfig {
    pub round: Option<bool>,
    pub clamp: Option<bool>,
    pub band_with_nested_offset_padding_inner: Option<f64>,
    pub band_with_nested_offset_padding_outer: Option<f64>,
    pub band_padding_inner: Option<f64>,
    pub band_padding_outer: Option<f64>,
    pub bar_band_padding_inner: Option<f64>,
    pub rect_band_padding_inner: Option<f64>,
    pub tick_band_padding_inner: Option<f64>,
    pub offset_band_padding_inner: Option<f64>,
    pub offset_band_padding_outer: Option<f64>,
    pub continuous_padding: Option<f64>,
    pub point_padding: Option<f64>,
    pub use_unaggregated_domain: Option<bool>,
    pub max_band_size: Option<f64>,
    pub min_band_size: Option<f64>,
    pub max_font_size: Option<f64>,
    pub min_font_size: Option<f64>,
    pub min_opacity: Option<f64>,
    pub max_opacity: Option<f64>,
    pub min_size: Option<f64>,
    pub max_size: Option<f64>,
    pub min_stroke_width: Option<f64>,
    pub max_stroke_width: Option<f64>,
    pub quantile_count: Option<f64>,
    pub quantize_count: Option<f64>,
    pub x_reverse: Option<bool>,
    pub zero: Option<bool>,
    pub frames_per_second: Option<f64>,
    pub animation_duration: Option<f64>,
}

/// The default scale config of the TypeScript source.
pub fn default_scale_config() -> ScaleConfig {
    ScaleConfig {
        point_padding: Some(0.5),

        bar_band_padding_inner: Some(0.1),
        rect_band_padding_inner: Some(0.0),
        tick_band_padding_inner: Some(0.25),

        band_with_nested_offset_padding_inner: Some(0.2),
        band_with_nested_offset_padding_outer: Some(0.2),

        min_band_size: Some(2.0),

        min_font_size: Some(8.0),
        max_font_size: Some(40.0),

        min_opacity: Some(0.3),
        max_opacity: Some(0.8),

        min_size: Some(4.0),

        min_stroke_width: Some(1.0),
        max_stroke_width: Some(4.0),
        quantile_count: Some(4.0),
        quantize_count: Some(4.0),

        zero: Some(true),

        frames_per_second: Some(2.0),
        animation_duration: Some(5.0),
        ..Default::default()
    }
}

impl ScaleConfig {
    pub fn new() -> Self {
        default_scale_config()
    }
}

// ---------------------------------------------------------------------------
// Domain / scheme / range helpers
// ---------------------------------------------------------------------------

/// Whether a scheme is an object with a `name` property.
pub fn is_extended_scheme(scheme: &Value) -> bool {
    !scheme.is_string() && has_property(scheme, "name")
}

/// Whether a domain is a parameter extent (`{param: ...}`).
pub fn is_parameter_domain(domain: &Value) -> bool {
    has_property(domain, "param")
}

/// Whether a domain is a `DomainUnionWith` (`{unionWith: [...]}`).
pub fn is_domain_union_with(domain: &Value) -> bool {
    has_property(domain, "unionWith")
}

/// Whether a range is a `FieldRange` (`{field: ...}`).
pub fn is_field_range(range: &Value) -> bool {
    range.is_object() && has_property(range, "field")
}

// ---------------------------------------------------------------------------
// Scale properties
// ---------------------------------------------------------------------------

pub const SCALE_PROPERTIES: [&str; 24] = [
    "type",
    "domain",
    "domainMax",
    "domainMin",
    "domainMid",
    "domainRaw",
    "align",
    "range",
    "rangeMax",
    "rangeMin",
    "scheme",
    "bins",
    // Other properties
    "reverse",
    "round",
    // quantitative / time
    "clamp",
    "nice",
    // quantitative
    "base",
    "exponent",
    "constant",
    "interpolate",
    "zero",
    // band/point
    "padding",
    "paddingInner",
    "paddingOuter",
];

pub const NON_TYPE_DOMAIN_RANGE_VEGA_SCALE_PROPERTIES: [&str; 18] = [
    "domainMax",
    "domainMin",
    "domainMid",
    "domainRaw",
    "align",
    "bins",
    // Other properties
    "reverse",
    "round",
    // quantitative / time
    "clamp",
    "nice",
    // quantitative
    "base",
    "exponent",
    "constant",
    "interpolate",
    "zero",
    // band/point
    "padding",
    "paddingInner",
    "paddingOuter",
];

/// Whether a scale type supports a given scale property.
pub fn scale_type_support_property(scale_type: ScaleType, prop_name: &str) -> bool {
    match prop_name {
        "type" | "domain" | "reverse" | "range" => true,
        "scheme" | "interpolate" => !matches!(
            scale_type,
            ScaleType::Point | ScaleType::Band | ScaleType::Identity
        ),
        "bins" => !matches!(
            scale_type,
            ScaleType::Point | ScaleType::Band | ScaleType::Identity | ScaleType::Ordinal
        ),
        "round" => {
            is_continuous_to_continuous(scale_type)
                || matches!(scale_type, ScaleType::Band | ScaleType::Point)
        }
        "padding" | "rangeMin" | "rangeMax" => {
            is_continuous_to_continuous(scale_type)
                || matches!(scale_type, ScaleType::Point | ScaleType::Band)
        }
        "paddingOuter" | "align" => matches!(scale_type, ScaleType::Point | ScaleType::Band),
        "paddingInner" => scale_type == ScaleType::Band,
        "domainMax" | "domainMid" | "domainMin" | "domainRaw" | "clamp" => {
            is_continuous_to_continuous(scale_type)
        }
        "nice" => {
            is_continuous_to_continuous(scale_type)
                || matches!(scale_type, ScaleType::Quantize | ScaleType::Threshold)
        }
        "exponent" => scale_type == ScaleType::Pow,
        "base" => scale_type == ScaleType::Log,
        "constant" => scale_type == ScaleType::Symlog,
        "zero" => {
            has_continuous_domain(scale_type)
                && !contains(
                    &[
                        ScaleType::Log,       // log scale cannot have zero value
                        ScaleType::Time,      // zero is not meaningful for time
                        ScaleType::Utc,       // zero is not meaningful for time
                        ScaleType::Threshold, // threshold requires custom domain
                        ScaleType::Quantile,  // quantile depends on distribution
                    ],
                    &scale_type,
                )
        }
        _ => false,
    }
}

/// Returns an error message if the input channel does not support the input
/// scale property name, `None` otherwise.
pub fn channel_scale_property_incompatability(channel: Channel, prop_name: &str) -> Option<String> {
    match prop_name {
        "interpolate" | "scheme" | "domainMid" => {
            if !channel::is_color_channel(channel) {
                Some(format!(
                    "Cannot use the scale property \"{prop_name}\" with non-color channel."
                ))
            } else {
                None
            }
        }
        "align" | "type" | "bins" | "domain" | "domainMax" | "domainMin" | "domainRaw"
        | "range" | "base" | "exponent" | "constant" | "nice" | "padding" | "paddingInner"
        | "paddingOuter" | "rangeMax" | "rangeMin" | "reverse" | "round" | "clamp" | "zero" => {
            None
        }
        _ => None,
    }
}

/// Whether a scale type is compatible with a field definition type.
pub fn scale_type_support_data_type(specified_type: Option<ScaleType>, field_def_type: Type) -> bool {
    if contains(&[ORDINAL, NOMINAL], &field_def_type) {
        match specified_type {
            None => true,
            Some(t) => has_discrete_domain(t),
        }
    } else if field_def_type == TEMPORAL {
        matches!(
            specified_type,
            None | Some(ScaleType::Time) | Some(ScaleType::Utc)
        )
    } else if field_def_type == QUANTITATIVE {
        match specified_type {
            None => true,
            Some(t) => is_quantitative(t) || is_continuous_to_discrete(t),
        }
    } else {
        true
    }
}

/// Whether a channel supports a given scale type.
pub fn channel_support_scale_type(
    channel: Channel,
    scale_type: ScaleType,
    has_nested_offset_scale: bool,
) -> bool {
    if !channel::is_scale_channel(channel) {
        return false;
    }
    match channel {
        channel::X
        | channel::Y
        | channel::XOFFSET
        | channel::YOFFSET
        | channel::THETA
        | channel::RADIUS => {
            if is_continuous_to_continuous(scale_type) || scale_type == ScaleType::Band {
                true
            } else if scale_type == ScaleType::Point {
                // Point scale can't be used if the position has a nested offset
                // scale because if there is a nested scale, then it's band.
                !has_nested_offset_scale
            } else {
                false
            }
        }
        channel::TIME => contains(&[ScaleType::Linear, ScaleType::Band], &scale_type),
        channel::SIZE
        | channel::STROKEWIDTH
        | channel::OPACITY
        | channel::FILLOPACITY
        | channel::STROKEOPACITY
        | channel::ANGLE => {
            // Although it generally doesn't make sense to use band with size and
            // opacity, it can also work since we use band: 0.5 to get midpoint.
            is_continuous_to_continuous(scale_type)
                || is_continuous_to_discrete(scale_type)
                || contains(
                    &[ScaleType::Band, ScaleType::Point, ScaleType::Ordinal],
                    &scale_type,
                )
        }
        channel::COLOR | channel::FILL | channel::STROKE => scale_type != ScaleType::Band,
        channel::STROKEDASH | channel::SHAPE => {
            scale_type == ScaleType::Ordinal || is_continuous_to_discrete(scale_type)
        }
        _ => false,
    }
}

/// Convenience overload with `hasNestedOffsetScale = false`.
pub fn channel_support_scale_type_default(channel: Channel, scale_type: ScaleType) -> bool {
    channel_support_scale_type(channel, scale_type, false)
}

/// A set of scale types, mirroring the TypeScript `Set` exports.
pub type ScaleTypeSet = BTreeSet<ScaleType>;

/// Build a set from a slice of scale types.
pub fn scale_type_set(types: &[ScaleType]) -> ScaleTypeSet {
    types.iter().copied().collect()
}

/// Whether a value is a boolean (re-exported for symmetry with vega-util).
pub fn is_boolean_value(v: &Value) -> bool {
    is_boolean(v)
}

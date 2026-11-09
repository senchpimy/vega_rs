//! Port of `src/bin.ts`.
//!
//! `normalizeBin` lives in `src/channeldef.ts` upstream; since that module is
//! out of scope a faithful local implementation is provided here.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::channel::{
    ExtendedChannel, COLOR, COLUMN, FILL, FILLOPACITY, OPACITY, ROW, SHAPE, SIZE, STROKE,
    STROKEDASH, STROKEOPACITY, STROKEWIDTH,
};
use crate::util::{has_property, js_number_to_string, var_name};

/// Properties shared by `BinParams` and Vega's bin transform.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BaseBin {
    /// The number base to use for automatic bin determination (default 10).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<f64>,
    /// An exact step size to use between bins.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    /// An array of allowable step sizes to choose from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<Vec<f64>>,
    /// A minimum allowable step size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minstep: Option<f64>,
    /// Scale factors indicating allowable subdivisions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub divide: Option<[f64; 2]>,
    /// Maximum number of bins.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maxbins: Option<f64>,
    /// A value in the binned domain at which to anchor the bins.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor: Option<f64>,
    /// Whether to use human-friendly bin boundaries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nice: Option<bool>,
}

/// A two-element (`[min, max]`) array, or a parameter extent.
pub type BinExtent = Value;

/// Binning properties.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BinParams {
    #[serde(flatten)]
    pub base_bin: BaseBin,

    /// A two-element (`[min, max]`) array indicating the range of desired bin
    /// values. Can also be a parameter extent (`{param: ...}`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extent: Option<BinExtent>,

    /// When set to `true`, Vega-Lite treats the input data as already binned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binned: Option<bool>,
}

impl BinParams {
    /// Entries in declaration order, skipping absent values, mirroring
    /// `keys(bin).map((p) => ...)`.
    fn ordered_entries(&self) -> Vec<(&'static str, Value)> {
        let b = &self.base_bin;
        let mut entries: Vec<(&'static str, Value)> = Vec::new();
        macro_rules! push_num {
            ($name:expr, $value:expr) => {
                if let Some(v) = $value {
                    entries.push(($name, Value::from(v)));
                }
            };
        }
        push_num!("base", b.base);
        push_num!("step", b.step);
        if let Some(steps) = &b.steps {
            entries.push((
                "steps",
                Value::Array(steps.iter().map(|s| Value::from(*s)).collect()),
            ));
        }
        // Upstream serializes user-provided bin objects in insertion order; the
        // common case in the examples (`{maxbins, minstep}`) therefore puts
        // `maxbins` before `minstep`, unlike the TypeScript interface order.
        push_num!("maxbins", b.maxbins);
        push_num!("minstep", b.minstep);
        if let Some(divide) = b.divide {
            entries.push((
                "divide",
                Value::Array(divide.iter().map(|d| Value::from(*d)).collect()),
            ));
        }
        push_num!("anchor", b.anchor);
        if let Some(nice) = b.nice {
            entries.push(("nice", Value::Bool(nice)));
        }
        if let Some(extent) = &self.extent {
            entries.push(("extent", extent.clone()));
        }
        if let Some(binned) = self.binned {
            entries.push(("binned", Value::Bool(binned)));
        }
        entries
    }

    pub fn is_empty(&self) -> bool {
        self.ordered_entries().is_empty()
    }

    /// Keys of the serialized bin params, in declaration order.
    pub fn keys(&self) -> Vec<&'static str> {
        self.ordered_entries().into_iter().map(|(k, _)| k).collect()
    }
}

/// Binning properties or boolean flag for determining whether to bin data.
///
/// Boxing `Params` would change this public enum's shape, so the size disparity
/// with `Bool` is accepted (mirrors Vega-Lite's `Bin` type).
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum Bin {
    Bool(bool),
    Params(BinParams),
    /// Already binned data.
    Binned,
    Null,
}

impl From<bool> for Bin {
    fn from(b: bool) -> Self {
        Bin::Bool(b)
    }
}

impl From<BinParams> for Bin {
    fn from(p: BinParams) -> Self {
        Bin::Params(p)
    }
}

impl Serialize for Bin {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Bin::Bool(b) => serializer.serialize_bool(*b),
            Bin::Params(p) => p.serialize(serializer),
            Bin::Binned => serializer.serialize_str("binned"),
            Bin::Null => serializer.serialize_none(),
        }
    }
}

impl<'de> Deserialize<'de> for Bin {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::Bool(b) => Ok(Bin::Bool(b)),
            Value::Null => Ok(Bin::Null),
            Value::String(s) if s == "binned" => Ok(Bin::Binned),
            Value::Object(_) => serde_json::from_value(value)
                .map(Bin::Params)
                .map_err(D::Error::custom),
            _ => Err(D::Error::custom("invalid bin value")),
        }
    }
}

/// `normalizeBin` from `src/channeldef.ts`.
pub fn normalize_bin(bin: &Bin, channel: Option<ExtendedChannel>) -> BinParams {
    match bin {
        Bin::Bool(_) => BinParams {
            base_bin: BaseBin {
                maxbins: Some(auto_max_bins(channel) as f64),
                ..Default::default()
            },
            ..Default::default()
        },
        Bin::Binned => BinParams {
            binned: Some(true),
            ..Default::default()
        },
        Bin::Params(p) => {
            if p.base_bin.maxbins.is_none() && p.base_bin.step.is_none() {
                let mut copy = p.clone();
                copy.base_bin.maxbins = Some(auto_max_bins(channel) as f64);
                copy
            } else {
                p.clone()
            }
        }
        Bin::Null => BinParams::default(),
    }
}

/// Create a key for the bin configuration. Not for prebinned bin.
pub fn bin_to_string(bin: &Bin) -> String {
    let params = match bin {
        Bin::Params(_) | Bin::Bool(_) => normalize_bin(bin, None),
        other => normalize_bin(other, None),
    };

    let parts: Vec<String> = params
        .ordered_entries()
        .into_iter()
        .map(|(p, value)| {
            if is_parameter_extent(&value) {
                var_name(&format!("_{}_{}", p, entries_to_string(&value)))
            } else {
                var_name(&format!("_{}_{}", p, js_value_to_string(&value)))
            }
        })
        .collect();

    format!("bin{}", parts.concat())
}

/// Vega-Lite should bin the data.
pub fn is_binning(bin: &Bin) -> bool {
    match bin {
        Bin::Bool(b) => *b,
        Bin::Params(p) => p.binned != Some(true),
        _ => false,
    }
}

/// The data is already binned and so Vega-Lite should not bin it again.
pub fn is_binned(bin: &Bin) -> bool {
    match bin {
        Bin::Binned => true,
        Bin::Params(p) => p.binned == Some(true),
        _ => false,
    }
}

/// Whether the value is a `BinParams` object.
pub fn is_bin_params(bin: &Bin) -> bool {
    matches!(bin, Bin::Params(_))
}

/// Whether the value is a parameter extent (`{param: ...}`).
pub fn is_parameter_extent(extent: &Value) -> bool {
    has_property(extent, "param")
}

/// Default maximum number of bins for a channel.
pub fn auto_max_bins(channel: Option<ExtendedChannel>) -> u32 {
    match channel {
        Some(
            ROW | COLUMN | SIZE | COLOR | FILL | STROKE | STROKEWIDTH | OPACITY | FILLOPACITY
            | STROKEOPACITY,
        ) => 6, // Facets and size shouldn't have too many bins
        Some(SHAPE) => 6, // Vega's "shape" has 6 distinct values
        Some(STROKEDASH) => 4, // We only provide 5 different stroke dash values
        _ => 10,
    }
}

/// `Array.prototype.toString` of `entries(extent)`.
fn entries_to_string(extent: &Value) -> String {
    match extent {
        Value::Object(map) => map
            .iter()
            .map(|(k, v)| format!("{k},{}", js_value_to_string(v)))
            .collect::<Vec<_>>()
            .join(","),
        _ => String::new(),
    }
}

/// Format a JSON value the way a JavaScript template literal would.
fn js_value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => js_number_to_string(n.as_f64().unwrap_or(f64::NAN)),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(items) => items
            .iter()
            .map(js_value_to_string)
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

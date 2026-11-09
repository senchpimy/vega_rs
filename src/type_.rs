//! Port of `src/type.ts`.

use serde::{Deserialize, Serialize};

/// Data type based on level of measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Type {
    Quantitative,
    Ordinal,
    Temporal,
    Nominal,
    Geojson,
}

pub const QUANTITATIVE: Type = Type::Quantitative;
pub const ORDINAL: Type = Type::Ordinal;
pub const TEMPORAL: Type = Type::Temporal;
pub const NOMINAL: Type = Type::Nominal;
pub const GEOJSON: Type = Type::Geojson;

pub const TYPES: [Type; 5] = [
    Type::Quantitative,
    Type::Ordinal,
    Type::Temporal,
    Type::Nominal,
    Type::Geojson,
];

pub const STANDARD_TYPES: [Type; 4] = [
    Type::Quantitative,
    Type::Ordinal,
    Type::Temporal,
    Type::Nominal,
];

impl Type {
    pub fn as_str(&self) -> &'static str {
        match self {
            Type::Quantitative => "quantitative",
            Type::Ordinal => "ordinal",
            Type::Temporal => "temporal",
            Type::Nominal => "nominal",
            Type::Geojson => "geojson",
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Type {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "quantitative" => Ok(Type::Quantitative),
            "ordinal" => Ok(Type::Ordinal),
            "temporal" => Ok(Type::Temporal),
            "nominal" => Ok(Type::Nominal),
            "geojson" => Ok(Type::Geojson),
            _ => Err(()),
        }
    }
}

pub fn is_type(t: &str) -> bool {
    matches!(
        t,
        "quantitative" | "ordinal" | "temporal" | "nominal" | "geojson"
    )
}

/// Type-only continuous check (does not consider binning).
pub fn is_continuous(t: Type) -> bool {
    matches!(t, Type::Quantitative | Type::Temporal)
}

/// Type-only discrete check (does not consider binning).
pub fn is_discrete(t: Type) -> bool {
    matches!(t, Type::Ordinal | Type::Nominal)
}

/// Get full, lowercase type name for a given type.
pub fn get_full_name(t: &str) -> Option<Type> {
    if t.is_empty() {
        return None;
    }
    match t.to_lowercase().as_str() {
        "q" | "quantitative" => Some(Type::Quantitative),
        "t" | "temporal" => Some(Type::Temporal),
        "o" | "ordinal" => Some(Type::Ordinal),
        "n" | "nominal" => Some(Type::Nominal),
        "geojson" => Some(Type::Geojson),
        _ => None,
    }
}

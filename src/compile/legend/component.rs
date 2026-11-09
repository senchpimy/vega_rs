//! Port of `src/compile/legend/component.ts`.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use crate::util::deep_equal;

/// Port of `LEGEND_SCALE_CHANNELS` from `src/legend.ts`.
pub const LEGEND_SCALE_CHANNELS: [&str; 7] = [
    "size",
    "shape",
    "fill",
    "stroke",
    "strokeDash",
    "strokeWidth",
    "opacity",
];

/// Port of `COMMON_LEGEND_PROPERTY_INDEX`, preserving upstream insertion order.
pub const COMMON_LEGEND_PROPERTIES: [&str; 65] = [
    "aria",
    "clipHeight",
    "columnPadding",
    "columns",
    "cornerRadius",
    "description",
    "direction",
    "fillColor",
    "format",
    "formatType",
    "gradientLength",
    "gradientOpacity",
    "gradientStrokeColor",
    "gradientStrokeWidth",
    "gradientThickness",
    "gridAlign",
    "labelAlign",
    "labelBaseline",
    "labelColor",
    "labelFont",
    "labelFontSize",
    "labelFontStyle",
    "labelFontWeight",
    "labelLimit",
    "labelOffset",
    "labelOpacity",
    "labelOverlap",
    "labelPadding",
    "labelSeparation",
    "legendX",
    "legendY",
    "offset",
    "orient",
    "padding",
    "rowPadding",
    "strokeColor",
    "symbolDash",
    "symbolDashOffset",
    "symbolFillColor",
    "symbolLimit",
    "symbolOffset",
    "symbolOpacity",
    "symbolSize",
    "symbolStrokeColor",
    "symbolStrokeWidth",
    "symbolType",
    "tickCount",
    "tickMinStep",
    "title",
    "titleAlign",
    "titleAnchor",
    "titleBaseline",
    "titleColor",
    "titleFont",
    "titleFontSize",
    "titleFontStyle",
    "titleFontWeight",
    "titleLimit",
    "titleLineHeight",
    "titleOpacity",
    "titleOrient",
    "titlePadding",
    "type",
    "values",
    "zindex",
];

/// Port of `LEGEND_COMPONENT_PROPERTY_INDEX`.
///
/// `COMMON_LEGEND_PROPERTIES`, followed by the legend-component-only props and
/// the channel scale properties.
pub fn legend_component_properties() -> Vec<String> {
    let mut props: Vec<String> = COMMON_LEGEND_PROPERTIES
        .iter()
        .map(|s| s.to_string())
        .collect();
    for prop in [
        "disable",
        "labelExpr",
        "selections",
        // channel scales
        "opacity",
        "shape",
        "stroke",
        "fill",
        "size",
        "strokeWidth",
        "strokeDash",
        // encode
        "encode",
    ] {
        props.push(prop.to_string());
    }
    props
}

/// Port of `LegendComponentProps` stored as a split of JSON values.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LegendComponent {
    pub explicit: BTreeMap<String, Value>,
    pub implicit: BTreeMap<String, Value>,
}

impl LegendComponent {
    pub fn new(explicit: BTreeMap<String, Value>, implicit: BTreeMap<String, Value>) -> Self {
        Self { explicit, implicit }
    }

    /// Port of `new LegendComponent({}, implicit)`.
    pub fn with_implicit(implicit: Map<String, Value>) -> Self {
        Self {
            explicit: BTreeMap::new(),
            implicit: implicit.into_iter().collect(),
        }
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        self.explicit
            .get(key)
            .or_else(|| self.implicit.get(key))
            .cloned()
    }

    pub fn get_with_explicit(&self, key: &str) -> crate::compile::split::Explicit {
        if let Some(v) = self.explicit.get(key) {
            crate::compile::split::Explicit {
                explicit: true,
                value: Some(v.clone()),
            }
        } else if let Some(v) = self.implicit.get(key) {
            crate::compile::split::Explicit {
                explicit: false,
                value: Some(v.clone()),
            }
        } else {
            crate::compile::split::Explicit::undefined()
        }
    }

    pub fn set(&mut self, key: &str, value: Value, explicit: bool) {
        if explicit {
            self.implicit.remove(key);
            self.explicit.insert(key.to_string(), value);
        } else {
            self.explicit.remove(key);
            self.implicit.insert(key.to_string(), value);
        }
    }

    pub fn set_with_explicit(&mut self, key: &str, value: crate::compile::split::Explicit) {
        if let Some(v) = value.value {
            self.set(key, v, value.explicit);
        }
    }

    pub fn combine(&self) -> BTreeMap<String, Value> {
        let mut out = self.explicit.clone();
        for (k, v) in &self.implicit {
            out.insert(k.clone(), v.clone());
        }
        out
    }

    pub fn combine_map(&self) -> Map<String, Value> {
        self.combine().into_iter().collect()
    }

    pub fn clone_component(&self) -> Self {
        self.clone()
    }

    /// Serialized form used by `Component::legends`.
    pub fn to_value(&self) -> Value {
        json!({
            "explicit": self.explicit,
            "implicit": self.implicit,
        })
    }

    pub fn from_value(value: &Value) -> Self {
        let explicit = value
            .get("explicit")
            .and_then(Value::as_object)
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        let implicit = value
            .get("implicit")
            .and_then(Value::as_object)
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        Self { explicit, implicit }
    }
}

/// Port of `LegendComponentIndex`.
pub type LegendComponentIndex = BTreeMap<String, LegendComponent>;

/// Port of `LegendComponentIndex` serialization for `Component::legends`.
pub fn legends_to_value(index: &LegendComponentIndex) -> Value {
    let mut out = Map::new();
    for (channel, component) in index {
        out.insert(channel.clone(), component.to_value());
    }
    Value::Object(out)
}

pub fn legends_from_value(value: &Value) -> LegendComponentIndex {
    let mut out = LegendComponentIndex::new();
    if let Some(map) = value.as_object() {
        for (channel, component) in map {
            out.insert(channel.clone(), LegendComponent::from_value(component));
        }
    }
    out
}

/// Port of `LegendInternalIndex` (specified legend definitions by channel).
pub type LegendInternalIndex = BTreeMap<String, Value>;

/// Helper comparing a value with a legend property (JS `===`, with deep
/// equality for objects/arrays).
pub fn value_matches(value: &Value, other: Option<&Value>) -> bool {
    match other {
        Some(other) => deep_equal(value, other),
        None => false,
    }
}

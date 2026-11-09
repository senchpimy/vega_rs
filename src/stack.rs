//! Port of `src/stack.ts`.
//!
//! The TypeScript module works with `Encoding` maps whose values are unions of
//! channel definitions. In this port those maps are [`serde_json::Value`]
//! objects and the mark argument is either a mark name (`"bar"`) or a mark
//! definition (`{"type": "bar", "orient": "vertical"}`), exactly like upstream
//! `Mark | MarkDef`.
//!
//! Upstream returns `null` when a mark cannot be stacked; here the equivalent is
//! `None` (see [`stack`]).
//!
//! [`stack`]: fn@stack

use std::collections::BTreeSet;
use std::str::FromStr;

use serde_json::{json, Value};

use crate::aggregate::SUM_OPS;
use crate::channel::{get_secondary_range_channel, NONPOSITION_CHANNELS};
use crate::channeldef::{
    channel_def_type, get_field_def, is_field_def, is_field_or_datum_def, vg_field,
};
use crate::channeldef_support::truthy;
use crate::encoding::{channel_has_field, channel_has_quantitative_offset, is_aggregate};
use crate::log;
use crate::mark::{
    is_bar_or_area, is_mark_def, is_path_mark, is_stack_by_default_mark, is_stackable_mark,
};
use crate::scale::ScaleType;

// Re-exported for parity with `import {STACKABLE_MARKS, ...} from './stack.js'`.
pub use crate::mark::{STACKABLE_MARKS, STACK_BY_DEFAULT_MARKS};

// ---------------------------------------------------------------------------
// StackOffset
// ---------------------------------------------------------------------------

/// Port of `StackOffset` (`'zero' | 'center' | 'normalize'`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StackOffset {
    Zero,
    Center,
    Normalize,
}

impl StackOffset {
    pub fn as_str(&self) -> &'static str {
        match self {
            StackOffset::Zero => "zero",
            StackOffset::Center => "center",
            StackOffset::Normalize => "normalize",
        }
    }
}

impl std::fmt::Display for StackOffset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for StackOffset {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "zero" => Ok(StackOffset::Zero),
            "center" => Ok(StackOffset::Center),
            "normalize" => Ok(StackOffset::Normalize),
            _ => Err(()),
        }
    }
}

/// Port of `isStackOffset`.
pub fn is_stack_offset(s: &str) -> bool {
    matches!(s, "zero" | "center" | "normalize")
}

// ---------------------------------------------------------------------------
// StackProperties
// ---------------------------------------------------------------------------

/// One entry of `StackProperties.stackBy`.
#[derive(Debug, Clone, PartialEq)]
pub struct StackBy {
    pub field_def: Value,
    pub channel: String,
}

/// Port of `StackProperties`.
#[derive(Debug, Clone, PartialEq)]
pub struct StackProperties {
    /// Dimension axis of the stack.
    pub groupby_channels: Vec<String>,
    /// Fields for `groupbyChannels`.
    pub groupby_fields: BTreeSet<String>,
    /// Measure axis of the stack.
    pub field_channel: String,
    /// Stack-by fields, e.g. color and detail.
    pub stack_by: Vec<StackBy>,
    /// See the `stack` property of a position field definition.
    pub offset: StackOffset,
    /// Whether this stack will produce an impute transform.
    pub impute: bool,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// JavaScript's `key in object`: presence of the property, even when its value
/// is `null` (which is *not* the same as `undefined`).
fn has_defined_property(obj: &Value, key: &str) -> bool {
    obj.as_object()
        .map(|map| map.contains_key(key))
        .unwrap_or(false)
}

fn effective_mark_def(m: &Value) -> Value {
    if is_mark_def(m) {
        m.clone()
    } else {
        json!({ "type": m })
    }
}

/// Port of the private `isUnbinnedQuantitative`.
fn is_unbinned_quantitative(channel_def: &Value) -> bool {
    is_field_def(channel_def)
        && channel_def_type(channel_def).as_deref() == Some("quantitative")
        && !truthy(&channel_def["bin"])
}

/// Port of `potentialStackedChannel`.
fn potential_stacked_channel(encoding: &Value, x: &str, mark_def: &Value) -> Option<String> {
    let y = if x == "x" { "y" } else { "radius" };
    let mark = mark_def.get("type").cloned().unwrap_or(Value::Null);
    let orient = mark_def.get("orient").and_then(|o| o.as_str());

    let is_cartesian_bar_or_area = x == "x" && is_bar_or_area(&mark);

    let x_def = &encoding[x];
    let y_def = &encoding[y];

    if is_field_def(x_def) && is_field_def(y_def) {
        if is_unbinned_quantitative(x_def) && is_unbinned_quantitative(y_def) {
            if truthy(&x_def["stack"]) {
                return Some(x.to_string());
            } else if truthy(&y_def["stack"]) {
                return Some(y.to_string());
            }
            let x_aggregate = is_field_def(x_def) && truthy(&x_def["aggregate"]);
            let y_aggregate = is_field_def(y_def) && truthy(&y_def["aggregate"]);
            // If there is no explicit stacking, only apply stack if there is
            // only one aggregate for x or y.
            if x_aggregate != y_aggregate {
                return Some(if x_aggregate { x } else { y }.to_string());
            }

            if is_cartesian_bar_or_area {
                if orient == Some("vertical") {
                    return Some(y.to_string());
                } else if orient == Some("horizontal") {
                    return Some(x.to_string());
                }
            }
        } else if is_unbinned_quantitative(x_def) {
            return Some(x.to_string());
        } else if is_unbinned_quantitative(y_def) {
            return Some(y.to_string());
        }
    } else if is_unbinned_quantitative(x_def) {
        if is_cartesian_bar_or_area && orient == Some("vertical") {
            return None;
        }
        return Some(x.to_string());
    } else if is_unbinned_quantitative(y_def) {
        if is_cartesian_bar_or_area && orient == Some("horizontal") {
            return None;
        }
        return Some(y.to_string());
    }
    None
}

/// Port of `getDimensionChannel`.
fn get_dimension_channel(channel: &str) -> &'static str {
    match channel {
        "x" => "y",
        "y" => "x",
        "theta" => "radius",
        "radius" => "theta",
        _ => unreachable!("invalid stack field channel"),
    }
}

// ---------------------------------------------------------------------------
// stack
// ---------------------------------------------------------------------------

/// Port of `stack(m, encoding)`.
///
/// Returns `None` where the TypeScript implementation returns `null`.
pub fn stack(mark: &Value, encoding: &Value) -> Option<StackProperties> {
    let mark_def = effective_mark_def(mark);
    let mark = mark_def.get("type").cloned().unwrap_or(Value::Null);

    // Should have stackable mark.
    if !is_stackable_mark(&mark) {
        return None;
    }

    // Run potential stacked twice, one for Cartesian and another for Polar, so
    // text marks can be stacked in any of the coordinates.
    let field_channel = potential_stacked_channel(encoding, "x", &mark_def)
        .or_else(|| potential_stacked_channel(encoding, "theta", &mark_def))?;

    let stacked_field_def = encoding.get(&field_channel).cloned().unwrap_or(Value::Null);
    let stacked_field = if is_field_def(&stacked_field_def) {
        Some(vg_field(&stacked_field_def))
    } else {
        None
    };

    let stack_defined = has_defined_property(&stacked_field_def, "stack");

    if !stack_defined
        && ((field_channel == "x"
            && !truthy(&encoding["y"])
            && channel_has_quantitative_offset(encoding, "y"))
            || (field_channel == "y"
                && !truthy(&encoding["x"])
                && channel_has_quantitative_offset(encoding, "x")))
    {
        return None;
    }

    let dimension_channel = get_dimension_channel(&field_channel);
    let mut groupby_channels: Vec<String> = Vec::new();
    let mut groupby_fields: BTreeSet<String> = BTreeSet::new();

    if truthy(&encoding[dimension_channel]) {
        let dimension_def = &encoding[dimension_channel];
        let dimension_field = if is_field_def(dimension_def) {
            Some(vg_field(dimension_def))
        } else {
            None
        };

        if let Some(dimension_field) = dimension_field {
            if Some(&dimension_field) != stacked_field.as_ref() {
                // Avoid grouping by the stacked field.
                groupby_channels.push(dimension_channel.to_string());
                groupby_fields.insert(dimension_field);
            }
        }
    }

    let dimension_offset_channel = if dimension_channel == "x" {
        "xOffset"
    } else {
        "yOffset"
    };
    let dimension_offset_def = &encoding[dimension_offset_channel];
    let dimension_offset_field = if is_field_def(dimension_offset_def) {
        Some(vg_field(dimension_offset_def))
    } else {
        None
    };

    if let Some(dimension_offset_field) = dimension_offset_field {
        if Some(&dimension_offset_field) != stacked_field.as_ref() {
            // Avoid grouping by the stacked field.
            groupby_channels.push(dimension_offset_channel.to_string());
            groupby_fields.insert(dimension_offset_field);
        }
    }

    // Should have grouping level of detail that is different from the
    // dimension field.
    let mut stack_by: Vec<StackBy> = Vec::new();
    for channel in NONPOSITION_CHANNELS {
        // Ignore tooltip in stackBy
        // (https://github.com/vega/vega-lite/issues/4001).
        if channel != "tooltip" && channel_has_field(encoding, channel) {
            let channel_def = &encoding[channel];
            let defs = match channel_def {
                Value::Array(items) => items.clone(),
                other => vec![other.clone()],
            };
            for c_def in &defs {
                let field_def = match get_field_def(c_def) {
                    Some(field_def) => field_def,
                    None => continue,
                };
                if truthy(&field_def["aggregate"]) {
                    continue;
                }

                // Check whether the channel's field is identical to x/y's
                // field or if the channel is a repeat.
                let f = vg_field(&field_def);
                if f.is_empty() || !groupby_fields.contains(&f) {
                    stack_by.push(StackBy {
                        channel: channel.to_string(),
                        field_def,
                    });
                }
            }
        }
    }

    // Automatically determine offset.
    let offset = if stack_defined {
        let stack_value = &stacked_field_def["stack"];
        if stack_value.is_boolean() {
            if stack_value.as_bool() == Some(true) {
                Some(StackOffset::Zero)
            } else {
                None
            }
        } else {
            stack_value
                .as_str()
                .and_then(|s| StackOffset::from_str(s).ok())
        }
    } else if is_stack_by_default_mark(&mark) {
        Some(StackOffset::Zero)
    } else {
        None
    };

    let offset = offset?;

    if is_aggregate(encoding) && stack_by.is_empty() {
        return None;
    }

    // Warn when stacking non-linear.
    if let Some(scale_type) = stacked_field_def
        .get("scale")
        .and_then(|scale| scale.get("type"))
        .and_then(|t| t.as_str())
    {
        if scale_type != ScaleType::Linear.as_str() && truthy(&stacked_field_def["stack"]) {
            log::warn(log::message::stack_non_linear_scale(scale_type));
        }
    }

    // Check if it is a ranged mark.
    if let Some(secondary_channel) = get_secondary_range_channel(&field_channel) {
        if is_field_or_datum_def(&encoding[secondary_channel]) {
            if stack_defined {
                log::warn(log::message::cannot_stack_ranged_mark(&field_channel));
            }
            return None;
        }
    }

    // Warn if stacking non-summative aggregate.
    if is_field_def(&stacked_field_def) && truthy(&stacked_field_def["aggregate"]) {
        let aggregate = &stacked_field_def["aggregate"];
        let is_summative = aggregate
            .as_str()
            .map(|a| SUM_OPS.contains(&a))
            .unwrap_or(false);
        if !is_summative {
            let name = aggregate
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| crate::channeldef_support::js_to_string(aggregate));
            log::warn(log::message::stack_non_summative_aggregate(&name));
        }
    }

    Some(StackProperties {
        groupby_channels,
        groupby_fields,
        field_channel,
        impute: if has_defined_property(&stacked_field_def, "impute")
            && stacked_field_def["impute"].is_null()
        {
            false
        } else {
            is_path_mark(&mark)
        },
        stack_by,
        offset,
    })
}

//! Port of `src/encoding.ts`.
//!
//! The TypeScript `Encoding` / `EncodingWithFacet` interfaces have dozens of
//! optional members whose shapes are unions; in this port they are represented
//! by [`serde_json::Value`].  Functions take `&Value` and return `Value` where
//! the upstream code returns an `Encoding` object.
//!
//! JSON objects in `serde_json` are ordered by key rather than by insertion
//! order, so helpers that upstream iterate with `keys(encoding)` use the
//! canonical channel order from [`crate::channel::CHANNEL_INDEX`] instead.
//! This gives stable, Vega-Lite-like ordering (e.g. `x` before `color`).

use std::str::FromStr;

use serde_json::{Map, Value};

use crate::channel::{
    self, is_x_or_y, is_x_or_y_offset, Channel, Mark, MarkSupport, CHANNELS, COLOR, DETAIL, FILL,
    HREF, KEY, ORDER, SIZE, STROKE, TEXT, THETA, TOOLTIP, UNIT_CHANNELS, URL, X2,
};
use crate::channeldef::{
    bin_requires_range, get_field_def, get_guide, has_conditional_field_def, init_channel_def,
    init_channel_def_with, init_field_def, is_conditional_def, is_datum_def, is_field_def,
    is_order_only_def, is_typed_field_def, is_value_def, title, vg_field, vg_field_with,
    FieldRefOption, InitOpt, TitleOpt,
};
use crate::channeldef_support as support;
use crate::channeldef_support::truthy;
use crate::log;
use crate::mark::MarkLike;
use crate::type_::{is_continuous, is_discrete, Type};
use crate::util::keys;

// ---------------------------------------------------------------------------
// Type aliases
// ---------------------------------------------------------------------------

/// Port of the `Encoding<F>` interface.
pub type Encoding = Value;

/// Port of the `EncodingWithFacet<F>` interface.
pub type EncodingWithFacet = Value;

// ---------------------------------------------------------------------------
// Log messages that live in `src/log/message.ts` upstream.
//
// `src/log.rs` is out of scope for this port, so the four messages used by
// `encoding.ts` are provided here with identical text.
// ---------------------------------------------------------------------------

/// Port of `log.message.REPLACE_ANGLE_WITH_THETA`.
pub const REPLACE_ANGLE_WITH_THETA: &str =
    "Arc marks uses theta channel rather than angle, replacing angle with theta.";

/// Port of `log.message.offsetNestedInsideContinuousPositionScaleDropped`.
pub fn offset_nested_inside_continuous_position_scale_dropped(main_channel: &str) -> String {
    format!("{main_channel}Offset dropped because {main_channel} is continuous")
}

/// Port of `log.message.droppingColor`.
pub fn dropping_color(type_: &str, fill: bool, stroke: bool) -> String {
    format!(
        "Dropping color {type_} as the plot also has {}.",
        if fill && stroke {
            "fill and stroke"
        } else if fill {
            "fill"
        } else {
            "stroke"
        }
    )
}

/// Port of `log.message.LINE_WITH_VARYING_SIZE`.
pub const LINE_WITH_VARYING_SIZE: &str =
    "Line marks cannot encode size with a non-groupby field. You may want to use trail marks instead.";

// ---------------------------------------------------------------------------
// Channel ordering helpers
// ---------------------------------------------------------------------------

/// Keys of an encoding object in canonical channel order.
///
/// Channels that are not part of `CHANNEL_INDEX` (i.e. invalid channels) come
/// last, sorted, so that warning output is deterministic.
fn ordered_keys(encoding: &Value) -> Vec<String> {
    let map = match encoding.as_object() {
        Some(map) => map,
        None => return Vec::new(),
    };
    let mut out: Vec<String> = Vec::with_capacity(map.len());
    for channel in CHANNELS {
        if map.contains_key(channel) {
            out.push(channel.to_string());
        }
    }
    let mut unknown: Vec<String> = map
        .keys()
        .filter(|k| !CHANNELS.contains(&k.as_str()))
        .cloned()
        .collect();
    unknown.sort();
    out.extend(unknown);
    out
}

/// `vega-util`'s `array()`: wrap a non-array value in a one-element vector.
fn as_array(value: &Value) -> Vec<Value> {
    match value {
        Value::Array(items) => items.clone(),
        other => vec![other.clone()],
    }
}

/// Type-only continuous check for a field/datum definition.
fn field_def_is_continuous(def: &Value) -> bool {
    def.get("type")
        .and_then(|t| t.as_str())
        .and_then(|t| Type::from_str(t).ok())
        .map(is_continuous)
        .unwrap_or(false)
}

/// Type-only discrete check for a field/datum definition.
fn field_def_is_discrete(def: &Value) -> bool {
    def.get("type")
        .and_then(|t| t.as_str())
        .and_then(|t| Type::from_str(t).ok())
        .map(is_discrete)
        .unwrap_or(false)
}

fn channel_mark(mark: &str) -> Option<Mark> {
    Mark::from_str(mark).ok()
}

// ---------------------------------------------------------------------------
// channelHasField / channelHasFieldOrDatum
// ---------------------------------------------------------------------------

/// Port of `channelHasField`.
pub fn channel_has_field(encoding: &Value, channel: &str) -> bool {
    let channel_def = &encoding[channel];
    if truthy(channel_def) {
        if channel_def.is_array() {
            crate::util::some(&as_array(channel_def), |field_def, _| {
                truthy(&field_def["field"])
            })
        } else {
            is_field_def(channel_def) || has_conditional_field_def(channel_def)
        }
    } else {
        false
    }
}

/// Port of `channelHasFieldOrDatum`.
pub fn channel_has_field_or_datum(encoding: &Value, channel: &str) -> bool {
    let channel_def = &encoding[channel];
    if truthy(channel_def) {
        if channel_def.is_array() {
            crate::util::some(&as_array(channel_def), |field_def, _| {
                truthy(&field_def["field"])
            })
        } else {
            is_field_def(channel_def)
                || is_datum_def(channel_def)
                || crate::channeldef::has_conditional_field_or_datum_def(channel_def)
        }
    } else {
        false
    }
}

/// Port of `channelHasNestedOffsetScale`.
pub fn channel_has_nested_offset_scale(encoding: &Value, channel: &str) -> bool {
    if is_x_or_y(channel) {
        let field_def = &encoding[channel];
        if (is_field_def(field_def) || is_datum_def(field_def))
            && (field_def_is_discrete(field_def)
                || (is_field_def(field_def) && truthy(&field_def["timeUnit"])))
        {
            if let Some(offset_channel) = channel::get_offset_scale_channel(channel) {
                return channel_has_field_or_datum(encoding, offset_channel);
            }
        }
    }
    false
}

/// Port of `channelHasQuantitativeOffset`.
pub fn channel_has_quantitative_offset(encoding: &Value, channel: &str) -> bool {
    let offset_channel = match channel::get_offset_scale_channel(channel) {
        Some(offset_channel) => offset_channel,
        None => return false,
    };
    crate::channeldef::is_unbinned_quantitative_field_or_datum_def(&encoding[offset_channel])
}

// ---------------------------------------------------------------------------
// isAggregate / extractTransformsFromEncoding
// ---------------------------------------------------------------------------

/// Port of `isAggregate`.
pub fn is_aggregate(encoding: &Value) -> bool {
    crate::util::some(&CHANNELS, |channel, _| {
        if channel_has_field(encoding, channel) {
            let channel_def = &encoding[*channel];
            if channel_def.is_array() {
                crate::util::some(&as_array(channel_def), |field_def, _| {
                    truthy(&field_def["aggregate"])
                })
            } else {
                get_field_def(channel_def)
                    .map(|field_def| truthy(&field_def["aggregate"]))
                    .unwrap_or(false)
            }
        } else {
            false
        }
    })
}

/// Result of [`extract_transforms_from_encoding`].
///
/// Upstream returns an anonymous object with `bins`, `timeUnits`, `aggregate`,
/// `groupby` and `encoding`.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtractTransformsOutput {
    pub bins: Vec<Value>,
    pub time_units: Vec<Value>,
    pub aggregate: Vec<Value>,
    pub groupby: Vec<String>,
    pub encoding: Value,
}

/// Port of `extractTransformsFromEncoding`.
pub fn extract_transforms_from_encoding(
    old_encoding: &Value,
    config: &Value,
) -> ExtractTransformsOutput {
    let mut groupby: Vec<String> = Vec::new();
    let mut bins: Vec<Value> = Vec::new();
    let mut time_units: Vec<Value> = Vec::new();
    let mut aggregate: Vec<Value> = Vec::new();
    let mut encoding = Map::new();

    for channel in ordered_keys(old_encoding) {
        // `forEach` flattens arrays of channel definitions.
        for channel_def in as_array(&old_encoding[&channel]) {
            let channel_def = &channel_def;

            // Extract potential embedded transformations along with remaining properties.
            if is_field_def(channel_def) {
                let field = channel_def.get("field").cloned().unwrap_or(Value::Null);
                let agg_op = channel_def.get("aggregate").cloned().unwrap_or(Value::Null);
                let bin = channel_def.get("bin").cloned().unwrap_or(Value::Null);
                let time_unit = channel_def.get("timeUnit").cloned().unwrap_or(Value::Null);

                let mut remaining = channel_def.as_object().cloned().unwrap_or_default();
                remaining.remove("field");
                remaining.remove("aggregate");
                remaining.remove("bin");
                remaining.remove("timeUnit");

                if truthy(&agg_op) || truthy(&time_unit) || truthy(&bin) {
                    let is_title_defined = get_guide(channel_def)
                        .and_then(|guide| guide.get("title"))
                        .map(truthy)
                        .unwrap_or(false);
                    let mut new_field = vg_field_with(
                        channel_def,
                        &FieldRefOption {
                            for_as: true,
                            ..Default::default()
                        },
                    );
                    let mut new_field_def = Map::new();

                    if !is_title_defined {
                        let new_title = title(
                            channel_def,
                            config,
                            &TitleOpt {
                                allow_disabling: true,
                                include_default: true,
                            },
                        );
                        if !new_title.is_null() {
                            new_field_def.insert("title".to_string(), new_title);
                        }
                    }

                    for (key, value) in remaining {
                        new_field_def.insert(key, value);
                    }
                    new_field_def.insert("field".to_string(), Value::String(new_field.clone()));

                    if truthy(&agg_op) {
                        let mut op: Option<Value> = None;

                        if support::is_argmax_def(&agg_op) {
                            op = Some(Value::String("argmax".to_string()));
                            new_field = vg_field_with(
                                &serde_json::json!({"op": "argmax", "field": agg_op["argmax"]}),
                                &FieldRefOption {
                                    for_as: true,
                                    ..Default::default()
                                },
                            );
                            new_field_def.insert(
                                "field".to_string(),
                                Value::String(format!(
                                    "{}.{}",
                                    new_field,
                                    support::js_to_string(&field)
                                )),
                            );
                        } else if support::is_argmin_def(&agg_op) {
                            op = Some(Value::String("argmin".to_string()));
                            new_field = vg_field_with(
                                &serde_json::json!({"op": "argmin", "field": agg_op["argmin"]}),
                                &FieldRefOption {
                                    for_as: true,
                                    ..Default::default()
                                },
                            );
                            new_field_def.insert(
                                "field".to_string(),
                                Value::String(format!(
                                    "{}.{}",
                                    new_field,
                                    support::js_to_string(&field)
                                )),
                            );
                        } else if support::is_parameterized_aggregate_def(&agg_op)
                            || (agg_op.as_str() != Some("boxplot")
                                && agg_op.as_str() != Some("errorbar")
                                && agg_op.as_str() != Some("errorband"))
                        {
                            op = Some(agg_op.clone());
                        }

                        if let Some(op) = op {
                            let mut aggregate_entry = Map::new();
                            aggregate_entry.insert("op".to_string(), op);
                            aggregate_entry.insert("as".to_string(), Value::String(new_field));
                            if truthy(&field) {
                                aggregate_entry.insert("field".to_string(), field.clone());
                            }
                            aggregate.push(Value::Object(aggregate_entry));
                        }
                    } else {
                        groupby.push(new_field.clone());
                        if is_typed_field_def(channel_def) && support::is_binning(&bin) {
                            bins.push(serde_json::json!({
                                "bin": bin,
                                "field": field,
                                "as": new_field,
                            }));
                            // Add additional groupbys for range and end of bins.
                            groupby.push(vg_field_with(
                                channel_def,
                                &FieldRefOption {
                                    bin_suffix: Some("end".to_string()),
                                    ..Default::default()
                                },
                            ));
                            if bin_requires_range(channel_def, &channel) {
                                groupby.push(vg_field_with(
                                    channel_def,
                                    &FieldRefOption {
                                        bin_suffix: Some("range".to_string()),
                                        ..Default::default()
                                    },
                                ));
                            }
                            // Create accompanying `x2` or `y2` field if channel is `x` or `y`.
                            if is_x_or_y(&channel) {
                                let mut secondary = Map::new();
                                secondary.insert(
                                    "field".to_string(),
                                    Value::String(format!("{new_field}_end")),
                                );
                                encoding.insert(format!("{channel}2"), Value::Object(secondary));
                            }
                            new_field_def
                                .insert("bin".to_string(), Value::String("binned".to_string()));
                            if !is_secondary_range_channel(&channel) {
                                new_field_def.insert(
                                    "type".to_string(),
                                    Value::String("quantitative".to_string()),
                                );
                            }
                        } else if truthy(&time_unit) && !support::is_binned_time_unit(&time_unit) {
                            time_units.push(serde_json::json!({
                                "timeUnit": time_unit,
                                "field": field,
                                "as": new_field,
                            }));

                            // Define the format type for later compilation.
                            let format_type = if is_typed_field_def(channel_def)
                                && channel_def.get("type").and_then(|t| t.as_str())
                                    != Some(Type::Temporal.as_str())
                            {
                                Some("time")
                            } else {
                                None
                            };
                            if let Some(format_type) = format_type {
                                if channel == TEXT || channel == TOOLTIP {
                                    new_field_def.insert(
                                        "formatType".to_string(),
                                        Value::String(format_type.to_string()),
                                    );
                                } else if channel::is_non_position_scale_channel(&channel) {
                                    let existing = new_field_def
                                        .get("legend")
                                        .and_then(|l| l.as_object())
                                        .cloned()
                                        .unwrap_or_default();
                                    let mut legend = Map::new();
                                    legend.insert(
                                        "formatType".to_string(),
                                        Value::String(format_type.to_string()),
                                    );
                                    for (key, value) in existing {
                                        legend.insert(key, value);
                                    }
                                    new_field_def
                                        .insert("legend".to_string(), Value::Object(legend));
                                } else if is_x_or_y(&channel) {
                                    let existing = new_field_def
                                        .get("axis")
                                        .and_then(|a| a.as_object())
                                        .cloned()
                                        .unwrap_or_default();
                                    let mut axis = Map::new();
                                    axis.insert(
                                        "formatType".to_string(),
                                        Value::String(format_type.to_string()),
                                    );
                                    for (key, value) in existing {
                                        axis.insert(key, value);
                                    }
                                    new_field_def.insert("axis".to_string(), Value::Object(axis));
                                }
                            }
                        }
                    }

                    // Now the field should refer to the post-transformed field instead.
                    encoding.insert(channel.clone(), Value::Object(new_field_def));
                } else {
                    groupby.push(support::js_to_string(&field));
                    encoding.insert(channel.clone(), old_encoding[&channel].clone());
                }
            } else {
                // For value def / signal ref / datum def, just copy.
                encoding.insert(channel.clone(), old_encoding[&channel].clone());
            }
        }
    }

    ExtractTransformsOutput {
        bins,
        time_units,
        aggregate,
        groupby,
        encoding: Value::Object(encoding),
    }
}

// ---------------------------------------------------------------------------
// markChannelCompatible
// ---------------------------------------------------------------------------

/// Port of `markChannelCompatible`.
pub fn mark_channel_compatible<M: MarkLike + ?Sized>(
    encoding: &Value,
    channel: &str,
    mark: &M,
) -> bool {
    let mark_str = match mark.mark_type() {
        Some(mark_str) => mark_str,
        None => return false,
    };
    let channel_mark = match channel_mark(mark_str) {
        Some(channel_mark) => channel_mark,
        None => return false,
    };

    match support_mark(channel, channel_mark) {
        None => false,
        Some(MarkSupport::Binned) => {
            let primary_field_def = &encoding[if channel == X2 { "x" } else { "y" }];

            // circle, point, square and tick only support x2/y2 when their
            // corresponding x/y fieldDef has "binned" data and thus needs
            // x2/y2 to specify the bin-end field.
            is_field_def(primary_field_def)
                && is_field_def(&encoding[channel])
                && support::is_binned(&primary_field_def["bin"])
        }
        Some(MarkSupport::Always) => true,
    }
}

// ---------------------------------------------------------------------------
// initEncoding / normalizeEncoding
// ---------------------------------------------------------------------------

/// Port of `initEncoding`.
pub fn init_encoding(
    encoding: &Value,
    mark: crate::mark::Mark,
    filled: Option<bool>,
    config: &Value,
) -> Value {
    let mut normalized_encoding = Map::new();
    let mark_str = mark.as_str();

    for key in keys(encoding) {
        if !channel::is_channel(&key) {
            // Drop invalid channel.
            log::warn(log::message::invalid_encoding_channel(&key));
        }
    }

    for channel in UNIT_CHANNELS {
        if !truthy(&encoding[channel]) {
            continue;
        }

        let mut channel: Channel = channel;
        let channel_def = &encoding[channel];

        if is_x_or_y_offset(channel) {
            if let Some(main_channel) = channel::get_main_channel_from_offset_channel(channel) {
                let position_def = normalized_encoding
                    .get(main_channel)
                    .unwrap_or(&Value::Null);
                if is_field_def(position_def) && field_def_is_continuous(position_def)
                    && is_field_def(channel_def) && !truthy(&position_def["timeUnit"]) {
                        // TODO: nesting continuous field instead continuous field should
                        // behave like offsetting the data in data domain.
                        log::warn(offset_nested_inside_continuous_position_scale_dropped(
                            main_channel,
                        ));
                        continue;
                    }
            }
        }

        if channel == "angle" && mark_str == "arc" && !truthy(&encoding["theta"]) {
            log::warn(REPLACE_ANGLE_WITH_THETA);
            channel = THETA;
        }

        if !mark_channel_compatible(encoding, channel, &mark) {
            // Drop unsupported channel.
            log::warn(log::message::incompatible_channel(channel, mark_str, None));
            continue;
        }

        // Drop line's size if the field is aggregated.
        if channel == SIZE && mark_str == "line" {
            if let Some(field_def) = get_field_def(&encoding[channel]) {
                if truthy(&field_def["aggregate"]) {
                    log::warn(LINE_WITH_VARYING_SIZE);
                    continue;
                }
            }
        }

        // Drop color if either fill or stroke is specified.
        let fill_present = encoding
            .as_object()
            .map(|map| map.contains_key(FILL))
            .unwrap_or(false);
        let stroke_present = encoding
            .as_object()
            .map(|map| map.contains_key(STROKE))
            .unwrap_or(false);
        if channel == COLOR
            && if filled == Some(true) {
                fill_present
            } else {
                stroke_present
            }
        {
            log::warn(dropping_color("encoding", fill_present, stroke_present));
            continue;
        }

        if channel == DETAIL
            || (channel == ORDER && !channel_def.is_array() && !is_value_def(channel_def))
            || (channel == TOOLTIP && channel_def.is_array())
        {
            if truthy(channel_def) {
                if channel == ORDER {
                    let def = &encoding[ORDER];
                    if is_order_only_def(def) {
                        normalized_encoding.insert(channel.to_string(), def.clone());
                        continue;
                    }
                }
                // Array of fieldDefs for detail channel (or production rule).
                let mut defs: Vec<Value> = Vec::new();
                for field_def in as_array(channel_def) {
                    if !is_field_def(&field_def) {
                        log::warn(log::message::empty_field_def(&field_def, channel));
                    } else {
                        defs.push(init_field_def(&field_def, channel, &InitOpt::default()));
                    }
                }
                normalized_encoding.insert(channel.to_string(), Value::Array(defs));
            }
        } else {
            if channel == TOOLTIP && channel_def.is_null() {
                // Preserve null so we can use it to disable tooltip.
                normalized_encoding.insert(channel.to_string(), Value::Null);
            } else if !is_field_def(channel_def)
                && !is_datum_def(channel_def)
                && !is_value_def(channel_def)
                && !is_conditional_def(channel_def)
                && !support::is_signal_ref(channel_def)
            {
                log::warn(log::message::empty_field_def(channel_def, channel));
                continue;
            }

            normalized_encoding.insert(
                channel.to_string(),
                init_channel_def(channel_def, channel, config),
            );
        }
    }

    Value::Object(normalized_encoding)
}

/// Port of `normalizeEncoding`.
///
/// For composite marks, `initChannelDef` has to be called during init so we can
/// infer types earlier.
pub fn normalize_encoding(encoding: &Value, config: &Value) -> Value {
    let mut normalized_encoding = Map::new();

    for channel in keys(encoding) {
        let new_channel_def = init_channel_def_with(
            &encoding[&channel],
            &channel,
            config,
            &InitOpt {
                composite_mark: true,
            },
        );
        normalized_encoding.insert(channel, new_channel_def);
    }

    Value::Object(normalized_encoding)
}

// ---------------------------------------------------------------------------
// fieldDefs / forEach / reduce
// ---------------------------------------------------------------------------

/// Port of `fieldDefs`.
pub fn field_defs(encoding: &Value) -> Vec<Value> {
    let mut arr: Vec<Value> = Vec::new();
    for channel in ordered_keys(encoding) {
        if channel_has_field(encoding, &channel) {
            let channel_def = &encoding[&channel];
            for def in as_array(channel_def) {
                if is_field_def(&def) {
                    arr.push(def);
                } else if has_conditional_field_def(&def) {
                    arr.push(def["condition"].clone());
                }
            }
        }
    }
    arr
}

/// Port of `forEach`. Visits each channel definition of an encoding mapping.
pub fn for_each<U: FnMut(&Value, &str)>(mapping: &Value, mut f: U) {
    if !truthy(mapping) {
        return;
    }
    for channel in ordered_keys(mapping) {
        let el = &mapping[&channel];
        if el.is_array() {
            for channel_def in as_array(el) {
                f(&channel_def, &channel);
            }
        } else {
            f(el, &channel);
        }
    }
}

/// Port of `reduce`. Folds over every channel definition of an encoding mapping.
pub fn reduce<T, U: FnMut(T, &Value, &str) -> T>(mapping: &Value, mut f: U, init: T) -> T {
    if !truthy(mapping) {
        return init;
    }
    let mut result = init;
    for channel in ordered_keys(mapping) {
        let map = &mapping[&channel];
        if map.is_array() {
            for channel_def in as_array(map) {
                result = f(result, &channel_def, &channel);
            }
        } else {
            result = f(result, map, &channel);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// pathGroupingFields
// ---------------------------------------------------------------------------

fn path_grouping_detail_fields(details: &mut Vec<String>, channel_def: &Value) {
    if channel_def.is_array() || is_field_def(channel_def) {
        for field_def in as_array(channel_def) {
            if !truthy(&field_def["aggregate"]) {
                details.push(vg_field(&field_def));
            }
        }
    }
}

/// Port of `pathGroupingFields`. Returns the list of path grouping fields for
/// the given encoding.
pub fn path_grouping_fields<M: MarkLike + ?Sized>(mark: &M, encoding: &Value) -> Vec<String> {
    let mark_str = mark.mark_type().unwrap_or("");
    let mut details: Vec<String> = Vec::new();

    for channel in ordered_keys(encoding) {
        let channel_def = &encoding[&channel];
        match channel.as_str() {
            // x, y, x2, y2, lat, long, lat1, long2, order, tooltip, href,
            // aria label, cursor should not cause lines to group.
            channel::X
            | channel::Y
            | HREF
            | channel::DESCRIPTION
            | URL
            | channel::X2
            | channel::Y2
            | channel::XOFFSET
            | channel::YOFFSET
            | channel::THETA
            | channel::THETA2
            | channel::RADIUS
            | channel::RADIUS2
            | channel::TIME
            | channel::LATITUDE
            | channel::LONGITUDE
            | channel::LATITUDE2
            | channel::LONGITUDE2
            // text, shape shouldn't be a part of line/trail/area.
            | TEXT
            | channel::SHAPE
            | channel::ANGLE
            // tooltip fields should not be added to group by.
            | TOOLTIP => {}

            ORDER => {
                // order should not group line / trail, but should group area
                // for stacking.
                if mark_str != "line" && mark_str != "trail" {
                    path_grouping_detail_fields(&mut details, channel_def);
                }
            }

            DETAIL | KEY => {
                path_grouping_detail_fields(&mut details, channel_def);
            }

            SIZE => {
                // For trail, size should not group trail lines. For line, size
                // should group lines.
                if mark_str != "trail" {
                    if let Some(field_def) = get_field_def(channel_def) {
                        if !truthy(&field_def["aggregate"]) {
                            details.push(vg_field(&field_def));
                        }
                    }
                }
            }

            // TODO: strokeDashOffset
            COLOR | FILL | STROKE | channel::OPACITY | channel::FILLOPACITY
            | channel::STROKEOPACITY | channel::STROKEDASH | channel::STROKEWIDTH => {
                if let Some(field_def) = get_field_def(channel_def) {
                    if !truthy(&field_def["aggregate"]) {
                        details.push(vg_field(&field_def));
                    }
                }
            }

            _ => {}
        }
    }

    details
}

// Re-exported helpers that `encoding.ts` imports from `util.ts`/`channel.ts`
// and that are commonly used alongside this module.
pub use crate::channel::{
    get_secondary_range_channel, is_channel, is_secondary_range_channel, support_mark,
};
pub use crate::mark::{is_mark_def, is_path_mark};
pub use crate::util::{every, some};

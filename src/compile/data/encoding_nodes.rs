//! Encoding-derived dataflow nodes (`makeFromEncoding` in `data/{bin,aggregate,timeunit,stack,calculate}.ts`).
//!
//! These constructors live in their respective upstream modules; they are
//! gathered here to keep the dataflow node modules focused on their nodes.

use indexmap::IndexSet;
use serde_json::{json, Map, Value};

use crate::channeldef::{
    get_field_def, get_field_or_datum_def, has_band_end, is_field_def, is_typed_field_def,
    vg_field_with, FieldRefOption,
};
use crate::channel::{get_secondary_range_channel, is_scale_channel};
use crate::mark::is_rect_based_mark;

use super::aggregate::{AggregateNode, Measure, Measures};
use super::bin::{BinComponent, BinNode, Bins};
use super::calculate::CalculateNode;
use super::dataflow::NodeRef;
use super::support::{
    bin_requires_range, field_filter_expression, get_aggregate_op, get_aggregate_param,
    is_argmax_def, is_argmin_def, is_binned_time_unit, is_binning, normalize_bin,
    normalize_time_unit, OFFSETTED_RECT_END_SUFFIX, OFFSETTED_RECT_START_SUFFIX,
};
use super::timeunit::{TimeUnitComponent, TimeUnitNode, TimeUnits};
use crate::compile::mark::encode::format::bin_format_expression;
use crate::compile::model::Model;

fn vg_field_for_as(def: &Value) -> String {
    vg_field_with(
        def,
        &FieldRefOption {
            for_as: true,
            ..Default::default()
        },
    )
}

fn vg_field_suffix(def: &Value, suffix: &str) -> String {
    vg_field_with(
        def,
        &FieldRefOption {
            bin_suffix: Some(suffix.to_string()),
            ..Default::default()
        },
    )
}

fn vg_field_as_suffix(def: &Value, suffix: &str) -> String {
    vg_field_with(
        def,
        &FieldRefOption {
            bin_suffix: Some(suffix.to_string()),
            for_as: true,
            ..Default::default()
        },
    )
}

/// Flattens an encoding mapping into `(channel, channelDef)` pairs, expanding
/// array-valued channels in place (mirrors `forEach` from `encoding.ts`).
fn channel_entries(encoding: &Value) -> Vec<(String, Value)> {
    let mut entries = Vec::new();
    if let Some(map) = encoding.as_object() {
        for (channel, value) in map {
            if let Some(items) = value.as_array() {
                for item in items {
                    entries.push((channel.clone(), item.clone()));
                }
            } else {
                entries.push((channel.clone(), value.clone()));
            }
        }
    }
    entries
}

/// Iterate over `encoding` entries that are field definitions.
fn for_each_field_def(model: &Model, mut f: impl FnMut(&Value, &str)) {
    let Some(encoding) = model.encoding().and_then(Value::as_object) else {
        return;
    };
    for (channel, value) in encoding {
        if let Some(items) = value.as_array() {
            for item in items {
                if let Some(field_def) = crate::channeldef::get_field_def_ref(item) {
                    f(field_def, channel);
                }
            }
        } else if let Some(field_def) = crate::channeldef::get_field_def_ref(value) {
            f(field_def, channel);
        }
    }
}

// ---------------------------------------------------------------------------
// Aggregates
// ---------------------------------------------------------------------------

fn add_dimension(dims: &mut IndexSet<String>, channel: &str, field_def: &Value, model: &Model) {
    let channel_def2 = model
        .encoding()
        .and_then(|e| get_secondary_range_channel(channel).and_then(|c| e.get(c).cloned()));
    let mark_def = model.mark_def().cloned().unwrap_or(Value::Null);
    let config = &model.config;

    if is_typed_field_def(field_def)
        && has_band_end(field_def, channel_def2.as_ref(), &mark_def, config)
    {
        dims.insert(crate::channeldef::vg_field(field_def));
        dims.insert(vg_field_suffix(field_def, "end"));

        let mark = model.mark_type();
        let band_position =
            get_band_position_value(field_def, channel_def2.as_ref(), &mark_def, config);

        if is_rect_based_mark(mark.as_str())
            && band_position.map(|p| p != 0.5).unwrap_or(false)
            && (channel == "x" || channel == "y")
        {
            dims.insert(vg_field_with(
                    field_def,
                    &FieldRefOption {
                        suffix: Some(OFFSETTED_RECT_START_SUFFIX.to_string()),
                        ..Default::default()
                    }
                ).to_string());
            dims.insert(vg_field_with(
                field_def,
                &FieldRefOption {
                    suffix: Some(OFFSETTED_RECT_END_SUFFIX.to_string()),
                    ..Default::default()
                },
            ));
        }

        if field_def.get("bin").map(is_binning).unwrap_or(false)
            && bin_requires_range(field_def, channel)
        {
            dims.insert(vg_field_suffix(field_def, "range"));
        }
    } else {
        dims.insert(crate::channeldef::vg_field(field_def));
    }

    if let Some(range) = field_def
        .get("scale")
        .and_then(|s| s.get("range"))
        .and_then(|r| r.get("field"))
        .and_then(Value::as_str)
    {
        dims.insert(range.to_string());
    }
}

fn get_band_position_value(
    field_def: &Value,
    field_def2: Option<&Value>,
    mark_def: &Value,
    config: &Value,
) -> Option<f64> {
    crate::channeldef::get_band_position_pub(field_def, field_def2, mark_def, config)
        .and_then(|v| v.as_f64())
}

/// Port of `AggregateNode.makeFromEncoding`.
pub fn make_aggregate_from_encoding(parent: &NodeRef, model: &Model) -> Option<NodeRef> {
    let mut is_aggregate = false;
    for_each_field_def(model, |field_def, _| {
        if field_def.get("aggregate").map(|a| !a.is_null()).unwrap_or(false) {
            is_aggregate = true;
        }
    });
    if !is_aggregate {
        return None;
    }

    let mut measures = Measures::new();
    let mut dims: IndexSet<String> = IndexSet::new();

    let encoding = model.encoding().cloned().unwrap_or(Value::Null);
    let entries = channel_entries(&encoding);


    for (channel, channel_def) in &entries {
        let field_def = match get_field_def(channel_def) {
            Some(f) => f,
            None => { if std::env::var("DBG_AGG").is_ok() { eprintln!("SKIP {channel} {channel_def}"); } continue },
        };
        if std::env::var("DBG_AGG").is_ok() { eprintln!("HAVE {channel} {field_def}"); }
        let aggregate = field_def.get("aggregate").cloned().unwrap_or(Value::Null);
        if !aggregate.is_null() {
            if aggregate.as_str() == Some("count") {
                measures
                    .entry("*".to_string())
                    .or_default()
                    .insert(
                        "count".to_string(),
                        Measure {
                            aliases: IndexSet::from([vg_field_for_as(&field_def)]),
                            aggregate_param: None,
                        },
                    );
            } else if is_argmin_def(&aggregate) || is_argmax_def(&aggregate) {
                let op = if is_argmin_def(&aggregate) {
                    "argmin"
                } else {
                    "argmax"
                };
                let arg_field = aggregate
                    .get(op)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                measures.entry(arg_field.clone()).or_default().insert(
                    op.to_string(),
                    Measure {
                        aliases: IndexSet::from([vg_field_with(
                            &json!({"op": op, "field": arg_field}),
                            &FieldRefOption {
                                for_as: true,
                                ..Default::default()
                            },
                        )]),
                        aggregate_param: None,
                    },
                );
            } else {
                let op_name = get_aggregate_op(&aggregate);
                let field = field_def
                    .get("field")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let entry = measures
                    .entry(field.clone())
                    .or_default()
                    .entry(op_name)
                    .or_insert_with(|| Measure {
                        aliases: IndexSet::new(),
                        aggregate_param: get_aggregate_param(&aggregate),
                    });
                entry.aliases.insert(vg_field_for_as(&field_def));
                if entry.aggregate_param.is_none() {
                    entry.aggregate_param = get_aggregate_param(&aggregate);
                }
            }

            if is_scale_channel(channel)
                && model.scale_domain(channel).as_str() == Some("unaggregated")
            {
                let field = field_def
                    .get("field")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let entry = measures.entry(field.clone()).or_default();
                entry.insert(
                    "min".to_string(),
                    Measure {
                        aliases: IndexSet::from([vg_field_with(
                            &json!({"field": field, "aggregate": "min"}),
                            &FieldRefOption {
                                for_as: true,
                                ..Default::default()
                            },
                        )]),
                        aggregate_param: None,
                    },
                );
                entry.insert(
                    "max".to_string(),
                    Measure {
                        aliases: IndexSet::from([vg_field_with(
                            &json!({"field": field, "aggregate": "max"}),
                            &FieldRefOption {
                                for_as: true,
                                ..Default::default()
                            },
                        )]),
                        aggregate_param: None,
                    },
                );
            }
        } else {
            add_dimension(&mut dims, channel, &field_def, model);
        }
    }

    if std::env::var("DBG_AGG").is_ok() {
        eprintln!("AGG dims={:?} entries={:?}", dims, entries.iter().map(|(c,_)| c.clone()).collect::<Vec<_>>());
    }
    if dims.len() + measures.len() == 0 {
        return None;
    }

    Some(AggregateNode::new(Some(parent), dims, measures))
}

// ---------------------------------------------------------------------------
// Binning
// ---------------------------------------------------------------------------

/// Port of `BinNode.makeFromEncoding`.
pub fn make_bin_from_encoding(parent: &NodeRef, model: &Model) -> Option<NodeRef> {
    let mut bins = Bins::new();

    for_each_field_def(model, |field_def, channel| {
        if !is_typed_field_def(field_def) {
            return;
        }
        let bin = field_def.get("bin").cloned().unwrap_or(Value::Null);
        if !is_binning(&bin) {
            return;
        }

        let as_ = [
            vg_field_for_as(field_def),
            vg_field_as_suffix(field_def, "end"),
        ];
        let normalized = normalize_bin(&bin);
        let field = field_def
            .get("field")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let key = super::support::bin_key(&normalized, &field);
        let signal = model.get_name(&format!("{key}_bins"));
        let extent_signal = model.get_name(&format!("{key}_extent"));

        let mut component = BinComponent {
            bin: normalized,
            field: field.clone(),
            extent_signal: Some(extent_signal),
            signal: Some(signal),
            span: None,
            as_: vec![as_],
            formula: None,
            formula_as: None,
        };

        if let Some(extent) = component
            .bin
            .get("extent")
            .filter(|e| super::support::is_parameter_extent(e))
        {
            let param = extent.get("param").and_then(Value::as_str).unwrap_or_default();
            component.span = Some(crate::compile::selection::parse::parse_selection_extent(
                model, param, extent,
            ));
            component.bin.remove("extent");
        }

        // rangeFormula
        if bin_requires_range(field_def, channel) {
            let guide = match model.axis(channel) {
                Some(axis) if !axis.is_null() => axis,
                _ => model
                    .unit
                    .specified_legends
                    .get(channel)
                    .cloned()
                    .unwrap_or(Value::Null),
            };
            let start_field = vg_field_with(
                field_def,
                &FieldRefOption {
                    expr: Some("datum".to_string()),
                    ..Default::default()
                },
            );
            let end_field = vg_field_with(
                field_def,
                &FieldRefOption {
                    expr: Some("datum".to_string()),
                    bin_suffix: Some("end".to_string()),
                    ..Default::default()
                },
            );
            component.formula_as = Some(vg_field_as_suffix(field_def, "range"));
            component.formula = Some(bin_format_expression(
                &start_field,
                &end_field,
                guide.get("format"),
                guide.get("formatType").and_then(Value::as_str),
                &model.config,
            ));
        }

        if let Some(existing) = bins.get_mut(&key) {
            let new = component;
            component = BinComponent {
                bin: new.bin,
                field: new.field,
                extent_signal: new.extent_signal,
                signal: new.signal,
                span: new.span,
                as_: new.as_,
                formula: new.formula.or(existing.formula.clone()),
                formula_as: new.formula_as.or(existing.formula_as.clone()),
            };
        }
        bins.insert(key, component);
    });

    if bins.is_empty() {
        return None;
    }
    Some(BinNode::new(Some(parent), bins))
}

// ---------------------------------------------------------------------------
// Time units
// ---------------------------------------------------------------------------

/// Port of `TimeUnitNode.makeFromEncoding`.
pub fn make_timeunit_from_encoding(parent: &NodeRef, model: &Model) -> Option<NodeRef> {
    let mut time_units = TimeUnits::new();
    let mark = model.mark_type();
    let null_value = Value::Null;
    let mark_def = model.mark_def().unwrap_or(&null_value);
    let config = &model.config;

    for_each_field_def(model, |field_def, channel| {
        let time_unit = field_def.get("timeUnit").cloned().unwrap_or(Value::Null);
        if time_unit.is_null() {
            return;
        }
        let field = field_def
            .get("field")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        let mut component: Option<TimeUnitComponent> = None;
        if is_binned_time_unit(&time_unit) {
            let band_position = get_band_position_value(field_def, None, mark_def, config);
            if is_rect_based_mark(mark.as_str())
                || band_position
                    .map(|v| v != 0.0 && !v.is_nan())
                    .unwrap_or(false)
            {
                component = Some(TimeUnitComponent {
                    as_: None,
                    field: field.clone(),
                    time_unit: normalize_time_unit(&time_unit).unwrap_or(Value::Null),
                    rect_band_position: None,
                });
            }
        } else {
            component = Some(TimeUnitComponent {
                as_: Some(vg_field_for_as(field_def)),
                field: field.clone(),
                time_unit: time_unit.clone(),
                rect_band_position: None,
            });
        }

        let band_position = get_band_position_value(field_def, None, mark_def, config);
        if is_rect_based_mark(mark.as_str())
            && (channel == "x" || channel == "y")
            && band_position.map(|p| p != 0.5).unwrap_or(false)
        {
            if let Some(component) = &mut component {
                component.rect_band_position = band_position;
            }
        }

        if let Some(component) = component {
            let key = crate::util::stringify(&crate::util::hash(&json!({
                "as": component.as_,
                "field": component.field,
                "timeUnit": component.time_unit,
            })));
            time_units.insert(key, component);
        }
    });

    if time_units.is_empty() {
        return None;
    }
    Some(TimeUnitNode::new(Some(parent), time_units))
}

// ---------------------------------------------------------------------------
// Sort-array index calculations
// ---------------------------------------------------------------------------

/// Port of `CalculateNode.parseAllForSortIndex`.
pub fn parse_all_for_sort_index(mut parent: NodeRef, model: &Model) -> NodeRef {
    let encoding = model.encoding().cloned().unwrap_or(Value::Null);
    let entries = channel_entries(&encoding);


    for (channel, channel_def) in &entries {
        let field_def = match get_field_def(channel_def) {
            Some(f) => f,
            None => continue,
        };
        let sort = field_def.get("sort").cloned().unwrap_or(Value::Null);
        if !sort.is_array() {
            continue;
        }
        let sort = sort.as_array().cloned().unwrap_or_default();
        let field = field_def.get("field").cloned().unwrap_or(Value::Null);
        let time_unit = field_def.get("timeUnit").cloned().unwrap_or(Value::Null);

        let mut calculate = String::new();
        for (i, sort_value) in sort.iter().enumerate() {
            let predicate = field_filter_expression(&json!({
                "field": field,
                "timeUnit": time_unit,
                "equal": sort_value,
            }));
            calculate.push_str(&format!("{predicate} ? {i} : "));
        }
        calculate.push_str(&sort.len().to_string());

        parent = CalculateNode::new(
            Some(&parent),
            &json!({
                "calculate": calculate,
                "as": crate::channeldef::vg_field_with(
                    &field_def,
                    &crate::channeldef::FieldRefOption {
                        prefix: Some(channel.clone()),
                        suffix: Some("sort_index".to_string()),
                        for_as: true,
                        ..Default::default()
                    },
                ),
            }),
        );
    }
    parent
}

// ---------------------------------------------------------------------------
// Stack
// ---------------------------------------------------------------------------

/// Port of `StackNode.makeFromEncoding`.
pub fn make_stack_from_encoding(parent: &NodeRef, model: &Model) -> Option<NodeRef> {
    let props = crate::stack::stack(&model.unit.mark_def, &model.unit.encoding)?;
    let encoding = model.encoding().cloned().unwrap_or(Value::Null);

    let dimension_field_defs: Vec<Value> = props
        .groupby_channels
        .iter()
        .filter_map(|channel| get_field_def(&encoding[channel]))
        .collect();

    let stackby: Vec<String> = props
        .stack_by
        .iter()
        .map(|by| crate::channeldef::vg_field(&by.field_def))
        .filter(|f| !f.is_empty())
        .collect();

    let order_def = encoding.get("order").cloned().unwrap_or(Value::Null);
    let mut sort = json!({"field": [], "order": []});

    if order_def.is_array() || is_field_def(&order_def) {
        let defs = if order_def.is_array() {
            order_def.as_array().cloned().unwrap_or_default()
        } else {
            vec![order_def.clone()]
        };
        let mut fields = Vec::new();
        let mut orders = Vec::new();
        for def in defs {
            fields.push(Value::String(crate::channeldef::vg_field(&def)));
            orders.push(def.get("sort").cloned().unwrap_or(json!("ascending")));
        }
        sort = json!({"field": fields, "order": orders});
    } else {
        let sort_order = if order_def.is_object() && order_def.get("field").is_none() {
            order_def.get("sort").cloned().unwrap_or(json!("ascending"))
        } else if props.field_channel == "y" {
            json!("descending")
        } else {
            json!("ascending")
        };
        let mut fields: Vec<Value> = Vec::new();
        let mut orders: Vec<Value> = Vec::new();
        for field in &stackby {
            if !fields.iter().any(|f| f.as_str() == Some(field.as_str())) {
                fields.push(Value::String(field.clone()));
                orders.push(sort_order.clone());
            }
        }
        sort = json!({"field": fields, "order": orders});
    }

    let field_channel = props.field_channel.clone();
    let stack_field = model.vg_field(&field_channel);

    let _ = model.unit.stack.as_object();
    let component = super::stack::StackComponent {
        facetby: Vec::new(),
        dimension_field_defs,
        stack_field,
        stackby: Some(stackby),
        sort,
        offset: props.offset.as_str().to_string(),
        impute: Some(props.impute),
        groupby: None,
        as_: [
            vg_field_with(
                &get_field_or_datum_def(&encoding[&field_channel]).unwrap_or(Value::Null),
                &FieldRefOption {
                    suffix: Some("start".to_string()),
                    for_as: true,
                    ..Default::default()
                },
            ),
            vg_field_with(
                &get_field_or_datum_def(&encoding[&field_channel]).unwrap_or(Value::Null),
                &FieldRefOption {
                    suffix: Some("end".to_string()),
                    for_as: true,
                    ..Default::default()
                },
            ),
        ],
    };

    Some(super::stack::StackNode::new(Some(parent), component))
}

#[allow(dead_code)]
fn _unused(_: &Map<String, Value>) {}

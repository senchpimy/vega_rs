//! Port of `src/compile/scale/domain.ts`.

use serde_json::{json, Map, Value};

use crate::aggregate::SHARED_DOMAIN_OPS;
use crate::channel::{get_secondary_range_channel, is_scale_channel, is_x_or_y};
use crate::channeldef::{
    bin_requires_range, get_band_position_pub, get_field_or_datum_def, has_band_end, is_datum_def,
    is_field_def, value_expr, vg_field, vg_field_with, FieldRefOption, ValueExprOpt,
};
use crate::log;
use crate::scale::{has_discrete_domain, is_domain_union_with, is_parameter_domain, ScaleType};
use crate::util::{has_property, stringify};

use super::super::model::{is_unit_model, Model};
use super::super::split::{
    make_explicit, make_implicit, merge_values_with_explicit, Explicit,
};
use super::super::support;

pub const FACET_SCALE_PREFIX: &str = "scale_";

/// Port of `parseScaleDomain`.
pub fn parse_scale_domain(model: &mut Model) {
    if is_unit_model(model) {
        parse_unit_scale_domain(model);
    } else {
        parse_non_unit_scale_domain(model);
    }
}

fn parse_unit_scale_domain(model: &mut Model) {
    let channels: Vec<String> = model.component.scales.keys().cloned().collect();

    for channel in channels {
        let domains = parse_domain_for_channel(model, &channel);
        if let Some(cmpt) = model.component.scales.get_mut(&channel) {
            cmpt.set_with_explicit("domains", domains.clone());
        }
        parse_selection_domain(model, &channel);

        if model.component.data.is_faceted {
            let resolve = model.facet_scale_resolve.get(&channel).cloned().unwrap_or_else(|| {
                support::default_scale_resolve(&channel, super::super::model::SpecType::Facet)
                    .to_string()
            });
            if resolve == "shared" {
                let mut values = domains.value.clone().unwrap_or(Value::Array(Vec::new()));
                if let Some(arr) = values.as_array_mut() {
                    for domain in arr {
                        if is_data_ref_domain(domain) {
                            let data = domain
                                .get("data")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string();
                            let data = format!(
                                "{FACET_SCALE_PREFIX}{}",
                                data.trim_start_matches(FACET_SCALE_PREFIX)
                            );
                            domain["data"] = Value::String(data);
                        }
                    }
                }
                if let Some(cmpt) = model.component.scales.get_mut(&channel) {
                    cmpt.set("domains", values, domains.explicit);
                }
            }
        }
    }
}

fn parse_non_unit_scale_domain(model: &mut Model) {
    for child in &mut model.children {
        parse_scale_domain(child);
    }

    let channels: Vec<String> = model.component.scales.keys().cloned().collect();
    for channel in channels {
        let mut domains: Option<Explicit> = None;
        let mut selection_extent: Option<Value> = None;

        for child in &model.children {
            if let Some(cmpt) = child.component.scales.get(&channel) {
                let child_domains = cmpt.get_with_explicit("domains");
                domains = Some(match domains {
                    None => child_domains,
                    Some(v1) => merge_values_with_explicit(
                        Some(v1),
                        child_domains,
                        "domains",
                        "scale",
                        domains_tie_breaker,
                    ),
                });

                let se = cmpt.get("selectionExtent");
                if let (Some(existing), Some(new)) = (&selection_extent, &se) {
                    if existing.get("param") != new.get("param") {
                        log::warn(support::NEEDS_SAME_SELECTION);
                    }
                }
                selection_extent = se;
            }
        }

        if let Some(cmpt) = model.component.scales.get_mut(&channel) {
            if let Some(d) = domains {
                cmpt.set_with_explicit("domains", d);
            }
            if let Some(se) = selection_extent {
                cmpt.set("selectionExtent", se, true);
            }
        }
    }
}

fn domains_tie_breaker(
    v1: &Explicit,
    v2: &Explicit,
    property: &str,
    property_of: &str,
) -> Explicit {
    if v1.explicit && v2.explicit {
        log::warn(support::merge_conflicting_domain_property(
            property,
            property_of,
            v1.value.as_ref().unwrap_or(&Value::Null),
            v2.value.as_ref().unwrap_or(&Value::Null),
        ));
    }
    let mut values = Vec::new();
    if let Some(Value::Array(a)) = &v1.value {
        values.extend(a.iter().cloned());
    }
    if let Some(Value::Array(b)) = &v2.value {
        values.extend(b.iter().cloned());
    }
    Explicit {
        explicit: v1.explicit,
        value: Some(Value::Array(values)),
    }
}

fn normalize_unaggregated_domain(
    domain: Value,
    field_def: &Value,
    scale_type: Option<ScaleType>,
    scale_config: &Value,
) -> Value {
    if domain.as_str() == Some("unaggregated") {
        let result = can_use_unaggregated_domain(field_def, scale_type);
        if let Some(reason) = result.1 {
            log::warn(reason);
            return Value::Null;
        }
    } else if domain.is_null()
        && scale_config
            .get("useUnaggregatedDomain")
            .and_then(Value::as_bool)
            == Some(true)
        && can_use_unaggregated_domain(field_def, scale_type).0 {
            return Value::String("unaggregated".to_string());
        }
    domain
}

fn can_use_unaggregated_domain(
    field_def: &Value,
    scale_type: Option<ScaleType>,
) -> (bool, Option<String>) {
    let aggregate = field_def.get("aggregate").cloned().unwrap_or(Value::Null);
    let field_type = field_def.get("type").and_then(Value::as_str).unwrap_or("");

    if !crate::channeldef_support::truthy(&aggregate) {
        return (
            false,
            Some(support::unaggregate_domain_has_no_effect_for_raw_field(field_def)),
        );
    }

    if let Some(aggregate) = aggregate.as_str() {
        if !SHARED_DOMAIN_OPS.contains(&aggregate) {
            return (
                false,
                Some(
                    support::unaggregate_domain_with_non_shared_domain_op(aggregate),
                ),
            );
        }
    }

    if field_type == "quantitative" && scale_type == Some(ScaleType::Log) {
        return (
            false,
            Some(support::unaggregated_domain_with_log_scale(field_def)),
        );
    }

    (true, None)
}

/// Port of `parseDomainForChannel`.
pub fn parse_domain_for_channel(model: &mut Model, channel: &str) -> Explicit {
    let scale_type = model
        .get_scale_component_ref(channel)
        .and_then(|c| c.get_type());

    let (has_x2, has_x, has_y2, has_y) = {
        let null_value = Value::Null;
        let encoding = model.encoding().unwrap_or(&null_value);
        (
            get_field_or_datum_def(&encoding["x2"]).is_some(),
            get_field_or_datum_def(&encoding["x"]).is_some(),
            get_field_or_datum_def(&encoding["y2"]).is_some(),
            get_field_or_datum_def(&encoding["y"]).is_some(),
        )
    };

    let original_domain = model.scale_domain(channel);
    let domain = {
        let null_value = Value::Null;
        let config_scale = model.config.get("scale").unwrap_or(&null_value);
        normalize_unaggregated_domain(
            original_domain.clone(),
            model.typed_field_def_ref(channel).unwrap_or(&null_value),
            scale_type,
            config_scale,
        )
    };
    if domain != original_domain {
        if let Some(scale) = model.unit.specified_scales.get_mut(channel) {
            if let Some(map) = scale.as_object_mut() {
                map.insert("domain".to_string(), domain.clone());
            }
        }
    }

    if channel == "x" && has_x2 {
        if has_x {
            return merge_values_with_explicit(
                Some(parse_single_channel_domain(
                    model, scale_type, &domain, "x",
                )),
                parse_single_channel_domain(model, scale_type, &domain, "x2"),
                "domain",
                "scale",
                domains_tie_breaker,
            );
        } else {
            return parse_single_channel_domain(model, scale_type, &domain, "x2");
        }
    } else if channel == "y" && has_y2 {
        if has_y {
            return merge_values_with_explicit(
                Some(parse_single_channel_domain(
                    model, scale_type, &domain, "y",
                )),
                parse_single_channel_domain(model, scale_type, &domain, "y2"),
                "domain",
                "scale",
                domains_tie_breaker,
            );
        } else {
            return parse_single_channel_domain(model, scale_type, &domain, "y2");
        }
    }

    parse_single_channel_domain(model, scale_type, &domain, channel)
}

fn map_domain_to_signals(domain: &[Value], type_: &str, time_unit: Option<&str>) -> Vec<Value> {
    domain
        .iter()
        .map(|v| {
            let opt = ValueExprOpt {
                time_unit: time_unit.map(|u| Value::String(u.to_string())),
                type_: Some(type_.to_string()),
                ..Default::default()
            };
            let expr = value_expr(v, &opt).unwrap_or_else(|| stringify(v));
            json!({"signal": expr})
        })
        .collect()
}

fn convert_domain_if_it_is_date_time(
    domain: &[Value],
    type_: &str,
    time_unit: &Value,
) -> Vec<Value> {
    let normalized = crate::channeldef_support::normalize_time_unit(time_unit);
    let unit = normalized.get("unit").and_then(Value::as_str);
    if type_ == "temporal" || unit.is_some() {
        vec![Value::Array(map_domain_to_signals(domain, type_, unit))]
    } else {
        vec![Value::Array(domain.to_vec())]
    }
}

fn parse_single_channel_domain(
    model: &mut Model,
    scale_type: Option<ScaleType>,
    domain: &Value,
    channel: &str,
) -> Explicit {
    let null_value = Value::Null;
    let (mark, _invalid, scale_data_source, field_or_datum_def, type_, time_unit, stack) = {
        let encoding = model.encoding().unwrap_or(&null_value);
        let mark_def = model.mark_def().unwrap_or(&null_value);
        let config = &model.config;
        let mark = model.mark_type();

        let invalid = crate::mark::get_mark_prop_or_config(
            "invalid",
            mark_def,
            config,
            None,
            false,
        );
        let scale_data_source =
            super::super::data::filterinvalid::get_scale_data_source_for_handling_invalid_values(
                invalid.as_ref(),
                crate::mark::is_path_mark(mark.as_str()),
            );

        let field_or_datum_def = get_field_or_datum_def(&encoding[channel]).unwrap_or(Value::Null);
        let type_ = field_or_datum_def
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let time_unit = field_or_datum_def
            .get("timeUnit")
            .cloned()
            .unwrap_or(Value::Null);
        (
            mark,
            invalid,
            scale_data_source,
            field_or_datum_def,
            type_,
            time_unit,
            model.unit.stack.clone(),
        )
    };
    let type_ = type_.as_str();

    if is_domain_union_with(domain) {
        let default_domain = parse_single_channel_domain(model, scale_type, &Value::Null, channel);
        let union_with = domain
            .get("unionWith")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut unioned = convert_domain_if_it_is_date_time(&union_with, type_, &time_unit);
        if let Some(Value::Array(default)) = default_domain.value {
            unioned.extend(default);
        }
        return make_explicit(Value::Array(unioned));
    } else if crate::channeldef_support::is_signal_ref(domain) {
        return make_explicit(Value::Array(vec![domain.clone()]));
    } else if !domain.is_null()
        && domain.as_str() != Some("unaggregated")
        && !is_parameter_domain(domain)
    {
        if let Some(arr) = domain.as_array() {
            return make_explicit(Value::Array(convert_domain_if_it_is_date_time(
                arr, type_, &time_unit,
            )));
        }
    }

    if let Some(stack) = stack.as_object() {
        if stack.get("fieldChannel").and_then(Value::as_str) == Some(channel) {
            if stack.get("offset").and_then(Value::as_str) == Some("normalize") {
                return make_implicit(Value::Array(vec![json!([0, 1])]));
            }
            let data = model.request_data_name(scale_data_source);
            let start = model.vg_field_with(
                channel,
                &FieldRefOption {
                    suffix: Some("start".to_string()),
                    ..Default::default()
                },
            );
            let end = model.vg_field_with(
                channel,
                &FieldRefOption {
                    suffix: Some("end".to_string()),
                    ..Default::default()
                },
            );
            return make_implicit(Value::Array(vec![
                json!({"data": data, "field": start}),
                json!({"data": data, "field": end}),
            ]));
        }
    }

    let sort: Value = if is_scale_channel(channel) && is_field_def(&field_or_datum_def) {
        domain_sort(model, channel, scale_type)
    } else {
        Value::Null
    };

    if is_datum_def(&field_or_datum_def) {
        let datum = field_or_datum_def.get("datum").cloned().unwrap_or(Value::Null);
        return make_implicit(Value::Array(convert_domain_if_it_is_date_time(
            &[datum],
            type_,
            &time_unit,
        )));
    }

    let field_def = &field_or_datum_def;
    if domain.as_str() == Some("unaggregated") {
        let field = field_def.get("field").cloned().unwrap_or(Value::Null);
        let data = model.request_data_name(scale_data_source);
        return make_implicit(Value::Array(vec![
            json!({"data": data, "field": vg_field(&json!({"field": field, "aggregate": "min"}))}),
            json!({"data": data, "field": vg_field(&json!({"field": field, "aggregate": "max"}))}),
        ]));
    } else if crate::channeldef_support::is_binning(&field_def["bin"]) {
        let bin = field_def.get("bin").cloned().unwrap_or(Value::Null);
        if let Some(st) = scale_type {
            if has_discrete_domain(st) {
                if st == ScaleType::BinOrdinal {
                    return make_implicit(Value::Array(Vec::new()));
                }
                let data = if sort.is_boolean() {
                    model.request_data_name(scale_data_source)
                } else {
                    model.request_data_name("raw")
                };
                let bin_suffix = if bin_requires_range(field_def, channel) {
                    Some("range".to_string())
                } else {
                    None
                };
                let field = model.vg_field_with(
                    channel,
                    &FieldRefOption {
                        bin_suffix,
                        ..Default::default()
                    },
                );
                let sort_value = if sort.as_bool() == Some(true) || !sort.is_object() {
                    json!({"field": model.vg_field(channel), "op": "min"})
                } else {
                    sort.clone()
                };
                return make_implicit(Value::Array(vec![json!({
                    "data": data,
                    "field": field,
                    "sort": sort_value
                })]));
            } else {
                let bin_signal = bin_signal_name(model, &field_def["field"], &bin);
                let signal = model.get_signal_name(&bin_signal);
                return make_implicit(Value::Array(vec![json!({
                    "signal": format!("[{signal}.start, {signal}.stop]")
                })]));
            }
        }
    } else if field_def.get("timeUnit").map(|t| !t.is_null()).unwrap_or(false) {
        let st = scale_type.map(|t| t.as_str()).unwrap_or("");
        if ["time", "utc"].contains(&st) {
            let (has_band, band_position) = {
                let encoding = model.encoding().unwrap_or(&null_value);
                let mark_def = model.mark_def().unwrap_or(&null_value);
                let config = &model.config;
                let field_def2 = get_secondary_range_channel(channel)
                    .and_then(|c| get_field_or_datum_def(&encoding[c]));
                (
                    has_band_end(field_def, field_def2.as_ref(), mark_def, config),
                    get_band_position_pub(field_def, field_def2.as_ref(), mark_def, config),
                )
            };
            if has_band {
                let data = model.request_data_name(scale_data_source);
                let is_rect_with_offset = crate::mark::is_rect_based_mark(mark.as_str())
                    && band_position
                        .as_ref()
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.5)
                        != 0.5
                    && is_x_or_y(channel);
                let start = model.vg_field_with(
                    channel,
                    &FieldRefOption {
                        suffix: if is_rect_with_offset {
                            Some(crate::compile::data::OFFSETTED_RECT_START_SUFFIX.to_string())
                        } else {
                            None
                        },
                        ..Default::default()
                    },
                );
                let end = model.vg_field_with(
                    channel,
                    &FieldRefOption {
                        suffix: Some(if is_rect_with_offset {
                            crate::compile::data::OFFSETTED_RECT_END_SUFFIX.to_string()
                        } else {
                            "end".to_string()
                        }),
                        ..Default::default()
                    },
                );
                return make_implicit(Value::Array(vec![
                    json!({"data": data, "field": start}),
                    json!({"data": data, "field": end}),
                ]));
            }
        }
    }

    if !sort.is_null() {
        let data = if sort.is_boolean() {
            model.request_data_name(scale_data_source)
        } else {
            model.request_data_name("raw")
        };
        return make_implicit(Value::Array(vec![json!({
            "data": data,
            "field": model.vg_field(channel),
            "sort": sort
        })]));
    }

    make_implicit(Value::Array(vec![json!({
        "data": model.request_data_name(scale_data_source),
        "field": model.vg_field(channel)
    })]))
}

fn normalize_sort_field(sort: &Value, is_stacked_measure: bool) -> Value {
    let mut out = Map::new();
    let op = sort
        .get("op")
        .filter(|v| !v.is_null())
        .cloned()
        .unwrap_or_else(|| Value::String(if is_stacked_measure { "sum" } else { "min" }.to_string()));
    out.insert("op".to_string(), op);
    if let Some(field) = sort.get("field") {
        out.insert(
            "field".to_string(),
            Value::String(crate::util::replace_path_in_field(
                field.as_str().unwrap_or_default(),
            )),
        );
    }
    if let Some(order) = sort.get("order").filter(|v| crate::channeldef_support::truthy(v)) {
        out.insert("order".to_string(), order.clone());
    }
    Value::Object(out)
}

fn parse_selection_domain(model: &mut Model, channel: &str) {
    let spec_domain = model
        .unit
        .specified_scales
        .get(channel)
        .and_then(|s| s.get("domain"))
        .cloned()
        .unwrap_or(Value::Null);
    let bin = model
        .field_def(channel)
        .and_then(|f| f.get("bin").cloned())
        .unwrap_or(Value::Null);
    let domain = if is_parameter_domain(&spec_domain) {
        Some(spec_domain.clone())
    } else {
        None
    };
    let extent = if crate::channeldef_support::is_bin_params(&bin)
        && crate::bin::is_parameter_extent(&bin["extent"])
    {
        Some(bin["extent"].clone())
    } else {
        None
    };

    if let Some(value) = domain.or(extent) {
        if let Some(cmpt) = model.component.scales.get_mut(channel) {
            cmpt.set("selectionExtent", value, true);
        }
    }
}

/// Port of `domainSort`.
pub fn domain_sort(model: &Model, channel: &str, scale_type: Option<ScaleType>) -> Value {
    let scale_type = match scale_type {
        Some(t) => t,
        None => return Value::Null,
    };
    if !has_discrete_domain(scale_type) {
        return Value::Null;
    }

    let field_def = match model.field_def(channel) {
        Some(f) => f,
        None => return Value::Null,
    };
    // `undefined` (no `sort` property) and an explicit `null` sort are not the
    // same upstream: the former defaults to ascending (true), the latter
    // produces no domain sort at all.
    let sort_defined = field_def.get("sort").is_some();
    let sort = field_def.get("sort").cloned().unwrap_or(Value::Null);

    if sort.is_array() {
        let field = vg_field_with(
            &field_def,
            &FieldRefOption {
                prefix: Some(channel.to_string()),
                suffix: Some("sort_index".to_string()),
                ..Default::default()
            },
        );
        return json!({"op": "min", "field": field, "order": "ascending"});
    }

    let stack_dimensions = model.unit.stack.as_object().map(|stack| {
        let mut set = Vec::new();
        if let Some(groupby) = stack.get("groupbyFields").and_then(Value::as_array) {
            set.extend(groupby.iter().filter_map(|v| v.as_str().map(str::to_string)));
        }
        if let Some(stack_by) = stack.get("stackBy").and_then(Value::as_array) {
            for s in stack_by {
                if let Some(field) = s.get("fieldDef").and_then(|f| f.get("field")).and_then(Value::as_str) {
                    set.push(field.to_string());
                }
            }
        }
        set
    });

    if sort.is_object() && (has_property(&sort, "field") || has_property(&sort, "op")) {
        let is_stacked_measure = stack_dimensions
            .as_ref()
            .map(|set| {
                !set.contains(
                    &sort
                        .get("field")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                )
            })
            .unwrap_or(false);
        return normalize_sort_field(&sort, is_stacked_measure);
    } else if sort.is_object() && has_property(&sort, "encoding") {
        let encoding = sort.get("encoding").and_then(Value::as_str).unwrap_or("");
        if let Some(field_def_to_sort_by) = model.field_def(encoding) {
            let aggregate = field_def_to_sort_by
                .get("aggregate")
                .cloned()
                .unwrap_or(Value::Null);
            let field = field_def_to_sort_by
                .get("field")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let is_stacked_measure = stack_dimensions
                .as_ref()
                .map(|set| !set.contains(&field))
                .unwrap_or(false);
            if crate::channeldef_support::is_argmin_def(&aggregate)
                || crate::channeldef_support::is_argmax_def(&aggregate)
            {
                return normalize_sort_field(
                    &json!({"field": vg_field(&field_def_to_sort_by), "order": sort.get("order").cloned().unwrap_or(Value::Null)}),
                    is_stacked_measure,
                );
            } else if aggregate.is_null()
                || aggregate
                    .as_str()
                    .map(|s| crate::channeldef_support::is_aggregate_op(&Value::String(s.to_string())))
                    .unwrap_or(false)
            {
                return normalize_sort_field(
                    &json!({"op": aggregate, "field": field, "order": sort.get("order").cloned().unwrap_or(Value::Null)}),
                    is_stacked_measure,
                );
            }
        }
    } else if sort.as_str() == Some("descending") {
        return json!({"op": "min", "field": model.vg_field(channel), "order": "descending"});
    } else if sort.as_str() == Some("ascending") || !sort_defined {
        return Value::Bool(true);
    }

    Value::Null
}

fn bin_signal_name(model: &Model, field: &Value, bin: &Value) -> String {
    let field = field.as_str().unwrap_or("");
    let channel = "";
    let normalized = normalize_bin_for_channel(bin, channel);
    let key = super::super::data::support::bin_key(&normalized, field);
    model.get_name(&format!("{key}_bins"))
}

pub fn normalize_bin_for_channel(bin: &Value, channel: &str) -> Map<String, Value> {
    match bin {
        Value::Bool(true) => {
            let mut m = Map::new();
            m.insert(
                "maxbins".to_string(),
                json!(crate::bin::auto_max_bins(if channel.is_empty() {
                    None
                } else {
                    Some(support::channel_static(channel))
                })),
            );
            m
        }
        Value::String(s) if s == "binned" => {
            let mut m = Map::new();
            m.insert("binned".to_string(), json!(true));
            m
        }
        Value::Object(o) => {
            let mut m = o.clone();
            let has_maxbins = m.get("maxbins").map(|v| !v.is_null()).unwrap_or(false);
            let has_step = m.get("step").map(|v| !v.is_null()).unwrap_or(false);
            if !has_maxbins && !has_step {
                m.insert(
                    "maxbins".to_string(),
                    json!(crate::bin::auto_max_bins(if channel.is_empty() {
                        None
                    } else {
                        Some(support::channel_static(channel))
                    })),
                );
            }
            m
        }
        _ => Map::new(),
    }
}

pub fn bin_key(bin: &Map<String, Value>, field: &str) -> String {
    super::super::data::support::bin_key(bin, field)
}

// ---------------------------------------------------------------------------
// Domain predicates / merging / assembling
// ---------------------------------------------------------------------------

pub fn is_data_ref_domain(domain: &Value) -> bool {
    domain.is_object()
        && has_property(domain, "data")
        && (has_property(domain, "field") || has_property(domain, "fields"))
}

pub fn is_vg_range_step(range: &Value) -> bool {
    range.is_object() && has_property(range, "step")
}

/// Port of `mergeDomains`.
pub fn merge_domains(domains: &[Value]) -> Value {
    let mut unique_domains: Vec<Value> = Vec::new();
    for domain in domains {
        let value = if is_data_ref_domain(domain) {
            let mut d = domain.clone();
            if let Some(map) = d.as_object_mut() {
                map.remove("sort");
            }
            d
        } else {
            domain.clone()
        };
        if !unique_domains.iter().any(|d| crate::util::deep_equal(d, &value)) {
            unique_domains.push(value);
        }
    }

    let mut sorts: Vec<Value> = Vec::new();
    for d in domains {
        if is_data_ref_domain(d) {
            if let Some(s) = d.get("sort") {
                // Upstream keeps boolean sorts (`true`/`false`); only
                // `undefined` (absent) and `null` are dropped.
                if !s.is_null() {
                    let mut s = s.clone();
                    if let Some(map) = s.as_object_mut() {
                        if map.get("op").and_then(Value::as_str) == Some("count") {
                            map.remove("field");
                        }
                        if map.get("order").and_then(Value::as_str) == Some("ascending") {
                            map.remove("order");
                        }
                    }
                    if !sorts.iter().any(|x| crate::util::deep_equal(x, &s)) {
                        sorts.push(s);
                    }
                }
            }
        }
    }

    if unique_domains.is_empty() {
        return Value::Null;
    } else if unique_domains.len() == 1 {
        let domain = &domains[0];
        if is_data_ref_domain(domain) && !sorts.is_empty() {
            let mut sort = sorts[0].clone();
            if sorts.len() > 1 {
                log::warn(support::MORE_THAN_ONE_SORT);
                let filtered: Vec<Value> = sorts
                    .iter()
                    .filter(|s| {
                        s.is_object()
                            && s.get("op").map(|o| o.as_str() != Some("min")).unwrap_or(false)
                    })
                    .cloned()
                    .collect();
                if sorts.iter().all(|s| s.is_object() && s.get("op").is_some())
                    && filtered.len() == 1
                {
                    sort = filtered[0].clone();
                } else {
                    sort = Value::Bool(true);
                }
            } else if let Some(map) = sort.as_object() {
                if let Some(sort_field) = map.get("field").and_then(Value::as_str) {
                    if domain.get("field").and_then(Value::as_str) == Some(sort_field) {
                        sort = map
                            .get("order")
                            .cloned()
                            .map(|order| json!({"order": order}))
                            .unwrap_or(Value::Bool(true));
                    }
                }
            }
            let mut out = domain.as_object().cloned().unwrap_or_default();
            out.insert("sort".to_string(), sort);
            return Value::Object(out);
        }
        return domain.clone();
    }

    let mut union_domain_sorts: Vec<Value> = Vec::new();
    for s in &sorts {
        let value = if s.is_boolean()
            || !s.is_object()
            || s.get("op")
                .and_then(Value::as_str)
                .map(|op| matches!(op, "count" | "min" | "max"))
                .unwrap_or(false)
        {
            s.clone()
        } else {
            log::warn(support::domain_sort_dropped(s));
            Value::Bool(true)
        };
        if !union_domain_sorts
            .iter()
            .any(|x| crate::util::deep_equal(x, &value))
        {
            union_domain_sorts.push(value);
        }
    }

    let mut sort = Value::Null;
    if union_domain_sorts.len() == 1 {
        sort = union_domain_sorts[0].clone();
    } else if union_domain_sorts.len() > 1 {
        log::warn(support::MORE_THAN_ONE_SORT);
        sort = Value::Bool(true);
    }

    let mut all_data: Vec<Value> = Vec::new();
    for d in domains {
        let value = if is_data_ref_domain(d) {
            d.get("data").cloned().unwrap_or(Value::Null)
        } else {
            Value::Null
        };
        if !all_data.iter().any(|x| crate::util::deep_equal(x, &value)) {
            all_data.push(value);
        }
    }

    if all_data.len() == 1 && !all_data[0].is_null() {
        let mut out = Map::new();
        out.insert("data".to_string(), all_data[0].clone());
        out.insert(
            "fields".to_string(),
            Value::Array(
                unique_domains
                    .iter()
                    .map(|d| d.get("field").cloned().unwrap_or(Value::Null))
                    .collect(),
            ),
        );
        if !sort.is_null() {
            out.insert("sort".to_string(), sort);
        }
        return Value::Object(out);
    }

    let mut out = Map::new();
    out.insert(
        "fields".to_string(),
        Value::Array(unique_domains.iter().map(union_domain_field).collect()),
    );
    if !sort.is_null() {
        out.insert("sort".to_string(), sort);
    }
    Value::Object(out)
}

fn union_domain_field(domain: &Value) -> Value {
    if let Some(arr) = domain.as_array() {
        if arr
            .iter()
            .any(crate::channeldef_support::is_signal_ref)
        {
            let entries: Vec<String> = arr
                .iter()
                .map(|v| {
                    if crate::channeldef_support::is_signal_ref(v) {
                        format!(
                            "{{data: {}}}",
                            v.get("signal").and_then(Value::as_str).unwrap_or("")
                        )
                    } else {
                        format!("{{data: {}}}", stringify(v))
                    }
                })
                .collect();
            return json!({"signal": format!("[{}]", entries.join(", "))});
        }
    }
    domain.clone()
}

/// Port of `getFieldFromDomain`.
pub fn get_field_from_domain(domain: &Value) -> Option<String> {
    if let Some(field) = domain.get("field").and_then(Value::as_str) {
        if is_data_ref_domain(domain) {
            return Some(field.to_string());
        }
    }
    if let Some(fields) = domain.get("fields").and_then(Value::as_array) {
        // `isDataRefUnionedDomain`: every entry is a data ref with a field.
        let mut found: Option<String> = None;
        let mut data_ref_union = false;
        for non_union in fields {
            if let Some(field) = non_union.get("field").and_then(Value::as_str) {
                data_ref_union = true;
                if found.is_none() {
                    found = Some(field.to_string());
                } else if found.as_deref() != Some(field) {
                    log::warn(support::FACETED_INDEPENDENT_DIFFERENT_SOURCES);
                    return found;
                }
            }
        }
        if data_ref_union {
            log::warn(support::FACETED_INDEPENDENT_SAME_FIELDS_DIFFERENT_SOURCES);
            return found;
        }
        // `isFieldRefUnionDomain`: plain field names.
        log::warn(support::FACETED_INDEPENDENT_SAME_SOURCE);
        return fields.first().and_then(|f| f.as_str()).map(str::to_string);
    }
    None
}

/// Port of `assembleDomain`.
pub fn assemble_domain(model: &Model, channel: &str) -> Value {
    let scale_component = match model.get_scale_component(channel) {
        Some(c) => c,
        None => return Value::Null,
    };
    let domains = scale_component
        .get("domains")
        .and_then(|d| d.as_array().cloned())
        .unwrap_or_default();
    let corrected: Vec<Value> = domains
        .into_iter()
        .map(|mut domain| {
            if is_data_ref_domain(&domain) {
                if let Some(data) = domain.get("data").and_then(Value::as_str) {
                    let data = model.lookup_data_source(data);
                    domain["data"] = Value::String(data);
                }
            }
            // Bin/time-unit signal names may have been unified by the dataflow
            // optimizer after scale parsing; resolve them before merging so
            // duplicate signal domains collapse like upstream.
            model.resolve_signal_names_in(&mut domain);
            domain
        })
        .collect();
    merge_domains(&corrected)
}

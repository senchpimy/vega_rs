//! Port of `src/compositemark/common.ts`.

use serde_json::{json, Map, Value};

use crate::channeldef::{title, TitleOpt};
use crate::mark::is_mark_def;
use crate::normalize_support::{
    assign, empty_object, field_defs, get, has_prop, is_continuous_field_or_datum_def,
    is_field_def, is_field_or_datum_def_for_time_format, omit, title_text, to_string_field_def,
    truthy,
};
use crate::util::{hash, unique};

/// Port of `getTitle`.
pub fn get_title(continuous_axis_channel_def: &Value) -> Value {
    get(continuous_axis_channel_def, "title")
        .cloned()
        .or_else(|| get(continuous_axis_channel_def, "field").cloned())
        .unwrap_or(Value::Null)
}

fn is_mark_def_value(mark: &Value) -> bool {
    is_mark_def(mark)
}

/// Port of `filterTooltipWithAggregatedField`.
///
/// Returns `(customTooltipWithoutAggregatedField, filteredEncoding)`.
pub fn filter_tooltip_with_aggregated_field(old_encoding: &Value) -> (Option<Value>, Value) {
    let filtered_encoding = omit(old_encoding, &["tooltip"]);
    let Some(tooltip) = get(old_encoding, "tooltip") else {
        return (None, filtered_encoding);
    };
    if !truthy(tooltip) {
        return (None, filtered_encoding);
    }

    let mut filtered = filtered_encoding;
    let mut custom_without_aggregated: Option<Value>;

    if let Value::Array(items) = tooltip {
        let mut with_aggregate: Vec<Value> = Vec::new();
        let mut without_aggregate: Vec<Value> = Vec::new();
        for item in items {
            if truthy(get(item, "aggregate").unwrap_or(&Value::Null)) {
                with_aggregate.push(item.clone());
            } else {
                without_aggregate.push(item.clone());
            }
        }
        if !with_aggregate.is_empty() {
            if let Some(map) = filtered.as_object_mut() {
                map.insert("tooltip".to_string(), Value::Array(with_aggregate));
            }
        }
        custom_without_aggregated = if without_aggregate.is_empty() {
            None
        } else {
            Some(Value::Array(without_aggregate))
        };
    } else if truthy(get(tooltip, "aggregate").unwrap_or(&Value::Null)) {
        if let Some(map) = filtered.as_object_mut() {
            map.insert("tooltip".to_string(), tooltip.clone());
        }
        custom_without_aggregated = None;
    } else {
        custom_without_aggregated = Some(tooltip.clone());
    }

    if let Some(Value::Array(items)) = &custom_without_aggregated {
        if items.len() == 1 {
            custom_without_aggregated = Some(items[0].clone());
        }
    }

    (custom_without_aggregated, filtered)
}

/// Port of `getCompositeMarkTooltip`.
pub fn get_composite_mark_tooltip(
    tooltip_summary: &[Value],
    continuous_axis_channel_def: &Value,
    encoding_without_continuous_axis: &Value,
    with_field_name: bool,
) -> Value {
    if has_prop(encoding_without_continuous_axis, "tooltip") {
        return json!({"tooltip": get(encoding_without_continuous_axis, "tooltip").unwrap().clone()});
    }

    let main_title = if with_field_name {
        format!(
            " of {}",
            title_text(&get_title(continuous_axis_channel_def))
        )
    } else {
        String::new()
    };

    let continuous_field = get(continuous_axis_channel_def, "field")
        .map(title_text)
        .unwrap_or_default();

    let mut five_summary_tooltip: Vec<Value> = Vec::new();
    for summary in tooltip_summary {
        let field_prefix = get(summary, "fieldPrefix")
            .and_then(|f| f.as_str())
            .unwrap_or("");
        let title_prefix = get(summary, "titlePrefix").cloned().unwrap_or(Value::Null);
        let field = format!("{}{}", field_prefix, continuous_field);
        let title = if title_prefix.is_object() {
            json!({"signal": format!("{}\"{}\"", title_text(&title_prefix), main_title)})
        } else {
            Value::String(format!("{}{}", title_text(&title_prefix), main_title))
        };

        let mut def = Map::new();
        def.insert("field".to_string(), Value::String(field));
        if let Some(type_value) = get(continuous_axis_channel_def, "type") {
            def.insert("type".to_string(), type_value.clone());
        }
        def.insert("title".to_string(), title);
        five_summary_tooltip.push(Value::Object(def));
    }

    let tooltip_field_defs: Vec<Value> = field_defs(encoding_without_continuous_axis)
        .iter()
        .map(to_string_field_def)
        .collect();
    let unique_fields = unique(&tooltip_field_defs, hash);

    let mut tooltip = five_summary_tooltip;
    tooltip.extend(unique_fields);
    json!({"tooltip": tooltip})
}

/// Parameters for one part of a composite mark.
#[derive(Debug, Clone)]
pub struct CompositePart {
    pub part_name: String,
    pub mark: Value,
    pub position_prefix: String,
    pub end_position_prefix: Option<String>,
    pub extra_encoding: Value,
}

impl CompositePart {
    pub fn new(part_name: &str, mark: Value, position_prefix: &str) -> Self {
        Self {
            part_name: part_name.to_string(),
            mark,
            position_prefix: position_prefix.to_string(),
            end_position_prefix: None,
            extra_encoding: empty_object(),
        }
    }

    pub fn end_position_prefix(mut self, prefix: &str) -> Self {
        self.end_position_prefix = Some(prefix.to_string());
        self
    }

    pub fn extra_encoding(mut self, encoding: Value) -> Self {
        self.extra_encoding = encoding;
        self
    }
}

/// Port of `makeCompositeAggregatePartFactory`'s returned function.
#[allow(clippy::too_many_arguments)]
pub fn make_composite_aggregate_part(
    composite_mark_def: &Value,
    continuous_axis: &str,
    continuous_axis_channel_def: &Value,
    shared_encoding: &Value,
    composite_mark_config: &Value,
    part: CompositePart,
) -> Vec<Value> {
    let title_value = get_title(continuous_axis_channel_def);
    let continuous_field = get(continuous_axis_channel_def, "field")
        .map(title_text)
        .unwrap_or_default();

    let mut axis_def = Map::new();
    axis_def.insert(
        "field".to_string(),
        Value::String(format!("{}_{}", part.position_prefix, continuous_field)),
    );
    if let Some(type_value) = get(continuous_axis_channel_def, "type") {
        axis_def.insert("type".to_string(), type_value.clone());
    }
    if !title_value.is_null() {
        axis_def.insert("title".to_string(), title_value);
    }
    if let Some(scale) = get(continuous_axis_channel_def, "scale") {
        axis_def.insert("scale".to_string(), scale.clone());
    }
    if let Some(axis) = get(continuous_axis_channel_def, "axis") {
        axis_def.insert("axis".to_string(), axis.clone());
    }

    let mut encoding = Map::new();
    encoding.insert(continuous_axis.to_string(), Value::Object(axis_def));
    if let Some(end_prefix) = &part.end_position_prefix {
        encoding.insert(
            format!("{}2", continuous_axis),
            json!({"field": format!("{}_{}", end_prefix, continuous_field)}),
        );
    }
    assign(&mut encoding, shared_encoding);
    assign(&mut encoding, &part.extra_encoding);

    let part_base_spec = json!({
        "mark": part.mark,
        "encoding": Value::Object(encoding),
    });

    part_layer_mixins(
        composite_mark_def,
        &part.part_name,
        composite_mark_config,
        &part_base_spec,
    )
}

/// Port of `partLayerMixins`.
pub fn part_layer_mixins(
    mark_def: &Value,
    part: &str,
    composite_mark_config: &Value,
    part_base_spec: &Value,
) -> Vec<Value> {
    let clip = get(mark_def, "clip").cloned();
    let color = get(mark_def, "color").cloned();
    let opacity = get(mark_def, "opacity").cloned();
    let mark_type = crate::mark::get_mark_type(mark_def)
        .unwrap_or("")
        .to_string();

    let part_value = get(mark_def, part).cloned();
    let enabled = match &part_value {
        Some(Value::Bool(b)) => *b,
        Some(Value::Null) => false,
        Some(_) => true,
        None => truthy(get(composite_mark_config, part).unwrap_or(&Value::Null)),
    };

    if !enabled {
        return Vec::new();
    }

    let mut mark = Map::new();
    if let Some(Value::Object(config)) = get(composite_mark_config, part) {
        for (key, value) in config {
            mark.insert(key.clone(), value.clone());
        }
    }
    if clip.as_ref().map(truthy).unwrap_or(false) {
        mark.insert("clip".to_string(), clip.unwrap());
    }
    if color.as_ref().map(truthy).unwrap_or(false) {
        mark.insert("color".to_string(), color.unwrap());
    }
    if opacity.as_ref().map(truthy).unwrap_or(false) {
        mark.insert("opacity".to_string(), opacity.unwrap());
    }

    let base_mark = get(part_base_spec, "mark").cloned().unwrap_or(Value::Null);
    if is_mark_def_value(&base_mark) {
        assign(&mut mark, &base_mark);
    } else {
        mark.insert("type".to_string(), base_mark);
    }

    mark.insert(
        "style".to_string(),
        Value::String(format!("{}-{}", mark_type, part)),
    );

    if let Some(value) = &part_value {
        if !value.is_boolean() {
            assign(&mut mark, value);
        }
    }

    let mut spec = part_base_spec.as_object().cloned().unwrap_or_default();
    spec.insert("mark".to_string(), Value::Object(mark));
    vec![Value::Object(spec)]
}

/// Port of `compositeMarkContinuousAxis`'s result.
#[derive(Debug, Clone)]
pub struct CompositeAxis {
    pub continuous_axis_channel_def: Value,
    pub continuous_axis_channel_def2: Value,
    pub continuous_axis_channel_def_error: Value,
    pub continuous_axis_channel_def_error2: Value,
    pub continuous_axis: String,
}

/// Port of `compositeMarkContinuousAxis`.
pub fn composite_mark_continuous_axis(
    spec: &Value,
    orient: &str,
    composite_mark: &str,
) -> CompositeAxis {
    let encoding = get(spec, "encoding").cloned().unwrap_or_else(empty_object);
    let continuous_axis = if orient == "vertical" { "y" } else { "x" };
    let def = get(&encoding, continuous_axis)
        .cloned()
        .unwrap_or(Value::Null);
    let def2 = get(&encoding, &format!("{}2", continuous_axis))
        .cloned()
        .unwrap_or(Value::Null);
    let def_error = get(&encoding, &format!("{}Error", continuous_axis))
        .cloned()
        .unwrap_or(Value::Null);
    let def_error2 = get(&encoding, &format!("{}Error2", continuous_axis))
        .cloned()
        .unwrap_or(Value::Null);

    CompositeAxis {
        continuous_axis_channel_def: filter_aggregate_from_channel_def(&def, composite_mark),
        continuous_axis_channel_def2: filter_aggregate_from_channel_def(&def2, composite_mark),
        continuous_axis_channel_def_error: filter_aggregate_from_channel_def(
            &def_error,
            composite_mark,
        ),
        continuous_axis_channel_def_error2: filter_aggregate_from_channel_def(
            &def_error2,
            composite_mark,
        ),
        continuous_axis: continuous_axis.to_string(),
    }
}

/// Port of `filterAggregateFromChannelDef`.
pub fn filter_aggregate_from_channel_def(def: &Value, composite_mark: &str) -> Value {
    if truthy(get(def, "aggregate").unwrap_or(&Value::Null)) {
        if let Some(aggregate) = get(def, "aggregate").and_then(|a| a.as_str()) {
            if aggregate != composite_mark {
                crate::log::warn(format!(
                    "Continuous axis should not have customized aggregation function {}; {} already agregates the axis.",
                    aggregate, composite_mark
                ));
            }
        }
        omit(def, &["aggregate"])
    } else {
        def.clone()
    }
}

/// Port of `compositeMarkOrient`.
pub fn composite_mark_orient(spec: &Value, composite_mark: &str) -> Result<String, String> {
    let mark = get(spec, "mark").cloned().unwrap_or(Value::Null);
    let encoding = get(spec, "encoding").cloned().unwrap_or_else(empty_object);
    let x = get(&encoding, "x").cloned().unwrap_or(Value::Null);
    let y = get(&encoding, "y").cloned().unwrap_or(Value::Null);

    if is_mark_def(&mark) {
        if let Some(orient) = get(&mark, "orient").and_then(|o| o.as_str()) {
            return Ok(orient.to_string());
        }
    }

    if is_continuous_field_or_datum_def(&x) {
        if is_continuous_field_or_datum_def(&y) {
            let x_aggregate = if is_field_def(&x) {
                get(&x, "aggregate").cloned().unwrap_or(Value::Null)
            } else {
                Value::Null
            };
            let y_aggregate = if is_field_def(&y) {
                get(&y, "aggregate").cloned().unwrap_or(Value::Null)
            } else {
                Value::Null
            };
            let x_agg_is_mark = x_aggregate.as_str() == Some(composite_mark);
            let y_agg_is_mark = y_aggregate.as_str() == Some(composite_mark);

            if !truthy(&x_aggregate) && y_agg_is_mark {
                return Ok("vertical".to_string());
            } else if !truthy(&y_aggregate) && x_agg_is_mark {
                return Ok("horizontal".to_string());
            } else if x_agg_is_mark && y_agg_is_mark {
                return Err("Both x and y cannot have aggregate".to_string());
            } else {
                if is_field_or_datum_def_for_time_format(&y)
                    && !is_field_or_datum_def_for_time_format(&x)
                {
                    return Ok("horizontal".to_string());
                }
                return Ok("vertical".to_string());
            }
        }
        return Ok("horizontal".to_string());
    } else if is_continuous_field_or_datum_def(&y) {
        return Ok("vertical".to_string());
    }
    Err(format!(
        "Need a valid continuous axis for {}s",
        composite_mark
    ))
}

/// Port of `titleCase` re-export used by errorbar tooltips.
pub fn aggregate_title(field: &str, op: &str) -> Value {
    title(
        &json!({"field": field, "aggregate": op, "type": "quantitative"}),
        &json!({}),
        &TitleOpt {
            allow_disabling: false,
            include_default: true,
        },
    )
}

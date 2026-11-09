//! Port of `src/normalize/core.ts`.

use serde_json::{json, Map, Value};

use crate::normalize::base::{NormalizerParams, RepeaterValue};
use crate::normalize::pathoverlay;
use crate::normalize::repeater::{replace_repeater_in_encoding, replace_repeater_in_facet};
use crate::normalize::ruleforrangedline;
use crate::normalize_support::*;

pub fn map(spec: &Value, params: &NormalizerParams) -> Result<Value, String> {
    // Special handling for a faceted unit spec as it can return a facet spec,
    // not just a layer or unit spec like a normal unit spec.
    if is_unit_spec(spec) {
        let encoding = get(spec, "encoding").cloned().unwrap_or(empty_object());
        let has_row = channel_has_field(&encoding, ROW);
        let has_column = channel_has_field(&encoding, COLUMN);
        let has_facet = channel_has_field(&encoding, FACET);
        if has_row || has_column || has_facet {
            return map_faceted_unit(spec, params);
        }
    }
    map_dispatch(spec, params)
}

fn map_dispatch(spec: &Value, params: &NormalizerParams) -> Result<Value, String> {
    if has_prop(spec, "facet") {
        map_facet(spec, params)
    } else if has_prop(spec, "repeat") {
        map_repeat(spec, params)
    } else if has_prop(spec, "hconcat") {
        map_hconcat(spec, params)
    } else if has_prop(spec, "vconcat") {
        map_vconcat(spec, params)
    } else if has_prop(spec, "concat") {
        map_concat(spec, params)
    } else {
        map_layer_or_unit(spec, params)
    }
}

pub fn map_layer_or_unit(spec: &Value, params: &NormalizerParams) -> Result<Value, String> {
    if has_prop(spec, "layer") {
        map_layer(spec, params)
    } else if is_unit_spec(spec) {
        map_unit(spec, params)
    } else {
        Err(crate::log::message::invalid_spec(spec))
    }
}

pub fn map_unit(spec: &Value, params: &NormalizerParams) -> Result<Value, String> {
    let encoding = replace_repeater_in_encoding(
        get(spec, "encoding").unwrap_or(&Value::Null),
        params.repeater.as_ref(),
    );

    let mut spec_with_replaced_encoding = spec.as_object().cloned().unwrap_or_default();
    if let Some(name) = get(spec, "name").and_then(|n| n.as_str()) {
        let prefix = params.repeater_prefix.clone().unwrap_or_default();
        let joined: Vec<&str> = [prefix.as_str(), name]
            .into_iter()
            .filter(|n| !n.is_empty())
            .collect();
        spec_with_replaced_encoding.insert(
            "name".to_string(),
            Value::String(joined.join("_")),
        );
    }
    if has_prop(spec, "encoding") {
        spec_with_replaced_encoding.insert("encoding".to_string(), encoding);
    }
    let spec_with_replaced_encoding = Value::Object(spec_with_replaced_encoding);

    if params.parent_encoding.is_some() || params.parent_projection.is_some() {
        return map_unit_with_parent_encoding_or_projection(&spec_with_replaced_encoding, params);
    }

    let mark_value = get(&spec_with_replaced_encoding, "mark")
        .cloned()
        .unwrap_or(Value::Null);
    if crate::compositemark::base::is_composite_unit_spec(&spec_with_replaced_encoding, "boxplot") {
        return crate::compositemark::boxplot::normalize_box_plot(
            &spec_with_replaced_encoding,
            &params.config,
        );
    }
    if crate::compositemark::base::is_composite_unit_spec(&spec_with_replaced_encoding, "errorbar") {
        return crate::compositemark::errorbar::normalize_error_bar(
            &spec_with_replaced_encoding,
            &params.config,
        );
    }
    if crate::compositemark::base::is_composite_unit_spec(&spec_with_replaced_encoding, "errorband") {
        return crate::compositemark::errorband::normalize_error_band(
            &spec_with_replaced_encoding,
            &params.config,
        );
    }
    let _ = mark_value;
    if pathoverlay::has_matching_type(&spec_with_replaced_encoding, &params.config) {
        return pathoverlay::run(&spec_with_replaced_encoding, params);
    }
    if ruleforrangedline::has_matching_type(&spec_with_replaced_encoding) {
        return ruleforrangedline::run(&spec_with_replaced_encoding, params);
    }

    Ok(spec_with_replaced_encoding)
}

fn map_unit_with_parent_encoding_or_projection(
    spec: &Value,
    params: &NormalizerParams,
) -> Result<Value, String> {
    let parent_encoding = params.parent_encoding.as_ref();
    let parent_projection = params.parent_projection.as_ref();
    let encoding = get(spec, "encoding");
    let projection = get(spec, "projection");

    let merged_projection = merge_projection(parent_projection, projection);
    let merged_encoding = merge_encoding(
        parent_encoding,
        Some(&replace_repeater_in_encoding(
            encoding.unwrap_or(&Value::Null),
            params.repeater.as_ref(),
        )),
        false,
    );

    let mut new_spec = spec.as_object().cloned().unwrap_or_default();
    if let Some(p) = merged_projection {
        new_spec.insert("projection".to_string(), p);
    }
    if let Some(e) = merged_encoding {
        new_spec.insert("encoding".to_string(), e);
    }
    map_unit(
        &Value::Object(new_spec),
        &NormalizerParams::with_config(params.config.clone()),
    )
}

fn map_faceted_unit(spec: &Value, norm_params: &NormalizerParams) -> Result<Value, String> {
    let encoding = get(spec, "encoding").cloned().unwrap_or(empty_object());
    let row = get(&encoding, ROW).cloned();
    let column = get(&encoding, COLUMN).cloned();
    let facet = get(&encoding, FACET).cloned();
    let inner_encoding = omit(&encoding, &[ROW, COLUMN, FACET]);

    let mut outer_spec = spec.as_object().cloned().unwrap_or_default();
    for prop in [
        "mark",
        "width",
        "projection",
        "height",
        "view",
        "params",
        "encoding",
    ] {
        outer_spec.remove(prop);
    }

    let (facet_mapping, layout) =
        get_facet_mapping_and_layout(row.as_ref(), column.as_ref(), facet.as_ref(), norm_params);
    let new_encoding = replace_repeater_in_encoding(&inner_encoding, norm_params.repeater.as_ref());

    let mut inner = Map::new();
    if let Some(width) = get(spec, "width") {
        if truthy(width) {
            inner.insert("width".to_string(), width.clone());
        }
    }
    if let Some(height) = get(spec, "height") {
        if truthy(height) {
            inner.insert("height".to_string(), height.clone());
        }
    }
    if let Some(view) = get(spec, "view") {
        if truthy(view) {
            inner.insert("view".to_string(), view.clone());
        }
    }
    if let Some(projection) = get(spec, "projection") {
        if truthy(projection) {
            inner.insert("projection".to_string(), projection.clone());
        }
    }
    inner.insert(
        "mark".to_string(),
        get(spec, "mark").cloned().unwrap_or(Value::Null),
    );
    inner.insert("encoding".to_string(), new_encoding);
    if let Some(params) = get(spec, "params") {
        if truthy(params) {
            inner.insert("params".to_string(), params.clone());
        }
    }

    let mut facet_spec = outer_spec;
    if let Value::Object(l) = layout {
        for (k, v) in l {
            facet_spec.insert(k, v);
        }
    }
    // row / column has higher precedence than facet
    facet_spec.insert("facet".to_string(), facet_mapping);
    facet_spec.insert("spec".to_string(), Value::Object(inner));

    map_facet(&Value::Object(facet_spec), norm_params)
}

fn get_facet_mapping_and_layout(
    row: Option<&Value>,
    column: Option<&Value>,
    facet: Option<&Value>,
    params: &NormalizerParams,
) -> (Value, Value) {
    let has_row = row.map(truthy).unwrap_or(false);
    let has_column = column.map(truthy).unwrap_or(false);

    if has_row || has_column {
        if facet.map(truthy).unwrap_or(false) {
            let mut channels: Vec<&str> = Vec::new();
            if has_row {
                channels.push(ROW);
            }
            if has_column {
                channels.push(COLUMN);
            }
            crate::log::warn(format!(
                "Facet encoding dropped as {} {} also specified.",
                channels.join(" and "),
                if channels.len() > 1 { "are" } else { "is" }
            ));
        }

        let mut facet_mapping = Map::new();
        let mut layout: Map<String, Value> = Map::new();

        for (channel, def) in [(ROW, row), (COLUMN, column)] {
            let Some(def) = def else { continue };
            if !truthy(def) {
                continue;
            }
            let def_without_layout = omit(def, &["align", "center", "spacing", "columns"]);
            facet_mapping.insert(channel.to_string(), def_without_layout);
            for prop in ["align", "center", "spacing"] {
                if let Some(value) = get(def, prop) {
                    let entry = layout
                        .entry(prop.to_string())
                        .or_insert_with(empty_object);
                    if let Some(m) = entry.as_object_mut() {
                        m.insert(channel.to_string(), value.clone());
                    }
                }
            }
        }

        (Value::Object(facet_mapping), Value::Object(layout))
    } else {
        let facet = facet.cloned().unwrap_or(empty_object());
        let facet_without_layout = omit(&facet, &["align", "center", "spacing", "columns"]);
        let facet_mapping = replace_repeater_in_facet(&facet_without_layout, params.repeater.as_ref());
        let mut layout = Map::new();
        for prop in ["align", "center", "spacing", "columns"] {
            if let Some(value) = get(&facet, prop) {
                if truthy(value) {
                    layout.insert(prop.to_string(), value.clone());
                }
            }
        }
        (facet_mapping, Value::Object(layout))
    }
}

fn merge_encoding(
    parent_encoding: Option<&Value>,
    encoding: Option<&Value>,
    layer: bool,
) -> Option<Value> {
    let encoding = encoding.cloned().unwrap_or(empty_object());
    let Some(parent_encoding) = parent_encoding else {
        return if is_empty_object(&encoding) {
            None
        } else {
            Some(encoding)
        };
    };

    let mut channels: Vec<String> = keys(parent_encoding);
    for key in keys(&encoding) {
        if !channels.contains(&key) {
            channels.push(key);
        }
    }

    let mut merged = Map::new();
    for channel in channels {
        let channel_def = get(&encoding, &channel).cloned();
        let parent_channel_def = get(parent_encoding, &channel);

        if let Some(channel_def) = &channel_def {
            if is_field_or_datum_def(channel_def) {
                // Field/Datum def can inherit properties from its parent
                merged.insert(channel, spread(parent_channel_def, Some(channel_def)));
            } else if has_conditional_field_or_datum_def(channel_def) {
                let condition = get(channel_def, "condition").unwrap();
                let merged_condition = spread(parent_channel_def, Some(condition));
                let mut m = channel_def.as_object().cloned().unwrap_or_default();
                m.insert("condition".to_string(), merged_condition);
                merged.insert(channel, Value::Object(m));
            } else if truthy(channel_def) || channel_def.is_null() {
                merged.insert(channel, channel_def.clone());
            } else if layer
                || parent_channel_def.map(is_value_def).unwrap_or(false)
                || parent_channel_def.map(is_signal_ref).unwrap_or(false)
                || parent_channel_def.map(is_field_or_datum_def).unwrap_or(false)
                || parent_channel_def.map(|p| p.is_array()).unwrap_or(false)
            {
                if let Some(parent) = parent_channel_def {
                    merged.insert(channel, parent.clone());
                }
            }
        } else if layer
            || parent_channel_def.map(is_value_def).unwrap_or(false)
            || parent_channel_def.map(is_signal_ref).unwrap_or(false)
            || parent_channel_def.map(is_field_or_datum_def).unwrap_or(false)
            || parent_channel_def.map(|p| p.is_array()).unwrap_or(false)
        {
            if let Some(parent) = parent_channel_def {
                merged.insert(channel, parent.clone());
            }
        }
    }

    if merged.is_empty() {
        None
    } else {
        Some(Value::Object(merged))
    }
}

fn is_signal_ref(v: &Value) -> bool {
    has_prop(v, "signal")
}

fn merge_projection(parent: Option<&Value>, projection: Option<&Value>) -> Option<Value> {
    if parent.map(truthy).unwrap_or(false) && projection.map(truthy).unwrap_or(false) {
        crate::log::warn(format!(
            "Layer's shared projection {} is overridden by a child projection {}.",
            crate::util::stringify(parent.unwrap()),
            crate::util::stringify(projection.unwrap())
        ));
    }
    match projection {
        Some(p) if !p.is_null() => Some(p.clone()),
        _ => parent.cloned(),
    }
}

pub fn map_layer(spec: &Value, params: &NormalizerParams) -> Result<Value, String> {
    let encoding = get(spec, "encoding").cloned();
    let projection = get(spec, "projection").cloned();

    let mut new_params = params.clone();
    new_params.parent_encoding =
        merge_encoding(params.parent_encoding.as_ref(), encoding.as_ref(), true);
    new_params.parent_projection = merge_projection(params.parent_projection.as_ref(), projection.as_ref());

    let mut rest = omit(spec, &["encoding", "projection"]);
    if let Some(name) = get(spec, "name").and_then(|n| n.as_str()) {
        let prefix = new_params.repeater_prefix.clone().unwrap_or_default();
        let joined: Vec<&str> = [prefix.as_str(), name]
            .into_iter()
            .filter(|n| !n.is_empty())
            .collect();
        if let Some(m) = rest.as_object_mut() {
            m.insert("name".to_string(), Value::String(joined.join("_")));
        }
    }

    let children: Vec<Value> = get(spec, "layer")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .map(|s| map_layer_or_unit(s, &new_params))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    if let Some(m) = rest.as_object_mut() {
        m.insert("layer".to_string(), Value::Array(children));
    }
    Ok(rest)
}

fn map_facet(spec: &Value, params: &NormalizerParams) -> Result<Value, String> {
    let mut spec = spec.clone();
    if let Some(facet) = get(&spec, "facet") {
        if is_facet_mapping(facet) && truthy(get(&spec, "columns").unwrap_or(&Value::Null)) {
            spec = omit(&spec, &["columns"]);
            crate::log::warn(
                "The \"columns\" property cannot be used when \"facet\" has nested row/column.",
            );
        }
    }
    let mut out = spec.as_object().cloned().unwrap_or_default();
    out.insert(
        "spec".to_string(),
        map(get(&spec, "spec").unwrap_or(&Value::Null), params)?,
    );
    Ok(Value::Object(out))
}

fn map_hconcat(spec: &Value, params: &NormalizerParams) -> Result<Value, String> {
    let mut out = spec.as_object().cloned().unwrap_or_default();
    let children: Vec<Value> = get(spec, "hconcat")
        .and_then(|a| a.as_array())
        .map(|a| a.iter().map(|s| map(s, params)).collect::<Result<Vec<_>, _>>())
        .transpose()?
        .unwrap_or_default();
    out.insert("hconcat".to_string(), Value::Array(children));
    Ok(Value::Object(out))
}

fn map_vconcat(spec: &Value, params: &NormalizerParams) -> Result<Value, String> {
    let mut out = spec.as_object().cloned().unwrap_or_default();
    let children: Vec<Value> = get(spec, "vconcat")
        .and_then(|a| a.as_array())
        .map(|a| a.iter().map(|s| map(s, params)).collect::<Result<Vec<_>, _>>())
        .transpose()?
        .unwrap_or_default();
    out.insert("vconcat".to_string(), Value::Array(children));
    Ok(Value::Object(out))
}

fn map_concat(spec: &Value, params: &NormalizerParams) -> Result<Value, String> {
    let mut out = omit(spec, &["concat"]);
    let children: Vec<Value> = get(spec, "concat")
        .and_then(|a| a.as_array())
        .map(|a| a.iter().map(|s| map(s, params)).collect::<Result<Vec<_>, _>>())
        .transpose()?
        .unwrap_or_default();
    if let Some(m) = out.as_object_mut() {
        m.insert("concat".to_string(), Value::Array(children));
    }
    Ok(out)
}

fn map_repeat(spec: &Value, params: &NormalizerParams) -> Result<Value, String> {
    if is_layer_repeat_spec(spec) {
        map_layer_repeat(spec, params)
    } else {
        map_non_layer_repeat(spec, params)
    }
}

fn map_layer_repeat(spec: &Value, params: &NormalizerParams) -> Result<Value, String> {
    let repeat = get(spec, "repeat").cloned().unwrap_or(empty_object());
    let child_spec = get(spec, "spec").cloned().unwrap_or(Value::Null);
    let row = get(&repeat, "row").cloned();
    let column = get(&repeat, "column").cloned();
    let layer = get(&repeat, "layer").and_then(|l| l.as_array()).cloned().unwrap_or_default();

    let repeater = params.repeater.clone().unwrap_or_default();
    let repeater_prefix = params.repeater_prefix.clone().unwrap_or_default();

    if row.as_ref().map(truthy).unwrap_or(false) || column.as_ref().map(truthy).unwrap_or(false) {
        let mut new_spec = spec.as_object().cloned().unwrap_or_default();
        let mut new_repeat = Map::new();
        if let Some(row) = row {
            if truthy(&row) {
                new_repeat.insert("row".to_string(), row);
            }
        }
        if let Some(column) = column {
            if truthy(&column) {
                new_repeat.insert("column".to_string(), column);
            }
        }
        new_spec.insert("repeat".to_string(), Value::Object(new_repeat));
        new_spec.insert(
            "spec".to_string(),
            json!({"repeat": {"layer": layer}, "spec": child_spec}),
        );
        return map_repeat(&Value::Object(new_spec), params);
    }

    let rest = omit(spec, &["repeat", "spec"]);
    let mut out = rest.as_object().cloned().unwrap_or_default();
    let mut layers: Vec<Value> = Vec::new();
    for layer_value in layer {
        let mut child_repeater = repeater.clone();
        child_repeater.layer = layer_value.as_str().map(|s| s.to_string());
        let value_name = match &layer_value {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        let child_name = format!(
            "{}{}child__layer_{}",
            get(&child_spec, "name")
                .and_then(|n| n.as_str())
                .map(|s| format!("{}_", s))
                .unwrap_or_default(),
            repeater_prefix,
            crate::util::var_name(&value_name)
        );
        let mut child_params = params.clone();
        child_params.repeater = Some(child_repeater);
        child_params.repeater_prefix = Some(child_name.clone());
        let mut child = map_layer_or_unit(&child_spec, &child_params)?;
        if let Some(m) = child.as_object_mut() {
            m.insert("name".to_string(), Value::String(child_name));
        }
        layers.push(child);
    }
    out.insert("layer".to_string(), Value::Array(layers));
    Ok(Value::Object(out))
}

fn map_non_layer_repeat(spec: &Value, params: &NormalizerParams) -> Result<Value, String> {
    let mut spec = spec.clone();
    let repeat_value = get(&spec, "repeat").cloned().unwrap_or(Value::Null);
    let child_spec = get(&spec, "spec").cloned().unwrap_or(Value::Null);

    if !repeat_value.is_array() && truthy(get(&spec, "columns").unwrap_or(&Value::Null)) {
        spec = omit(&spec, &["columns"]);
        crate::log::warn(
            "The \"columns\" property cannot be used when \"repeat\" has nested row/column.",
        );
    }

    let repeat = get(&spec, "repeat").cloned().unwrap_or(Value::Null);
    let data = get(&spec, "data").cloned();

    let repeater = params.repeater.clone().unwrap_or_default();
    let repeater_prefix = params.repeater_prefix.clone().unwrap_or_default();

    let rows: Vec<Value> = if !repeat.is_array() && truthy(get(&repeat, "row").unwrap_or(&Value::Null)) {
        get(&repeat, "row")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default()
    } else {
        vec![repeater
            .row
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null)]
    };
    let columns_values: Vec<Value> =
        if !repeat.is_array() && truthy(get(&repeat, "column").unwrap_or(&Value::Null)) {
            get(&repeat, "column")
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default()
        } else {
            vec![repeater
                .column
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null)]
        };
    let repeat_values: Vec<Value> = if repeat.is_array() {
        repeat.as_array().cloned().unwrap_or_default()
    } else {
        vec![repeater
            .repeat
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null)]
    };

    let child_name_prefix = format!(
        "{}{}",
        get(&child_spec, "name")
            .and_then(|n| n.as_str())
            .map(|s| format!("{}_", s))
            .unwrap_or_default(),
        repeater_prefix
    );

    let mut concat: Vec<Value> = Vec::new();
    for repeat_value_item in &repeat_values {
        for row_value in &rows {
            for column_value in &columns_values {
                let child_repeater = RepeaterValue {
                    repeat: repeat_value_item.as_str().map(|s| s.to_string()),
                    row: row_value.as_str().map(|s| s.to_string()),
                    column: column_value.as_str().map(|s| s.to_string()),
                    layer: repeater.layer.clone(),
                };

                let name_suffix = if repeat.is_array() {
                    value_to_name(repeat_value_item)
                } else {
                    format!(
                        "{}{}",
                        if truthy(get(&repeat, "row").unwrap_or(&Value::Null)) {
                            format!("row_{}", value_to_name(row_value))
                        } else {
                            String::new()
                        },
                        if truthy(get(&repeat, "column").unwrap_or(&Value::Null)) {
                            format!("column_{}", value_to_name(column_value))
                        } else {
                            String::new()
                        }
                    )
                };
                let child_name = format!("{}child__{}", child_name_prefix, name_suffix);

                let mut child_params = params.clone();
                child_params.repeater = Some(child_repeater);
                child_params.repeater_prefix = Some(child_name.clone());
                let mut child = map(&child_spec, &child_params)?;
                if let Some(m) = child.as_object_mut() {
                    m.insert("name".to_string(), Value::String(child_name));
                }
                concat.push(omit(&child, &["data"]));
            }
        }
    }

    let columns: Option<Value> = if repeat.is_array() {
        get(&spec, "columns").cloned()
    } else if truthy(get(&repeat, "column").unwrap_or(&Value::Null)) {
        Some(json!(
            get(&repeat, "column")
                .and_then(|c| c.as_array())
                .map(|c| c.len())
                .unwrap_or(0)
        ))
    } else {
        Some(json!(1))
    };

    let mut remaining = omit(&spec, &["repeat", "spec", "data"]);
    let mut out = Map::new();
    let data_value = match get(&child_spec, "data") {
        Some(d) if !d.is_null() => Some(d.clone()),
        _ => data.filter(|d| !d.is_null()),
    };
    if let Some(data) = data_value {
        out.insert("data".to_string(), data);
    }
    out.insert("align".to_string(), Value::String("all".to_string()));
    if let Some(r) = remaining.as_object() {
        for (k, v) in r {
            out.insert(k.clone(), v.clone());
        }
    }
    if let Some(columns) = columns {
        if !columns.is_null() {
            out.insert("columns".to_string(), columns);
        }
    }
    out.insert("concat".to_string(), Value::Array(concat));
    let _ = remaining.as_object_mut();
    Ok(Value::Object(out))
}

fn value_to_name(v: &Value) -> String {
    match v {
        Value::String(s) => crate::util::var_name(s),
        other => crate::util::var_name(&other.to_string()),
    }
}


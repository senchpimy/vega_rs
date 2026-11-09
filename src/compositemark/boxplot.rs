//! Port of `src/compositemark/boxplot.ts`.

use serde_json::{json, Map, Value};

use crate::compositemark::common::{
    composite_mark_continuous_axis, composite_mark_orient, filter_tooltip_with_aggregated_field,
    get_composite_mark_tooltip, get_title, make_composite_aggregate_part, CompositeAxis,
    CompositePart,
};
use crate::encoding::extract_transforms_from_encoding;
use crate::mark::get_mark_prop_or_config;
use crate::normalize_support::{empty_object, get, normalize_encoding, omit, title_text, truthy};
use crate::util::{access_with_datum_to_unescaped_path, remove_path_from_field};

pub const BOXPLOT: &str = "boxplot";
pub const BOXPLOT_PARTS: [&str; 5] = ["box", "median", "outliers", "rule", "ticks"];

/// Port of `getBoxPlotType`.
pub fn get_box_plot_type(extent: &Value) -> String {
    if extent.is_number() {
        "tukey".to_string()
    } else {
        extent.as_str().unwrap_or("tukey").to_string()
    }
}

fn box_params_quartiles(continuous_axis_field: &str) -> Vec<Value> {
    let aliased_field_name = remove_path_from_field(continuous_axis_field);
    vec![
        json!({"op": "q1", "field": continuous_axis_field, "as": format!("lower_box_{}", aliased_field_name)}),
        json!({"op": "q3", "field": continuous_axis_field, "as": format!("upper_box_{}", aliased_field_name)}),
    ]
}

struct BoxParams {
    bins: Vec<Value>,
    time_units: Vec<Value>,
    transform: Vec<Value>,
    groupby: Vec<String>,
    aggregate: Vec<Value>,
    continuous_axis_channel_def: Value,
    continuous_axis: String,
    encoding_without_continuous_axis: Value,
    ticks_orient: String,
    box_orient: String,
    custom_tooltip_without_aggregated_field: Option<Value>,
}

fn box_params(spec: &Value, extent: &Value, config: &Value) -> Result<BoxParams, String> {
    let orient = composite_mark_orient(spec, BOXPLOT)?;
    let axis: CompositeAxis = composite_mark_continuous_axis(spec, &orient, BOXPLOT);
    let continuous_axis_channel_def = axis.continuous_axis_channel_def;
    let continuous_axis = axis.continuous_axis;

    let continuous_field_name = get(&continuous_axis_channel_def, "field")
        .map(title_text)
        .unwrap_or_default();
    let aliased_field_name = remove_path_from_field(&continuous_field_name);

    let box_plot_type = get_box_plot_type(extent);

    let mut boxplot_specific_aggregate = box_params_quartiles(&continuous_field_name);
    boxplot_specific_aggregate.push(json!({
        "op": "median",
        "field": continuous_field_name,
        "as": format!("mid_box_{}", aliased_field_name),
    }));
    boxplot_specific_aggregate.push(json!({
        "op": "min",
        "field": continuous_field_name,
        "as": format!("{}{}", if box_plot_type == "min-max" { "lower_whisker_" } else { "min_" }, aliased_field_name),
    }));
    boxplot_specific_aggregate.push(json!({
        "op": "max",
        "field": continuous_field_name,
        "as": format!("{}{}", if box_plot_type == "min-max" { "upper_whisker_" } else { "max_" }, aliased_field_name),
    }));

    let post_aggregate_calculates: Vec<Value> = if box_plot_type == "min-max"
        || box_plot_type == "tukey"
    {
        Vec::new()
    } else {
        let extent_str = extent
            .as_f64()
            .map(crate::util::js_number_to_string)
            .unwrap_or_default();
        vec![
            json!({
                "calculate": format!(
                    "{} - {}",
                    access_with_datum_to_unescaped_path(&format!("upper_box_{}", aliased_field_name)),
                    access_with_datum_to_unescaped_path(&format!("lower_box_{}", aliased_field_name))
                ),
                "as": format!("iqr_{}", aliased_field_name)
            }),
            json!({
                "calculate": format!(
                    "min({} + {} * {}, {})",
                    access_with_datum_to_unescaped_path(&format!("upper_box_{}", aliased_field_name)),
                    access_with_datum_to_unescaped_path(&format!("iqr_{}", aliased_field_name)),
                    extent_str,
                    access_with_datum_to_unescaped_path(&format!("max_{}", aliased_field_name))
                ),
                "as": format!("upper_whisker_{}", aliased_field_name)
            }),
            json!({
                "calculate": format!(
                    "max({} - {} * {}, {})",
                    access_with_datum_to_unescaped_path(&format!("lower_box_{}", aliased_field_name)),
                    access_with_datum_to_unescaped_path(&format!("iqr_{}", aliased_field_name)),
                    extent_str,
                    access_with_datum_to_unescaped_path(&format!("min_{}", aliased_field_name))
                ),
                "as": format!("lower_whisker_{}", aliased_field_name)
            }),
        ]
    };

    let spec_encoding = get(spec, "encoding").cloned().unwrap_or_else(empty_object);
    let old_encoding_without_continuous_axis = omit(&spec_encoding, &[continuous_axis.as_str()]);

    let (custom_tooltip_without_aggregated_field, filtered_encoding) =
        filter_tooltip_with_aggregated_field(&old_encoding_without_continuous_axis);

    let extracted = extract_transforms_from_encoding(&filtered_encoding, config);
    let bins = extracted.bins;
    let time_units = extracted.time_units;
    let aggregate = extracted.aggregate;
    let groupby = extracted.groupby;
    let encoding_without_continuous_axis = extracted.encoding;

    let ticks_orient = if orient == "vertical" {
        "horizontal".to_string()
    } else {
        "vertical".to_string()
    };
    let box_orient = orient;

    let mut aggregate_all = aggregate.clone();
    aggregate_all.extend(boxplot_specific_aggregate);

    let mut transform: Vec<Value> = Vec::new();
    transform.extend(bins.clone());
    transform.extend(time_units.clone());
    transform.push(json!({"aggregate": aggregate_all, "groupby": groupby}));
    transform.extend(post_aggregate_calculates);

    Ok(BoxParams {
        bins,
        time_units,
        transform,
        groupby,
        aggregate,
        continuous_axis_channel_def,
        continuous_axis,
        encoding_without_continuous_axis,
        ticks_orient,
        box_orient,
        custom_tooltip_without_aggregated_field,
    })
}

fn default_box_color(config: &Value) -> Value {
    let box_config = get(config, "boxplot").and_then(|b| get(b, "box"));
    let color = match box_config {
        Some(Value::Object(_)) => get(box_config.unwrap(), "color").cloned(),
        _ => None,
    };
    let color = color.or_else(|| get(config, "mark").and_then(|m| get(m, "color")).cloned());
    match color {
        Some(value) if truthy(&value) => value,
        _ => Value::String("#4c78a8".to_string()),
    }
}

/// Port of `normalizeBoxPlot`.
pub fn normalize_box_plot(spec: &Value, config: &Value) -> Result<Value, String> {
    let mut spec = spec.clone();
    let encoding = normalize_encoding(get(&spec, "encoding").unwrap_or(&Value::Null), config);
    if let Some(map) = spec.as_object_mut() {
        map.insert("encoding".to_string(), encoding);
    }

    let mark = get(&spec, "mark").cloned().unwrap_or(Value::Null);
    let mark_def = if crate::mark::is_mark_def(&mark) {
        mark.clone()
    } else {
        json!({"type": mark})
    };
    let outer_spec = omit(&spec, &["mark", "encoding", "params", "projection"]);

    if truthy(get(&spec, "params").unwrap_or(&Value::Null)) {
        crate::log::warn("Selection not supported for boxplot yet.".to_string());
    }

    let boxplot_config = get(config, BOXPLOT).cloned().unwrap_or_else(empty_object);
    let extent = get(&mark_def, "extent")
        .or_else(|| get(&boxplot_config, "extent"))
        .cloned()
        .unwrap_or(Value::Null);

    let size_value = get_mark_prop_or_config("size", &mark_def, config, None, false);
    let invalid = get(&mark_def, "invalid").cloned();

    let box_plot_type = get_box_plot_type(&extent);

    let params = box_params(&spec, &extent, config)?;

    let aliased_field_name = remove_path_from_field(
        &get(&params.continuous_axis_channel_def, "field")
            .map(title_text)
            .unwrap_or_default(),
    );

    let color_encoding = get(&params.encoding_without_continuous_axis, "color").cloned();
    let size_encoding = get(&params.encoding_without_continuous_axis, "size").cloned();
    let encoding_without_size_color_and_continuous_axis =
        omit(&params.encoding_without_continuous_axis, &["color", "size"]);

    let five_summary_tooltip_encoding = get_composite_mark_tooltip(
        &[
            json!({"fieldPrefix": if box_plot_type == "min-max" { "upper_whisker_" } else { "max_" }, "titlePrefix": "Max"}),
            json!({"fieldPrefix": "upper_box_", "titlePrefix": "Q3"}),
            json!({"fieldPrefix": "mid_box_", "titlePrefix": "Median"}),
            json!({"fieldPrefix": "lower_box_", "titlePrefix": "Q1"}),
            json!({"fieldPrefix": if box_plot_type == "min-max" { "lower_whisker_" } else { "min_" }, "titlePrefix": "Min"}),
        ],
        &params.continuous_axis_channel_def,
        &params.encoding_without_continuous_axis,
        true,
    );

    let mut end_tick = Map::new();
    end_tick.insert("type".to_string(), Value::String("tick".to_string()));
    end_tick.insert("color".to_string(), Value::String("black".to_string()));
    end_tick.insert("opacity".to_string(), json!(1));
    end_tick.insert(
        "orient".to_string(),
        Value::String(params.ticks_orient.clone()),
    );
    if let Some(invalid) = &invalid {
        end_tick.insert("invalid".to_string(), invalid.clone());
    }
    end_tick.insert("aria".to_string(), Value::Bool(false));
    let end_tick = Value::Object(end_tick);

    let whisker_tooltip_encoding = if box_plot_type == "min-max" {
        five_summary_tooltip_encoding.clone()
    } else {
        get_composite_mark_tooltip(
            &[
                json!({"fieldPrefix": "upper_whisker_", "titlePrefix": "Upper Whisker"}),
                json!({"fieldPrefix": "lower_whisker_", "titlePrefix": "Lower Whisker"}),
            ],
            &params.continuous_axis_channel_def,
            &params.encoding_without_continuous_axis,
            true,
        )
    };

    let mut whisker_layers: Vec<Value> = Vec::new();
    whisker_layers.extend(make_composite_aggregate_part(
        &mark_def,
        &params.continuous_axis,
        &params.continuous_axis_channel_def,
        &encoding_without_size_color_and_continuous_axis,
        &boxplot_config,
        CompositePart::new(
            "rule",
            {
                let mut m = Map::new();
                m.insert("type".to_string(), Value::String("rule".to_string()));
                if let Some(invalid) = &invalid {
                    m.insert("invalid".to_string(), invalid.clone());
                }
                m.insert("aria".to_string(), Value::Bool(false));
                Value::Object(m)
            },
            "lower_whisker",
        )
        .end_position_prefix("lower_box")
        .extra_encoding(whisker_tooltip_encoding.clone()),
    ));
    whisker_layers.extend(make_composite_aggregate_part(
        &mark_def,
        &params.continuous_axis,
        &params.continuous_axis_channel_def,
        &encoding_without_size_color_and_continuous_axis,
        &boxplot_config,
        CompositePart::new(
            "rule",
            {
                let mut m = Map::new();
                m.insert("type".to_string(), Value::String("rule".to_string()));
                if let Some(invalid) = &invalid {
                    m.insert("invalid".to_string(), invalid.clone());
                }
                m.insert("aria".to_string(), Value::Bool(false));
                Value::Object(m)
            },
            "upper_box",
        )
        .end_position_prefix("upper_whisker")
        .extra_encoding(whisker_tooltip_encoding.clone()),
    ));
    whisker_layers.extend(make_composite_aggregate_part(
        &mark_def,
        &params.continuous_axis,
        &params.continuous_axis_channel_def,
        &encoding_without_size_color_and_continuous_axis,
        &boxplot_config,
        CompositePart::new("ticks", end_tick.clone(), "lower_whisker")
            .extra_encoding(whisker_tooltip_encoding.clone()),
    ));
    whisker_layers.extend(make_composite_aggregate_part(
        &mark_def,
        &params.continuous_axis,
        &params.continuous_axis_channel_def,
        &encoding_without_size_color_and_continuous_axis,
        &boxplot_config,
        CompositePart::new("ticks", end_tick, "upper_whisker")
            .extra_encoding(whisker_tooltip_encoding),
    ));

    // ## Box layers
    let mut box_layers: Vec<Value> = Vec::new();
    if box_plot_type != "tukey" {
        box_layers.extend(whisker_layers.clone());
    }

    let box_mark = {
        let mut m = Map::new();
        m.insert("type".to_string(), Value::String("bar".to_string()));
        if let Some(size) = &size_value {
            if truthy(size) {
                m.insert("size".to_string(), size.clone());
            }
        }
        m.insert(
            "orient".to_string(),
            Value::String(params.box_orient.clone()),
        );
        if let Some(invalid) = &invalid {
            m.insert("invalid".to_string(), invalid.clone());
        }
        m.insert(
            "ariaRoleDescription".to_string(),
            Value::String("box".to_string()),
        );
        Value::Object(m)
    };
    box_layers.extend(make_composite_aggregate_part(
        &mark_def,
        &params.continuous_axis,
        &params.continuous_axis_channel_def,
        &params.encoding_without_continuous_axis,
        &boxplot_config,
        CompositePart::new("box", box_mark, "lower_box")
            .end_position_prefix("upper_box")
            .extra_encoding(five_summary_tooltip_encoding.clone()),
    ));

    let default_box_color = default_box_color(config);
    let mid_tick_shared_encoding = {
        let mut m = encoding_without_size_color_and_continuous_axis
            .as_object()
            .cloned()
            .unwrap_or_default();
        if let Some(size) = &size_encoding {
            if truthy(size) {
                m.insert("size".to_string(), size.clone());
            }
        }
        let continuous_field = get(&params.continuous_axis_channel_def, "field")
            .map(title_text)
            .unwrap_or_default();
        let mut condition = Map::new();
        condition.insert(
            "test".to_string(),
            Value::String(format!(
                "{} >= {}",
                access_with_datum_to_unescaped_path(&format!("lower_box_{}", continuous_field)),
                access_with_datum_to_unescaped_path(&format!("upper_box_{}", continuous_field))
            )),
        );
        match &color_encoding {
            Some(color) if truthy(color) => {
                crate::normalize_support::assign(&mut condition, color);
            }
            _ => {
                condition.insert("value".to_string(), default_box_color);
            }
        }
        m.insert(
            "color".to_string(),
            json!({"condition": Value::Object(condition)}),
        );
        Value::Object(m)
    };

    let mid_tick_mark = {
        let mut m = Map::new();
        m.insert("type".to_string(), Value::String("tick".to_string()));
        if let Some(invalid) = &invalid {
            m.insert("invalid".to_string(), invalid.clone());
        }
        if let Some(median) = get(&boxplot_config, "median") {
            if let Some(color) = get(median, "color") {
                if truthy(color) {
                    m.insert("color".to_string(), color.clone());
                }
            }
        }
        if let Some(size) = &size_value {
            if truthy(size) {
                m.insert("size".to_string(), size.clone());
            }
        }
        m.insert(
            "orient".to_string(),
            Value::String(params.ticks_orient.clone()),
        );
        m.insert("aria".to_string(), Value::Bool(false));
        Value::Object(m)
    };
    box_layers.extend(make_composite_aggregate_part(
        &mark_def,
        &params.continuous_axis,
        &params.continuous_axis_channel_def,
        &mid_tick_shared_encoding,
        &boxplot_config,
        CompositePart::new("median", mid_tick_mark, "mid_box")
            .extra_encoding(five_summary_tooltip_encoding),
    ));

    if box_plot_type == "min-max" {
        let mut out = outer_spec.as_object().cloned().unwrap_or_default();
        let mut transform = get(&outer_spec, "transform")
            .and_then(|t| t.as_array().cloned())
            .unwrap_or_default();
        transform.extend(params.transform.clone());
        out.insert("transform".to_string(), Value::Array(transform));
        out.insert("layer".to_string(), Value::Array(box_layers));
        return Ok(Value::Object(out));
    }

    // Tukey box plot
    let field_name = get(&params.continuous_axis_channel_def, "field")
        .map(title_text)
        .unwrap_or_default();
    let lower_box_expr = access_with_datum_to_unescaped_path(&format!("lower_box_{}", field_name));
    let upper_box_expr = access_with_datum_to_unescaped_path(&format!("upper_box_{}", field_name));
    let iqr_expr = format!("({} - {})", upper_box_expr, lower_box_expr);
    let extent_str = extent
        .as_f64()
        .map(crate::util::js_number_to_string)
        .unwrap_or_default();
    let lower_whisker_expr = format!("{} - {} * {}", lower_box_expr, extent_str, iqr_expr);
    let upper_whisker_expr = format!("{} + {} * {}", upper_box_expr, extent_str, iqr_expr);
    let field_expr = access_with_datum_to_unescaped_path(&field_name);

    let joinaggregate_transform = json!({
        "joinaggregate": box_params_quartiles(&field_name),
        "groupby": params.groupby,
    });

    let mut whisser_transform: Vec<Value> = Vec::new();
    whisser_transform.push(json!({
        "filter": format!("({} <= {}) && ({} <= {})", lower_whisker_expr, field_expr, field_expr, upper_whisker_expr)
    }));
    let mut aggregate: Vec<Value> = vec![
        json!({"op": "min", "field": field_name, "as": format!("lower_whisker_{}", aliased_field_name)}),
        json!({"op": "max", "field": field_name, "as": format!("upper_whisker_{}", aliased_field_name)}),
        json!({"op": "min", "field": format!("lower_box_{}", field_name), "as": format!("lower_box_{}", aliased_field_name)}),
        json!({"op": "max", "field": format!("upper_box_{}", field_name), "as": format!("upper_box_{}", aliased_field_name)}),
    ];
    aggregate.extend(params.aggregate.clone());
    whisser_transform.push(json!({"aggregate": aggregate, "groupby": params.groupby}));

    let filtered_whisker_spec = json!({
        "transform": whisser_transform,
        "layer": whisker_layers,
    });

    let encoding_without_size_color_continuous_axis_and_tooltip = omit(
        &encoding_without_size_color_and_continuous_axis,
        &["tooltip"],
    );

    let mut outlier_encoding = Map::new();
    let mut outlier_axis = Map::new();
    outlier_axis.insert("field".to_string(), Value::String(field_name.clone()));
    if let Some(type_value) = get(&params.continuous_axis_channel_def, "type") {
        outlier_axis.insert("type".to_string(), type_value.clone());
    }
    let title = get_title(&params.continuous_axis_channel_def);
    if !title.is_null() {
        outlier_axis.insert("title".to_string(), title);
    }
    if let Some(scale) = get(&params.continuous_axis_channel_def, "scale") {
        outlier_axis.insert("scale".to_string(), scale.clone());
    }
    if let Some(axis) = get(&params.continuous_axis_channel_def, "axis") {
        outlier_axis.insert("axis".to_string(), axis.clone());
    }
    outlier_encoding.insert(params.continuous_axis.clone(), Value::Object(outlier_axis));
    if let Value::Object(map) = &encoding_without_size_color_continuous_axis_and_tooltip {
        for (key, value) in map {
            outlier_encoding.insert(key.clone(), value.clone());
        }
    }
    if let Some(color) = &color_encoding {
        if truthy(color) {
            outlier_encoding.insert("color".to_string(), color.clone());
        }
    }
    if let Some(tooltip) = &params.custom_tooltip_without_aggregated_field {
        outlier_encoding.insert("tooltip".to_string(), tooltip.clone());
    }

    let outlier_layer = crate::compositemark::common::part_layer_mixins(
        &mark_def,
        "outliers",
        &boxplot_config,
        &json!({
            "transform": [{"filter": format!("({} < {}) || ({} > {})", field_expr, lower_whisker_expr, field_expr, upper_whisker_expr)}],
            "mark": "point",
            "encoding": Value::Object(outlier_encoding),
        }),
    )
    .into_iter()
    .next();

    let mut filtered_layers_mixins_transforms: Vec<Value> = Vec::new();
    filtered_layers_mixins_transforms.extend(params.bins.clone());
    filtered_layers_mixins_transforms.extend(params.time_units.clone());
    filtered_layers_mixins_transforms.push(joinaggregate_transform);

    let filtered_layers_mixins = if let Some(outlier) = outlier_layer {
        json!({
            "transform": filtered_layers_mixins_transforms,
            "layer": [outlier, filtered_whisker_spec],
        })
    } else {
        let mut spec = filtered_whisker_spec;
        let mut transform = filtered_layers_mixins_transforms;
        if let Some(existing) = spec.get("transform").and_then(|t| t.as_array()).cloned() {
            transform.extend(existing);
        }
        if let Some(map) = spec.as_object_mut() {
            map.insert("transform".to_string(), Value::Array(transform));
        }
        spec
    };

    let mut out = outer_spec.as_object().cloned().unwrap_or_default();
    out.insert(
        "layer".to_string(),
        Value::Array(vec![
            filtered_layers_mixins,
            json!({"transform": params.transform, "layer": box_layers}),
        ]),
    );
    Ok(Value::Object(out))
}

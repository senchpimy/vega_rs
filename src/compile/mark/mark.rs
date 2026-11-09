//! Port of `src/compile/mark/mark.ts`.

use serde_json::{json, Map, Value};

use crate::channel::get_secondary_range_channel;
use crate::channeldef::is_field_def;
use crate::compile::model::Model;
use crate::compile_common::signal_or_value_ref;
use crate::mark::{get_mark_config, get_mark_prop_or_config, get_styles};
use crate::util::{array, get_first_defined, is_null_or_false, omit, pick};

use super::base::mark_compiler;
use super::encode::corner_radius::corner_radius_end_for_stacked_bar;

pub const VG_CORNERRADIUS_CHANNELS: [&str; 5] = [
    "cornerRadius",
    "cornerRadiusTopLeft",
    "cornerRadiusTopRight",
    "cornerRadiusBottomLeft",
    "cornerRadiusBottomRight",
];

const FACETED_PATH_PREFIX: &str = "faceted_path_";
const STACK_GROUP_PREFIX: &str = "stack_group_";

/// Compute the stack properties for a unit model.
///
/// Upstream `UnitModel` computes this in its constructor; this port computes it
/// lazily from the mark definition and encoding.
pub fn stack_of(model: &Model) -> Option<Value> {
    if let Some(stack) = model.unit.stack.as_object() {
        return Some(Value::Object(stack.clone()));
    }
    let props = crate::stack::stack(&model.unit.mark_def, &model.unit.encoding)?;
    Some(json!({
        "groupbyChannels": props.groupby_channels,
        "groupbyFields": props.groupby_fields.iter().cloned().collect::<Vec<_>>(),
        "fieldChannel": props.field_channel,
        "stackBy": props.stack_by.iter().map(|s| json!({
            "channel": s.channel,
            "fieldDef": s.field_def,
        })).collect::<Vec<_>>(),
        "offset": props.offset.as_str(),
        "impute": props.impute,
    }))
}

/// Port of `UnitModel.isRangedOffset`.
pub fn is_ranged_offset(model: &Model, channel: &str) -> bool {
    let encoding = &model.unit.encoding;
    let mark_type = model.mark_type();
    if !crate::mark::is_bar_or_area(mark_type.as_str()) {
        return false;
    }
    if get_secondary_range_channel(channel)
        .and_then(|c2| encoding.get(c2))
        .map(|v| !v.is_null())
        .unwrap_or(false)
        || !crate::encoding::channel_has_quantitative_offset(encoding, channel)
    {
        return false;
    }
    let channel_def = encoding.get(channel);
    match channel_def {
        None => true,
        Some(channel_def) => {
            crate::channeldef::is_field_or_datum_def(channel_def)
                && model
                    .get_scale_type(channel)
                    .map(crate::scale::has_discrete_domain)
                    .unwrap_or(false)
        }
    }
}

pub fn parse_mark_groups(model: &Model) -> Vec<Value> {
    let mark = model.mark_type();

    if matches!(mark.as_str(), "line" | "area" | "trail") {
        let details = crate::encoding::path_grouping_fields(&mark, &model.unit.encoding);
        if !details.is_empty() {
            return get_path_groups(model, &details);
        }
    } else if mark == "bar" {
        let mark_def = &model.unit.mark_def;
        let config = &model.config;
        let corner_radius_end =
            get_mark_prop_or_config("cornerRadiusEnd", mark_def, config, None, false);
        let has_corner_radius = corner_radius_end
            .as_ref()
            .map(crate::channeldef_support::truthy)
            .unwrap_or(false)
            || VG_CORNERRADIUS_CHANNELS.iter().any(|prop| {
                get_mark_prop_or_config(prop, mark_def, config, None, false)
                    .as_ref()
                    .map(crate::channeldef_support::truthy)
                    .unwrap_or(false)
            });
        if stack_of(model).is_some()
            && model.field_def("size").is_none()
            && has_corner_radius
        {
            return get_groups_for_stacked_bar_with_corner_radius(
                model,
                corner_radius_end.as_ref(),
            );
        }
    }

    vec![get_mark_group(model, "")]
}

fn get_path_groups(model: &Model, details: &[String]) -> Vec<Value> {
    let data = model.get_data_name("main");
    let facet_name = format!("{FACETED_PATH_PREFIX}{data}");
    let mark_group = get_mark_group(model, FACETED_PATH_PREFIX);

    vec![json!({
        "name": model.get_name("pathgroup"),
        "type": "group",
        "from": {
            "facet": {
                "name": facet_name,
                "data": data,
                "groupby": details,
            },
        },
        "encode": {
            "update": {
                "width": {"field": {"group": "width"}},
                "height": {"field": {"group": "height"}},
            },
        },
        "marks": [mark_group],
    })]
}

fn get_groups_for_stacked_bar_with_corner_radius(
    model: &Model,
    corner_radius_end: Option<&Value>,
) -> Vec<Value> {
    let mark = get_mark_group(model, STACK_GROUP_PREFIX);
    let mut mark = mark;
    let stack = stack_of(model).unwrap_or(Value::Null);
    let field_channel = stack
        .get("fieldChannel")
        .and_then(Value::as_str)
        .unwrap_or("");
    let field_scale = model.scale_name(field_channel, false).unwrap_or_default();

    let stack_field = |prefix: Option<&str>, suffix: Option<&str>, expr: Option<&str>| -> String {
        model.vg_field_with(
            field_channel,
            &crate::channeldef::FieldRefOption {
                prefix: prefix.map(str::to_string),
                suffix: suffix.map(str::to_string),
                expr: expr.map(str::to_string),
                ..Default::default()
            },
        )
    };

    let stack_field_group = |func: &str, expr: &str| -> String {
        let fields = [
            stack_field(Some("min"), Some("start"), Some(expr)),
            stack_field(Some("max"), Some("start"), Some(expr)),
            stack_field(Some("min"), Some("end"), Some(expr)),
            stack_field(Some("max"), Some("end"), Some(expr)),
        ];
        format!(
            "{func}({})",
            fields
                .iter()
                .map(|f| format!("scale('{field_scale}',{f})"))
                .collect::<Vec<_>>()
                .join(",")
        )
    };

    let mark_update = mark
        .get("encode")
        .and_then(|e| e.get("update"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let (mut group_update, inner_group_update, new_mark_update) = if field_channel == "x" {
        let mut group = pick(&mark_update, &["y", "yc", "y2", "height"]);
        for prop in VG_CORNERRADIUS_CHANNELS {
            if let Some(v) = mark_update.get(prop) {
                group.insert(prop.to_string(), v.clone());
            }
        }
        group.insert("x".to_string(), json!({"signal": stack_field_group("min", "datum")}));
        group.insert("x2".to_string(), json!({"signal": stack_field_group("max", "datum")}));
        group.insert("clip".to_string(), json!({"value": true}));

        let inner = json!({
            "x": {"field": {"group": "x"}, "mult": -1},
            "height": {"field": {"group": "height"}},
        });

        let mut mark_update = omit(&mark_update, &["y", "yc", "y2"]);
        mark_update.insert("height".to_string(), json!({"field": {"group": "height"}}));
        (group, inner, mark_update)
    } else {
        let mut group = pick(&mark_update, &["x", "xc", "x2", "width"]);
        group.insert("y".to_string(), json!({"signal": stack_field_group("min", "datum")}));
        group.insert("y2".to_string(), json!({"signal": stack_field_group("max", "datum")}));
        group.insert("clip".to_string(), json!({"value": true}));

        let inner = json!({
            "y": {"field": {"group": "y"}, "mult": -1},
            "width": {"field": {"group": "width"}},
        });

        let mut mark_update = omit(&mark_update, &["x", "xc", "x2"]);
        mark_update.insert("width".to_string(), json!({"field": {"group": "width"}}));
        (group, inner, mark_update)
    };

    for key in VG_CORNERRADIUS_CHANNELS {
        let config_value = get_mark_config(key, &model.unit.mark_def, &model.config, None);
        if let Some(v) = mark_update.get(key).cloned() {
            group_update.insert(key.to_string(), v);
            // Remove from the mark update.
            if let Some(map) = mark.as_object_mut() {
                if let Some(update) = map
                    .get_mut("encode")
                    .and_then(|e| e.get_mut("update"))
                    .and_then(Value::as_object_mut)
                {
                    update.remove(key);
                }
            }
        } else if let Some(config_value) = config_value.as_ref().filter(|v| !v.is_null()) {
            group_update.insert(key.to_string(), signal_or_value_ref(config_value));
        }
        if let Some(config_value) = config_value.as_ref().filter(|v| !v.is_null()) {
            let _ = config_value;
            if let Some(map) = mark.as_object_mut() {
                if let Some(update) = map
                    .get_mut("encode")
                    .and_then(|e| e.get_mut("update"))
                    .and_then(Value::as_object_mut)
                {
                    update.insert(key.to_string(), json!({"value": 0}));
                }
            }
        }
    }

    let fields = (
        stack_field(Some("min"), Some("start"), None),
        stack_field(Some("max"), Some("start"), None),
        stack_field(Some("min"), Some("end"), None),
        stack_field(Some("max"), Some("end"), None),
    );
    if let Value::Object(extra) = corner_radius_end_for_stacked_bar(
        model,
        corner_radius_end,
        &field_scale,
        (fields.0.as_str(), fields.1.as_str(), fields.2.as_str(), fields.3.as_str()),
    ) {
        for (k, v) in extra {
            group_update.insert(k, v);
        }
    }

    let mut groupby: Vec<String> = Vec::new();
    if let Some(channels) = stack.get("groupbyChannels").and_then(Value::as_array) {
        for channel in channels {
            let channel = channel.as_str().unwrap_or("");
            if let Some(group_by_field) = model.field_def(channel) {
                let field = crate::channeldef::vg_field(&group_by_field);
                if !field.is_empty() {
                    groupby.push(field);
                }
                if group_by_field.get("bin").map(|v| !v.is_null()).unwrap_or(false)
                    || group_by_field.get("timeUnit").map(|v| !v.is_null()).unwrap_or(false)
                {
                    groupby.push(crate::channeldef::vg_field_with(
                        &group_by_field,
                        &crate::channeldef::FieldRefOption {
                            bin_suffix: Some("end".to_string()),
                            ..Default::default()
                        },
                    ));
                }
            }
        }
    }

    let stroke_properties = [
        "stroke",
        "strokeWidth",
        "strokeJoin",
        "strokeCap",
        "strokeDash",
        "strokeDashOffset",
        "strokeMiterLimit",
        "strokeOpacity",
    ];
    for prop in stroke_properties {
        if let Some(v) = mark_update.get(prop).cloned() {
            group_update.insert(prop.to_string(), v);
        } else if let Some(config_value) =
            get_mark_config(prop, &model.unit.mark_def, &model.config, None)
        {
            group_update.insert(prop.to_string(), signal_or_value_ref(&config_value));
        }
    }

    if group_update.get("stroke").map(|v| !v.is_null()).unwrap_or(false) {
        group_update.insert("strokeForeground".to_string(), json!({"value": true}));
        group_update.insert("strokeOffset".to_string(), json!({"value": 0}));
    }

    if let Some(map) = mark
        .get_mut("encode")
        .and_then(|e| e.get_mut("update"))
        .and_then(Value::as_object_mut)
    {
        for key in if field_channel == "x" {
            &["y", "yc", "y2"][..]
        } else {
            &["x", "xc", "x2"][..]
        } {
            map.remove(*key);
        }
        for (k, v) in new_mark_update {
            if VG_CORNERRADIUS_CHANNELS.contains(&k.as_str()) {
                continue;
            }
            map.insert(k, v);
        }
    }

    let data = model.get_data_name("main");
    vec![json!({
        "type": "group",
        "from": {
            "facet": {
                "data": data,
                "name": format!("{STACK_GROUP_PREFIX}{data}"),
                "groupby": groupby,
                "aggregate": {
                    "fields": [
                        stack_field(None, Some("start"), None),
                        stack_field(None, Some("start"), None),
                        stack_field(None, Some("end"), None),
                        stack_field(None, Some("end"), None),
                    ],
                    "ops": ["min", "max", "min", "max"],
                },
            },
        },
        "encode": {"update": group_update},
        "marks": [{
            "type": "group",
            "encode": {"update": inner_group_update},
            "marks": [mark],
        }],
    })]
}

pub fn get_sort(model: &Model) -> Option<Value> {
    let encoding = &model.unit.encoding;
    let mark_def = &model.unit.mark_def;
    let config = &model.config;
    let mark = model.mark_type();
    let order = encoding.get("order").cloned();

    // `{value: null}` is normalized to an empty array by this crate's encoding
    // initializer; an empty order list means "do not order".
    if order
        .as_ref()
        .and_then(Value::as_array)
        .map(|a| a.is_empty())
        .unwrap_or(false)
    {
        return None;
    }

    let order_is_null_false_value = order
        .as_ref()
        .filter(|o| !o.is_array())
        .filter(|o| o.as_object().map(|m| m.contains_key("value")).unwrap_or(false))
        .map(|o| is_null_or_false(&o["value"]))
        .unwrap_or(false);
    let order_absent = order.as_ref().map(|o| o.is_null()).unwrap_or(true);
    let mark_order_null_false = get_mark_prop_or_config("order", mark_def, config, None, false)
        .map(|v| is_null_or_false(&v))
        .unwrap_or(false);

    if order_is_null_false_value || (order_absent && mark_order_null_false) {
        return None;
    } else if order
        .as_ref()
        .map(|o| o.is_array() || is_field_def(o))
        .unwrap_or(false)
        && stack_of(model).is_none()
    {
        let order = order.unwrap();
        let defs = array(&order);
        let mut fields = Vec::new();
        let mut orders = Vec::new();
        for def in defs {
            fields.push(Value::String(crate::channeldef::vg_field_with(
                &def,
                &crate::channeldef::FieldRefOption {
                    expr: Some("datum".to_string()),
                    ..Default::default()
                },
            )));
            orders.push(
                def.get("sort")
                    .cloned()
                    .unwrap_or_else(|| Value::String("ascending".to_string())),
            );
        }
        return Some(json!({"field": fields, "order": orders}));
    } else if crate::mark::is_path_mark(mark.as_str()) {
        let orient = mark_def.get("orient").and_then(Value::as_str).unwrap_or("");
        let dimension_channel = if orient == "horizontal" { "y" } else { "x" };
        if model
            .field_def(dimension_channel)
            .map(|f| is_field_def(&f))
            .unwrap_or(false)
        {
            return Some(json!({"field": dimension_channel}));
        }
    }
    None
}

fn scale_clip(model: &Model) -> Option<Value> {
    let x_scale = model.get_scale_component("x");
    let y_scale = model.get_scale_component("y");
    let has_selection_extent = x_scale
        .as_ref()
        .and_then(|s| s.get("selectionExtent"))
        .is_some()
        || y_scale
            .as_ref()
            .and_then(|s| s.get("selectionExtent"))
            .is_some();
    if has_selection_extent {
        Some(json!(true))
    } else {
        None
    }
}

fn projection_clip(model: &Model) -> Option<Value> {
    let projection = &model.unit.specified_projection;
    if projection.is_null() {
        return None;
    }
    let fit = !(projection.get("scale").map(|v| !v.is_null()).unwrap_or(false)
        || projection
            .get("translate")
            .map(|v| !v.is_null())
            .unwrap_or(false));
    if !fit {
        Some(json!(true))
    } else {
        None
    }
}

fn count_selections(model: &Model) -> usize {
    let local = if model.unit.selection.is_empty() {
        model
            .component
            .selection
            .as_object()
            .map(|m| m.len())
            .unwrap_or(0)
    } else {
        model.unit.selection.len()
    };
    local + model.children.iter().map(count_selections).sum::<usize>()
}

fn interactive_flag(model: &Model) -> Option<Value> {
    let unit_count = if model.is_unit_model() {
        model.unit.selection.len()
    } else {
        0
    };
    let parent_count = count_selections(model);
    if parent_count + unit_count == 0 {
        return None;
    }

    let encoding_has_tooltip = model
        .unit
        .encoding
        .get("tooltip")
        .map(|v| !v.is_null())
        .unwrap_or(false);
    let mark_has_tooltip = model
        .unit
        .mark_def
        .get("tooltip")
        .map(|v| !v.is_null() && v != &Value::Bool(false))
        .unwrap_or(false);

    Some(json!({
        "interactive": unit_count > 0
            || model.mark_type() == "geoshape"
            || encoding_has_tooltip
            || mark_has_tooltip,
    }))
}

fn get_mark_group(model: &Model, from_prefix: &str) -> Value {
    let mark = model.mark_type();
    let mark_def = &model.unit.mark_def;
    let encoding = &model.unit.encoding;
    let config = &model.config;

    let clip = get_first_defined(&[
        mark_def.get("clip"),
        scale_clip(model).as_ref(),
        projection_clip(model).as_ref(),
    ])
    .cloned()
    .map(|clip| crate::compile_common::signal_ref_or_value(&clip));

    let style = get_styles(mark_def);
    let key = encoding
        .get("key")
        .and_then(|k| k.get("field"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let sort = get_sort(model);
    let interactive = interactive_flag(model);

    let aria = get_mark_prop_or_config("aria", mark_def, config, None, false);

    let compiler = mark_compiler(&mark).unwrap_or_else(|| {
        panic!("Unsupported mark type: {mark}");
    });

    let post_encoding_transform = compiler
        .post_encoding_transform
        .map(|f| f(model));

    let mut group = Map::new();
    group.insert(
        "name".to_string(),
        Value::String(model.get_name("marks")),
    );
    group.insert(
        "type".to_string(),
        Value::String(compiler.vg_mark.to_string()),
    );
    if let Some(clip) = clip.filter(crate::channeldef_support::truthy) {
        group.insert("clip".to_string(), clip);
    }
    if !style.is_empty() {
        group.insert(
            "style".to_string(),
            Value::Array(style.into_iter().map(Value::String).collect()),
        );
    }
    if let Some(key) = key {
        group.insert("key".to_string(), Value::String(key));
    }
    if let Some(sort) = sort {
        group.insert("sort".to_string(), sort);
    }
    let has_interactive = interactive.is_some();
    if let Some(Value::Object(map)) = interactive {
        for (k, v) in map {
            group.insert(k, v);
        }
    }
    if aria.as_ref().and_then(Value::as_bool) == Some(false) {
        group.insert("aria".to_string(), Value::Bool(false));
    }
    group.insert(
        "from".to_string(),
        json!({"data": format!("{from_prefix}{}", model.get_data_name("main"))}),
    );
    let mut update = (compiler.encode_entry)(model);
    // Set pointer cursor for point selections that are not bound (and are not
    // triggered on pointerover).
    if has_interactive {
        let has_clickable_point = crate::compile::selection::component::ordered_components(model)
            .iter()
            .any(|sel| {
                sel.type_ == "point"
                    && !crate::compile::selection::component::truthy(sel.bind.as_ref())
                    && sel.extra.get("on").and_then(Value::as_str) != Some("pointerover")
            });
        if has_clickable_point {
            if let Some(map) = update.as_object_mut() {
                map.entry("cursor".to_string())
                    .or_insert_with(|| json!({"value": "pointer"}));
            }
        }
    }
    group.insert("encode".to_string(), json!({"update": update}));
    if let Some(transform) = post_encoding_transform {
        group.insert("transform".to_string(), transform);
    }
    Value::Object(group)
}

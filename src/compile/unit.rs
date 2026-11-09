//! Port of `src/compile/unit.ts`.

use serde_json::{Map, Value};

use crate::channel::{POSITION_SCALE_CHANNELS, SCALE_CHANNELS};
use crate::channeldef::{
    get_field_or_datum_def, is_field_or_datum_def,
};
use crate::encoding::init_encoding;
use crate::mark::is_mark_def;
use crate::util::keys;

use super::layoutsize::init::init_layout_size;
use super::mark::init::{default_filled, init_markdef};
use super::model::{Model, SpecType, UnitData};
use super::support::{self, is_graticule_generator};

impl Model {
    /// `new UnitModel(spec, parent, parentGivenName, parentGivenSize, config)`
    pub fn new_unit(
        spec: &Value,
        parent: Option<&Model>,
        parent_given_name: &str,
        parent_given_size: Option<&Value>,
        config: &Value,
    ) -> Model {
        let view = if support::is_frame_mixins(spec) {
            spec.get("view").cloned()
        } else {
            None
        };

        let mut model = Model::new_common(
            spec,
            SpecType::Unit,
            parent,
            parent_given_name,
            config,
            view.map(|v| support::replace_expr_ref(&v)),
        );

        let original_mark = spec.get("mark").cloned().unwrap_or(Value::Null);
        let mut mark_def = if is_mark_def(&original_mark) {
            original_mark.clone()
        } else {
            serde_json::json!({"type": original_mark})
        };
        if !mark_def.is_object() {
            mark_def = serde_json::json!({"type": Value::Null});
        }
        let mark_name = mark_def
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        if mark_def.get("filled").is_none() {
            let graticule = spec
                .get("data")
                .map(is_graticule_generator)
                .unwrap_or(false);
            let filled = default_filled(&mark_def, config, graticule);
            mark_def["filled"] = Value::Bool(filled);
        }
        let filled = mark_def.get("filled").and_then(Value::as_bool);

        let mark_enum = mark_name
            .parse::<crate::mark::Mark>()
            .unwrap_or(crate::mark::Mark::Point);
        let encoding = init_encoding(
            &spec.get("encoding").cloned().unwrap_or(serde_json::json!({})),
            mark_enum,
            filled,
            config,
        );

        let mark_def = init_markdef(&mark_def, &encoding, config);

        let size_input = if support::is_frame_mixins(spec) {
            let mut size = parent_given_size
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            if let Some(width) = spec.get("width") {
                if !width.is_null() {
                    size.insert("width".to_string(), width.clone());
                }
            }
            if let Some(height) = spec.get("height") {
                if !height.is_null() {
                    size.insert("height".to_string(), height.clone());
                }
            }
            Value::Object(size)
        } else {
            parent_given_size.cloned().unwrap_or(serde_json::json!({}))
        };
        let size = init_layout_size(&encoding, &size_input);

        let specified_scales = init_scales(&encoding, config);
        let specified_axes = init_axes(&encoding, config);
        let specified_legends = init_legends(&encoding);

        let stack = crate::stack::stack(&mark_def, &encoding).map(|props| {
            serde_json::json!({
                "groupbyChannels": props.groupby_channels,
                "groupbyFields": props.groupby_fields.iter().cloned().collect::<Vec<_>>(),
                "fieldChannel": props.field_channel,
                "stackBy": props.stack_by.iter().map(|s| serde_json::json!({
                    "channel": s.channel,
                    "fieldDef": s.field_def,
                })).collect::<Vec<_>>(),
                "offset": props.offset.as_str(),
                "impute": props.impute,
            })
        }).unwrap_or(Value::Null);

        let mut encoding = encoding;
        align_stack_order_with_color_domain(
            &mut encoding,
            &mark_def,
            &stack,
            &mut model.transforms,
        );

        let selection = spec
            .get("params")
            .and_then(Value::as_array)
            .map(|params| {
                params
                    .iter()
                    .filter(|p| p.get("select").is_some())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();

        model.unit = UnitData {
            mark_def,
            encoding,
            specified_scales,
            specified_axes,
            specified_legends,
            specified_projection: spec.get("projection").cloned().unwrap_or(Value::Null),
            selection,
            stack,
        };
        model.size = size;
        model
    }

    pub fn specified_scale(&self, channel: &str) -> Option<&Value> {
        self.unit.specified_scales.get(channel)
    }

    pub fn has_projection(&self) -> bool {
        let encoding = match self.encoding() {
            Some(e) => e,
            None => return false,
        };
        let is_geo_shape_mark = self.mark_type() == crate::mark::GEOSHAPE.as_str();
        let has_geo_position = crate::channel::GEOPOSITION_CHANNELS
            .iter()
            .any(|channel| is_field_or_datum_def(&encoding[*channel]));
        is_geo_shape_mark || has_geo_position
    }
}

fn init_scales(encoding: &Value, _config: &Value) -> std::collections::BTreeMap<String, Value> {
    let mut scales = std::collections::BTreeMap::new();
    for channel in SCALE_CHANNELS {
        if let Some(field_or_datum_def) = get_field_or_datum_def(&encoding[channel]) {
            let scale = field_or_datum_def
                .get("scale")
                .cloned()
                .unwrap_or(Value::Object(Map::new()));
            scales.insert(channel.to_string(), init_scale(&scale));
        }
    }
    scales
}

fn init_scale(scale: &Value) -> Value {
    // `replaceExprRef(scale)` replaces ExprRefs one level deep (each property).
    let mut scale_internal = if let Some(map) = scale.as_object() {
        let mut out = Map::new();
        for (k, v) in map {
            out.insert(k.clone(), support::signal_ref_or_value(v));
        }
        Value::Object(out)
    } else {
        support::replace_expr_ref(scale)
    };
    if !scale_internal.is_object() {
        return scale_internal;
    }
    for key in ["domain", "range"] {
        if let Some(Value::Array(items)) = scale.get(key) {
            let mapped = items
                .iter()
                .map(support::signal_ref_or_value)
                .collect();
            scale_internal[key] = Value::Array(mapped);
        }
    }
    scale_internal
}

fn init_axes(encoding: &Value, config: &Value) -> Value {
    let mut axes = Map::new();
    for channel in POSITION_SCALE_CHANNELS {
        let channel_def = &encoding[channel];
        let relevant = is_field_or_datum_def(channel_def)
            || (channel == "x" && is_field_or_datum_def(&encoding["x2"]))
            || (channel == "y" && is_field_or_datum_def(&encoding["y2"]));
        if relevant {
            let axis_spec = if is_field_or_datum_def(channel_def) {
                channel_def.get("axis").cloned()
            } else {
                None
            };
            if let Some(axis_spec) = axis_spec {
                axes.insert(
                    channel.to_string(),
                    init_axis(&axis_spec, config),
                );
            }
        }
    }
    Value::Object(axes)
}

fn init_axis(axis: &Value, _config: &Value) -> Value {
    let mut axis_internal = Map::new();
    for prop in keys(axis) {
        let val = axis.get(&prop).cloned().unwrap_or(Value::Null);
        axis_internal.insert(prop, support::replace_expr_ref(&val));
    }
    Value::Object(axis_internal)
}

fn init_legends(encoding: &Value) -> Value {
    let mut legends = Map::new();
    for channel in crate::channel::NONPOSITION_SCALE_CHANNELS {
        if let Some(field_or_datum_def) = get_field_or_datum_def(&encoding[channel]) {
            if crate::channel::support_legend(channel) {
                if let Some(legend) = field_or_datum_def.get("legend") {
                    legends.insert(channel.to_string(), support::replace_expr_ref(legend));
                }
            }
        }
    }
    Value::Object(legends)
}

/// Port of `UnitModel.alignStackOrderWithColorDomain`.
///
/// If this unit lacks an order encoding but does contain a color domain, add a
/// transform and an order encoding that aligns the stack order with the color
/// domain.
fn align_stack_order_with_color_domain(
    encoding: &mut Value,
    mark_def: &Value,
    stack: &Value,
    transforms: &mut Vec<Value>,
) {
    let order = encoding.get("order").cloned().unwrap_or(Value::Null);
    let color_field = match encoding
        .get("fill")
        .filter(|v| crate::channeldef_support::truthy(v))
    {
        Some(fill) => fill.clone(),
        None => encoding.get("color").cloned().unwrap_or(Value::Null),
    };
    let color_encoding = if crate::channeldef::is_field_def(&color_field) {
        Some(color_field)
    } else {
        None
    };
    let color_encoding = match color_encoding {
        Some(c) => c,
        None => return,
    };
    let field = match color_encoding.get("field").and_then(Value::as_str) {
        Some(f) => f.to_string(),
        None => return,
    };
    let domain = color_encoding
        .get("scale")
        .and_then(|s| s.get("domain"))
        .cloned()
        .unwrap_or(Value::Null);
    let color_type = color_encoding.get("type").and_then(Value::as_str);

    let offset_channel = if crate::channeldef_support::truthy(
        encoding.get("xOffset").unwrap_or(&Value::Null),
    ) {
        "xOffset"
    } else {
        "yOffset"
    };
    let offset_def = encoding.get(offset_channel).cloned().unwrap_or(Value::Null);
    let offset_is_field = crate::channeldef::is_field_def(&offset_def);
    let order_absent = !crate::channeldef_support::truthy(&order);
    let order_field_name = format!("_{field}_sort_index");

    if !order_absent || !domain.is_array() || color_type != Some("nominal") {
        return;
    }

    if offset_is_field && !crate::channeldef_support::truthy(&offset_def["sort"]) {
        if let Some(map) = encoding.as_object_mut() {
            if let Some(def) = map.get_mut(offset_channel).and_then(Value::as_object_mut) {
                def.insert("sort".to_string(), domain);
            }
        }
    } else {
        // Align stacked chart order with the color domain.
        if stack.is_null() {
            return;
        }
        let order_expression = format!(
            "indexof({}, datum['{}'])",
            crate::util::string_value(&domain),
            field
        );
        let sort = if mark_def.get("orient").and_then(Value::as_str) == Some("horizontal") {
            "ascending"
        } else {
            "descending"
        };
        transforms.push(serde_json::json!({
            "calculate": order_expression,
            "as": order_field_name,
        }));
        if let Some(map) = encoding.as_object_mut() {
            map.insert(
                "order".to_string(),
                serde_json::json!({
                    "field": order_field_name,
                    "type": "quantitative",
                    "sort": sort,
                }),
            );
        }
    }
}

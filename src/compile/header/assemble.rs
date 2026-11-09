//! Port of `src/compile/header/assemble.ts`.

use serde_json::{json, Map, Value};

use crate::channel::FACET_CHANNELS;
use crate::channeldef::{vg_field_with, FieldRefOption};
use crate::compile::axis::properties::{default_label_align, default_label_baseline};
use crate::compile::mark::encode::format::format_signal_ref;
use crate::compile::model::Model;
use crate::compile::support::is_facet_mapping;
use crate::normalize_support::is_sort_field;
use crate::util::{normalize_angle, replace_all};

use super::common::{get_header_channel, get_header_properties, get_header_property};
use super::component::{HEADER_CHANNELS, HEADER_TYPES};

const HEADER_TITLE_PROPERTIES_MAP: &[(&str, &str)] = &[
    ("titleAlign", "align"),
    ("titleAnchor", "anchor"),
    ("titleAngle", "angle"),
    ("titleBaseline", "baseline"),
    ("titleColor", "color"),
    ("titleFont", "font"),
    ("titleFontSize", "fontSize"),
    ("titleFontStyle", "fontStyle"),
    ("titleFontWeight", "fontWeight"),
    ("titleLimit", "limit"),
    ("titleLineHeight", "lineHeight"),
    ("titleOrient", "orient"),
    ("titlePadding", "offset"),
];

const HEADER_LABEL_PROPERTIES_MAP: &[(&str, &str)] = &[
    ("labelAlign", "align"),
    ("labelAnchor", "anchor"),
    ("labelAngle", "angle"),
    ("labelBaseline", "baseline"),
    ("labelColor", "color"),
    ("labelFont", "font"),
    ("labelFontSize", "fontSize"),
    ("labelFontStyle", "fontStyle"),
    ("labelFontWeight", "fontWeight"),
    ("labelLimit", "limit"),
    ("labelLineHeight", "lineHeight"),
    ("labelOrient", "orient"),
    ("labelPadding", "offset"),
];

fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(_) => true,
    }
}

fn layout_header(model: &Model, channel: &str) -> Map<String, Value> {
    model
        .component
        .layout_headers
        .get(channel)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn header_field_def(lh: &Map<String, Value>) -> Option<Value> {
    lh.get("facetFieldDef")
        .filter(|v| !v.is_null())
        .cloned()
}

fn facet_header(field_def: Option<&Value>) -> Option<&Value> {
    field_def.and_then(|fd| fd.get("header"))
}

fn insert_value(map: &mut Map<String, Value>, key: &str, value: Option<&Value>) {
    if let Some(value) = value {
        map.insert(key.to_string(), value.clone());
    }
}

/// `defaultHeaderGuideBaseline`.
fn default_header_guide_baseline(angle: Option<&Value>, channel: &str) -> Option<Value> {
    let baseline = default_label_baseline(
        angle,
        Some(&json!(if channel == "row" { "left" } else { "top" })),
        if channel == "row" { "y" } else { "x" },
        true,
    )?;
    if baseline.is_null() {
        None
    } else {
        Some(baseline)
    }
}

/// `defaultHeaderGuideAlign`.
fn default_header_guide_align(
    header_channel: &str,
    angle: Option<&Value>,
    anchor: Option<&Value>,
) -> Option<Value> {
    match anchor.and_then(Value::as_str) {
        Some("start") => return Some(json!("left")),
        Some("end") => return Some(json!("right")),
        _ => {}
    }

    let align = default_label_align(
        angle,
        Some(&json!(if header_channel == "row" { "left" } else { "top" })),
        if header_channel == "row" { "y" } else { "x" },
    )?;
    if align.is_null() {
        None
    } else {
        Some(align)
    }
}

fn assemble_header_properties(
    config: &Value,
    field_def: Option<&Value>,
    channel: &str,
    properties: &[(&str, &str)],
) -> Map<String, Value> {
    let header = facet_header(field_def);
    let mut props = Map::new();
    for (prop, vg_prop) in properties {
        if let Some(value) = get_header_property(prop, header, config, channel) {
            props.insert((*vg_prop).to_string(), value);
        }
    }
    props
}

/// `assembleTitleGroup`.
pub fn assemble_title_group(model: &Model, channel: &str) -> Option<Value> {
    let lh = layout_header(model, channel);
    let title = lh.get("title")?.clone();
    let field_def = header_field_def(&lh);
    let config = &model.config;

    let props = get_header_properties(
        &["titleAnchor", "titleAngle", "titleOrient"],
        facet_header(field_def.as_ref()),
        config,
        channel,
    );
    let title_anchor = props.get("titleAnchor");
    let title_angle = normalize_angle(props.get("titleAngle").and_then(Value::as_f64));
    let title_orient = props.get("titleOrient");
    let header_channel = get_header_channel(channel, title_orient);

    let angle_value = title_angle.map(|a| json!(a));

    let mut title_obj = Map::new();
    title_obj.insert("text".to_string(), title);
    if channel == "row" {
        title_obj.insert("orient".to_string(), json!("left"));
    }
    title_obj.insert("style".to_string(), json!("guide-title"));
    insert_value(
        &mut title_obj,
        "baseline",
        default_header_guide_baseline(angle_value.as_ref(), header_channel).as_ref(),
    );
    insert_value(
        &mut title_obj,
        "align",
        default_header_guide_align(header_channel, angle_value.as_ref(), title_anchor).as_ref(),
    );
    for (k, v) in assemble_header_properties(
        config,
        field_def.as_ref(),
        channel,
        HEADER_TITLE_PROPERTIES_MAP,
    ) {
        title_obj.insert(k, v);
    }

    Some(json!({
        "name": format!("{channel}-title"),
        "type": "group",
        "role": format!("{header_channel}-title"),
        "title": Value::Object(title_obj),
    }))
}

/// `assembleHeaderGroups`.
pub fn assemble_header_groups(model: &Model, channel: &str) -> Vec<Value> {
    let lh = layout_header(model, channel);
    let mut groups = Vec::new();
    for header_type in HEADER_TYPES {
        if let Some(components) = lh.get(header_type).and_then(Value::as_array) {
            for header_component in components {
                if let Some(group) =
                    assemble_header_group(model, channel, header_type, &lh, header_component)
                {
                    groups.push(group);
                }
            }
        }
    }
    groups
}

fn get_sort(field_def: &Value, channel: &str) -> Value {
    let sort = field_def.get("sort").cloned().unwrap_or(Value::Null);
    if is_sort_field(&sort) {
        let order = sort
            .get("order")
            .filter(|v| !v.is_null())
            .cloned()
            .unwrap_or(json!("ascending"));
        return json!({
            "field": vg_field_with(&sort, &FieldRefOption {
                expr: Some("datum".to_string()),
                ..Default::default()
            }),
            "order": order,
        });
    } else if sort.is_array() {
        return json!({
            "field": vg_field_with(field_def, &FieldRefOption {
                expr: Some("datum".to_string()),
                prefix: Some(channel.to_string()),
                suffix: Some("sort_index".to_string()),
                ..Default::default()
            }),
            "order": "ascending",
        });
    }

    let order = if sort.is_null() {
        json!("ascending")
    } else {
        sort
    };
    json!({
        "field": vg_field_with(field_def, &FieldRefOption {
            expr: Some("datum".to_string()),
            ..Default::default()
        }),
        "order": order,
    })
}

/// `assembleLabelTitle`.
pub fn assemble_label_title(field_def: &Value, channel: &str, config: &Value) -> Value {
    let header = facet_header(Some(field_def));
    let props = get_header_properties(
        &[
            "format",
            "formatType",
            "labelAngle",
            "labelAnchor",
            "labelOrient",
            "labelExpr",
        ],
        header,
        config,
        channel,
    );

    let format = props.get("format");
    let format_type = props.get("formatType").and_then(Value::as_str);
    let label_angle = props.get("labelAngle");
    let label_anchor = props.get("labelAnchor");
    let label_orient = props.get("labelOrient");
    let label_expr = props.get("labelExpr").and_then(Value::as_str);

    let title_text_expr = format_signal_ref(
        field_def,
        format,
        format_type,
        Some("parent"),
        false,
        config,
    )
    .and_then(|v| v.get("signal").and_then(Value::as_str).map(str::to_string))
    .unwrap_or_default();
    let header_channel = get_header_channel(channel, label_orient);

    let text = if let Some(label_expr) = label_expr {
        let expr = replace_all(label_expr, "datum.label", &title_text_expr);
        let value = vg_field_with(
            field_def,
            &FieldRefOption {
                expr: Some("parent".to_string()),
                ..Default::default()
            },
        );
        replace_all(&expr, "datum.value", &value)
    } else {
        title_text_expr
    };

    let mut out = Map::new();
    out.insert("text".to_string(), json!({"signal": text}));
    if channel == "row" {
        out.insert("orient".to_string(), json!("left"));
    }
    out.insert("style".to_string(), json!("guide-label"));
    out.insert("frame".to_string(), json!("group"));
    insert_value(
        &mut out,
        "baseline",
        default_header_guide_baseline(label_angle, header_channel).as_ref(),
    );
    insert_value(
        &mut out,
        "align",
        default_header_guide_align(header_channel, label_angle, label_anchor).as_ref(),
    );
    for (k, v) in assemble_header_properties(
        config,
        Some(field_def),
        channel,
        HEADER_LABEL_PROPERTIES_MAP,
    ) {
        out.insert(k, v);
    }

    Value::Object(out)
}

/// `assembleHeaderGroup`.
fn assemble_header_group(
    model: &Model,
    channel: &str,
    header_type: &str,
    lh: &Map<String, Value>,
    header_component: &Value,
) -> Option<Value> {
    let field_def = header_field_def(lh);
    let config = &model.config;

    let labels = header_component
        .get("labels")
        .map(is_truthy)
        .unwrap_or(false);

    let mut title = None;
    if let Some(field_def) = field_def.as_ref() {
        if labels {
            let props = get_header_properties(
                &["labelOrient"],
                facet_header(Some(field_def)),
                config,
                channel,
            );
            let label_orient = props.get("labelOrient").and_then(Value::as_str);
            let aligns = match channel {
                "row" => !matches!(label_orient, Some("top") | Some("bottom")),
                "column" => !matches!(label_orient, Some("left") | Some("right")),
                _ => false,
            };
            if aligns {
                title = Some(assemble_label_title(field_def, channel, config));
            }
        }
    }

    let is_facet_without_row_col =
        model.is_facet_model() && !is_facet_mapping(&model.facet_data.facet);

    let axes = header_component
        .get("axes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let has_axes = !axes.is_empty();

    if title.is_none() && !has_axes {
        return None;
    }

    let size_channel = if channel == "row" { "height" } else { "width" };

    let mut group = Map::new();
    group.insert(
        "name".to_string(),
        json!(model.get_name(&format!("{channel}_{header_type}"))),
    );
    group.insert("type".to_string(), json!("group"));
    group.insert(
        "role".to_string(),
        json!(format!("{channel}-{header_type}")),
    );

    if let Some(field_def) = field_def.as_ref() {
        group.insert(
            "from".to_string(),
            json!({"data": model.get_name(&format!("{channel}_domain"))}),
        );
        group.insert("sort".to_string(), get_sort(field_def, channel));
    }

    if has_axes && is_facet_without_row_col {
        group.insert(
            "from".to_string(),
            json!({"data": model.get_name(&format!("facet_domain_{channel}"))}),
        );
    }

    if let Some(title) = title {
        group.insert("title".to_string(), title);
    }

    if let Some(size_signal) = header_component.get("sizeSignal").filter(|v| !v.is_null()) {
        group.insert(
            "encode".to_string(),
            json!({"update": {size_channel: size_signal.clone()}}),
        );
    }

    if has_axes {
        group.insert("axes".to_string(), Value::Array(axes));
    }

    Some(Value::Object(group))
}

/// `assembleHeaderMarks`.
pub fn assemble_header_marks(model: &Model) -> Vec<Value> {
    let mut marks = Vec::new();

    for channel in FACET_CHANNELS {
        let lh = layout_header(model, channel);
        if lh.get("title").map(is_truthy).unwrap_or(false) {
            if let Some(group) = assemble_title_group(model, channel) {
                marks.push(group);
            }
        }
    }

    for channel in HEADER_CHANNELS {
        marks.extend(assemble_header_groups(model, channel));
    }

    marks
}

/// `assembleLayoutTitleBand`.
pub fn assemble_layout_title_band(model: &Model) -> Option<Value> {
    let config = &model.config;
    let mut title_band = Map::new();

    for channel in FACET_CHANNELS {
        let lh = layout_header(model, channel);
        let Some(field_def) = header_field_def(&lh) else {
            continue;
        };
        let props = get_header_properties(
            &["titleAnchor", "titleOrient"],
            facet_header(Some(&field_def)),
            config,
            channel,
        );
        let title_anchor = props.get("titleAnchor").and_then(Value::as_str);
        let title_orient = props.get("titleOrient");
        let header_channel = get_header_channel(channel, title_orient);

        let band = match (header_channel, title_anchor) {
            ("column", Some("start")) => Some(0),
            ("column", Some("end")) => Some(1),
            ("row", Some("start")) => Some(1),
            ("row", Some("end")) => Some(0),
            _ => None,
        };
        if let Some(band) = band {
            title_band.insert(header_channel.to_string(), json!(band));
        }
    }

    if title_band.is_empty() {
        None
    } else {
        Some(Value::Object(title_band))
    }
}

/// `getHeaderLayoutMixins` (part of `FacetModel.assembleDefaultLayout`).
pub fn header_layout_mixins(model: &Model) -> Map<String, Value> {
    let mut mixins = Map::new();
    let child_size_available = |size_type: &str| {
        model
            .children
            .first()
            .and_then(|child| child.component.layout_size.get(size_type))
            .map(|v| is_truthy(&v))
            .unwrap_or(false)
    };

    for channel in FACET_CHANNELS {
        let lh = layout_header(model, channel);
        let Some(field_def) = header_field_def(&lh) else {
            continue;
        };
        let config = &model.config;

        let title_orient = get_header_property(
            "titleOrient",
            facet_header(Some(&field_def)),
            config,
            channel,
        );
        if title_orient
            .as_ref()
            .and_then(Value::as_str)
            .map(|o| o == "right" || o == "bottom")
            .unwrap_or(false)
        {
            let header_channel = get_header_channel(channel, title_orient.as_ref());
            let title_anchor = mixins
                .entry("titleAnchor".to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            if let Some(obj) = title_anchor.as_object_mut() {
                obj.insert(header_channel.to_string(), json!("end"));
            }
        }

        for header_type in HEADER_TYPES {
            let has_header = lh
                .get(header_type)
                .and_then(Value::as_array)
                .map(|a| a.first().map(is_truthy).unwrap_or(false))
                .unwrap_or(false);
            if !has_header {
                continue;
            }

            let size_type = if channel == "row" { "height" } else { "width" };
            let band_type = if header_type == "header" {
                "headerBand"
            } else {
                "footerBand"
            };
            if channel != "facet" && !child_size_available(size_type) {
                let band = mixins
                    .entry(band_type.to_string())
                    .or_insert_with(|| Value::Object(Map::new()));
                if let Some(obj) = band.as_object_mut() {
                    obj.insert(channel.to_string(), json!(0.5));
                }
            }

            if lh.get("title").map(is_truthy).unwrap_or(false) {
                let offset = mixins
                    .entry("offset".to_string())
                    .or_insert_with(|| Value::Object(Map::new()));
                if let Some(obj) = offset.as_object_mut() {
                    obj.insert(
                        if channel == "row" {
                            "rowTitle".to_string()
                        } else {
                            "columnTitle".to_string()
                        },
                        json!(10),
                    );
                }
            }
        }
    }

    mixins
}

/// Expose the channel helper for tests.
pub fn header_channel_for(channel: &str, orient: Option<&Value>) -> &'static str {
    get_header_channel(channel, orient)
}

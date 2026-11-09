//! Port of `src/compile/header/parse.ts`.

use serde_json::{json, Map, Value};

use crate::channel::FACET_CHANNELS;
use crate::channeldef::{title as field_def_title, TitleOpt};
use crate::compile::axis::assemble::assemble_axis;
use crate::compile::axis::component::{axis_index_from_value, axis_index_to_value};
use crate::compile::data::dataflow::with_node_mut;
use crate::compile::data::facet::FacetNode;
use crate::compile::model::Model;
use crate::compile::support::{default_scale_resolve, Resolve};
use crate::compile_common::is_signal_ref;
use crate::util::get_first_defined;

use super::common::get_header_property;
use super::component::make_header_component;

/// `getHeaderType`.
pub fn get_header_type(orient: Option<&Value>) -> &'static str {
    match orient {
        Some(o) if is_signal_ref(o) => "header",
        Some(o) if o.as_str() == Some("top") || o.as_str() == Some("left") => "header",
        _ => "footer",
    }
}

/// `parseFacetHeaders`.
pub fn parse_facet_headers(model: &mut Model) {
    for channel in FACET_CHANNELS {
        parse_facet_header(model, channel);
    }

    merge_child_axis(model, "x");
    merge_child_axis(model, "y");

    sync_facet_node_model(model);
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

fn set_layout_header(model: &mut Model, channel: &str, value: Map<String, Value>) {
    if let Some(obj) = model.component.layout_headers.as_object_mut() {
        obj.insert(channel.to_string(), Value::Object(value));
    }
}

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

fn parse_facet_header(model: &mut Model, channel: &str) {
    let Some(field_def) = model.facet_data.facet.get(channel).cloned() else {
        return;
    };
    if !model.channel_has_field(channel) {
        return;
    }

    let config = model.config.clone();
    let header = field_def.get("header").cloned();
    let header_is_null = matches!(header, Some(Value::Null));

    let title_config = get_header_property("title", None, &config, channel);
    let include_default = title_config
        .as_ref()
        .map(is_truthy)
        .unwrap_or(true);

    let mut title = field_def_title(
        &field_def,
        &config,
        &TitleOpt {
            allow_disabling: true,
            include_default,
        },
    );

    // Merge the child's own header title (for nested facets).
    let child_title = model
        .children
        .first()
        .map(|child| {
            child
                .component
                .layout_headers
                .get(channel)
                .and_then(|h| h.get("title"))
                .cloned()
                .unwrap_or(Value::Null)
        })
        .unwrap_or(Value::Null);
    if is_truthy(&child_title) {
        let base = if let Some(arr) = title.as_array() {
            arr.iter()
                .map(|v| v.as_str().unwrap_or_default())
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            title.as_str().unwrap_or_default().to_string()
        };
        let child_text = if let Some(arr) = child_title.as_array() {
            arr.iter()
                .map(|v| v.as_str().unwrap_or_default())
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            child_title.as_str().unwrap_or_default().to_string()
        };
        title = Value::String(format!("{base} / {child_text}"));

        if let Some(child) = model.children.first_mut() {
            if let Some(obj) = child
                .component
                .layout_headers
                .get_mut(channel)
                .and_then(Value::as_object_mut)
            {
                obj.insert("title".to_string(), Value::Null);
            }
        }
    }

    let label_orient = get_header_property("labelOrient", header.as_ref(), &config, channel);
    let labels = if header_is_null {
        false
    } else {
        let explicit = header
            .as_ref()
            .and_then(|h| h.as_object())
            .and_then(|h| h.get("labels"))
            .cloned();
        let config_labels = config
            .get("header")
            .and_then(|h| h.get("labels"))
            .cloned();
        get_first_defined(&[explicit.as_ref(), config_labels.as_ref()])
            .map(is_truthy)
            .unwrap_or(true)
    };

    let header_type = if label_orient
        .as_ref()
        .and_then(Value::as_str)
        .map(|o| o == "bottom" || o == "right")
        .unwrap_or(false)
    {
        "footer"
    } else {
        "header"
    };

    let size_type = if channel == "row" { "height" } else { "width" };
    let size_signal = model
        .children
        .first()
        .filter(|child| {
            child
                .component
                .layout_size
                .get(size_type)
                .map(|v| is_truthy(&v))
                .unwrap_or(false)
        })
        .map(|child| child.get_size_signal_ref(size_type));

    let mut lh = Map::new();
    lh.insert(
        "title".to_string(),
        if header_is_null { Value::Null } else { title },
    );
    lh.insert("facetFieldDef".to_string(), field_def);
    let components = if channel == "facet" {
        Value::Array(Vec::new())
    } else {
        Value::Array(vec![make_header_component(labels, size_signal)])
    };
    lh.insert(header_type.to_string(), components);
    set_layout_header(model, channel, lh);
}

fn parse_guide_resolve_axis(resolve: &Resolve, channel: &str) -> String {
    let channel_scale_resolve = resolve.get_scale(channel);
    if channel_scale_resolve.as_deref() == Some("independent") {
        if resolve.axis.get(channel).and_then(Value::as_str) == Some("shared") {
            crate::log::warn(format!(
                "Setting the scale to be independent for \"{channel}\" means we also have to set the guide (axis or legend) to be independent."
            ));
        }
        return "independent".to_string();
    }

    resolve
        .axis
        .get(channel)
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| {
            default_scale_resolve(channel, crate::compile::model::SpecType::Facet).to_string()
        })
}

fn merge_child_axis(model: &mut Model, channel: &str) {
    let Some(child) = model.children.first() else {
        return;
    };
    let mut index = axis_index_from_value(&child.component.axes);
    let Some(components) = index.get(channel).cloned() else {
        return;
    };

    let mode = parse_guide_resolve_axis(&model.component.resolve, channel);
    model
        .component
        .resolve
        .axis
        .insert(channel.to_string(), json!(mode));

    if mode == "shared" {
        let header_channel = if channel == "x" { "column" } else { "row" };

        for component in &components {
            let header_type = get_header_type(component.get("orient").as_ref());
            ensure_header_component(model, header_channel, header_type);

            let main_axis = assemble_axis(component, "main", &model.config, true);
            if let Some(main_axis) = main_axis {
                if let Some(obj) = model
                    .component
                    .layout_headers
                    .get_mut(header_channel)
                    .and_then(Value::as_object_mut)
                {
                    let arr = obj
                        .entry(header_type.to_string())
                        .or_insert_with(|| Value::Array(Vec::new()));
                    if let Some(arr) = arr.as_array_mut() {
                        if let Some(first) = arr.first_mut() {
                            if let Some(axes) = first
                                .get_mut("axes")
                                .and_then(Value::as_array_mut)
                            {
                                axes.push(main_axis);
                            }
                        }
                    }
                }
            }
        }

        // Mark the main axes as extracted on the child.
        if let Some(list) = index.get_mut(channel) {
            for component in list.iter_mut() {
                component.main_extracted = true;
            }
        }
        if let Some(child) = model.children.first_mut() {
            child.component.axes = axis_index_to_value(&index);
        }
    }
}

fn ensure_header_component(model: &mut Model, header_channel: &str, header_type: &str) {
    let mut lh = layout_header(model, header_channel);
    let needs_component = !lh
        .get(header_type)
        .and_then(Value::as_array)
        .map(|a| !a.is_empty())
        .unwrap_or(false);

    if needs_component {
        let size_type = if header_channel == "row" {
            "height"
        } else {
            "width"
        };
        let size_signal = model
            .children
            .first()
            .filter(|child| {
                child
                    .component
                    .layout_size
                    .get(size_type)
                    .map(|v| is_truthy(&v))
                    .unwrap_or(false)
            })
            .map(|child| child.get_size_signal_ref(size_type));
        lh.insert(
            header_type.to_string(),
            Value::Array(vec![make_header_component(false, size_signal)]),
        );
        set_layout_header(model, header_channel, lh);
    }
}

/// Keep the facet dataflow node's snapshot of the model in sync with the
/// parsed header components.  Upstream shares the model object; the Rust port
/// stores a clone in [`FacetNode`], so it has to be refreshed explicitly.
fn sync_facet_node_model(model: &mut Model) {
    let snapshot = model.clone();
    let Some(facet_root) = model
        .data_flow
        .as_ref()
        .and_then(|df| df.facet_root.clone())
    else {
        return;
    };
    if FacetNode::is_facet_node(&facet_root) {
        with_node_mut::<FacetNode, _>(&facet_root, |facet| {
            facet.model = snapshot.clone();
        });
    }
}

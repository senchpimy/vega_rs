//! Port of `src/compile/facet.ts` (model construction / facet field defs).

use serde_json::{json, Map, Value};

use crate::channel::FACET_CHANNELS;
use crate::channeldef::{init_field_def, vg_field, vg_field_with, FieldRefOption, InitOpt};
use crate::log;
use crate::scale::has_discrete_domain;
use crate::util::keys;

use super::buildmodel::build_model;
use super::data::assemble::assemble_facet_data;
use super::data::calculate::sort_array_index_field;
use super::data::facet::is_crossed_facet_with_custom_sort;
use super::model::{FacetData, Model, SpecType};
use super::scale::domain::{assemble_domain, get_field_from_domain, is_vg_range_step};
use super::support;

impl Model {
    /// `new FacetModel(spec, parent, parentGivenName, config)`
    pub fn new_facet(
        spec: &Value,
        parent: Option<&Model>,
        parent_given_name: &str,
        config: &Value,
    ) -> Model {
        let mut model = Model::new_common(
            spec,
            SpecType::Facet,
            parent,
            parent_given_name,
            config,
            None,
        );

        let child_spec = spec.get("spec").cloned().unwrap_or(Value::Null);
        let child_name = model.get_name("child");
        let child = build_model(&child_spec, Some(&model), &child_name, None, config);
        model.children = vec![child];

        let facet = init_facet(&spec.get("facet").cloned().unwrap_or(Value::Null));
        model.facet_data = FacetData { facet };
        model
    }

    pub fn facet_field_def(&self, channel: &str) -> Option<Value> {
        self.facet_data.facet.get(channel).cloned()
    }
}

fn init_facet(facet: &Value) -> Value {
    if !support::is_facet_mapping(facet) {
        return serde_json::json!({"facet": init_facet_field_def(facet, "facet")});
    }

    let mut normalized = Map::new();
    for channel in keys(facet) {
        if channel != "row" && channel != "column" {
            log::warn(log::message::incompatible_channel(&channel, "facet", None));
            break;
        }
        let field_def = &facet[&channel];
        if field_def.get("field").is_none() {
            log::warn(log::message::empty_field_def(field_def, &channel));
            break;
        }
        normalized.insert(
            channel.clone(),
            init_facet_field_def(field_def, &channel),
        );
    }
    Value::Object(normalized)
}

fn init_facet_field_def(field_def: &Value, channel: &str) -> Value {
    let mut facet_field_def = init_field_def(field_def, channel, &InitOpt::default());
    if let Some(header) = facet_field_def.get("header").cloned() {
        if !header.is_null() {
            facet_field_def["header"] = support::replace_expr_ref(&header);
        }
    }
    facet_field_def
}


// ---------------------------------------------------------------------------
// Assembly
// ---------------------------------------------------------------------------

/// Port of `facetSortFieldName`.
fn facet_sort_field_name(field_def: &Value, sort: &Value, expr: Option<&str>) -> String {
    vg_field_with(
        sort,
        &FieldRefOption {
            suffix: Some(format!("by_{}", vg_field(field_def))),
            expr: expr.map(str::to_string),
            ..Default::default()
        },
    )
}

fn is_sort_field(sort: &Value) -> bool {
    crate::normalize_support::is_sort_field(sort)
}

impl Model {
    fn facet_channel(&self, channel: &str) -> Option<Value> {
        self.facet_data
            .facet
            .get(channel)
            .filter(|v| !v.is_null())
            .cloned()
    }

    /// True when `header: {title: null}` disables the facet title for
    /// `channel` (upstream `getFirstDefined` treats an explicit `null` guide
    /// title as defined, so it wins over the field-derived default).
    fn facet_header_title_disabled(&self, channel: &str) -> bool {
        self.facet_data
            .facet
            .get(channel)
            .and_then(|field_def| field_def.get("header"))
            .and_then(Value::as_object)
            .map(|header| header.get("title").map(Value::is_null).unwrap_or(false))
            .unwrap_or(false)
    }

    /// Port of `assembleDefaultLayout`.
    pub fn assemble_facet_default_layout(&self) -> Map<String, Value> {
        let mut out = Map::new();
        let row = self.facet_channel("row");
        let column = self.facet_channel("column");

        let columns = if column.is_some() {
            self.column_distinct_signal()
        } else if row.is_some() {
            Some(json!(1))
        } else {
            None
        };

        let mut align = "all";
        let independent_x =
            row.is_none() && self.component.resolve.get_scale("x").as_deref() == Some("independent");
        let independent_y = column.is_none()
            && self.component.resolve.get_scale("y").as_deref() == Some("independent");
        if independent_x || independent_y {
            align = "none";
        }

        // `getHeaderLayoutMixins()`: header/footer bands, title anchors and the
        // title offset used by Vega's layout engine.
        for (k, v) in super::header::assemble::header_layout_mixins(self) {
            out.insert(k, v);
        }
        // A disabled facet title must not contribute a layout offset.
        if let Some(Value::Object(offset)) = out.get_mut("offset") {
            if self.facet_header_title_disabled("row") {
                offset.remove("rowTitle");
            }
            if self.facet_header_title_disabled("column") {
                offset.remove("columnTitle");
            }
            if offset.is_empty() {
                out.remove("offset");
            }
        }

        if let Some(columns) = columns {
            out.insert("columns".to_string(), columns);
        }
        out.insert("bounds".to_string(), json!("full"));
        out.insert("align".to_string(), json!(align));

        if let Some(title_band) = super::header::assemble::assemble_layout_title_band(self) {
            out.insert("titleBand".to_string(), title_band);
        }
        out
    }

    fn column_distinct_signal(&self) -> Option<Value> {
        if self.parent_type == Some(SpecType::Facet) {
            return None;
        }
        let facet_layout_data_name = self.get_name("column_domain");
        Some(json!({"signal": format!("length(data('{facet_layout_data_name}'))")}))
    }

    fn get_cardinality_aggregate_for_child(&self) -> (Vec<String>, Vec<String>, Vec<String>) {
        let mut fields: Vec<String> = Vec::new();
        let mut ops: Vec<String> = Vec::new();
        let mut as_: Vec<String> = Vec::new();

        let child = match self.children.first() {
            Some(child) => child,
            None => return (fields, ops, as_),
        };

        if child.is_facet_model() {
            if child.channel_has_field("column") {
                let field = child
                    .facet_channel("column")
                    .map(|fd| vg_field(&fd))
                    .unwrap_or_default();
                fields.push(field.clone());
                ops.push("distinct".to_string());
                as_.push(format!("distinct_{field}"));
            }
        } else {
            for channel in crate::channel::POSITION_SCALE_CHANNELS {
                let child_scale = match child.component.scales.get(channel) {
                    Some(scale) if !scale.merged => scale,
                    _ => continue,
                };
                let type_ = child_scale.get_type();
                let range = child_scale.get("range").unwrap_or(Value::Null);
                if type_.map(has_discrete_domain).unwrap_or(false) && is_vg_range_step(&range) {
                    let domain = assemble_domain(child, channel);
                    if let Some(field) = get_field_from_domain(&domain) {
                        fields.push(field.clone());
                        ops.push("distinct".to_string());
                        as_.push(format!("distinct_{field}"));
                    } else {
                        log::warn(support::unknown_field(channel));
                    }
                }
            }
        }
        (fields, ops, as_)
    }

    /// Port of `assembleFacet`.
    fn assemble_facet_definition(&self) -> Value {
        let (name, data) = match &self.data_flow {
            Some(df) => match &df.facet_root {
                Some(node) => (
                    super::data::facet::FacetNode::node_name(node).unwrap_or_default(),
                    super::data::facet::FacetNode::get_data(node).unwrap_or_default(),
                ),
                None => (String::new(), String::new()),
            },
            None => (String::new(), String::new()),
        };
        let row = self.facet_channel("row");
        let column = self.facet_channel("column");
        let cross = row.is_some() && column.is_some();
        let (mut fields, mut ops, mut as_) = self.get_cardinality_aggregate_for_child();
        let mut groupby: Vec<String> = Vec::new();

        for channel in FACET_CHANNELS {
            let field_def = match self.facet_channel(channel) {
                Some(fd) => fd,
                None => continue,
            };
            groupby.push(vg_field(&field_def));

            let bin = field_def.get("bin").cloned().unwrap_or(Value::Null);
            if crate::compile::data::support::is_binning(&bin) {
                groupby.push(vg_field_with(
                    &field_def,
                    &FieldRefOption {
                        bin_suffix: Some("end".to_string()),
                        ..Default::default()
                    },
                ));
            }

            let sort = field_def.get("sort").cloned().unwrap_or(Value::Null);
            if is_sort_field(&sort) {
                let field = sort
                    .get("field")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let op = sort
                    .get("op")
                    .and_then(Value::as_str)
                    .unwrap_or("min")
                    .to_string();
                let output_name = facet_sort_field_name(&field_def, &sort, None);
                if !cross {
                    fields.push(field);
                    ops.push(op);
                    as_.push(output_name);
                }
            } else if sort.is_array() {
                let output_name = sort_array_index_field(
                    field_def.get("field").and_then(Value::as_str),
                    Some(channel),
                    None,
                    true,
                );
                if !cross {
                    fields.push(output_name.clone());
                    ops.push("max".to_string());
                    as_.push(output_name);
                }
            }
        }

        let mut out = Map::new();
        out.insert("name".to_string(), json!(name));
        out.insert("data".to_string(), json!(data));
        out.insert("groupby".to_string(), json!(groupby));
        if cross || !fields.is_empty() {
            let mut aggregate = Map::new();
            if cross {
                aggregate.insert("cross".to_string(), json!(true));
            }
            if !fields.is_empty() {
                aggregate.insert("fields".to_string(), json!(fields));
                aggregate.insert("ops".to_string(), json!(ops));
                aggregate.insert("as".to_string(), json!(as_));
            }
            out.insert("aggregate".to_string(), Value::Object(aggregate));
        }
        Value::Object(out)
    }

    /// Port of `crossedFacetLookupSortExpr`.
    fn crossed_facet_lookup_sort_expr(&self, channel: &str) -> Option<String> {
        let field_def = self.facet_channel(channel)?;
        let sort = field_def.get("sort").cloned().unwrap_or(Value::Null);
        if !is_sort_field(&sort) && !sort.is_array() {
            return None;
        }
        let lookup_data_name = self.get_name(&format!("{channel}_lookup_domain"));
        let lookup_data_expr = format!("data({})", json!(lookup_data_name));
        let lookup_field =
            super::data::facet::facet_lookup_key_field_name(self, channel);
        let lookup_index_expr = format!(
            "indexof(pluck({lookup_data_expr}, {}), {})",
            json!(lookup_field),
            super::data::facet::facet_lookup_key_expr(&field_def, "datum.datum")
        );
        let sort_value_field = if is_sort_field(&sort) {
            vg_field_with(
                &sort,
                &FieldRefOption {
                    for_as: true,
                    ..Default::default()
                },
            )
        } else {
            sort_array_index_field(
                field_def.get("field").and_then(Value::as_str),
                Some(channel),
                None,
                false,
            )
        };
        Some(format!(
            "{lookup_index_expr} >= 0 ? {lookup_data_expr}[{lookup_index_expr}][{}] : null",
            json!(sort_value_field)
        ))
    }

    fn facet_sort_fields(&self, channel: &str) -> Vec<Value> {
        let field_def = match self.facet_channel(channel) {
            Some(fd) => fd,
            None => return Vec::new(),
        };
        let sort = field_def.get("sort").cloned().unwrap_or(Value::Null);
        if is_crossed_facet_with_custom_sort(&self.facet_data.facet) {
            if let Some(expr) = self.crossed_facet_lookup_sort_expr(channel) {
                return vec![json!({"expr": expr})];
            }
        }
        if is_sort_field(&sort) {
            return vec![json!(facet_sort_field_name(&field_def, &sort, Some("datum")))];
        }
        if sort.is_array() {
            return vec![json!(vg_field_with(
                &field_def,
                &FieldRefOption {
                    expr: Some("datum".to_string()),
                    prefix: Some(channel.to_string()),
                    suffix: Some("sort_index".to_string()),
                    ..Default::default()
                },
            ))];
        }
        vec![json!(vg_field_with(
            &field_def,
            &FieldRefOption {
                expr: Some("datum".to_string()),
                ..Default::default()
            }
        ))]
    }

    fn facet_sort_order(&self, channel: &str) -> Vec<Value> {
        let field_def = match self.facet_channel(channel) {
            Some(fd) => fd,
            None => return Vec::new(),
        };
        let sort = field_def.get("sort").cloned().unwrap_or(Value::Null);
        let order = if is_sort_field(&sort) {
            sort.get("order")
                .filter(|v| !v.is_null())
                .cloned()
                .unwrap_or(json!("ascending"))
        } else if !sort.is_array() && sort.as_bool() == Some(true) {
            json!(true)
        } else {
            json!("ascending")
        };
        vec![order]
    }

    /// Port of `assembleLabelTitle` for facet models.
    fn assemble_facet_label_title(&self) -> Option<Value> {
        if let Some(facet_field_def) = self.facet_channel("facet") {
            return Some(super::header::assemble::assemble_label_title(
                &facet_field_def,
                "facet",
                &self.config,
            ));
        }

        for channel in ["row", "column"] {
            if let Some(field_def) = self.facet_channel(channel) {
                let header = field_def.get("header").cloned().unwrap_or(Value::Null);
                let label_orient = super::header::common::get_header_property(
                    "labelOrient",
                    Some(&header),
                    &self.config,
                    channel,
                );
                let orthogonal = if channel == "row" {
                    vec!["top", "bottom"]
                } else {
                    vec!["left", "right"]
                };
                if let Some(orient) = label_orient.as_ref().and_then(Value::as_str) {
                    if orthogonal.contains(&orient) {
                        return Some(super::header::assemble::assemble_label_title(
                            &field_def,
                            channel,
                            &self.config,
                        ));
                    }
                }
            }
        }
        None
    }

    /// Port of `assembleMarks` for facet models.
    pub fn assemble_facet_marks(&self) -> Vec<Value> {
        let child = match self.children.first() {
            Some(child) => child,
            None => return Vec::new(),
        };

        let data = self
            .data_flow
            .as_ref()
            .and_then(|df| df.facet_root.as_ref())
            .map(assemble_facet_data)
            .unwrap_or_default();

        // The dataflow assembler has now resolved the final data source names.
        // Upstream shares node objects with the child model; here the child
        // keeps its own snapshot, so refresh it from the assembled tree.
        let child = self.child_with_assembled_data_names(child);

        let encode_entry = child.assemble_group_encode_entry(false);
        let title = self
            .assemble_facet_label_title()
            .or_else(|| child.assemble_title());
        let style = child.assemble_group_style();

        let mut mark_group = Map::new();
        mark_group.insert("name".to_string(), json!(self.get_name("cell")));
        mark_group.insert("type".to_string(), json!("group"));
        if let Some(title) = title {
            mark_group.insert("title".to_string(), title);
        }
        if let Some(style) = style {
            mark_group.insert("style".to_string(), style);
        }
        mark_group.insert(
            "from".to_string(),
            json!({"facet": self.assemble_facet_definition()}),
        );

        let mut sort_fields: Vec<Value> = Vec::new();
        let mut sort_order: Vec<Value> = Vec::new();
        for channel in FACET_CHANNELS {
            sort_fields.extend(self.facet_sort_fields(channel));
            sort_order.extend(self.facet_sort_order(channel));
        }
        mark_group.insert(
            "sort".to_string(),
            json!({"field": sort_fields, "order": sort_order}),
        );

        if !data.is_empty() {
            mark_group.insert("data".to_string(), Value::Array(data));
        }
        if let Some(encode_entry) = encode_entry {
            mark_group.insert("encode".to_string(), json!({"update": encode_entry}));
        }

        let signals = super::selection::assemble::assemble_facet_signals(self, Vec::new());
        if let Value::Object(inner) = child.assemble_group(signals) {
            for (k, v) in inner {
                mark_group.insert(k, v);
            }
        }

        let mut marks = super::header::assemble::assemble_header_marks(self);
        // Drop title groups for channels whose facet title is disabled.
        marks.retain(|mark| {
            let name = mark.get("name").and_then(Value::as_str).unwrap_or("");
            !FACET_CHANNELS.iter().any(|channel| {
                self.facet_header_title_disabled(channel)
                    && name == format!("{channel}-title")
            })
        });
        marks.push(Value::Object(mark_group));
        marks
    }

    /// Refresh the child's output-node sources from the assembled facet subtree.
    fn child_with_assembled_data_names(&self, child: &Model) -> Model {
        use super::data::dataflow::{children, with_node, OutputNode};

        let mut names: std::collections::BTreeMap<String, String> =
            std::collections::BTreeMap::new();

        fn collect(
            node: &super::data::dataflow::NodeRef,
            names: &mut std::collections::BTreeMap<String, String>,
        ) {
            if let Some(info) = with_node::<OutputNode, _>(node, |n| {
                (n.name.clone(), n.source.clone())
            }) {
                let (name, source) = info;
                if let Some(original) = name.strip_prefix("clone_") {
                    names.insert(format!("scale_{original}"), source.clone());
                }
                names.insert(name, source);
            }
            for child in children(node) {
                collect(&child, names);
            }
        }

        if let Some(df) = &self.data_flow {
            for source in &df.sources {
                collect(source, &mut names);
            }
            if let Some(root) = &df.facet_root {
                collect(root, &mut names);
            }
        }

        let mut child = child.clone();
        if let Some(df) = child.data_flow.as_mut() {
            for (name, source) in names {
                if name.is_empty() {
                    continue;
                }
                if let Some(node) = df.output_nodes.get(&name) {
                    OutputNode::set_source(node, &source);
                } else if name.starts_with("scale_") {
                    let node = OutputNode::new(
                        None,
                        &source,
                        super::data::dataflow::OutputNodeType::Main,
                    );
                    df.output_nodes.insert(name, node);
                }
            }
        }
        child
    }
}

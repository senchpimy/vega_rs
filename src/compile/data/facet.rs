//! Port of `src/compile/data/facet.ts`.

use std::any::Any;

use serde_json::{json, Map, Value};

use super::support::is_binning;
use crate::channeldef::{vg_field_with, FieldRefOption};
use crate::scale::has_discrete_domain;
use crate::util::{hash, keys, var_name};

use super::calculate::sort_array_index_field;
use super::dataflow::{
    with_node, with_node_mut, DataFlowNode, FieldSet, NodeBehavior, NodeRef,
};
use super::support::hash_str;
use crate::compile::model::Model;
use crate::compile::scale::domain::{assemble_domain, get_field_from_domain, is_vg_range_step};

pub const FACET_CHANNELS: [&str; 3] = ["row", "column", "facet"];
pub const HEADER_CHANNELS: [&str; 2] = ["row", "column"];
pub const HEADER_TYPES: [&str; 2] = ["header", "footer"];
pub const POSITION_SCALE_CHANNELS: [&str; 2] = ["x", "y"];

#[derive(Debug, Clone)]
pub struct FacetChannelInfo {
    pub name: String,
    pub fields: Vec<String>,
    pub sort_field: Option<Value>,
    pub sort_index_field: Option<String>,
}

impl FacetChannelInfo {
    fn to_value(&self) -> Value {
        let mut out = Map::new();
        out.insert("name".into(), json!(self.name));
        out.insert("fields".into(), json!(self.fields));
        if let Some(sort_field) = &self.sort_field {
            out.insert("sortField".into(), sort_field.clone());
        }
        if let Some(sort_index_field) = &self.sort_index_field {
            out.insert("sortIndexField".into(), json!(sort_index_field));
        }
        Value::Object(out)
    }
}

/// Port of `isCrossedFacetWithCustomSort`.
pub fn is_crossed_facet_with_custom_sort(facet: &Value) -> bool {
    let row = facet.get("row");
    let column = facet.get("column");
    row.map(|v| !v.is_null()).unwrap_or(false)
        && column.map(|v| !v.is_null()).unwrap_or(false)
        && (row.map(is_custom_sort_field).unwrap_or(false)
            || column.map(is_custom_sort_field).unwrap_or(false))
}

fn is_custom_sort_field(field_def: &Value) -> bool {
    let Some(sort) = field_def.get("sort") else {
        return false;
    };
    is_sort_field(sort) || sort.is_array()
}

fn is_sort_field(sort: &Value) -> bool {
    crate::normalize_support::is_sort_field(sort)
}

/// Port of `facetLookupKeyFieldName`.
pub fn facet_lookup_key_field_name(model: &Model, channel: &str) -> String {
    model.get_name(&format!("{channel}_facet_key"))
}

/// Port of `facetLookupKeyExpr`.
pub fn facet_lookup_key_expr(field_def: &Value, datum: &str) -> String {
    let mut fields = vec![vg_field_with(
        field_def,
        &FieldRefOption {
            expr: Some(datum.to_string()),
            ..Default::default()
        },
    )];

    if field_def
        .get("bin")
        .map(is_binning)
        .unwrap_or(false)
    {
        fields.push(vg_field_with(
            field_def,
            &FieldRefOption {
                expr: Some(datum.to_string()),
                bin_suffix: Some("end".to_string()),
                ..Default::default()
            },
        ));
    }

    let joined = fields
        .iter()
        .map(|field| {
            format!(
                "isValid({field}) ? length(toString({field})) + ':' + toString({field}) : '-1:'"
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("join([{joined}], '|')")
}

#[derive(Debug, Clone)]
pub struct FacetNode {
    pub model: Model,
    pub name: String,
    pub data: String,
    pub row: Option<FacetChannelInfo>,
    pub column: Option<FacetChannelInfo>,
    pub facet: Option<FacetChannelInfo>,
}

impl FacetNode {
    /// `new FacetNode(parent, model, name, data)`.
    pub fn new(parent: Option<&NodeRef>, model: &Model, name: &str, data: &str) -> NodeRef {
        let mut row = None;
        let mut column = None;
        let mut facet = None;

        for channel in FACET_CHANNELS {
            let Some(field_def) = model.facet_field_def(channel) else {
                continue;
            };
            let bin = field_def.get("bin").cloned().unwrap_or(Value::Null);
            let sort = field_def.get("sort").cloned().unwrap_or(Value::Null);

            let mut fields = vec![vg_field_with(&field_def, &FieldRefOption::default())];
            if field_def.get("bin").map(is_binning).unwrap_or(false) {
                fields.push(vg_field_with(
                    &field_def,
                    &FieldRefOption {
                        bin_suffix: Some("end".to_string()),
                        ..Default::default()
                    },
                ));
            }

            let info = FacetChannelInfo {
                name: model.get_name(&format!("{channel}_domain")),
                fields,
                sort_field: if is_sort_field(&sort) {
                    Some(sort.clone())
                } else {
                    None
                },
                sort_index_field: if sort.is_array() {
                    Some(sort_array_index_field(
                        field_def.get("field").and_then(Value::as_str),
                        Some(channel),
                        None,
                        false,
                    ))
                } else {
                    None
                },
            };
            let _ = bin;

            match channel {
                "row" => row = Some(info),
                "column" => column = Some(info),
                "facet" => facet = Some(info),
                _ => {}
            }
        }

        DataFlowNode::new(
            Box::new(FacetNode {
                model: model.clone(),
                name: name.to_string(),
                data: data.to_string(),
                row,
                column,
                facet,
            }),
            parent,
            None,
        )
    }

    pub fn info(&self, channel: &str) -> Option<&FacetChannelInfo> {
        match channel {
            "row" => self.row.as_ref(),
            "column" => self.column.as_ref(),
            "facet" => self.facet.as_ref(),
            _ => None,
        }
    }

    pub fn fields(&self) -> Vec<String> {
        let mut f = Vec::new();
        for channel in FACET_CHANNELS {
            if let Some(info) = self.info(channel) {
                f.extend(info.fields.iter().cloned());
            }
        }
        f
    }

    pub fn sort_index_fields(&self) -> Vec<String> {
        let mut fields = Vec::new();
        for channel in FACET_CHANNELS {
            if let Some(info) = self.info(channel) {
                if let Some(f) = &info.sort_index_field {
                    fields.push(f.clone());
                }
            }
        }
        fields
    }

    pub fn can_move_below_aggregate(&self) -> bool {
        for channel in FACET_CHANNELS {
            let Some(info) = self.info(channel) else {
                continue;
            };
            let bin = self
                .model
                .facet_data
                .facet
                .get(channel)
                .and_then(|f| f.get("bin"))
                .cloned()
                .unwrap_or(Value::Null);
            if info.sort_field.is_some()
                || (info.sort_index_field.is_some() && is_binning(&bin))
            {
                return false;
            }
        }
        true
    }

    pub fn get_source(&self) -> String {
        self.name.clone()
    }

    pub fn is_facet_node(node: &NodeRef) -> bool {
        with_node::<FacetNode, _>(node, |_| ()).is_some()
    }

    pub fn node_name(node: &NodeRef) -> Option<String> {
        with_node::<FacetNode, _>(node, |n| n.name.clone())
    }

    pub fn node_fields(node: &NodeRef) -> Vec<String> {
        with_node::<FacetNode, _>(node, |n| n.fields()).unwrap_or_default()
    }

    pub fn node_sort_index_fields(node: &NodeRef) -> Vec<String> {
        with_node::<FacetNode, _>(node, |n| n.sort_index_fields()).unwrap_or_default()
    }

    pub fn node_can_move_below_aggregate(node: &NodeRef) -> bool {
        with_node::<FacetNode, _>(node, |n| n.can_move_below_aggregate()).unwrap_or(true)
    }

    pub fn set_data(node: &NodeRef, data: &str) {
        with_node_mut::<FacetNode, _>(node, |n| n.data = data.to_string());
    }

    pub fn get_data(node: &NodeRef) -> Option<String> {
        with_node::<FacetNode, _>(node, |n| n.data.clone())
    }

    pub fn assemble_facet(node: &NodeRef) -> Vec<Value> {
        with_node::<FacetNode, _>(node, |n| n.assemble()).unwrap_or_default()
    }

    fn child_independent_fields_with_step(&self) -> Map<String, Value> {
        let mut out = Map::new();
        let Some(child) = self.model.children.first() else {
            return out;
        };
        for channel in POSITION_SCALE_CHANNELS {
            let Some(child_scale_component) = child.component.scales.get(channel) else {
                continue;
            };
            if child_scale_component.merged {
                continue;
            }
            let type_ = child_scale_component
                .get_type()
                .map(|t| t.as_str().to_string())
                .unwrap_or_default();
            let range = child_scale_component.get("range").unwrap_or(Value::Null);
            if type_
                .parse::<crate::scale::ScaleType>()
                .map(has_discrete_domain)
                .unwrap_or(false)
                && is_vg_range_step(&range)
            {
                let domain = assemble_domain(child, channel);
                if let Some(field) = get_field_from_domain(&domain) {
                    out.insert(channel.to_string(), json!(field));
                } else {
                    crate::log::warn(format!("Cannot determine the field for the {channel} scale domain."));
                }
            }
        }
        out
    }

    fn assemble_row_column_header_data(
        &self,
        channel: &str,
        crossed_data_name: Option<&str>,
        child_independent: &Map<String, Value>,
    ) -> Value {
        let child_channel = match channel {
            "row" => Some("y"),
            "column" => Some("x"),
            _ => None,
        };
        let info = self.info(channel).cloned().unwrap_or(FacetChannelInfo {
            name: String::new(),
            fields: Vec::new(),
            sort_field: None,
            sort_index_field: None,
        });

        let mut fields: Vec<String> = Vec::new();
        let mut ops: Vec<String> = Vec::new();
        let mut as_: Vec<String> = Vec::new();

        if let Some(child_channel) = child_channel {
            if let Some(field) = child_independent
                .get(child_channel)
                .and_then(Value::as_str)
            {
                if crossed_data_name.is_some() {
                    fields.push(format!("distinct_{field}"));
                    ops.push("max".to_string());
                } else {
                    fields.push(field.to_string());
                    ops.push("distinct".to_string());
                }
                as_.push(format!("distinct_{field}"));
            }
        }

        if let Some(sort_field) = &info.sort_field {
            let op = sort_field
                .get("op")
                .and_then(Value::as_str)
                .unwrap_or("min")
                .to_string();
            let field = sort_field
                .get("field")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            fields.push(field.clone());
            ops.push(op.clone());
            as_.push(vg_field_with(
                sort_field,
                &FieldRefOption {
                    for_as: true,
                    ..Default::default()
                },
            ));
        } else if let Some(sort_index_field) = &info.sort_index_field {
            fields.push(sort_index_field.clone());
            ops.push("max".to_string());
            as_.push(sort_index_field.clone());
        }

        let mut aggregate = Map::new();
        aggregate.insert("type".into(), json!("aggregate"));
        aggregate.insert("groupby".into(), json!(info.fields));
        if !fields.is_empty() {
            aggregate.insert("fields".into(), json!(fields));
            aggregate.insert("ops".into(), json!(ops));
            aggregate.insert("as".into(), json!(as_));
        }

        let mut out = Map::new();
        out.insert("name".into(), json!(info.name));
        out.insert(
            "source".into(),
            json!(crossed_data_name.unwrap_or(&self.data)),
        );
        out.insert("transform".into(), json!([Value::Object(aggregate)]));
        Value::Object(out)
    }

    fn assemble_facet_header_data(&self, child_independent: &Map<String, Value>) -> Vec<Value> {
        let columns = self.model.layout.get("columns").cloned();
        let layout_headers = &self.model.component.layout_headers;
        let mut data = Vec::new();
        let mut has_shared_axis: Map<String, Value> = Map::new();

        for header_channel in HEADER_CHANNELS {
            for header_type in HEADER_TYPES {
                let headers = layout_headers
                    .get(header_channel)
                    .and_then(|h| h.get(header_type))
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                for header in headers {
                    if header
                        .get("axes")
                        .and_then(Value::as_array)
                        .map(|a| !a.is_empty())
                        .unwrap_or(false)
                    {
                        has_shared_axis.insert(header_channel.to_string(), json!(true));
                        break;
                    }
                }
            }

            if has_shared_axis.contains_key(header_channel) {
                let facet_name = self
                    .facet
                    .as_ref()
                    .map(|f| f.name.clone())
                    .unwrap_or_default();
                let cardinality = format!("length(data(\"{facet_name}\"))");

                let stop = if header_channel == "row" {
                    match &columns {
                        Some(c) if !c.is_null() => {
                            json!({"signal": format!("ceil({cardinality} / {})", super::support::js_value_string(c))})
                        }
                        _ => json!(1),
                    }
                } else {
                    match &columns {
                        Some(c) if !c.is_null() => {
                            json!({"signal": format!("min({cardinality}, {})", super::support::js_value_string(c))})
                        }
                        _ => json!({"signal": cardinality}),
                    }
                };

                data.push(json!({
                    "name": format!("{facet_name}_{header_channel}"),
                    "transform": [{
                        "type": "sequence",
                        "start": 0,
                        "stop": stop,
                    }],
                }));
            }
        }

        if has_shared_axis.contains_key("row") || has_shared_axis.contains_key("column") {
            data.insert(
                0,
                self.assemble_row_column_header_data("facet", None, child_independent),
            );
        }

        data
    }

    fn assemble_facet_lookup_domain_data(&self, channel: &str) -> Option<Value> {
        let facet_channel = self.info(channel)?;
        if facet_channel.sort_field.is_none() && facet_channel.sort_index_field.is_none() {
            return None;
        }
        let field_def = self.model.facet_data.facet.get(channel).cloned()?;
        Some(json!({
            "name": self.model.get_name(&format!("{channel}_lookup_domain")),
            "source": facet_channel.name,
            "transform": [{
                "type": "formula",
                "expr": facet_lookup_key_expr(&field_def, "datum"),
                "as": facet_lookup_key_field_name(&self.model, channel),
            }],
        }))
    }

    fn assemble(&self) -> Vec<Value> {
        let mut data: Vec<Value> = Vec::new();
        let mut crossed_data_name: Option<String> = None;
        let child_independent = self.child_independent_fields_with_step();

        let crossed_sort = is_crossed_facet_with_custom_sort(&self.model.facet_data.facet);

        if let (Some(column), Some(row)) = (self.column.as_ref(), self.row.as_ref()) {
            if child_independent.contains_key("x") || child_independent.contains_key("y") {
                let column_name = column.name.clone();
                let row_name = row.name.clone();
                let name = format!("cross_{column_name}_{row_name}");

                let mut fields: Vec<String> = Vec::new();
                if let Some(x) = child_independent.get("x").and_then(Value::as_str) {
                    fields.push(x.to_string());
                }
                if let Some(y) = child_independent.get("y").and_then(Value::as_str) {
                    fields.push(y.to_string());
                }
                let ops: Vec<String> = fields.iter().map(|_| "distinct".to_string()).collect();

                data.push(json!({
                    "name": name,
                    "source": self.data,
                    "transform": [{
                        "type": "aggregate",
                        "groupby": self.fields(),
                        "fields": fields,
                        "ops": ops,
                    }],
                }));
                crossed_data_name = Some(name);
            }
        }

        for channel in ["column", "row"] {
            if self.info(channel).is_some() {
                data.push(self.assemble_row_column_header_data(
                    channel,
                    crossed_data_name.as_deref(),
                    &child_independent,
                ));
            }
        }

        if crossed_sort {
            for channel in ["row", "column"] {
                if let Some(d) = self.assemble_facet_lookup_domain_data(channel) {
                    data.push(d);
                }
            }
        }

        if self.facet.is_some() {
            data.extend(self.assemble_facet_header_data(&child_independent));
        }

        data
    }
}

impl NodeBehavior for FacetNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        let mut out = String::from("Facet");
        for channel in FACET_CHANNELS {
            if let Some(info) = self.info(channel) {
                out.push_str(&format!(
                    " {}:{}",
                    &channel[..1],
                    hash_str(&hash(&info.to_value()))
                ));
            }
        }
        Value::String(out)
    }

    fn dependent_fields(&self) -> FieldSet {
        let mut dep_fields: FieldSet = self.fields().into_iter().collect();
        for channel in FACET_CHANNELS {
            if let Some(info) = self.info(channel) {
                if let Some(sort_field) = &info.sort_field {
                    if let Some(field) = sort_field.get("field").and_then(Value::as_str) {
                        dep_fields.insert(field.to_string());
                    }
                }
                if let Some(sort_index_field) = &info.sort_index_field {
                    dep_fields.insert(sort_index_field.clone());
                }
            }
        }
        dep_fields
    }

    fn produced_fields(&self) -> Option<FieldSet> {
        Some(FieldSet::new())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Format a channel name (used by `var_name`-style naming in tests).
pub fn facet_channel_name(model: &Model, channel: &str) -> String {
    var_name(&model.get_name(channel))
}

/// Utility mirroring `keys` for facet mappings (kept for parity with upstream).
pub fn facet_channels() -> Vec<String> {
    keys(&json!({"row": 1, "column": 1, "facet": 1}))
}

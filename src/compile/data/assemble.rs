//! Port of `src/compile/data/assemble.ts`.

use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use super::aggregate::AggregateNode;
use super::bin::BinNode;
use super::calculate::CalculateNode;
use super::dataflow::{with_node, OutputNode, OutputNodeType};
use super::density::DensityTransformNode;
use super::extent::ExtentTransformNode;
use super::facet::FacetNode;
use super::filter::FilterNode;
use super::filterinvalid::FilterInvalidNode;
use super::flatten::FlattenTransformNode;
use super::fold::FoldTransformNode;
use super::formatparse::ParseNode;
use super::geojson::GeoJSONNode;
use super::geopoint::GeoPointNode;
use super::graticule::GraticuleNode;
use super::identifier::IdentifierNode;
use super::impute::ImputeNode;
use super::index::DataComponent;
use super::joinaggregate::JoinAggregateTransformNode;
use super::loess::LoessTransformNode;
use super::lookup::LookupNode;
use super::pivot::PivotTransformNode;
use super::quantile::QuantileTransformNode;
use super::regression::RegressionTransformNode;
use super::sample::SampleTransformNode;
use super::sequence::SequenceNode;
use super::source::{is_url_data, SourceNode};
use super::stack::StackNode;
use super::timeunit::TimeUnitNode;
use super::window::WindowTransformNode;
use super::dataflow::{children, num_children, parent, NodeRef};

/// A Vega data source (`VgData`).
#[derive(Debug, Clone, Default)]
pub struct VgData {
    pub name: Option<String>,
    pub source: Option<String>,
    pub transform: Vec<Value>,
    pub format: Option<Value>,
    pub values: Option<Value>,
    pub url: Option<Value>,
}

impl VgData {
    pub fn from_value(v: &Value) -> Self {
        VgData {
            name: v.get("name").and_then(Value::as_str).map(str::to_string),
            source: v.get("source").and_then(Value::as_str).map(str::to_string),
            transform: v
                .get("transform")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            format: v.get("format").cloned(),
            values: v.get("values").cloned(),
            url: v.get("url").cloned(),
        }
    }

    pub fn to_value(&self) -> Value {
        let mut out = Map::new();
        if let Some(name) = &self.name {
            out.insert("name".into(), json!(name));
        }
        if let Some(source) = &self.source {
            out.insert("source".into(), json!(source));
        }
        if !self.transform.is_empty() {
            out.insert("transform".into(), Value::Array(self.transform.clone()));
        }
        if let Some(format) = &self.format {
            out.insert("format".into(), format.clone());
        }
        if let Some(values) = &self.values {
            out.insert("values".into(), values.clone());
        }
        if let Some(url) = &self.url {
            out.insert("url".into(), url.clone());
        }
        Value::Object(out)
    }
}

fn is_node<T: crate::compile::data::dataflow::NodeBehavior + 'static>(node: &NodeRef) -> bool {
    with_node::<T, _>(node, |_| ()).is_some()
}

fn is_output(node: &NodeRef) -> bool {
    is_node::<OutputNode>(node)
}

fn output_type(node: &NodeRef) -> Option<OutputNodeType> {
    with_node::<OutputNode, _>(node, |n| n.data_source_type)
}

/// Port of `makeWalkTree`.  The returned value owns the assembled data and the
/// dataset counter.
pub struct WalkTree {
    pub data: Vec<Value>,
    dataset_index: usize,
}

impl WalkTree {
    pub fn new() -> Self {
        WalkTree {
            data: Vec::new(),
            dataset_index: 0,
        }
    }

    pub fn walk(&mut self, node: &NodeRef, data_source: &mut VgData) {
        if is_node::<SourceNode>(node) {
            let data = with_node::<SourceNode, _>(node, |n| n.data.clone()).unwrap_or(Value::Null);
            if !SourceNode::is_generator_node(node) && !is_url_data(&data) {
                self.data.push(data_source.to_value());
                *data_source = VgData {
                    name: None,
                    source: data_source.name.clone(),
                    transform: Vec::new(),
                    ..Default::default()
                };
            }
        }

        if is_node::<ParseNode>(node) {
            let parent_is_source = parent(node)
                .map(|p| is_node::<SourceNode>(&p))
                .unwrap_or(false);
            if parent_is_source && data_source.source.is_none() {
                let (format_parse, transforms) = with_node::<ParseNode, _>(node, |n| {
                    (n.assemble_format_parse(), n.assemble_transforms(true))
                })
                .unwrap_or((json!({}), Vec::new()));

                let mut format = data_source
                    .format
                    .as_ref()
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                format.insert("parse".into(), format_parse);
                data_source.format = Some(Value::Object(format));
                data_source.transform.extend(transforms);
            } else {
                let transforms = with_node::<ParseNode, _>(node, |n| n.assemble_transforms(false))
                    .unwrap_or_default();
                data_source.transform.extend(transforms);
            }
        }

        if is_node::<FacetNode>(node) {
            if data_source.name.is_none() {
                data_source.name = Some(format!("data_{}", self.dataset_index));
                self.dataset_index += 1;
            }

            if data_source.source.is_none() || !data_source.transform.is_empty() {
                self.data.push(data_source.to_value());
                let name = data_source.name.clone().unwrap_or_default();
                FacetNode::set_data(node, &name);
            } else {
                FacetNode::set_data(node, data_source.source.as_deref().unwrap_or(""));
            }

            self.data.extend(FacetNode::assemble_facet(node));

            // break here because the rest of the tree has to be taken care of
            // by the facet.
            return;
        }

        let is_single_transform = is_node::<GraticuleNode>(node)
            || is_node::<SequenceNode>(node)
            || is_node::<FilterInvalidNode>(node)
            || is_node::<FilterNode>(node)
            || is_node::<CalculateNode>(node)
            || is_node::<GeoPointNode>(node)
            || is_node::<AggregateNode>(node)
            || is_node::<LookupNode>(node)
            || is_node::<WindowTransformNode>(node)
            || is_node::<JoinAggregateTransformNode>(node)
            || is_node::<FoldTransformNode>(node)
            || is_node::<FlattenTransformNode>(node)
            || is_node::<DensityTransformNode>(node)
            || is_node::<LoessTransformNode>(node)
            || is_node::<QuantileTransformNode>(node)
            || is_node::<RegressionTransformNode>(node)
            || is_node::<IdentifierNode>(node)
            || is_node::<SampleTransformNode>(node)
            || is_node::<PivotTransformNode>(node)
            || is_node::<ExtentTransformNode>(node);

        if is_single_transform {
            let transform = super::dataflow::assemble(node);
            data_source.transform.push(transform);
        }

        let is_multi_transform = is_node::<BinNode>(node)
            || is_node::<TimeUnitNode>(node)
            || is_node::<ImputeNode>(node)
            || is_node::<StackNode>(node)
            || is_node::<GeoJSONNode>(node);

        if is_multi_transform {
            let transform = super::dataflow::assemble(node);
            if let Value::Array(arr) = transform {
                data_source.transform.extend(arr);
            } else {
                data_source.transform.push(transform);
            }
        }

        if is_output(node) {
            let parent_is_output = parent(node).map(|p| is_output(&p)).unwrap_or(false);
            if data_source.source.is_some() && data_source.transform.is_empty() {
                OutputNode::set_source(node, data_source.source.as_deref().unwrap_or(""));
            } else if parent_is_output {
                OutputNode::set_source(node, data_source.name.as_deref().unwrap_or(""));
            } else {
                if data_source.name.is_none() {
                    data_source.name = Some(format!("data_{}", self.dataset_index));
                    self.dataset_index += 1;
                }
                let name = data_source.name.clone().unwrap_or_default();
                OutputNode::set_source(node, &name);

                if num_children(node) == 1 {
                    self.data.push(data_source.to_value());
                    *data_source = VgData {
                        name: None,
                        source: data_source.name.clone(),
                        transform: Vec::new(),
                        ..Default::default()
                    };
                }
            }
        }

        match num_children(node) {
            0 => {
                if is_output(node)
                    && (data_source.source.is_none() || !data_source.transform.is_empty())
                {
                    self.data.push(data_source.to_value());
                }
            }
            1 => {
                let child = children(node)[0].clone();
                self.walk(&child, data_source);
            }
            _ => {
                if data_source.name.is_none() {
                    data_source.name = Some(format!("data_{}", self.dataset_index));
                    self.dataset_index += 1;
                }

                let mut source = data_source.name.clone().unwrap_or_default();
                if data_source.source.is_none() || !data_source.transform.is_empty() {
                    self.data.push(data_source.to_value());
                } else {
                    source = data_source.source.clone().unwrap_or_default();
                }

                for child in children(node) {
                    let mut new_data = VgData {
                        name: None,
                        source: Some(source.clone()),
                        transform: Vec::new(),
                        ..Default::default()
                    };
                    self.walk(&child, &mut new_data);
                }
            }
        }
    }
}

impl Default for WalkTree {
    fn default() -> Self {
        Self::new()
    }
}

/// Port of `assembleFacetData`.
pub fn assemble_facet_data(root: &NodeRef) -> Vec<Value> {
    let mut walk_tree = WalkTree::new();
    let name = FacetNode::node_name(root).unwrap_or_default();
    for child in children(root) {
        let mut data_source = VgData {
            name: None,
            source: Some(name.clone()),
            transform: Vec::new(),
            ..Default::default()
        };
        walk_tree.walk(&child, &mut data_source);
    }
    walk_tree.data
}

/// Port of `assembleRootData`.
pub fn assemble_root_data(
    data_component: &DataComponent,
    datasets: &IndexMap<String, Value>,
) -> Vec<Value> {
    let mut walk_tree = WalkTree::new();
    let mut source_index = 0;

    for root in &data_component.sources {
        if !SourceNode::has_name(root) {
            SourceNode::set_data_name(root, &format!("source_{source_index}"));
            source_index += 1;
        }

        let mut new_data = VgData::from_value(&SourceNode::assemble_source(root));
        walk_tree.walk(root, &mut new_data);
    }

    let mut data = walk_tree.data;

    // remove empty transform arrays for cleaner output
    for d in &mut data {
        if let Some(o) = d.as_object_mut() {
            if o.get("transform")
                .and_then(Value::as_array)
                .map(|a| a.is_empty())
                .unwrap_or(false)
            {
                o.remove("transform");
            }
        }
    }

    // move sources without transforms (the ones that are potentially used in
    // lookups) to the beginning
    let mut without_transforms: Vec<Value> = Vec::new();
    let mut with_transforms: Vec<Value> = Vec::new();
    for d in data {
        let has_transform = d
            .get("transform")
            .and_then(Value::as_array)
            .map(|a| !a.is_empty())
            .unwrap_or(false);
        if !has_transform && d.get("source").is_none() {
            without_transforms.push(d);
        } else {
            with_transforms.push(d);
        }
    }
    without_transforms.extend(with_transforms);
    let mut data = without_transforms;

    // now fix the `from` references in lookup transforms
    for d in &mut data {
        if let Some(transforms) = d.get_mut("transform").and_then(Value::as_array_mut) {
            for t in transforms {
                if t.get("type").and_then(Value::as_str) == Some("lookup") {
                    if let Some(from) = t.get("from").and_then(Value::as_str).map(str::to_string) {
                        if let Some(output_node) = data_component.output_nodes.get(&from) {
                            t["from"] = json!(OutputNode::get_source(output_node));
                        }
                    }
                }
            }
        }
    }

    // inline values for datasets that are in the datastore
    for d in &mut data {
        if let Some(name) = d.get("name").and_then(Value::as_str).map(str::to_string) {
            if let Some(values) = datasets.get(&name) {
                d["values"] = values.clone();
            }
        }
    }

    data
}

/// Port of the `DataSourceType` used by `OutputNode`.
pub fn output_node_type(node: &NodeRef) -> Option<OutputNodeType> {
    output_type(node)
}

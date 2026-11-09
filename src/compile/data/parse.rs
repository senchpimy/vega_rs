//! Port of `src/compile/data/parse.ts`.
//!
//! The parts of the pipeline that depend on encoding-derived aggregate / bin /
//! time-unit / stack nodes are implemented in their respective modules.  This
//! module ports the source discovery, transform-array parsing and the overall
//! `parseData` skeleton.

use serde_json::{json, Map, Value};

use crate::util::deep_equal;

use super::aggregate::AggregateNode;
use super::bin::BinNode;
use super::calculate::CalculateNode;
use super::dataflow::{DataFlowNode, NodeRef, OutputNode, OutputNodeType};
use super::density::DensityTransformNode;
use super::extent::ExtentTransformNode;
use super::facet::FacetNode;
use super::filter::FilterNode;
use super::filterinvalid::FilterInvalidNode;

use super::flatten::FlattenTransformNode;
use super::fold::FoldTransformNode;
use super::formatparse::{
    get_implicit_from_encoding, get_implicit_from_filter_transform,
    get_implicit_from_selection, ParseNode,
};
use super::geojson::GeoJSONNode;
use super::geopoint::GeoPointNode;
use super::graticule::GraticuleNode;
use super::identifier::IdentifierNode;
use super::impute::ImputeNode;
use super::index::{AncestorParse, DataComponent};
use super::joinaggregate::JoinAggregateTransformNode;
use super::loess::LoessTransformNode;
use super::lookup::LookupNode;
use super::pivot::PivotTransformNode;
use super::quantile::QuantileTransformNode;
use super::regression::RegressionTransformNode;
use super::sample::SampleTransformNode;
use super::sequence::SequenceNode;
use super::source::{
    is_generator, is_graticule_generator, is_inline_data, is_named_data, is_sequence_generator,
    is_url_data, SourceNode,
};
use super::stack::StackNode;
use super::timeunit::TimeUnitNode;
use super::window::WindowTransformNode;
use crate::compile::model::Model;

// ---------------------------------------------------------------------------
// Transform type guards (`src/transform.ts`)
// ---------------------------------------------------------------------------

fn has_non_null(t: &Value, key: &str) -> bool {
    t.get(key).map(|v| !v.is_null()).unwrap_or(false)
}

pub fn is_calculate(t: &Value) -> bool {
    has_non_null(t, "calculate") && has_non_null(t, "as")
}

pub fn is_filter(t: &Value) -> bool {
    has_non_null(t, "filter")
}

pub fn is_bin(t: &Value) -> bool {
    has_non_null(t, "bin")
}

pub fn is_time_unit(t: &Value) -> bool {
    has_non_null(t, "timeUnit")
}

pub fn is_aggregate(t: &Value) -> bool {
    has_non_null(t, "aggregate") && !is_join_aggregate(t) && !is_stack(t)
}

pub fn is_join_aggregate(t: &Value) -> bool {
    has_non_null(t, "joinaggregate")
}

pub fn is_lookup(t: &Value) -> bool {
    has_non_null(t, "lookup")
}

pub fn is_window(t: &Value) -> bool {
    has_non_null(t, "window")
}

pub fn is_stack(t: &Value) -> bool {
    has_non_null(t, "stack")
}

pub fn is_fold(t: &Value) -> bool {
    has_non_null(t, "fold")
}

pub fn is_extent(t: &Value) -> bool {
    has_non_null(t, "extent")
        && has_non_null(t, "param")
        && !has_non_null(t, "density")
        && !has_non_null(t, "regression")
}

pub fn is_flatten(t: &Value) -> bool {
    has_non_null(t, "flatten")
}

pub fn is_pivot(t: &Value) -> bool {
    has_non_null(t, "pivot")
}

pub fn is_sample(t: &Value) -> bool {
    has_non_null(t, "sample")
}

pub fn is_impute(t: &Value) -> bool {
    has_non_null(t, "impute")
}

pub fn is_density(t: &Value) -> bool {
    has_non_null(t, "density")
}

pub fn is_quantile(t: &Value) -> bool {
    has_non_null(t, "quantile")
}

pub fn is_regression(t: &Value) -> bool {
    has_non_null(t, "regression")
}

pub fn is_loess(t: &Value) -> bool {
    has_non_null(t, "loess")
}

// ---------------------------------------------------------------------------
// Source discovery
// ---------------------------------------------------------------------------

fn format_of(v: &Value) -> Option<&Map<String, Value>> {
    v.get("format").and_then(Value::as_object)
}

/// Port of `findSource`.
pub fn find_source(data: &Value, sources: &[NodeRef]) -> Option<NodeRef> {
    for other in sources {
        let other_data = SourceNode::data(other);

        // if both datasets have a name defined, we cannot merge
        if is_named_data(data) && SourceNode::has_name(other)
            && data.get("name").and_then(Value::as_str) != SourceNode::data_name(other).as_deref() {
                continue;
            }

        let format_mesh = format_of(data).and_then(|f| f.get("mesh")).cloned();
        let other_feature = format_of(&other_data)
            .and_then(|f| f.get("feature"))
            .cloned();

        // feature and mesh are mutually exclusive
        if format_mesh.is_some() && other_feature.is_some() {
            continue;
        }

        // we have to extract the same feature or mesh
        let format_feature = format_of(data).and_then(|f| f.get("feature")).cloned();
        if (format_feature.is_some() || other_feature.is_some())
            && format_feature != other_feature
        {
            continue;
        }

        let other_mesh = format_of(&other_data).and_then(|f| f.get("mesh")).cloned();
        if (format_mesh.is_some() || other_mesh.is_some()) && format_mesh != other_mesh {
            continue;
        }

        if is_inline_data(data) && is_inline_data(&other_data) {
            if deep_equal(
                data.get("values").unwrap_or(&Value::Null),
                other_data.get("values").unwrap_or(&Value::Null),
            ) {
                return Some(other.clone());
            }
        } else if is_url_data(data) && is_url_data(&other_data) {
            if data.get("url") == other_data.get("url") {
                return Some(other.clone());
            }
        } else if is_named_data(data)
            && data.get("name").and_then(Value::as_str) == SourceNode::data_name(other).as_deref() {
                return Some(other.clone());
            }
    }
    None
}

fn merge_deep(base: Value, other: Option<&Value>) -> Value {
    match (base, other) {
        (Value::Object(mut a), Some(Value::Object(b))) => {
            for (k, v) in b {
                let merged = merge_deep(a.remove(k).unwrap_or(Value::Null), Some(v));
                a.insert(k.clone(), merged);
            }
            Value::Object(a)
        }
        (_, Some(v)) => v.clone(),
        (a, None) => a,
    }
}

/// Port of `parseRoot`.
pub fn parse_root(
    model: &Model,
    sources: &mut Vec<NodeRef>,
    fallback: Option<&NodeRef>,
) -> NodeRef {
    let data = model.data.clone();
    if data.is_some() || model.parent_type.is_none() {
        if let Some(Value::Null) = data {
            // data: null means we should ignore the parent's data
            let source = SourceNode::new(Some(&json!({"values": []})));
            sources.push(source.clone());
            return source;
        }

        let model_data = data.unwrap_or_else(|| json!({"name": "source"}));
        if let Some(existing_source) = find_source(&model_data, sources) {
            if !is_generator(&model_data) {
                let existing_data = SourceNode::data(&existing_source);
                let merged = merge_deep(
                    existing_data
                        .get("format")
                        .cloned()
                        .unwrap_or(Value::Null),
                    model_data.get("format"),
                );
                let merged = merge_deep(
                    merged,
                    Some(
                        &existing_data
                            .get("format")
                            .cloned()
                            .unwrap_or(Value::Null),
                    ),
                );
                super::dataflow::with_node_mut::<SourceNode, _>(&existing_source, |n| {
                    n.data["format"] = merged;
                });
            }
            if !SourceNode::has_name(&existing_source) && is_named_data(&model_data) {
                SourceNode::set_data_name(
                    &existing_source,
                    model_data.get("name").and_then(Value::as_str).unwrap_or(""),
                );
            }
            return existing_source;
        }

        let source = SourceNode::new(Some(&model_data));
        sources.push(source.clone());
        source
    } else {
        // If we don't have a source defined (overriding parent's data), use the
        // parent's facet root or main.
        fallback.cloned().unwrap_or_else(|| {
            let source = SourceNode::new(Some(&json!({"values": []})));
            sources.push(source.clone());
            source
        })
    }
}

// ---------------------------------------------------------------------------
// Transform array parsing
// ---------------------------------------------------------------------------

/// Parses a transform array into a chain of connected dataflow nodes.
///
/// Uses a throwaway data component for nodes that need to register sources or
/// output nodes (only lookups do).  Compiled models should use
/// [`parse_transform_array_with_component`] so those registrations end up in
/// the real dataflow.
pub fn parse_transform_array(
    head: NodeRef,
    model: &Model,
    ancestor_parse: &mut AncestorParse,
) -> NodeRef {
    let mut component = DataComponent::default();
    parse_transform_array_with_component(head, model, ancestor_parse, &mut component)
}

/// Parses a transform array into a chain of connected dataflow nodes,
/// registering any sources/output nodes on `component`.
pub fn parse_transform_array_with_component(
    head: NodeRef,
    model: &Model,
    ancestor_parse: &mut AncestorParse,
    component: &mut DataComponent,
) -> NodeRef {
    let mut head = head;
    let mut lookup_counter = 0;

    for t in &model.transforms {
        let mut derived_type: Option<&str> = None;
        let mut transform_node: Option<NodeRef> = None;

        if is_calculate(t) {
            head = CalculateNode::new(Some(&head), t);
            transform_node = Some(head.clone());
            derived_type = Some("derived");
        } else if is_filter(t) {
            let implicit = get_implicit_from_filter_transform(t);
            head = ParseNode::make_with_ancestors(
                Some(&head),
                Default::default(),
                implicit,
                ancestor_parse,
            )
            .unwrap_or(head);
            head = FilterNode::new(
                Some(&head),
                Some(model),
                t.get("filter").unwrap_or(&Value::Null),
            );
        } else if is_bin(t) {
            head = BinNode::make_from_transform(Some(&head), t, &super::support::SimpleModel);
            transform_node = Some(head.clone());
            derived_type = Some("number");
        } else if is_time_unit(t) {
            derived_type = Some("date");
            let field = t.get("field").and_then(Value::as_str).unwrap_or_default();
            let parsed_as = ancestor_parse.get_with_explicit(field);
            if parsed_as.value.is_none() {
                let mut parse = super::formatparse::Parse::new();
                parse.insert(field.to_string(), json!("date"));
                head = ParseNode::new(Some(&head), parse);
                ancestor_parse.set(field, json!("date"), false);
            }
            head = TimeUnitNode::make_from_transform(Some(&head), t);
            transform_node = Some(head.clone());
        } else if is_aggregate(t) {
            if let Some(node) = AggregateNode::make_from_transform(Some(&head), t) {
                head = node;
                transform_node = Some(head.clone());
                if requires_selection_id_for_model(model) {
                    head = IdentifierNode::new(Some(&head));
                }
            }
            derived_type = Some("number");
        } else if is_lookup(t) {
            head = LookupNode::make(
                Some(&head),
                model,
                t,
                lookup_counter,
                component,
            );
            lookup_counter += 1;
            transform_node = Some(head.clone());
            derived_type = Some("derived");
        } else if is_window(t) {
            head = WindowTransformNode::new(Some(&head), t);
            transform_node = Some(head.clone());
            derived_type = Some("number");
        } else if is_join_aggregate(t) {
            head = JoinAggregateTransformNode::new(Some(&head), t);
            transform_node = Some(head.clone());
            derived_type = Some("number");
        } else if is_stack(t) {
            head = StackNode::make_from_transform(Some(&head), t);
            transform_node = Some(head.clone());
            derived_type = Some("derived");
        } else if is_fold(t) {
            head = FoldTransformNode::new(Some(&head), t);
            transform_node = Some(head.clone());
            derived_type = Some("derived");
        } else if is_extent(t) {
            head = ExtentTransformNode::new(Some(&head), t);
            transform_node = Some(head.clone());
            derived_type = Some("derived");
        } else if is_flatten(t) {
            head = FlattenTransformNode::new(Some(&head), t);
            transform_node = Some(head.clone());
            derived_type = Some("derived");
        } else if is_pivot(t) {
            head = PivotTransformNode::new(Some(&head), t);
            transform_node = Some(head.clone());
            derived_type = Some("derived");
        } else if is_sample(t) {
            head = SampleTransformNode::new(Some(&head), t);
        } else if is_impute(t) {
            head = ImputeNode::make_from_transform(Some(&head), t);
            transform_node = Some(head.clone());
            derived_type = Some("derived");
        } else if is_density(t) {
            head = DensityTransformNode::new(Some(&head), t);
            transform_node = Some(head.clone());
            derived_type = Some("derived");
        } else if is_quantile(t) {
            head = QuantileTransformNode::new(Some(&head), t);
            transform_node = Some(head.clone());
            derived_type = Some("derived");
        } else if is_regression(t) {
            head = RegressionTransformNode::new(Some(&head), t);
            transform_node = Some(head.clone());
            derived_type = Some("derived");
        } else if is_loess(t) {
            head = LoessTransformNode::new(Some(&head), t);
            transform_node = Some(head.clone());
            derived_type = Some("derived");
        } else {
            crate::log::warn(format!(
                "Invalid transform ignored: {}",
                crate::util::stringify(t)
            ));
            continue;
        }

        if let (Some(transform_node), Some(derived_type)) = (transform_node, derived_type) {
            let produced = super::dataflow::produced_fields(&transform_node).unwrap_or_default();
            for field in produced {
                ancestor_parse.set(&field, json!(derived_type), false);
            }
        }
    }

    head
}

fn make_output_node(
    data_source_type: OutputNodeType,
    model: &Model,
    head: &NodeRef,
) -> NodeRef {
    let name = model.get_data_name(output_node_type_name(data_source_type));
    let node = OutputNode::new(Some(head), &name, data_source_type);
    if let Some(count) = model.component.data.output_node_ref_counts.get(&name) {
        if *count > 0 {
            OutputNode::set_ref_count(&node, *count as u64);
        }
    }
    node
}

fn output_node_type_name(type_: OutputNodeType) -> &'static str {
    match type_ {
        OutputNodeType::Main => "main",
        OutputNodeType::Raw => "raw",
        OutputNodeType::Lookup => "lookup",
        OutputNodeType::Filter => "filter",
        OutputNodeType::Facet => "facet",
        OutputNodeType::Data => "data",
        OutputNodeType::PreFilterInvalid => "pre_filter_invalid",
        OutputNodeType::PostFilterInvalid => "post_filter_invalid",
    }
}

/// Like `requiresSelectionId`, but ignores selection components inherited from
/// an ancestor/sibling multi-view model.  Inherited components only exist for
/// lookups in this port and must not cause this unit to add tuple identifiers.
fn requires_selection_id_for_model(model: &Model) -> bool {
    match &model.component.selection {
        Value::Object(map) => map.values().any(|v| {
            !v.get("inherited")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                && v.get("project")
                    .and_then(|p| p.get("hasSelectionId"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
        }),
        _ => false,
    }
}

/// Port of `parseData`.
pub fn parse_data(model: &Model) -> DataComponent {
    let mut component = DataComponent::default();
    parse_data_into(model, &mut component, None);
    component
}

/// Port of `parseData`, mutating the shared data component (sources and output
/// nodes are shared with ancestors in the TypeScript implementation).
pub fn parse_data_into(
    model: &Model,
    component: &mut DataComponent,
    fallback: Option<&NodeRef>,
) -> (Option<NodeRef>, Option<NodeRef>) {
    let head = parse_root(model, &mut component.sources, fallback);
    let mut head = head;

    let data = model.data.clone();
    let new_data = data
        .as_ref()
        .map(|d| is_generator(d) || is_url_data(d) || is_inline_data(d))
        .unwrap_or(false);
    let mut ancestor_parse = if !new_data {
        component
            .ancestor_parse
            .clone()
            .unwrap_or_default()
    } else {
        AncestorParse::new()
    };

    if let Some(data) = &data {
        if is_generator(data) {
            if is_sequence_generator(data) {
                head = SequenceNode::new(
                    Some(&head),
                    data.get("sequence").unwrap_or(&Value::Null),
                );
            } else if is_graticule_generator(data) {
                head = GraticuleNode::new(
                    Some(&head),
                    data.get("graticule").unwrap_or(&Value::Null),
                );
            }
            ancestor_parse.parse_nothing = true;
        } else if data
            .get("format")
            .and_then(|f| f.get("parse"))
            .map(Value::is_null)
            .unwrap_or(false)
        {
            ancestor_parse.parse_nothing = true;
        }
    }

    head = ParseNode::make_explicit(Some(&head), model, &mut ancestor_parse).unwrap_or(head);

    head = IdentifierNode::new(Some(&head));

    let parent_is_layer = model.parent_type == Some(crate::compile::model::SpecType::Layer);
    if (model.is_unit_model() || model.is_facet_model()) && parent_is_layer {
        head = super::encoding_nodes::make_bin_from_encoding(&head, model).unwrap_or(head);
    }

    if !model.transforms.is_empty() {
        head = parse_transform_array_with_component(head, model, &mut ancestor_parse, component);
    }

    // create parse nodes for fields that need to be parsed (or flattened)
    let implicit_selection = get_implicit_from_selection(model);
    let implicit_encoding = get_implicit_from_encoding(model);
    let mut implicit = implicit_selection;
    for (k, v) in implicit_encoding {
        implicit.insert(k, v);
    }
    head = ParseNode::make_with_ancestors(
        Some(&head),
        Default::default(),
        implicit,
        &mut ancestor_parse,
    )
    .unwrap_or(head);

    if model.is_unit_model() {
        head = GeoJSONNode::parse_all(&head, model);
        head = GeoPointNode::parse_all(&head, model);
    }

    if model.is_unit_model() || model.is_facet_model() {
        if !parent_is_layer {
            head = super::encoding_nodes::make_bin_from_encoding(&head, model).unwrap_or(head);
        }
        head = super::encoding_nodes::make_timeunit_from_encoding(&head, model).unwrap_or(head);
        head = super::encoding_nodes::parse_all_for_sort_index(head, model);
    }

    // add an output node pre aggregation
    let raw = make_output_node(OutputNodeType::Raw, model, &head);
    head = raw.clone();
    component
        .output_nodes
        .insert(with_output_name(&raw), raw.clone());

    if model.is_unit_model() {
        if let Some(agg) = super::encoding_nodes::make_aggregate_from_encoding(&head, model) {
            head = agg;
            if requires_selection_id_for_model(model) {
                head = IdentifierNode::new(Some(&head));
            }
        }
        head = ImputeNode::make_from_encoding(Some(&head), model).unwrap_or(head);
        head = super::encoding_nodes::make_stack_from_encoding(&head, model).unwrap_or(head);
    }

    let mut pre_filter_invalid: Option<NodeRef> = None;
    let mut data_sources_for_invalid: Option<(String, String)> = None;
    if model.is_unit_model() {
        let mark_def = model.mark_def().cloned().unwrap_or(Value::Null);
        let mark = model.mark_type();
        let invalid = crate::mark::get_mark_prop_or_config(
            "invalid",
            &mark_def,
            &model.config,
            None,
            false,
        );
        let is_path = crate::mark::is_path_mark(mark.as_str());
        let data_sources = super::filterinvalid::get_data_sources_for_handling_invalid_values(
            invalid.as_ref(),
            is_path,
        );
        data_sources_for_invalid = Some((data_sources.marks.to_string(), data_sources.scales.to_string()));

        if data_sources.marks != data_sources.scales
            && data_sources.scales == "include-invalid-values"
        {
            let node = make_output_node(OutputNodeType::PreFilterInvalid, model, &head);
            head = node.clone();
            component
                .output_nodes
                .insert(with_output_name(&node), node.clone());
            // Scale domains consume this data source (upstream requests it via
            // `requestDataName`); keep the node alive in the optimized dataflow.
            OutputNode::mark_required(&node);
            pre_filter_invalid = Some(node);
        }

        if data_sources.marks == "exclude-invalid-values" {
            if let Some(node) = FilterInvalidNode::make(Some(&head), model, data_sources) {
                head = node;
            }
        }
    }

    // output "main" node for marks
    let main = make_output_node(OutputNodeType::Main, model, &head);
    head = main.clone();
    component
        .output_nodes
        .insert(with_output_name(&main), main.clone());

    // Port of `materializeSelections` (called by `parseData` right after the
    // main output node).  Each selection gets a lookup output node whose parent
    // is a filter on the main output; lookup transforms with `from.param` then
    // reference this node.
    if model.is_unit_model() {
        if let Value::Object(selections) = &model.component.selection {
            for (selection, sel_cmpt) in selections {
                if sel_cmpt
                    .get("inherited")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    continue;
                }
                let lookup_name = model.get_name(&format!("lookup_{selection}"));
                let filter_node =
                    FilterNode::new(Some(&main), Some(model), &json!({"param": selection}));
                let output =
                    OutputNode::new(Some(&filter_node), &lookup_name, OutputNodeType::Lookup);
                component.output_nodes.insert(lookup_name, output);
            }
        }
    }

    let mut post_filter_invalid: Option<NodeRef> = None;
    if model.is_unit_model() {
        if let Some((marks, scales)) = &data_sources_for_invalid {
            if marks == "include-invalid-values" && scales == "exclude-invalid-values" {
                let mark_def = model.mark_def().cloned().unwrap_or(Value::Null);
                let mark = model.mark_type();
                let invalid = crate::mark::get_mark_prop_or_config(
                    "invalid",
                    &mark_def,
                    &model.config,
                    None,
                    false,
                );
                if let Some(node) = FilterInvalidNode::make(
                    Some(&head),
                    model,
                    super::filterinvalid::get_data_sources_for_handling_invalid_values(
                        invalid.as_ref(),
                        crate::mark::is_path_mark(mark.as_str()),
                    ),
                ) {
                    head = node;
                }
                let node = make_output_node(OutputNodeType::PostFilterInvalid, model, &head);
                head = node.clone();
                component
                    .output_nodes
                    .insert(with_output_name(&node), node.clone());
                // Scale domains consume this data source (upstream requests it
                // via `requestDataName`); keep the node alive.
                OutputNode::mark_required(&node);
                post_filter_invalid = Some(node);
            }
        }
    }

    // add facet marker
    let mut facet_root = None;
    if model.is_facet_model() {
        let facet_name = model.get_name("facet");
        // Upstream calls `main.getSource()` (marking main required) when the
        // facet node is constructed. In this single-component port that keeps
        // the main output alive; only do so where it affects the optimized
        // dataflow shape (nested facets and custom-sorted facet domains).
        let has_custom_sort = ["row", "column", "facet"].iter().any(|channel| {
            model
                .facet_data
                .facet
                .get(*channel)
                .and_then(|fd| fd.get("sort"))
                .map(|sort| {
                    crate::normalize_support::is_sort_field(sort) || sort.is_array()
                })
                .unwrap_or(false)
        });
        let has_child_facet = model
            .children
            .first()
            .map(|c| c.is_facet_model())
            .unwrap_or(false);
        let has_layer_child = model
            .children
            .first()
            .map(|c| c.is_layer_model())
            .unwrap_or(false);
        let single_facet_custom_sort =
            has_custom_sort && model.facet_field_def("facet").is_some();
        let needs_main = has_child_facet
            || single_facet_custom_sort
            || (has_custom_sort && has_layer_child)
            || model.parent_type == Some(crate::compile::model::SpecType::Facet);
        let main_source = if needs_main {
            super::dataflow::OutputNode::get_source(&main)
        } else {
            with_output_name(&main)
        };
        let node = FacetNode::new(Some(&head), model, &facet_name, &main_source);
        component.output_nodes.insert(facet_name.clone(), node.clone());
        facet_root = Some(node);
    }

    component.raw = Some(raw);
    component.main = Some(main);
    component.facet_root = facet_root.clone();
    component.pre_filter_invalid = pre_filter_invalid;
    component.post_filter_invalid = post_filter_invalid;
    component.ancestor_parse = Some(ancestor_parse);

    (Some(head), facet_root)
}

fn with_output_name(node: &NodeRef) -> String {
    super::dataflow::with_node::<OutputNode, _>(node, |n| n.name.clone()).unwrap_or_default()
}

/// Convenience for filtering data sources by type (kept for parity).
pub fn output_node_type_filter(node: &NodeRef, type_: OutputNodeType) -> bool {
    super::dataflow::with_node::<OutputNode, _>(node, |n| n.data_source_type == type_)
        .unwrap_or(false)
}

// Re-exported for tests that build nodes directly.
pub use super::dataflow::OutputNode as OutputNodeReexport;
pub type DataFlowNodeAlias = DataFlowNode;

//! Port of `src/compile/data/debug.ts` (console / Graphviz helpers).

use serde_json::Value;

use super::dataflow::{children, debug_name, dependent_fields, hash, produced_fields, NodeRef};
use super::optimize::check_links;
use super::source::SourceNode;

/// Print debug information for a dataflow tree.
pub fn print_debug_dataflow(node: &NodeRef) {
    let name = debug_name(node).unwrap_or_default();
    let kids: Vec<String> = children(node)
        .iter()
        .map(|c| debug_name(c).unwrap_or_default())
        .collect();
    crate::log::debug(format!("{name} -> {kids:?}"));
    for child in children(node) {
        print_debug_dataflow(&child);
    }
}

/// Return the dataflow tree as a Graphviz dot string.
pub fn dot_string(roots: &[NodeRef]) -> String {
    // check the graph before printing it since the logic below assumes a
    // consistent graph
    check_links(roots);

    let mut nodes: Vec<(String, String, String)> = Vec::new();
    let mut edges: Vec<(String, String)> = Vec::new();

    fn collect(
        node: &NodeRef,
        nodes: &mut Vec<(String, String, String)>,
        edges: &mut Vec<(String, String)>,
    ) {
        let id = format!("{:p}", std::rc::Rc::as_ptr(node));
        let label = label(node);
        let hash = if SourceNode::is_source(node) {
            let data = SourceNode::data(node);
            data.get("url")
                .or_else(|| data.get("name"))
                .map(|v| match v {
                    Value::String(s) => s.clone(),
                    other => crate::util::stringify(other),
                })
                .unwrap_or_else(|| debug_name(node).unwrap_or_default())
        } else {
            match hash(node) {
                Value::String(s) => s.replace('"', ""),
                other => crate::util::stringify(&other).replace('"', ""),
            }
        };
        nodes.push((id.clone(), label, hash));
        for child in children(node) {
            edges.push((id.clone(), format!("{:p}", std::rc::Rc::as_ptr(&child))));
            collect(&child, nodes, edges);
        }
    }

    fn type_label(node: &NodeRef) -> &'static str {
        use super::aggregate::AggregateNode;
        use super::bin::BinNode;
        use super::calculate::CalculateNode;
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
        use super::joinaggregate::JoinAggregateTransformNode;
        use super::loess::LoessTransformNode;
        use super::lookup::LookupNode;
        use super::pivot::PivotTransformNode;
        use super::quantile::QuantileTransformNode;
        use super::regression::RegressionTransformNode;
        use super::sample::SampleTransformNode;
        use super::sequence::SequenceNode;
        use super::stack::StackNode;
        use super::timeunit::TimeUnitNode;
        use super::window::WindowTransformNode;
        use super::dataflow::{OutputNode, PlaceholderDataFlowNode};

        macro_rules! check {
            ($t:ty, $name:expr) => {
                if super::dataflow::with_node::<$t, _>(node, |_| ()).is_some() {
                    return $name;
                }
            };
        }
        check!(PlaceholderDataFlowNode, "PlaceholderDataFlow");
        check!(SourceNode, "Source");
        check!(ParseNode, "Parse");
        check!(BinNode, "Bin");
        check!(AggregateNode, "Aggregate");
        check!(CalculateNode, "Calculate");
        check!(FilterNode, "Filter");
        check!(FilterInvalidNode, "FilterInvalid");
        check!(FlattenTransformNode, "FlattenTransform");
        check!(FoldTransformNode, "FoldTransform");
        check!(DensityTransformNode, "DensityTransform");
        check!(ExtentTransformNode, "ExtentTransform");
        check!(FacetNode, "Facet");
        check!(GeoJSONNode, "GeoJSON");
        check!(GeoPointNode, "GeoPoint");
        check!(GraticuleNode, "Graticule");
        check!(IdentifierNode, "Identifier");
        check!(ImputeNode, "Impute");
        check!(JoinAggregateTransformNode, "JoinAggregateTransform");
        check!(LoessTransformNode, "LoessTransform");
        check!(LookupNode, "Lookup");
        check!(PivotTransformNode, "PivotTransform");
        check!(QuantileTransformNode, "QuantileTransform");
        check!(RegressionTransformNode, "RegressionTransform");
        check!(SampleTransformNode, "SampleTransform");
        check!(SequenceNode, "Sequence");
        check!(StackNode, "Stack");
        check!(TimeUnitNode, "TimeUnit");
        check!(WindowTransformNode, "WindowTransform");
        check!(OutputNode, "Output");
        "DataFlowNode"
    }

    fn label(node: &NodeRef) -> String {
        let mut out = vec![type_label(node).to_string()];
        if let Some(name) = debug_name(node) {
            out.push(format!("<i>{name}</i>"));
        } else if SourceNode::is_source(node) {
            let data = SourceNode::data(node);
            if let Some(v) = data.get("name").or_else(|| data.get("url")) {
                let s = match v {
                    Value::String(s) => s.clone(),
                    other => crate::util::stringify(other),
                };
                out.push(format!("<i>{s}</i>"));
            }
        }
        let dep = dependent_fields(node);
        if !dep.is_empty() {
            out.push(format!(
                "<font color=\"grey\" point-size=\"10\">IN:</font> {}",
                dep.iter().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
        let prod = produced_fields(node);
        if let Some(prod) = prod {
            if !prod.is_empty() {
                out.push(format!(
                    "<font color=\"grey\" point-size=\"10\">OUT:</font> {}",
                    prod.iter().cloned().collect::<Vec<_>>().join(", ")
                ));
            }
        }
        out.join("<br/>")
    }

    for root in roots {
        collect(root, &mut nodes, &mut edges);
    }

    let mut dot = String::from("digraph DataFlow {\n  rankdir = TB;\n  node [shape=record]\n");
    for (key, value, hash) in &nodes {
        dot.push_str(&format!(
            "  \"{key}\" [\n    label = <{value}>;\n    tooltip = \"[{key}]&#010;{hash}\"\n  ]\n"
        ));
    }
    for (source, target) in &edges {
        dot.push_str(&format!("\"{source}\" -> \"{target}\" "));
    }
    dot.push_str("\n}");
    dot
}

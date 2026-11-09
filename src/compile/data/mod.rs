//! Port of `src/compile/data/` (data-transform compilation).
//!
//! All dataflow nodes from the TypeScript implementation are ported here,
//! together with the data assembly (`assemble`), parsing (`parse`,
//! `formatparse`) and optimization layers.

// Node constructors are factory methods that return a boxed `NodeRef` (the
// dataflow node trait object), mirroring the upstream `new AggregateNode(...)`
// pattern. They intentionally do not return `Self`.
#![allow(clippy::new_ret_no_self)]

pub mod aggregate;
pub mod assemble;
pub mod bin;
pub mod calculate;
pub mod dataflow;
pub mod debug;
pub mod density;
pub mod encoding_nodes;
pub mod extent;
pub mod expressions;
pub mod facet;
pub mod filter;
pub mod filterinvalid;
pub mod flatten;
pub mod fold;
pub mod formatparse;
pub mod geojson;
pub mod geopoint;
pub mod graticule;
pub mod identifier;
pub mod impute;
pub mod index;
pub mod joinaggregate;
pub mod loess;
pub mod lookup;
pub mod optimize;
pub mod optimizer;
pub mod optimizers;
pub mod parse;
pub mod pivot;
pub mod quantile;
pub mod regression;
pub mod sample;
pub mod sequence;
pub mod source;
pub mod stack;
pub mod subtree;
pub mod support;
pub mod timeunit;
pub mod window;

pub use aggregate::AggregateNode;
pub use assemble::{assemble_facet_data, assemble_root_data, VgData};
pub use bin::{get_bin_signal_name, BinComponent, BinNode};
pub use calculate::CalculateNode;
pub use dataflow::{
    add_child, assemble, children, clone_node, debug_name, dependent_fields, hash, num_children,
    parent, produced_fields, DataFlowNode, FieldSet, NodeBehavior, NodeRef, OutputNode,
    OutputNodeType, PlaceholderDataFlowNode,
};
pub use density::DensityTransformNode;
pub use extent::ExtentTransformNode;
pub use facet::{facet_lookup_key_expr, facet_lookup_key_field_name, FacetNode};
pub use filter::FilterNode;
pub use filterinvalid::{
    get_data_sources_for_handling_invalid_values, get_scale_invalid_data_mode,
    normalize_invalid_data_mode, DataSourcesForHandlingInvalidValues, FilterInvalidNode,
};
pub use flatten::FlattenTransformNode;
pub use fold::FoldTransformNode;
pub use formatparse::{
    get_implicit_from_encoding, get_implicit_from_filter_transform, get_implicit_from_selection,
    ParseNode,
};
pub use geojson::GeoJSONNode;
pub use geopoint::GeoPointNode;
pub use graticule::GraticuleNode;
pub use identifier::IdentifierNode;
pub use impute::ImputeNode;
pub use index::{AncestorParse, DataComponent};
pub use joinaggregate::JoinAggregateTransformNode;
pub use loess::LoessTransformNode;
pub use lookup::LookupNode;
pub use optimize::{check_links, optimize_dataflow, FACET_SCALE_PREFIX, MAX_OPTIMIZATION_RUNS};
pub use parse::{parse_data, parse_transform_array};
pub use pivot::PivotTransformNode;
pub use quantile::QuantileTransformNode;
pub use regression::RegressionTransformNode;
pub use sample::SampleTransformNode;
pub use sequence::SequenceNode;
pub use source::SourceNode;
pub use stack::{StackComponent, StackNode};
pub use support::{
    Model, SimpleModel, OFFSETTED_RECT_END_SUFFIX, OFFSETTED_RECT_START_SUFFIX,
};
pub use timeunit::{TimeUnitComponent, TimeUnitNode};
pub use window::WindowTransformNode;

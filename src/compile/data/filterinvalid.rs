//! Port of `src/compile/data/filterinvalid.ts`.

use std::any::Any;

use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use crate::channeldef::{get_field_def, vg_field_with, FieldRefOption};
use crate::mark::{get_mark_prop_or_config, is_path_mark};
use crate::scale::has_continuous_domain;
use crate::util::hash;

use super::dataflow::{DataFlowNode, FieldSet, NodeBehavior, NodeRef};
use super::support::hash_str;
use crate::compile::model::Model;

/// `Dict<TypedFieldDef<string>>`.
pub type FilterMap = IndexMap<String, Value>;

/// Port of `normalizeInvalidDataMode`.
pub fn normalize_invalid_data_mode(mode: Option<&Value>, is_path: bool) -> String {
    match mode {
        None => {
            if is_path {
                "break-paths-show-domains".to_string()
            } else {
                "filter".to_string()
            }
        }
        Some(Value::Null) => "show".to_string(),
        Some(Value::String(s)) if s == "break-paths-show-path-domains" => {
            if is_path {
                "break-paths-show-domains".to_string()
            } else {
                "filter".to_string()
            }
        }
        Some(Value::String(s)) => s.clone(),
        _ => {
            if is_path {
                "break-paths-show-domains".to_string()
            } else {
                "filter".to_string()
            }
        }
    }
}

/// Port of `DataSourcesForHandlingInvalidValues`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataSourcesForHandlingInvalidValues {
    pub marks: &'static str,
    pub scales: &'static str,
}

/// Port of `getDataSourcesForHandlingInvalidValues`.
pub fn get_data_sources_for_handling_invalid_values(
    invalid: Option<&Value>,
    is_path: bool,
) -> DataSourcesForHandlingInvalidValues {
    match normalize_invalid_data_mode(invalid, is_path).as_str() {
        "filter" => DataSourcesForHandlingInvalidValues {
            marks: "exclude-invalid-values",
            scales: "exclude-invalid-values",
        },
        "break-paths-show-domains" => DataSourcesForHandlingInvalidValues {
            marks: if is_path {
                "include-invalid-values"
            } else {
                "exclude-invalid-values"
            },
            scales: "include-invalid-values",
        },
        "break-paths-filter-domains" => DataSourcesForHandlingInvalidValues {
            marks: if is_path {
                "include-invalid-values"
            } else {
                "exclude-invalid-values"
            },
            scales: "exclude-invalid-values",
        },
        _ => DataSourcesForHandlingInvalidValues {
            marks: "include-invalid-values",
            scales: "include-invalid-values",
        },
    }
}

/// Port of `getScaleDataSourceForHandlingInvalidValues`.
///
/// Returns the `request_data_name` key ("main", "pre_filter_invalid" or
/// "post_filter_invalid") that scale domains should consume.
pub fn get_scale_data_source_for_handling_invalid_values(
    invalid: Option<&Value>,
    is_path: bool,
) -> &'static str {
    let ds = get_data_sources_for_handling_invalid_values(invalid, is_path);
    if ds.marks == ds.scales {
        "main"
    } else if ds.scales == "include-invalid-values" {
        "pre_filter_invalid"
    } else {
        "post_filter_invalid"
    }
}

/// Port of `getScaleInvalidDataMode`.
pub fn get_scale_invalid_data_mode(
    mark_def: &Value,
    config: &Value,
    scale_channel: &str,
    scale_type: Option<&str>,
    is_count_aggregate: bool,
) -> String {
    let continuous = scale_type
        .and_then(|t| t.parse::<crate::scale::ScaleType>().ok())
        .map(has_continuous_domain)
        .unwrap_or(false);
    if scale_type.is_none() || !continuous || is_count_aggregate {
        return "always-valid".to_string();
    }

    let is_path = is_path_mark(
        mark_def
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    let invalid = get_mark_prop_or_config("invalid", mark_def, config, None, false);
    let mode = normalize_invalid_data_mode(invalid.as_ref(), is_path);

    let scale_output_for_invalid = config
        .get("scale")
        .and_then(|s| s.get("invalid"))
        .and_then(|i| i.get(scale_channel))
        .map(|v| !v.is_null())
        .unwrap_or(false);
    if scale_output_for_invalid {
        return "show".to_string();
    }

    mode
}

#[derive(Debug, Clone, Default)]
pub struct FilterInvalidNode {
    pub filter: FilterMap,
}

impl FilterInvalidNode {
    pub fn new(parent: Option<&NodeRef>, filter: FilterMap) -> NodeRef {
        DataFlowNode::new(Box::new(FilterInvalidNode { filter }), parent, None)
    }

    /// Port of `FilterInvalidNode.make`.
    pub fn make(
        parent: Option<&NodeRef>,
        model: &Model,
        data_sources: DataSourcesForHandlingInvalidValues,
    ) -> Option<NodeRef> {
        if data_sources.marks == "include-invalid-values"
            && data_sources.scales == "include-invalid-values"
        {
            return None;
        }

        let mut filter = FilterMap::new();
        let mark_def = model.mark_def().cloned().unwrap_or(Value::Null);
        let encoding = model.encoding().cloned().unwrap_or_else(|| json!({}));

        if encoding.is_object() {
            // Upstream iterates `UNIT_CHANNELS` (encoding insertion order), so
            // when several channels share a field the last one in that order
            // wins.  `serde_json`'s map is sorted, hence the explicit order.
            for channel in crate::channel::UNIT_CHANNELS {
                let Some(channel_def) = encoding.get(channel) else {
                    continue;
                };
                let Some(field_def) = get_field_def(channel_def) else {
                    continue;
                };
                if !crate::channel::is_scale_channel(channel) {
                    continue;
                }
                let Some(scale_component) = model.get_scale_component(channel) else {
                    continue;
                };
                let scale_type = scale_component.get_type().map(|t| t.as_str().to_string());
                let aggregate = field_def.get("aggregate").cloned().unwrap_or(Value::Null);
                let is_count = match &aggregate {
                    Value::String(s) => crate::aggregate::is_counting_op_name(s),
                    Value::Object(o) => o
                        .get("op")
                        .and_then(Value::as_str)
                        .map(crate::aggregate::is_counting_op_name)
                        .unwrap_or(false),
                    _ => false,
                };

                let invalid_data_mode = get_scale_invalid_data_mode(
                    &mark_def,
                    &model.config,
                    channel,
                    scale_type.as_deref(),
                    is_count,
                );

                if invalid_data_mode != "show" && invalid_data_mode != "always-valid" {
                    if let Some(field) = field_def.get("field").and_then(Value::as_str) {
                        filter.insert(field.to_string(), field_def.clone());
                    }
                }
            }
        }

        if filter.is_empty() {
            return None;
        }

        Some(FilterInvalidNode::new(parent, filter))
    }

    pub fn clone_filter_invalid(node: &NodeRef) -> NodeRef {
        let filter = super::dataflow::with_node::<FilterInvalidNode, _>(node, |n| n.filter.clone())
            .unwrap_or_default();
        FilterInvalidNode::new(None, filter)
    }
}

impl NodeBehavior for FilterInvalidNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        Value::String(format!("FilterInvalid {}", hash_str(&hash(&json!(self.filter)))))
    }

    fn dependent_fields(&self) -> FieldSet {
        self.filter.keys().cloned().collect()
    }

    fn produced_fields(&self) -> Option<FieldSet> {
        Some(FieldSet::new())
    }

    fn assemble(&self) -> Value {
        let mut filters: Vec<String> = Vec::new();
        for field_def in self.filter.values() {
            let ref_ = vg_field_with(
                field_def,
                &FieldRefOption {
                    expr: Some("datum".to_string()),
                    ..Default::default()
                },
            );
            let type_ = field_def.get("type").and_then(Value::as_str).unwrap_or("");
            if type_ == "temporal" {
                filters.push(format!(
                    "(isDate({ref_}) || ({}))",
                    is_valid_finite_number_expr(&ref_)
                ));
            } else if type_ == "quantitative" {
                filters.push(is_valid_finite_number_expr(&ref_));
            }
        }

        if filters.is_empty() {
            Value::Null
        } else {
            let mut result = Map::new();
            result.insert("type".into(), json!("filter"));
            result.insert("expr".into(), json!(filters.join(" && ")));
            Value::Object(result)
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Port of `isValidFiniteNumberExpr`.
pub fn is_valid_finite_number_expr(ref_: &str) -> String {
    format!("isValid({ref_}) && isFinite(+{ref_})")
}

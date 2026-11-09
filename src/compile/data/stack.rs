//! Port of `src/compile/data/stack.ts` (the data stack node).

use std::any::Any;

use serde_json::{json, Map, Value};

use crate::util::hash;

use super::dataflow::{DataFlowNode, FieldSet, NodeBehavior, NodeRef};
use super::support::hash_str;

#[derive(Debug, Clone, Default)]
pub struct StackComponent {
    pub facetby: Vec<String>,
    pub dimension_field_defs: Vec<Value>,
    pub stack_field: String,
    pub stackby: Option<Vec<String>>,
    pub sort: Value,
    pub offset: String,
    pub impute: Option<bool>,
    pub groupby: Option<Vec<String>>,
    pub as_: [String; 2],
}

impl StackComponent {
    fn to_json(&self) -> Value {
        let mut obj = Map::new();
        obj.insert(
            "dimensionFieldDefs".into(),
            Value::Array(self.dimension_field_defs.clone()),
        );
        obj.insert(
            "facetby".into(),
            Value::Array(self.facetby.iter().map(|s| json!(s)).collect()),
        );
        obj.insert("stackField".into(), json!(self.stack_field));
        if let Some(stackby) = &self.stackby {
            obj.insert("stackby".into(), Value::Array(stackby.iter().map(|s| json!(s)).collect()));
        }
        obj.insert("sort".into(), self.sort.clone());
        obj.insert("offset".into(), json!(self.offset));
        if let Some(impute) = self.impute {
            obj.insert("impute".into(), json!(impute));
        }
        if let Some(groupby) = &self.groupby {
            obj.insert("groupby".into(), Value::Array(groupby.iter().map(|s| json!(s)).collect()));
        }
        obj.insert("as".into(), json!([self.as_[0], self.as_[1]]));
        Value::Object(obj)
    }
}

#[derive(Debug, Clone, Default)]
pub struct StackNode {
    pub stack: StackComponent,
}

fn string_array(value: Option<&Value>) -> Option<Vec<String>> {
    value.and_then(Value::as_array).map(|a| {
        a.iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect()
    })
}

fn is_valid_as_array(as_: &Value) -> bool {
    as_.as_array()
        .map(|a| a.len() > 1 && a.iter().all(Value::is_string))
        .unwrap_or(false)
}

impl StackNode {
    pub fn new(parent: Option<&NodeRef>, stack: StackComponent) -> NodeRef {
        DataFlowNode::new(Box::new(StackNode { stack }), parent, None)
    }

    pub fn make_from_transform(parent: Option<&NodeRef>, stack_transform: &Value) -> NodeRef {
        let stack = stack_transform
            .get("stack")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let groupby = string_array(stack_transform.get("groupby"));
        let as_ = stack_transform.get("as").unwrap_or(&Value::Null);
        let offset = stack_transform
            .get("offset")
            .and_then(Value::as_str)
            .unwrap_or("zero")
            .to_string();

        let mut sort_fields: Vec<Value> = Vec::new();
        let mut sort_order: Vec<Value> = Vec::new();
        if let Some(sort) = stack_transform.get("sort").and_then(Value::as_array) {
            for sort_field in sort {
                sort_fields.push(sort_field.get("field").cloned().unwrap_or(Value::Null));
                sort_order.push(json!(sort_field
                    .get("order")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .unwrap_or("ascending")));
            }
        }

        let normalized_as = if is_valid_as_array(as_) {
            let a = as_.as_array().unwrap();
            [
                a[0].as_str().unwrap_or_default().to_string(),
                a[1].as_str().unwrap_or_default().to_string(),
            ]
        } else if let Some(s) = as_.as_str() {
            [s.to_string(), format!("{s}_end")]
        } else {
            [format!("{stack}_start"), format!("{stack}_end")]
        };

        StackNode::new(
            parent,
            StackComponent {
                dimension_field_defs: Vec::new(),
                stack_field: stack,
                groupby,
                offset,
                sort: json!({"field": sort_fields, "order": sort_order}),
                facetby: Vec::new(),
                as_: normalized_as,
                ..Default::default()
            },
        )
    }

    pub fn add_dimensions(node: &NodeRef, fields: &[String]) {
        super::dataflow::with_node_mut::<StackNode, _>(node, |n| {
            n.stack.facetby.extend(fields.iter().cloned());
        });
    }

    fn groupby_fields(stack: &StackComponent) -> Vec<String> {
        if !stack.dimension_field_defs.is_empty() {
            let impute = stack.impute.unwrap_or(false);
            let mut out = Vec::new();
            for def in &stack.dimension_field_defs {
                let bin = def.get("bin").cloned().unwrap_or(Value::Null);
                if crate::channeldef_support::truthy(&bin) {
                    // Upstream checks `dimensionFieldDef.bin` for truthiness,
                    // so an already-binned field (`bin: "binned"`) also yields
                    // both the start and end refs (which may be identical).
                    if impute {
                        out.push(crate::channeldef::vg_field_with(
                            def,
                            &crate::channeldef::FieldRefOption {
                                bin_suffix: Some("mid".to_string()),
                                ..Default::default()
                            },
                        ));
                    } else {
                        out.push(crate::channeldef::vg_field_with(
                            def,
                            &crate::channeldef::FieldRefOption::default(),
                        ));
                        out.push(crate::channeldef::vg_field_with(
                            def,
                            &crate::channeldef::FieldRefOption {
                                bin_suffix: Some("end".to_string()),
                                ..Default::default()
                            },
                        ));
                    }
                    continue;
                }
                out.push(crate::channeldef::vg_field_with(
                    def,
                    &crate::channeldef::FieldRefOption::default(),
                ));
            }
            out
        } else {
            stack.groupby.clone().unwrap_or_default()
        }
    }
}

impl NodeBehavior for StackNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        Value::String(format!("Stack {}", hash_str(&hash(&self.stack.to_json()))))
    }

    fn dependent_fields(&self) -> FieldSet {
        let mut out = FieldSet::new();
        out.insert(self.stack.stack_field.clone());
        for f in StackNode::groupby_fields(&self.stack) {
            out.insert(f);
        }
        for f in &self.stack.facetby {
            out.insert(f.clone());
        }
        if let Some(sort_fields) = self.stack.sort.get("field").and_then(Value::as_array) {
            for f in sort_fields.iter().filter_map(Value::as_str) {
                out.insert(f.to_string());
            }
        }
        out
    }

    fn produced_fields(&self) -> Option<FieldSet> {
        Some(FieldSet::from([
            self.stack.as_[0].clone(),
            self.stack.as_[1].clone(),
        ]))
    }

    fn assemble(&self) -> Value {
        let mut transform: Vec<Value> = Vec::new();
        let stack = &self.stack;

        if stack.impute.unwrap_or(false) {
            for def in &stack.dimension_field_defs {
                let band_position = def.get("bandPosition").and_then(Value::as_f64).unwrap_or(0.5);
                let bin = def.get("bin").filter(|v| !v.is_null());

                let bin_mid_key = if bin.is_some() {
                    let bin_start_name = crate::channeldef::vg_field_with(
                        def,
                        &crate::channeldef::FieldRefOption {
                            expr: Some("datum".to_string()),
                            ..Default::default()
                        },
                    );
                    let bin_end_name = crate::channeldef::vg_field_with(
                        def,
                        &crate::channeldef::FieldRefOption {
                            expr: Some("datum".to_string()),
                            bin_suffix: Some("end".to_string()),
                            ..Default::default()
                        },
                    );
                    let bin_start = bin_start_name;
                    let bin_end = bin_end_name;
                    transform.push(json!({
                        "type": "formula",
                        "expr": format!(
                            "isValid({bin_start}) && isFinite(+{bin_start}) ? {bp}*{bin_start}+{one_minus}*{bin_end} : {bin_start}",
                            bp = crate::util::js_number_to_string(band_position),
                            one_minus = crate::util::js_number_to_string(1.0 - band_position),
                        ),
                        "as": crate::channeldef::vg_field_with(
                            def,
                            &crate::channeldef::FieldRefOption {
                                bin_suffix: Some("mid".to_string()),
                                for_as: true,
                                ..Default::default()
                            },
                        ),
                    }));
                    Some(crate::channeldef::vg_field_with(
                        def,
                        &crate::channeldef::FieldRefOption {
                            bin_suffix: Some("mid".to_string()),
                            ..Default::default()
                        },
                    ))
                } else {
                    None
                };

                let key = bin_mid_key
                    .unwrap_or_else(|| crate::channeldef::vg_field(def));
                let mut groupby: Vec<String> =
                    stack.stackby.clone().unwrap_or_default();
                groupby.extend(stack.facetby.iter().cloned());
                transform.push(json!({
                    "type": "impute",
                    "field": stack.stack_field,
                    "groupby": groupby,
                    "key": key,
                    "method": "value",
                    "value": 0,
                }));
            }
        }

        let mut groupby = StackNode::groupby_fields(stack);
        groupby.extend(stack.facetby.iter().cloned());

        transform.push(json!({
            "type": "stack",
            "groupby": groupby,
            "field": stack.stack_field,
            "sort": stack.sort,
            "as": [stack.as_[0], stack.as_[1]],
            "offset": stack.offset,
        }));

        Value::Array(transform)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}


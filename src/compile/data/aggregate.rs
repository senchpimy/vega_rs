//! Port of `src/compile/data/aggregate.ts`.

use std::any::Any;

use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Map, Value};

use crate::log;
use crate::util::replace_path_in_field;

use super::dataflow::{DataFlowNode, FieldSet, NodeBehavior, NodeRef};
use super::support::{
    default_op_field_name, get_aggregate_op, get_aggregate_param, hash_str, number_value, set_json,
};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Measure {
    pub aliases: IndexSet<String>,
    pub aggregate_param: Option<f64>,
}

pub type OpMeasures = IndexMap<String, Measure>;
pub type Measures = IndexMap<String, OpMeasures>;

#[derive(Debug, Clone, Default)]
pub struct AggregateNode {
    pub dimensions: IndexSet<String>,
    pub measures: Measures,
}

impl AggregateNode {
    pub fn new(
        parent: Option<&NodeRef>,
        dimensions: IndexSet<String>,
        measures: Measures,
    ) -> NodeRef {
        DataFlowNode::new(
            Box::new(AggregateNode {
                dimensions,
                measures,
            }),
            parent,
            None,
        )
    }

    pub fn make_from_transform(parent: Option<&NodeRef>, t: &Value) -> Option<NodeRef> {
        let mut dims = IndexSet::new();
        let mut meas = Measures::new();

        if let Some(aggregate) = t.get("aggregate").and_then(Value::as_array) {
            for s in aggregate {
                let Some(op) = s.get("op").filter(|v| !v.is_null()) else {
                    continue;
                };
                let alias = s
                    .get("as")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| default_op_field_name(s));
                let op_name = get_aggregate_op(op);
                if op_name == "count" {
                    let field_measures = meas.entry("*".to_string()).or_default();
                    field_measures.insert(
                        "count".to_string(),
                        Measure {
                            aliases: IndexSet::from([alias]),
                            aggregate_param: None,
                        },
                    );
                } else {
                    let field = s
                        .get("field")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let field_measures = meas.entry(field).or_default();
                    let entry = field_measures.entry(op_name).or_insert_with(|| Measure {
                        aliases: IndexSet::new(),
                        aggregate_param: get_aggregate_param(op),
                    });
                    entry.aliases.insert(alias);
                    if entry.aggregate_param.is_none() {
                        entry.aggregate_param = get_aggregate_param(op);
                    }
                }
            }
        }

        if let Some(groupby) = t.get("groupby").and_then(Value::as_array) {
            for s in groupby {
                if let Some(field) = s.as_str() {
                    dims.insert(field.to_string());
                }
            }
        }

        if dims.len() + meas.len() == 0 {
            return None;
        }

        Some(AggregateNode::new(parent, dims, meas))
    }

    pub fn group_by(&self) -> &IndexSet<String> {
        &self.dimensions
    }

    pub fn add_dimensions(node: &NodeRef, fields: &[String]) {
        super::dataflow::with_node_mut::<AggregateNode, _>(node, |n| {
            for f in fields {
                n.dimensions.insert(f.clone());
            }
        });
    }

    pub fn merge(node: &NodeRef, other: &NodeRef) -> bool {
        let other_dims = super::dataflow::with_node::<AggregateNode, _>(other, |o| o.dimensions.clone())
            .unwrap_or_default();
        let same = super::dataflow::with_node::<AggregateNode, _>(node, |n| {
            n.dimensions.len() == other_dims.len()
                && n.dimensions.iter().all(|d| other_dims.contains(d))
        })
        .unwrap_or(false);

        if !same {
            log::debug("different dimensions, cannot merge");
            return false;
        }

        let other_measures =
            super::dataflow::with_node::<AggregateNode, _>(other, |o| o.measures.clone())
                .unwrap_or_default();
        super::dataflow::with_node_mut::<AggregateNode, _>(node, |n| {
            merge_measures(&mut n.measures, &other_measures);
        });
        true
    }

    fn measures_json(&self) -> Value {
        let mut measures = Map::new();
        for (field, ops) in &self.measures {
            let mut op_map = Map::new();
            for (op, measure) in ops {
                let mut m = Map::new();
                m.insert("aliases".into(), set_json(&measure.aliases));
                if let Some(p) = measure.aggregate_param {
                    m.insert("aggregateParam".into(), number_value(p));
                }
                op_map.insert(op.clone(), Value::Object(m));
            }
            measures.insert(field.clone(), Value::Object(op_map));
        }
        Value::Object(measures)
    }
}

fn merge_measures(parent: &mut Measures, child: &Measures) {
    for (field, ops) in child {
        for (op, child_measure) in ops {
            if let Some(parent_ops) = parent.get_mut(field) {
                let existing = parent_ops.get_mut(op);
                match existing {
                    Some(parent_measure) => {
                        if let (Some(pp), Some(cp)) =
                            (parent_measure.aggregate_param, child_measure.aggregate_param)
                        {
                            if pp != cp {
                                log::warn(format!(
                                    "The \"{op}\" aggregate of the field \"{field}\" is used with different parameters ({pp} and {cp}). Using {cp}."
                                ));
                            }
                        }
                        for alias in &child_measure.aliases {
                            parent_measure.aliases.insert(alias.clone());
                        }
                        if child_measure.aggregate_param.is_some() {
                            parent_measure.aggregate_param = child_measure.aggregate_param;
                        }
                    }
                    None => {
                        parent_ops.insert(op.clone(), child_measure.clone());
                    }
                }
            } else {
                let mut map = OpMeasures::new();
                map.insert(op.clone(), child_measure.clone());
                parent.insert(field.clone(), map);
            }
        }
    }
}

impl NodeBehavior for AggregateNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        let obj = json!({
            "dimensions": set_json(&self.dimensions),
            "measures": self.measures_json(),
        });
        Value::String(format!("Aggregate {}", hash_str(&obj)))
    }

    fn dependent_fields(&self) -> FieldSet {
        let mut set = self.dimensions.clone();
        for field in self.measures.keys() {
            set.insert(field.clone());
        }
        set
    }

    fn produced_fields(&self) -> Option<FieldSet> {
        let mut out = FieldSet::new();
        for (field, ops) in &self.measures {
            for (op, measure) in ops {
                if measure.aliases.is_empty() {
                    out.insert(format!("{op}_{field}"));
                } else {
                    for alias in &measure.aliases {
                        out.insert(alias.clone());
                    }
                }
            }
        }
        Some(out)
    }

    fn assemble(&self) -> Value {
        let mut ops: Vec<Value> = Vec::new();
        let mut fields: Vec<Value> = Vec::new();
        let mut as_: Vec<Value> = Vec::new();
        let mut aggregate_params: Vec<Value> = Vec::new();

        for (field, field_measures) in &self.measures {
            for (op, measure) in field_measures {
                for alias in &measure.aliases {
                    as_.push(json!(alias));
                    ops.push(json!(op));
                    fields.push(if field == "*" {
                        Value::Null
                    } else {
                        json!(replace_path_in_field(field))
                    });
                    aggregate_params.push(
                        measure
                            .aggregate_param
                            .map(number_value)
                            .unwrap_or(Value::Null),
                    );
                }
            }
        }

        let mut result = Map::new();
        result.insert("type".into(), json!("aggregate"));
        result.insert(
            "groupby".into(),
            Value::Array(
                self.dimensions
                    .iter()
                    .map(|d| json!(replace_path_in_field(d)))
                    .collect(),
            ),
        );
        result.insert("ops".into(), Value::Array(ops));
        result.insert("fields".into(), Value::Array(fields));
        result.insert("as".into(), Value::Array(as_));

        if aggregate_params.iter().any(Value::is_number) {
            result.insert("aggregate_params".into(), Value::Array(aggregate_params));
        }

        Value::Object(result)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}


//! Port of `src/compile/data/filter.ts`.

use std::any::Any;

use serde_json::{json, Map, Value};

use crate::compile::model::Model;
use crate::util::duplicate;

use super::dataflow::{DataFlowNode, FieldSet, NodeBehavior, NodeRef};
use super::support::{expression, get_dependent_fields};

#[derive(Debug, Clone)]
pub struct FilterNode {
    pub filter: Value,
    pub expr: String,
    pub dependent_fields: FieldSet,
}

impl FilterNode {
    /// Port of `new FilterNode(parent, model, filter)`. The model is needed to
    /// resolve selection predicates (upstream `expression(model, filter)`).
    pub fn new(parent: Option<&NodeRef>, model: Option<&Model>, filter: &Value) -> NodeRef {
        let filter = duplicate(filter);
        let expr = match model {
            Some(model) => crate::compile_predicate::expression_with_model(model, &filter),
            None => expression(&filter),
        };
        let dependent_fields = get_dependent_fields(&expr);
        let node = DataFlowNode::new(
            Box::new(FilterNode {
                filter: filter.clone(),
                expr,
                dependent_fields,
            }),
            parent,
            None,
        );

        if let Some(model) = model {
            // `parseSelectionPredicate` inserts the projection's time unit
            // transform into the dataflow so the predicate can compare
            // against the projected field.
            for component in
                crate::compile::selection::parse::selection_predicate_time_units(model, &filter)
            {
                if let Some(obj) = component.as_object() {
                    let as_ = obj
                        .get("as")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    let field = obj
                        .get("field")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let time_unit = obj.get("timeUnit").cloned().unwrap_or(Value::Null);
                    let unit = super::timeunit::TimeUnitComponent {
                        as_,
                        field,
                        time_unit,
                        rect_band_position: None,
                    };
                    let key = super::support::hash_str(&unit_to_json(&unit));
                    let mut units = super::timeunit::TimeUnits::new();
                    units.insert(key, unit);
                    let tunode = super::timeunit::TimeUnitNode::new(None, units);
                    super::dataflow::insert_as_parent_of(&tunode, &node);
                }
            }
        }

        node
    }
}

fn unit_to_json(unit: &super::timeunit::TimeUnitComponent) -> Value {
    let mut obj = Map::new();
    if let Some(as_) = &unit.as_ {
        obj.insert("as".into(), json!(as_));
    }
    obj.insert("field".into(), json!(unit.field));
    obj.insert("timeUnit".into(), unit.time_unit.clone());
    Value::Object(obj)
}

impl NodeBehavior for FilterNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        Value::String(format!("Filter {}", self.expr))
    }

    fn dependent_fields(&self) -> FieldSet {
        self.dependent_fields.clone()
    }

    fn produced_fields(&self) -> Option<FieldSet> {
        Some(FieldSet::new())
    }

    fn assemble(&self) -> Value {
        let mut result = Map::new();
        result.insert("type".into(), json!("filter"));
        result.insert("expr".into(), json!(self.expr));
        Value::Object(result)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

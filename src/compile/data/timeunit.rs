//! Port of `src/compile/data/timeunit.ts`.

use std::any::Any;

use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use crate::util::{
    access_with_datum_to_unescaped_path, hash, replace_path_in_field,
    unescape_single_quote_and_path_dot,
};

use super::dataflow::{DataFlowNode, FieldSet, NodeBehavior, NodeRef};
use super::support::{
    get_time_unit_parts, hash_str, interpolate_expr, normalize_time_unit, offset_expr,
    OFFSETTED_RECT_END_SUFFIX, OFFSETTED_RECT_START_SUFFIX,
};

#[derive(Debug, Clone)]
pub struct TimeUnitComponent {
    pub as_: Option<String>,
    pub field: String,
    pub time_unit: Value,
    pub rect_band_position: Option<f64>,
}

impl TimeUnitComponent {
    fn is_transform_component(&self) -> bool {
        self.as_.is_some()
    }

    fn offset_as(&self) -> String {
        format!("{}_end", self.field)
    }

    fn to_json(&self) -> Value {
        let mut obj = Map::new();
        if let Some(as_) = &self.as_ {
            obj.insert("as".into(), json!(as_));
        }
        obj.insert("field".into(), json!(self.field));
        obj.insert("timeUnit".into(), self.time_unit.clone());
        Value::Object(obj)
    }
}

pub type TimeUnits = IndexMap<String, TimeUnitComponent>;

#[derive(Debug, Clone, Default)]
pub struct TimeUnitNode {
    pub time_units: TimeUnits,
}

impl TimeUnitNode {
    pub fn new(parent: Option<&NodeRef>, time_units: TimeUnits) -> NodeRef {
        DataFlowNode::new(Box::new(TimeUnitNode { time_units }), parent, None)
    }

    pub fn make_from_transform(parent: Option<&NodeRef>, t: &Value) -> NodeRef {
        let time_unit = normalize_time_unit(t.get("timeUnit").unwrap_or(&Value::Null))
            .unwrap_or(Value::Null);
        let component = TimeUnitComponent {
            as_: t.get("as").and_then(Value::as_str).map(str::to_string),
            field: t
                .get("field")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            time_unit,
            rect_band_position: None,
        };
        let key = hash_str(&hash(&component.to_json()));
        let mut time_units = TimeUnits::new();
        time_units.insert(key, component);
        TimeUnitNode::new(parent, time_units)
    }

    pub fn merge(node: &NodeRef, other: &NodeRef) {
        let other_units = super::dataflow::with_node::<TimeUnitNode, _>(other, |o| o.time_units.clone())
            .unwrap_or_default();
        super::dataflow::with_node_mut::<TimeUnitNode, _>(node, |n| {
            for (key, unit) in other_units {
                // Components may be constructed from selection projections and
                // encoding field defs with equivalent but differently-keyed
                // time units; keep only one transform for the same component.
                if n.time_units
                    .values()
                    .any(|existing| existing.to_json() == unit.to_json())
                {
                    continue;
                }
                n.time_units.entry(key).or_insert(unit);
            }
        });

        for child in super::dataflow::children(other) {
            super::dataflow::remove_child(other, &child);
            super::dataflow::set_parent(&child, Some(node));
        }
        super::dataflow::remove(other);
    }

    pub fn remove_formulas(node: &NodeRef, fields: &FieldSet) {
        super::dataflow::with_node_mut::<TimeUnitNode, _>(node, |n| {
            n.time_units.retain(|_, component| {
                let field_as = if component.is_transform_component() {
                    component.as_.clone().unwrap_or_default()
                } else {
                    component.offset_as()
                };
                !fields.contains(&field_as)
            });
        });
    }
}

impl NodeBehavior for TimeUnitNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        let mut obj = Map::new();
        for (key, component) in &self.time_units {
            obj.insert(key.clone(), component.to_json());
        }
        Value::String(format!("TimeUnit {}", hash_str(&hash(&Value::Object(obj)))))
    }

    fn dependent_fields(&self) -> FieldSet {
        self.time_units.values().map(|c| c.field.clone()).collect()
    }

    fn produced_fields(&self) -> Option<FieldSet> {
        Some(
            self.time_units
                .values()
                .map(|c| {
                    if c.is_transform_component() {
                        c.as_.clone().unwrap_or_default()
                    } else {
                        c.offset_as()
                    }
                })
                .collect(),
        )
    }

    fn assemble(&self) -> Value {
        let mut transforms: Vec<Value> = Vec::new();

        for f in self.time_units.values() {
            let normalized = normalize_time_unit(&f.time_unit).unwrap_or_default();
            let rect_band_position = f.rect_band_position;

            if f.is_transform_component() {
                let as_ = f.as_.clone().unwrap_or_default();
                let start_end = [as_.clone(), format!("{as_}_end")];

                let mut transform = Map::new();
                transform.insert("field".into(), json!(replace_path_in_field(&f.field)));
                transform.insert("type".into(), json!("timeunit"));
                if let Some(unit) = normalized.get("unit").and_then(Value::as_str) {
                    transform.insert(
                        "units".into(),
                        Value::Array(
                            get_time_unit_parts(unit)
                                .into_iter()
                                .map(Value::String)
                                .collect(),
                        ),
                    );
                }
                if normalized.get("utc").and_then(Value::as_bool).unwrap_or(false) {
                    transform.insert("timezone".into(), json!("utc"));
                }
                for (k, v) in normalized.as_object().unwrap() {
                    if k != "unit" && k != "utc" {
                        transform.insert(k.clone(), v.clone());
                    }
                }
                transform.insert("as".into(), json!(start_end));
                transforms.push(Value::Object(transform));

                transforms.extend(offseted_rect_formulas(
                    &start_end,
                    rect_band_position,
                    &normalized,
                ));
            } else {
                let escaped_field = f.field.clone();
                let field = unescape_single_quote_and_path_dot(&escaped_field);
                let expr = offset_expr(&normalized, &field, false);
                let end_as = format!("{field}_end");
                transforms.push(json!({
                    "type": "formula",
                    "expr": expr,
                    "as": end_as,
                }));

                transforms.extend(offseted_rect_formulas(
                    &[field, end_as],
                    rect_band_position,
                    &normalized,
                ));
            }
        }

        Value::Array(transforms)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

fn offseted_rect_formulas(
    fields: &[String; 2],
    rect_band_position: Option<f64>,
    time_unit: &Value,
) -> Vec<Value> {
    let Some(rect_band_position) = rect_band_position else {
        return Vec::new();
    };
    if rect_band_position == 0.5 {
        return Vec::new();
    }
    let start_field = &fields[0];
    let end_field = &fields[1];
    let start_expr = access_with_datum_to_unescaped_path(start_field);
    let end_expr = access_with_datum_to_unescaped_path(end_field);

    let start = offset_expr(time_unit, start_field, true);
    let fraction = rect_band_position + 0.5;

    vec![
        json!({
            "type": "formula",
            "expr": interpolate_expr(&start, &start_expr, fraction),
            "as": format!("{start_field}_{OFFSETTED_RECT_START_SUFFIX}"),
        }),
        json!({
            "type": "formula",
            "expr": interpolate_expr(&start_expr, &end_expr, fraction),
            "as": format!("{start_field}_{OFFSETTED_RECT_END_SUFFIX}"),
        }),
    ]
}


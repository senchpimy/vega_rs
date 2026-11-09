//! Port of `src/compile/data/bin.ts`.

use std::any::Any;

use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use crate::util::{hash, replace_path_in_field, unique};

use super::dataflow::{DataFlowNode, FieldSet, NodeBehavior, NodeRef};
use super::support::{
    bin_key, hash_str, is_parameter_extent, normalize_bin, Model,
};

#[derive(Debug, Clone, PartialEq)]
pub struct BinComponent {
    pub bin: Map<String, Value>,
    pub field: String,
    pub extent_signal: Option<String>,
    pub signal: Option<String>,
    pub span: Option<String>,
    /// Pairs of names of start and end signals.
    pub as_: Vec<[String; 2]>,
    pub formula: Option<String>,
    pub formula_as: Option<String>,
}

impl BinComponent {
    /// Convenience constructor mirroring the object literals used in tests.
    pub fn new(bin: Map<String, Value>, field: &str, as_pairs: Vec<[String; 2]>) -> Self {
        BinComponent {
            bin,
            field: field.to_string(),
            extent_signal: None,
            signal: None,
            span: None,
            as_: as_pairs,
            formula: None,
            formula_as: None,
        }
    }
}

pub type Bins = IndexMap<String, BinComponent>;

#[derive(Debug, Clone, Default)]
pub struct BinNode {
    pub bins: Bins,
}

impl BinNode {
    pub fn new(parent: Option<&NodeRef>, bins: Bins) -> NodeRef {
        DataFlowNode::new(Box::new(BinNode { bins }), parent, None)
    }

    pub fn make_from_transform(parent: Option<&NodeRef>, t: &Value, model: &dyn Model) -> NodeRef {
        let (key, component) = create_bin_component(t, t.get("bin").unwrap_or(&Value::Null), model);
        let mut bins = Bins::new();
        bins.insert(key, component);
        BinNode::new(parent, bins)
    }

    pub fn merge(node: &NodeRef, other: &NodeRef, rename_signal: &mut dyn FnMut(&str, &str)) {
        let other_bins = super::dataflow::with_node::<BinNode, _>(other, |o| o.bins.clone())
            .unwrap_or_default();

        super::dataflow::with_node_mut::<BinNode, _>(node, |n| {
            for (key, other_component) in other_bins {
                if let Some(component) = n.bins.get_mut(&key) {
                    if let (Some(s1), Some(s2)) = (&other_component.signal, &component.signal) {
                        rename_signal(s1, s2);
                    }
                    let mut pairs: Vec<Value> = component
                        .as_
                        .iter()
                        .map(|p| json!([p[0], p[1]]))
                        .collect();
                    for pair in &other_component.as_ {
                        pairs.push(json!([pair[0], pair[1]]));
                    }
                    let unique_pairs = unique(&pairs, hash);
                    component.as_ = unique_pairs
                        .into_iter()
                        .filter_map(|v| {
                            let a = v.as_array()?;
                            Some([a.first()?.as_str()?.to_string(), a.get(1)?.as_str()?.to_string()])
                        })
                        .collect();
                } else {
                    n.bins.insert(key, other_component);
                }
            }
        });

        // Move children of `other` onto this node, then remove `other`.
        for child in super::dataflow::children(other) {
            super::dataflow::remove_child(other, &child);
            super::dataflow::set_parent(&child, Some(node));
        }
        super::dataflow::remove(other);
    }

    fn bins_json(&self) -> Value {
        let mut result = Map::new();
        for (key, c) in &self.bins {
            let mut obj = Map::new();
            obj.insert("bin".into(), Value::Object(c.bin.clone()));
            obj.insert("field".into(), json!(c.field));
            if let Some(s) = &c.extent_signal {
                obj.insert("extentSignal".into(), json!(s));
            }
            if let Some(s) = &c.signal {
                obj.insert("signal".into(), json!(s));
            }
            if let Some(s) = &c.span {
                obj.insert("span".into(), json!(s));
            }
            obj.insert(
                "as".into(),
                Value::Array(
                    c.as_
                        .iter()
                        .map(|p| json!([p[0], p[1]]))
                        .collect(),
                ),
            );
            if let Some(f) = &c.formula {
                obj.insert("formula".into(), json!(f));
            }
            if let Some(f) = &c.formula_as {
                obj.insert("formulaAs".into(), json!(f));
            }
            result.insert(key.clone(), Value::Object(obj));
        }
        Value::Object(result)
    }
}

fn create_bin_component(
    t: &Value,
    bin: &Value,
    model: &dyn Model,
) -> (String, BinComponent) {
    let as_ = match t.get("as") {
        Some(Value::String(s)) => [s.clone(), format!("{s}_end")],
        Some(Value::Array(a)) => [
            a.first().and_then(Value::as_str).unwrap_or_default().to_string(),
            a.get(1).and_then(Value::as_str).unwrap_or_default().to_string(),
        ],
        _ => [String::new(), String::new()],
    };

    let mut normalized = normalize_bin(bin);
    let field = t
        .get("field")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let key = bin_key(&normalized, &field);
    let signal = model.name(&format!("{key}_bins"));
    let extent_signal = model.name(&format!("{key}_extent"));

    let mut span = None;
    if let Some(extent) = normalized.get("extent").filter(|e| is_parameter_extent(e)) {
        let param = extent.get("param").and_then(Value::as_str).unwrap_or_default();
        span = Some(model.selection_extent(param, extent));
        normalized.remove("extent");
    }

    (
        key,
        BinComponent {
            bin: normalized,
            field,
            extent_signal: Some(extent_signal),
            signal: Some(signal),
            span,
            as_: vec![as_],
            formula: None,
            formula_as: None,
        },
    )
}

/// Port of `getBinSignalName`.
pub fn get_bin_signal_name(model: &dyn Model, field: &str, bin: &Value) -> String {
    let normalized = normalize_bin(bin);
    let key = bin_key(&normalized, field);
    model.name(&format!("{key}_bins"))
}

/// Port of the internal `getSignalsFromModel` (exposed for completeness).
pub fn get_signals_from_model(model: &dyn Model, key: &str) -> (String, String) {
    (
        model.name(&format!("{key}_bins")),
        model.name(&format!("{key}_extent")),
    )
}

impl NodeBehavior for BinNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Value {
        Value::String(format!("Bin {}", hash_str(&hash(&self.bins_json()))))
    }

    fn dependent_fields(&self) -> FieldSet {
        self.bins.values().map(|c| c.field.clone()).collect()
    }

    fn produced_fields(&self) -> Option<FieldSet> {
        let mut out = FieldSet::new();
        for c in self.bins.values() {
            for pair in &c.as_ {
                out.insert(pair[0].clone());
                out.insert(pair[1].clone());
            }
        }
        Some(out)
    }

    fn assemble(&self) -> Value {
        let mut transforms: Vec<Value> = Vec::new();

        for bin in self.bins.values() {
            let Some(bin_as) = bin.as_.first() else {
                continue;
            };
            let remaining_as = &bin.as_[1..];

            let extent = bin.bin.get("extent").cloned().unwrap_or(Value::Null);
            let mut bin_trans = Map::new();
            bin_trans.insert("type".into(), json!("bin"));
            bin_trans.insert("field".into(), json!(replace_path_in_field(&bin.field)));
            bin_trans.insert("as".into(), json!([bin_as[0], bin_as[1]]));
            if let Some(signal) = &bin.signal {
                bin_trans.insert("signal".into(), json!(signal));
            }
            if is_parameter_extent(&extent) {
                bin_trans.insert("extent".into(), Value::Null);
            } else {
                bin_trans.insert("extent".into(), extent.clone());
            }
            if let Some(span) = &bin.span {
                bin_trans.insert("span".into(), json!({"signal": format!("span({span})")}));
            }
            for (k, v) in &bin.bin {
                if k != "extent" {
                    bin_trans.insert(k.clone(), v.clone());
                }
            }

            if extent.is_null() {
                if let Some(extent_signal) = &bin.extent_signal {
                    transforms.push(json!({
                        "type": "extent",
                        "field": replace_path_in_field(&bin.field),
                        "signal": extent_signal,
                    }));
                    bin_trans.insert("extent".into(), json!({"signal": extent_signal}));
                }
            }

            transforms.push(Value::Object(bin_trans));

            for as_pair in remaining_as {
                for i in 0..2 {
                    transforms.push(json!({
                        "type": "formula",
                        "expr": crate::util::flat_access_with_datum(&bin_as[i], "datum"),
                        "as": as_pair[i],
                    }));
                }
            }

            if let Some(formula) = &bin.formula {
                transforms.push(json!({
                    "type": "formula",
                    "expr": formula,
                    "as": bin.formula_as,
                }));
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

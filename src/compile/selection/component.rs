//! Port of `src/compile/selection/index.ts`.
//!
//! The TypeScript compiler registry uses objects with optional methods.  In
//! Rust the registry is a slice of [`SelectionCompiler`] descriptors holding
//! function pointers, keeping the upstream ordering ("Order matters for parsing
//! and assembly").

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::compile::model::Model;
use crate::util::{keys, string_value, vals, var_name};

use super::message;

pub const STORE: &str = "_store";
pub const TUPLE: &str = "_tuple";
pub const MODIFY: &str = "_modify";
pub const SELECTION_DOMAIN: &str = "_selection_domain_";
pub const VL_SELECTION_RESOLVE: &str = "vlSelectionResolve";

/// Port of `SelectionProjection`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SelectionProjection {
    #[serde(rename = "type", default)]
    pub type_: String,
    #[serde(default)]
    pub field: String,
    #[serde(default)]
    pub index: usize,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub channel: Option<String>,
    #[serde(
        rename = "geoChannel",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub geo_channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub signals: Option<ProjectionSignals>,
    #[serde(rename = "hasLegend", default, skip_serializing_if = "is_false")]
    pub has_legend: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_zero(v: &usize) -> bool {
    *v == 0
}

/// The `signals` part of a selection projection.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProjectionSignals {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub visual: Option<String>,
}

impl ProjectionSignals {
    pub fn get(&self, range: &str) -> Option<&String> {
        if range == "visual" {
            self.visual.as_ref()
        } else {
            self.data.as_ref()
        }
    }
}

/// Port of `SelectionProjectionComponent`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SelectionProjectionComponent {
    #[serde(default)]
    pub items: Vec<SelectionProjection>,
    #[serde(rename = "hasChannel", default)]
    pub has_channel: BTreeMap<String, usize>,
    #[serde(rename = "hasField", default)]
    pub has_field: BTreeMap<String, usize>,
    #[serde(rename = "hasSelectionId", default)]
    pub has_selection_id: bool,
    #[serde(rename = "timeUnit", skip_serializing_if = "Option::is_none", default)]
    pub time_unit: Option<Value>,
}

impl SelectionProjectionComponent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn channel(&self, channel: &str) -> Option<&SelectionProjection> {
        self.has_channel
            .get(channel)
            .and_then(|i| self.items.get(*i))
    }

    pub fn channel_mut(&mut self, channel: &str) -> Option<&mut SelectionProjection> {
        let index = *self.has_channel.get(channel)?;
        self.items.get_mut(index)
    }

    pub fn field(&self, field: &str) -> Option<&SelectionProjection> {
        self.has_field.get(field).and_then(|i| self.items.get(*i))
    }

    pub fn field_mut(&mut self, field: &str) -> Option<&mut SelectionProjection> {
        let index = *self.has_field.get(field)?;
        self.items.get_mut(index)
    }
}

/// Port of `SelectionComponent`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SelectionComponent {
    pub name: String,
    /// Insertion index of the selection within its unit spec.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub order: usize,
    /// True for selections inherited from an ancestor multi-view model (used
    /// for lookups only; they never materialize signals/marks in this unit).
    #[serde(default, skip_serializing_if = "is_false")]
    pub inherited: bool,
    #[serde(rename = "type", default)]
    pub type_: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub init: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub events: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub bind: Option<Value>,
    #[serde(default = "default_resolve")]
    pub resolve: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub mark: Option<Value>,
    #[serde(default)]
    pub project: SelectionProjectionComponent,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub scales: Option<Vec<SelectionProjection>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub toggle: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub translate: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub zoom: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub nearest: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub clear: Option<Value>,
    /// Any remaining defaults from the selection definition (e.g. `on`).
    #[serde(flatten, default)]
    pub extra: Map<String, Value>,
}

fn default_resolve() -> String {
    "global".to_string()
}

impl SelectionComponent {
    pub fn truthy_toggle(&self) -> bool {
        truthy(self.toggle.as_ref())
    }

    pub fn truthy_translate(&self) -> bool {
        truthy(self.translate.as_ref())
    }

    pub fn truthy_zoom(&self) -> bool {
        truthy(self.zoom.as_ref())
    }

    pub fn truthy_nearest(&self) -> bool {
        truthy(self.nearest.as_ref())
    }
}

pub fn truthy_value(v: &Value) -> bool {
    truthy(Some(v))
}

pub fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0 && !f.is_nan()).unwrap_or(true),
        Some(Value::String(s)) => !s.is_empty(),
        _ => true,
    }
}

pub type ParseFn = fn(&mut Model, &mut SelectionComponent, &mut Value);
pub type SignalsFn = fn(&Model, &SelectionComponent, Vec<Value>) -> Vec<Value>;
pub type ModifyExprFn = fn(&Model, &SelectionComponent, &str) -> String;
pub type MarksFn = fn(&Model, &SelectionComponent, Vec<Value>) -> Vec<Value>;

/// Port of `SelectionCompiler`.
pub struct SelectionCompiler {
    pub defined: fn(&SelectionComponent) -> bool,
    pub parse: Option<ParseFn>,
    pub signals: Option<SignalsFn>,
    pub top_level_signals: Option<SignalsFn>,
    pub modify_expr: Option<ModifyExprFn>,
    pub marks: Option<MarksFn>,
}

/// `selectionCompilers`, in upstream order.
pub fn selection_compilers() -> Vec<SelectionCompiler> {
    vec![
        super::point::compiler(),
        super::interval::compiler(),
        super::project::compiler(),
        super::toggle::compiler(),
        // Bindings may disable direct manipulation.
        super::inputs::compiler(),
        super::scales::compiler(),
        super::legends::compiler(),
        super::clear::compiler(),
        super::translate::compiler(),
        super::zoom::compiler(),
        super::nearest::compiler(),
    ]
}

/// Port of `unitName`.
pub fn unit_name(model: &Model, escape: bool) -> String {
    let mut name = if escape {
        string_value(&Value::String(model.name.clone()))
    } else {
        model.name.clone()
    };
    if let Value::Object(components) = &model.component.selection {
        for component in components.values() {
            if let Some(suffix) = component
                .get(super::parse::FACET_SUFFIX_KEY)
                .and_then(Value::as_str)
            {
                name.push_str(suffix);
                break;
            }
        }
    }
    name
}

/// Port of `requiresSelectionId`.
pub fn requires_selection_id(model: &Model) -> bool {
    vals(&model.component.selection)
        .iter()
        .any(|v| {
            v.get("project")
                .and_then(|p| p.get("hasSelectionId"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
}

/// Port of `disableDirectManipulation`.
pub fn disable_direct_manipulation(sel_cmpt: &mut SelectionComponent, sel_def: &Value) {
    let select = sel_def.get("select");
    let select_is_string = select.map(|s| s.is_string()).unwrap_or(false);
    if select_is_string
        || !select
            .and_then(|s| s.get("on"))
            .map(truthy_value)
            .unwrap_or(false)
    {
        sel_cmpt.events = None;
    }
    if select_is_string
        || !select
            .and_then(|s| s.get("clear"))
            .map(truthy_value)
            .unwrap_or(false)
    {
        sel_cmpt.clear = None;
    }
    if select_is_string
        || !select
            .and_then(|s| s.get("toggle"))
            .map(truthy_value)
            .unwrap_or(false)
    {
        sel_cmpt.toggle = None;
    }
}

/// Port of `isTimerSelection`.
pub fn is_timer_selection(sel_cmpt: &SelectionComponent) -> bool {
    sel_cmpt
        .events
        .as_ref()
        .map(|events| {
            events
                .iter()
                .any(|e| e.get("type").and_then(Value::as_str) == Some("timer"))
        })
        .unwrap_or(false)
}

/// Retrieve `model.component.selection` as strong components.
pub fn selection_components(model: &Model) -> BTreeMap<String, SelectionComponent> {
    selection_components_shared(model)
        .iter()
        .map(|(k, v)| (k.clone(), (**v).clone()))
        .collect()
}

/// Memoized selection components behind `Rc`s, for callers that only need to
/// read them (avoids deserializing and deep-cloning on every lookup).
pub fn selection_components_shared(
    model: &Model,
) -> std::rc::Rc<BTreeMap<String, std::rc::Rc<SelectionComponent>>> {
    if let Some((cached_selection, cached)) = &*model.selection_cache.borrow() {
        if cached_selection == &model.component.selection {
            return cached.clone();
        }
    }
    let components: BTreeMap<String, std::rc::Rc<SelectionComponent>> =
        match &model.component.selection {
            Value::Object(map) => map
                .iter()
                .filter_map(|(k, v)| {
                    serde_json::from_value::<SelectionComponent>(v.clone())
                        .ok()
                        .map(|c| (k.clone(), std::rc::Rc::new(c)))
                })
                .collect(),
            _ => BTreeMap::new(),
        };
    let shared = std::rc::Rc::new(components);
    *model.selection_cache.borrow_mut() =
        Some((model.component.selection.clone(), shared.clone()));
    shared
}

/// Components in upstream insertion order.
pub fn ordered_components(model: &Model) -> Vec<std::rc::Rc<SelectionComponent>> {
    let mut components: Vec<std::rc::Rc<SelectionComponent>> = selection_components_shared(model)
        .values()
        .filter(|c| !c.inherited)
        .cloned()
        .collect();
    components.sort_by_key(|c| c.order);
    components
}

/// Store components on the model (emulating `model.component.selection = ...`).
pub fn set_selection_components(
    model: &mut Model,
    components: &BTreeMap<String, SelectionComponent>,
) {
    model.component.selection =
        serde_json::to_value(components).unwrap_or(Value::Object(Map::new()));
}

/// Port of `Model.getSelectionComponent` for the JSON backed component.
pub fn get_selection_component(
    model: &Model,
    name: &str,
    original_name: &str,
) -> Option<std::rc::Rc<SelectionComponent>> {
    let map = selection_components_shared(model);
    map.get(name)
        .or_else(|| map.get(original_name))
        .or_else(|| map.get(&var_name(original_name)))
        .cloned()
}

/// Apply the default selection config, returning the effective definition plus
/// the event stream derived from `on`.
pub fn merge_selection_defaults(
    raw: &Value,
    selection_config: &Value,
) -> (Map<String, Value>, String) {
    let select = raw.get("select").cloned().unwrap_or(Value::Null);
    let type_ = if select.is_string() {
        select.as_str().unwrap().to_string()
    } else {
        select
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };

    let mut defaults: Map<String, Value> = if select.is_object() {
        select.as_object().cloned().unwrap_or_default()
    } else {
        let mut m = Map::new();
        m.insert("type".to_string(), Value::String(type_.clone()));
        m
    };

    let cfg = selection_config.get(&type_).cloned().unwrap_or(Value::Null);
    let cfg = cfg.as_object().cloned().unwrap_or_default();
    for key in keys(&Value::Object(cfg.clone())) {
        if key == "fields" || key == "encodings" {
            continue;
        }
        if key == "mark" {
            let cfg_mark = cfg.get("mark").cloned().unwrap_or(Value::Null);
            let mut mark = cfg_mark.as_object().cloned().unwrap_or_default();
            if let Some(Value::Object(dm)) = defaults.get("mark") {
                for (k, v) in dm {
                    mark.insert(k.clone(), v.clone());
                }
            }
            defaults.insert("mark".to_string(), Value::Object(mark));
        }
        let current = defaults.get(&key).cloned();
        if current.is_none() || current == Some(Value::Bool(true)) {
            let value = cfg.get(&key).cloned().or(current).unwrap_or(Value::Null);
            defaults.insert(key, value);
        }
    }

    (defaults, type_)
}

#[allow(dead_code)]
fn _unused(_: &Model) {
    let _ = message::cannot_project_on_channel_without_field("x");
}

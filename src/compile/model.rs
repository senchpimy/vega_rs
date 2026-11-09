//! Port of `src/compile/model.ts`.
//!
//! The TypeScript version uses an abstract class hierarchy.  In this port a
//! single [`Model`] struct with a [`SpecType`] discriminator holds the common
//! state; variant-specific state lives in [`UnitData`] / [`FacetData`].  The
//! public methods mirror the upstream class so later ports of mark/axis/legend
//! can build on it.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use serde_json::{json, Map, Value};

use crate::channeldef::{self, is_typed_field_def, FieldRefOption};
use crate::util::var_name;

use super::projection::component::ProjectionComponent;
use super::scale::component::ScaleComponent;
use super::scale::parse::parse_scales;
use super::split::Split;
use super::support;

/// Port of the `SpecType` union.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecType {
    Unit,
    Layer,
    Facet,
    Concat,
}

impl SpecType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SpecType::Unit => "unit",
            SpecType::Layer => "layer",
            SpecType::Facet => "facet",
            SpecType::Concat => "concat",
        }
    }
}

// ---------------------------------------------------------------------------
// NameMap
// ---------------------------------------------------------------------------

/// Port of `NameMapInterface` / `NameMap`.
#[derive(Debug, Clone, Default)]
pub struct NameMap {
    name_map: BTreeMap<String, String>,
}

impl NameMap {
    pub fn new() -> Self {
        NameMap::default()
    }

    pub fn rename(&mut self, old_name: &str, new_name: &str) {
        self.name_map
            .insert(old_name.to_string(), new_name.to_string());
    }

    pub fn has(&self, name: &str) -> bool {
        self.name_map.contains_key(name)
    }

    /// All registered old -> new mappings.
    pub fn entries(&self) -> Vec<(String, String)> {
        self.name_map
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    pub fn get(&self, name: &str) -> String {
        let mut current = match self.name_map.get(name) {
            Some(new_name) => new_name,
            None => return name.to_string(),
        };
        // Follow rename chains (matching `NameMapInterface.get`).
        while let Some(new_name) = self.name_map.get(current) {
            if new_name == current {
                break;
            }
            current = new_name;
        }
        current.clone()
    }
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct DataComponent {
    pub sources: Vec<Value>,
    pub output_nodes: BTreeMap<String, Value>,
    pub output_node_ref_counts: BTreeMap<String, i64>,
    pub is_faceted: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Component {
    pub data: DataComponent,
    pub layout_size: Split,
    pub layout_headers: Value,
    pub mark: Vec<Value>,
    pub scales: indexmap::IndexMap<String, ScaleComponent>,
    pub projection: Option<ProjectionComponent>,
    pub selection: Value,
    pub axes: Value,
    pub legends: Value,
    pub resolve: support::Resolve,
}

/// Variant-specific state for unit models.
#[derive(Debug, Clone, Default)]
pub struct UnitData {
    pub mark_def: Value,
    pub encoding: Value,
    pub specified_scales: BTreeMap<String, Value>,
    pub specified_axes: Value,
    pub specified_legends: Value,
    pub specified_projection: Value,
    pub selection: Vec<Value>,
    pub stack: Value,
}

/// Variant-specific state for facet models.
#[derive(Debug, Clone, Default)]
pub struct FacetData {
    pub facet: Value,
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Model {
    pub type_: SpecType,
    pub name: String,
    pub size: Value,
    pub title: Value,
    pub description: Option<String>,
    /// `None` means the spec did not define `data` (upstream `undefined`).
    pub data: Option<Value>,
    pub transforms: Vec<Value>,
    pub layout: Map<String, Value>,
    pub component: Component,
    pub view: Option<Value>,
    pub children: Vec<Model>,
    pub config: Value,

    pub unit: UnitData,
    pub facet_data: FacetData,

    pub scale_name_map: Rc<RefCell<NameMap>>,
    pub projection_name_map: Rc<RefCell<NameMap>>,
    pub signal_name_map: Rc<RefCell<NameMap>>,

    /// Channels for which the local scale component was merged into an
    /// ancestor's component.
    pub merged_channels: Vec<String>,
    /// Snapshot of the merged scale components from the closest composite
    /// parent.  `sync_merged_scales` keeps these up to date after each parse
    /// stage, emulating the shared component objects of the TS implementation.
    pub merged_scales: BTreeMap<String, Rc<ScaleComponent>>,

    /// `resolve.scale` of the closest facet ancestor (or this model when it is
    /// a facet).  Used while parsing faceted scale domains.
    pub facet_scale_resolve: BTreeMap<String, String>,

    pub parent_type: Option<SpecType>,

    /// The assembled dataflow component for this model (only meaningful on the
    /// root model after `parse`).
    pub data_flow: Option<super::data::index::DataComponent>,

    /// Memoized strongly-typed view of `component.selection`. The raw JSON is
    /// kept alongside the parsed components so that any write (including ones
    /// performed directly on the public `component.selection` field) is
    /// detected by a cheap structural comparison.
    // The nested generic spells out the cache value type; a type alias would
    // only move the same complexity elsewhere.
    #[allow(clippy::type_complexity)]
    pub selection_cache: RefCell<
        Option<(
            Value,
            std::rc::Rc<
                BTreeMap<String, std::rc::Rc<super::selection::component::SelectionComponent>>,
            >,
        )>,
    >,

    /// Reusable immutable snapshots for `sync_merged_scales`, so unchanged
    /// parent scale components are shared instead of deep-cloned per sync.
    pub sync_cache: RefCell<BTreeMap<String, Rc<ScaleComponent>>>,
}

impl Model {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_common(
        spec: &Value,
        type_: SpecType,
        parent: Option<&Model>,
        parent_given_name: &str,
        config: &Value,
        view: Option<Value>,
    ) -> Model {
        let name = spec
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(parent_given_name)
            .to_string();

        let title = match spec.get("title") {
            Some(Value::String(s)) => serde_json::json!({"text": s}),
            Some(Value::Object(_)) => support::replace_expr_ref(&spec["title"]),
            _ => Value::Null,
        };
        let description = spec
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_string);
        let data = spec.get("data").cloned();
        let transforms = spec
            .get("transform")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let layout = if matches!(type_, SpecType::Layer | SpecType::Unit) {
            Map::new()
        } else {
            crate::spec::extract_composition_layout(spec, type_.as_str(), config)
        };

        let (scale_name_map, projection_name_map, signal_name_map) = match parent {
            Some(parent) => (
                parent.scale_name_map.clone(),
                parent.projection_name_map.clone(),
                parent.signal_name_map.clone(),
            ),
            None => (
                Rc::new(RefCell::new(NameMap::new())),
                Rc::new(RefCell::new(NameMap::new())),
                Rc::new(RefCell::new(NameMap::new())),
            ),
        };

        let resolve = support::Resolve::from_spec(spec);
        let is_faceted = support::is_facet_spec(spec)
            || (parent
                .map(|p| p.component.data.is_faceted)
                .unwrap_or(false)
                && !spec.as_object().map(|m| m.contains_key("data")).unwrap_or(false));

        let mut facet_scale_resolve: BTreeMap<String, String> = parent
            .map(|p| p.facet_scale_resolve.clone())
            .unwrap_or_default();
        if type_ == SpecType::Facet {
            facet_scale_resolve = resolve
                .scale
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect();
        }

        Model {
            type_,
            name,
            size: Value::Object(Map::new()),
            title,
            description,
            data,
            transforms,
            layout,
            component: Component {
                data: DataComponent {
                    sources: parent
                        .map(|p| p.component.data.sources.clone())
                        .unwrap_or_default(),
                    output_nodes: parent
                        .map(|p| p.component.data.output_nodes.clone())
                        .unwrap_or_default(),
                    output_node_ref_counts: parent
                        .map(|p| p.component.data.output_node_ref_counts.clone())
                        .unwrap_or_default(),
                    is_faceted,
                },
                layout_size: Split::default(),
                layout_headers: serde_json::json!({"row": {}, "column": {}, "facet": {}}),
                mark: Vec::new(),
                scales: indexmap::IndexMap::new(),
                projection: None,
                selection: Value::Null,
                axes: serde_json::json!({}),
                legends: serde_json::json!({}),
                resolve,
            },
            view,
            children: Vec::new(),
            config: config.clone(),
            unit: UnitData::default(),
            facet_data: FacetData::default(),
            scale_name_map,
            projection_name_map,
            signal_name_map,
            merged_channels: Vec::new(),
            merged_scales: BTreeMap::new(),
            facet_scale_resolve,
            parent_type: parent.map(|p| p.type_),
            data_flow: None,
            selection_cache: RefCell::new(None),
            sync_cache: RefCell::new(BTreeMap::new()),
        }
    }

    // -- type guards --------------------------------------------------------

    pub fn is_unit_model(&self) -> bool {
        self.type_ == SpecType::Unit
    }

    pub fn is_layer_model(&self) -> bool {
        self.type_ == SpecType::Layer
    }

    pub fn is_facet_model(&self) -> bool {
        self.type_ == SpecType::Facet
    }

    pub fn is_concat_model(&self) -> bool {
        self.type_ == SpecType::Concat
    }

    // -- unit accessors -----------------------------------------------------

    pub fn mark_def(&self) -> Option<&Value> {
        if self.is_unit_model() {
            Some(&self.unit.mark_def)
        } else {
            None
        }
    }

    pub fn encoding(&self) -> Option<&Value> {
        if self.is_unit_model() {
            Some(&self.unit.encoding)
        } else if self.is_facet_model() {
            Some(&self.facet_data.facet)
        } else {
            None
        }
    }

    pub fn mark_type(&self) -> String {
        self.unit
            .mark_def
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    }

    pub fn scale_domain(&self, channel: &str) -> Value {
        self.unit
            .specified_scales
            .get(channel)
            .and_then(|s| s.get("domain"))
            .cloned()
            .unwrap_or(Value::Null)
    }

    pub fn field_def(&self, channel: &str) -> Option<Value> {
        self.field_def_ref(channel).cloned()
    }

    /// Borrowing variant of [`Model::field_def`].
    pub fn field_def_ref(&self, channel: &str) -> Option<&Value> {
        if self.is_facet_model() {
            return self.facet_data.facet.get(channel);
        }
        let encoding = self.encoding()?;
        crate::channeldef::get_field_def_ref(&encoding[channel])
    }

    pub fn typed_field_def(&self, channel: &str) -> Option<Value> {
        self.field_def(channel)
            .filter(is_typed_field_def)
    }

    pub fn typed_field_def_ref(&self, channel: &str) -> Option<&Value> {
        self.field_def_ref(channel)
            .filter(|field_def| is_typed_field_def(field_def))
    }

    pub fn vg_field(&self, channel: &str) -> String {
        self.field_def(channel)
            .map(|field_def| channeldef::vg_field(&field_def))
            .unwrap_or_default()
    }

    pub fn vg_field_with(&self, channel: &str, opt: &FieldRefOption) -> String {
        self.field_def(channel)
            .map(|field_def| channeldef::vg_field_with(&field_def, opt))
            .unwrap_or_default()
    }

    pub fn channel_has_field(&self, channel: &str) -> bool {
        match self.encoding() {
            Some(encoding) if self.is_facet_model() => encoding.get(channel).is_some(),
            Some(encoding) => crate::encoding::channel_has_field(encoding, channel),
            None => false,
        }
    }

    // -- names --------------------------------------------------------------

    pub fn get_name(&self, text: &str) -> String {
        if self.name.is_empty() {
            return var_name(text);
        }
        var_name(&format!("{}_{}", self.name, text))
    }

    pub fn get_data_name(&self, type_: &str) -> String {
        self.get_name(&type_.to_lowercase())
    }

    pub fn request_data_name(&mut self, type_: &str) -> String {
        let full_name = self.get_data_name(type_);
        let count = self
            .component
            .data
            .output_node_ref_counts
            .entry(full_name.clone())
            .or_insert(0);
        *count += 1;
        full_name
    }

    pub fn get_signal_name(&self, old_signal_name: &str) -> String {
        self.signal_name_map.borrow().get(old_signal_name)
    }

    pub fn rename_signal(&self, old_name: &str, new_name: &str) {
        self.signal_name_map.borrow_mut().rename(old_name, new_name);
    }

    /// Applies the signal rename map to a string (e.g. a signal reference that
    /// was created before the dataflow optimizer unified it).
    pub fn resolve_signal_name(&self, name: &str) -> String {
        let mut out = name.to_string();
        let mut entries = self.signal_name_map.borrow().entries();
        entries.sort_by_key(|(old, _)| std::cmp::Reverse(old.len()));
        for (old, _) in entries {
            if !old.is_empty() && out.contains(&old) {
                let resolved = self.signal_name_map.borrow().get(&old);
                out = out.replace(&old, &resolved);
            }
        }
        out
    }

    /// Recursively applies `resolve_signal_name` to every `signal` string.
    pub fn resolve_signal_names_in(&self, value: &mut Value) {
        match value {
            Value::String(s) => {
                *s = self.resolve_signal_name(s);
            }
            Value::Array(items) => items.iter_mut().for_each(|v| self.resolve_signal_names_in(v)),
            Value::Object(map) => {
                if let Some(Value::String(s)) = map.get_mut("signal") {
                    *s = self.resolve_signal_name(s);
                }
                map.values_mut().for_each(|v| self.resolve_signal_names_in(v));
            }
            _ => {}
        }
    }

    pub fn rename_scale(&self, old_name: &str, new_name: &str) {
        self.scale_name_map.borrow_mut().rename(old_name, new_name);
    }

    pub fn rename_projection(&self, old_name: &str, new_name: &str) {
        self.projection_name_map
            .borrow_mut()
            .rename(old_name, new_name);
    }

    /// Port of `Model.projectionName`.
    ///
    /// During the parse phase the original name is returned; afterwards the
    /// shared rename map resolves names that a parent merged away.
    pub fn projection_name(&self, parse: bool) -> Option<String> {
        let name = self.get_name("projection");
        if parse {
            return Some(name);
        }
        if self
            .component
            .projection
            .as_ref()
            .map(|p| !p.merged)
            .unwrap_or(false)
            || self.projection_name_map.borrow().has(&name)
        {
            return Some(self.projection_name_map.borrow().get(&name));
        }
        None
    }

    pub fn scale_name(&self, original_scale_name: &str, parse: bool) -> Option<String> {
        if parse {
            return Some(self.get_name(original_scale_name));
        }
        let name = self.get_name(original_scale_name);
        if self.component.scales.contains_key(original_scale_name)
            || self.scale_name_map.borrow().has(&name)
        {
            return Some(self.scale_name_map.borrow().get(&name));
        }
        None
    }

    pub fn lookup_data_source(&self, name: &str) -> String {
        if let Some(component) = &self.data_flow {
            if let Some(node) = component.output_nodes.get(name) {
                return super::data::dataflow::OutputNode::get_source(node);
            }

            // Faceted shared scales refer to the cloned subtree output
            // (`scale_<name>`). Upstream registers those clones in the facet
            // model's `outputNodes`; here they are reachable through the
            // assembled dataflow tree instead.
            if let Some(original) = name.strip_prefix(super::data::optimize::FACET_SCALE_PREFIX) {
                let clone_name = format!("clone_{original}");
                for source in &component.sources {
                    if let Some(source) = find_output_source(source, &clone_name) {
                        return source;
                    }
                }
            }
        }
        name.to_string()
    }

    // -- scale accessors ----------------------------------------------------

    pub fn get_scale_component(&self, channel: &str) -> Option<ScaleComponent> {
        if let Some(local) = self.component.scales.get(channel) {
            if !local.merged {
                return Some(local.clone());
            }
        }
        self.merged_scales.get(channel).map(|c| (**c).clone())
    }

    /// Borrowing variant of [`Model::get_scale_component`].
    pub fn get_scale_component_ref(&self, channel: &str) -> Option<&ScaleComponent> {
        if let Some(local) = self.component.scales.get(channel) {
            if !local.merged {
                return Some(local);
            }
        }
        self.merged_scales.get(channel).map(|c| c.as_ref())
    }

    pub fn get_scale_type(&self, channel: &str) -> Option<crate::scale::ScaleType> {
        self.get_scale_component(channel).and_then(|c| c.get_type())
    }

    /// Keep the snapshots of merged parent components in sync.
    pub fn sync_merged_scales(&mut self) {
        if self.children.is_empty() {
            return;
        }
        // Snapshot the parent components once; every child shares the same
        // immutable `Rc`s (upstream shares the component objects directly).
        let snapshots: Vec<(String, Rc<ScaleComponent>)> = {
            let mut cache = self.sync_cache.borrow_mut();
            self.component
                .scales
                .iter()
                .map(|(channel, component)| {
                    let shared = match cache.get(channel) {
                        Some(existing) if **existing == *component => existing.clone(),
                        _ => {
                            let shared = Rc::new(component.clone());
                            cache.insert(channel.clone(), shared.clone());
                            shared
                        }
                    };
                    (channel.clone(), shared)
                })
                .collect()
        };
        for child in &mut self.children {
            for (channel, component) in &snapshots {
                // Upstream `getScaleComponent` walks up the hierarchy, so a
                // child that has no local component for a channel still sees
                // the parent's merged one.
                child
                    .merged_scales
                    .insert(channel.clone(), component.clone());
            }
            child.sync_merged_scales();
        }
    }

    // -- parse --------------------------------------------------------------

    pub fn parse_scale(&mut self) {
        parse_scales(self, false);
    }

    pub fn parse_scale_except_range(&mut self) {
        parse_scales(self, true);
    }

    pub fn parse(&mut self) {
        self.parse_scale();
        self.parse_layout_size();
        self.rename_top_level_layout_size_signal();
        self.parse_selections();
        self.parse_projection();
        self.parse_data();
        self.parse_axes_and_headers();
        self.parse_legends();
        self.parse_mark_group();
    }

    /// Port of `renameTopLevelLayoutSizeSignal`.
    pub fn rename_top_level_layout_size_signal(&mut self) {
        if self.get_name("width") != "width" {
            let old = self.get_name("width");
            self.rename_signal(&old, "width");
        }
        if self.get_name("height") != "height" {
            let old = self.get_name("height");
            self.rename_signal(&old, "height");
        }
    }

    /// Port of the per-model `parseData` methods.
    pub fn parse_data(&mut self) {
        let mut component = super::data::index::DataComponent::default();
        self.parse_data_into(&mut component, None);
        self.data_flow = Some(component);
    }

    fn parse_data_into(
        &mut self,
        component: &mut super::data::index::DataComponent,
        fallback: Option<&super::data::dataflow::NodeRef>,
    ) {
        use super::data::dataflow::NodeRef;
        let _: Option<&NodeRef> = fallback;
        super::data::parse::parse_data_into(self, component, fallback);

        match self.type_ {
            SpecType::Unit => {
                self.data_flow = Some(component.clone());
                // A non-null `invalid` is a Vega-Lite dataflow option (it
                // creates filter/scale handling), not a Vega mark property;
                // upstream never copies it into the mark encoding. Keep an
                // explicit `null` (it disables default invalid handling and is
                // still consulted during mark encoding).
                if let Value::Object(mark_def) = &mut self.unit.mark_def {
                    if mark_def.get("invalid").map(|v| !v.is_null()).unwrap_or(false) {
                        mark_def.remove("invalid");
                    }
                }
            }
            SpecType::Layer | SpecType::Concat => {
                let child_fallback = component.main.clone().or_else(|| component.facet_root.clone());
                // Children inherit the parent's parse state, but a sibling's
                // explicit/implicit parses must not leak into later siblings.
                let ancestor_parse = component.ancestor_parse.clone();
                for child in &mut self.children {
                    component.ancestor_parse = ancestor_parse.clone();
                    child.parse_data_into(component, child_fallback.as_ref());
                }
                component.ancestor_parse = ancestor_parse;
            }
            SpecType::Facet => {
                let facet_root = component.facet_root.clone();
                let ancestor_parse = component.ancestor_parse.clone();
                let child_fallback = facet_root
                    .clone()
                    .or_else(|| component.main.clone());
                if let Some(child) = self.children.get_mut(0) {
                    child.parse_data_into(component, child_fallback.as_ref());
                }
                // The child shares the data component in this port; restore the
                // parent's facet node and parse state, which the child's parse
                // clears/overwrites.
                component.facet_root = facet_root;
                component.ancestor_parse = ancestor_parse;
                self.data_flow = Some(component.clone());
            }
        }
    }

    pub fn parse_selections(&mut self) {
        super::selection::parse::parse_selections(self);
    }

    /// Share the root data component with unit/layer descendants.
    pub fn propagate_data_flow(&mut self) {
        let component = self.data_flow.clone();
        self.propagate_data_flow_to_children(&component);
    }

    fn propagate_data_flow_to_children(
        &mut self,
        component: &Option<super::data::index::DataComponent>,
    ) {
        for child in &mut self.children {
            if !matches!(child.type_, SpecType::Facet) {
                child.data_flow = component.clone();
            }
            child.propagate_data_flow_to_children(component);
        }
    }

    pub fn parse_layout_size(&mut self) {        if self.is_unit_model() {
            super::layoutsize::parse::parse_unit_layout_size(self);
        } else if self.is_layer_model() {
            super::layoutsize::parse::parse_layer_layout_size(self);
        } else if self.is_facet_model() {
            super::layoutsize::parse::parse_children_layout_size(self);
        } else {
            super::layoutsize::parse::parse_concat_layout_size(self);
        }
    }

    pub fn parse_mark_group(&mut self) {
        match self.type_ {
            SpecType::Unit => {
                self.component.mark = super::mark::mark::parse_mark_groups(self);
                self.request_data_name("main");
                self.mark_output_required("main");
            }
            SpecType::Layer | SpecType::Concat => {
                for child in &mut self.children {
                    child.parse_mark_group();
                }
            }
            SpecType::Facet => {
                if let Some(child) = self.children.get_mut(0) {
                    child.parse_mark_group();
                }
            }
        }
    }

    pub fn parse_axes_and_headers(&mut self) {
        super::axis::parse::parse_axes_and_headers(self);
    }

    pub fn parse_legends(&mut self) {
        super::legend::parse::parse_legend(self);
    }

    pub fn parse_projection(&mut self) {
        super::projection::parse::parse_projection(self);
    }

    pub fn width(&self) -> Value {
        self.get_size_signal_ref("width")
    }

    pub fn height(&self) -> Value {
        self.get_size_signal_ref("height")
    }

    pub fn get_size_signal_ref(&self, layout_size_type: &str) -> Value {
        if self.parent_type == Some(SpecType::Facet) {
            let size_type =
                super::layoutsize::component::get_size_type_from_layout_size_type(layout_size_type);
            let channel = crate::channel::get_position_scale_channel(size_type);
            if let Some(scale_component) = self.component.scales.get(channel) {
                if !scale_component.merged {
                    let type_ = scale_component.get_type();
                    let range = scale_component.get("range").unwrap_or(Value::Null);
                    if type_.map(crate::scale::has_discrete_domain).unwrap_or(false)
                        && super::scale::domain::is_vg_range_step(&range)
                    {
                        let scale_name = scale_component
                            .get("name")
                            .and_then(|n| n.as_str().map(str::to_string))
                            .unwrap_or_default();
                        let domain = super::scale::domain::assemble_domain(self, channel);
                        if let Some(field) = super::scale::domain::get_field_from_domain(&domain) {
                            let field_ref = channeldef::vg_field_with(
                                &serde_json::json!({"aggregate": "distinct", "field": field}),
                                &FieldRefOption {
                                    expr: Some("datum".to_string()),
                                    ..Default::default()
                                },
                            );
                            return serde_json::json!({
                                "signal": super::layoutsize::assemble::size_expr(
                                    &scale_name,
                                    scale_component,
                                    &field_ref
                                )
                            });
                        }
                        crate::log::warn(support::unknown_field(channel));
                        return Value::Null;
                    }
                }
            }
        }
        serde_json::json!({"signal": self.get_signal_name(&self.get_name(layout_size_type))})
    }

    // -- assembly -----------------------------------------------------------

    /// Port of `assembleEncodeFromView`.
    fn assemble_encode_from_view(&self, view: &Value) -> Map<String, Value> {
        let mut encode = Map::new();
        if let Some(map) = view.as_object() {
            for (key, value) in map {
                if key == "style" {
                    continue;
                }
                // Upstream only excludes `undefined`; explicit JSON null is
                // kept (e.g. `"view": {"stroke": null}`).
                encode.insert(
                    key.clone(),
                    crate::compile_common::signal_or_value_ref(value),
                );
            }
        }
        encode
    }

    /// Port of `assembleGroupEncodeEntry`.
    pub fn assemble_group_encode_entry(&self, is_top_level: bool) -> Option<Value> {
        let mut encode_entry = if let Some(view) = &self.view {
            self.assemble_encode_from_view(view)
        } else {
            Map::new()
        };

        if !is_top_level {
            if let Some(description) = &self.description {
                if !description.is_empty() {
                    encode_entry.insert(
                        "description".to_string(),
                        crate::compile_common::signal_or_value_ref(&Value::String(
                            description.clone(),
                        )),
                    );
                }
            }
            if self.type_ == SpecType::Unit || self.type_ == SpecType::Layer {
                let mut out = Map::new();
                out.insert("width".to_string(), self.get_size_signal_ref("width"));
                out.insert("height".to_string(), self.get_size_signal_ref("height"));
                for (k, v) in encode_entry {
                    out.insert(k, v);
                }
                return Some(Value::Object(out));
            }
        }

        if encode_entry.is_empty() {
            None
        } else {
            Some(Value::Object(encode_entry))
        }
    }

    /// Port of `assembleLayout`.
    pub fn assemble_layout(&self) -> Option<Value> {
        if self.is_unit_model() || self.is_layer_model() {
            return None;
        }

        let mut layout = self.layout.clone();
        let spacing = layout.remove("spacing");

        let mut out = Map::new();
        if let Some(spacing) = spacing {
            out.insert("padding".to_string(), spacing);
        }
        let default_layout = self.assemble_default_layout();
        for (k, v) in default_layout {
            out.insert(k, v);
        }
        for (k, v) in layout {
            out.insert(k, v);
        }
        Some(Value::Object(out))
    }

    pub fn assemble_default_layout(&self) -> Map<String, Value> {
        if self.is_facet_model() {
            return self.assemble_facet_default_layout();
        }
        if self.is_concat_model() {
            // Port of `ConcatModel.assembleDefaultLayout`.
            let mut out = Map::new();
            if let Some(columns) = self.layout.get("columns") {
                if !columns.is_null() {
                    out.insert("columns".to_string(), columns.clone());
                }
            }
            out.insert("bounds".to_string(), json!("full"));
            out.insert("align".to_string(), json!("each"));
            return out;
        }
        Map::new()
    }

    /// Port of `assembleHeaderMarks` (header assembly is not ported yet).
    pub fn assemble_header_marks(&self) -> Vec<Value> {
        Vec::new()
    }

    pub fn assemble_axes(&self) -> Vec<Value> {
        super::axis::assemble::assemble_axes_from_value(&self.component.axes, &self.config)
    }

    pub fn assemble_legends(&self) -> Vec<Value> {
        let mut legends = super::legend::assemble::assemble_legends(self);
        // Port of `LayerModel.assembleLegends`: independent child legends
        // (e.g. `resolve.legend.* = "independent"`) stay in the layer's scope.
        if self.is_layer_model() {
            for child in &self.children {
                legends.extend(child.assemble_legends());
            }
        }
        legends
    }

    pub fn assemble_projections(&self) -> Vec<Value> {
        super::projection::assemble::assemble_projections(self)
    }

    /// Port of `assembleTitle`.
    pub fn assemble_title(&self) -> Option<Value> {
        if self.is_layer_model() {
            if let Some(title) = self.assemble_own_title() {
                return Some(title);
            }
            for child in &self.children {
                if let Some(title) = child.assemble_title() {
                    return Some(title);
                }
            }
            return None;
        }
        self.assemble_own_title()
    }

    fn assemble_own_title(&self) -> Option<Value> {
        let source = if self.title.is_object() {
            self.title.clone()
        } else {
            Value::Object(Map::new())
        };
        let mut title_no_encoding = source.as_object().cloned().unwrap_or_default();
        let encoding = title_no_encoding.remove("encoding");

        let mut title: Map<String, Value> = Map::new();
        let title_config = self
            .config
            .get("title")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let extracted = crate::title::extract_title_config(&title_config);
        for (k, v) in extracted.non_mark_title_properties {
            title.insert(k, v);
        }
        for (k, v) in title_no_encoding {
            title.insert(k, v);
        }
        if let Some(encoding) = encoding {
            let mut encode = Map::new();
            encode.insert("update".to_string(), encoding);
            title.insert("encode".to_string(), Value::Object(encode));
        }

        let has_text = title
            .get("text")
            .map(|t| !t.is_null())
            .unwrap_or(false);
        if !has_text {
            return None;
        }

        if matches!(self.type_, SpecType::Unit | SpecType::Layer) {
            let anchor = title.get("anchor").and_then(Value::as_str);
            if anchor.is_none() || anchor == Some("middle") {
                title
                    .entry("frame".to_string())
                    .or_insert_with(|| Value::String("group".to_string()));
            }
        } else {
            title
                .entry("anchor".to_string())
                .or_insert_with(|| Value::String("start".to_string()));
        }

        Some(Value::Object(title))
    }

    /// Port of `assembleGroup`.
    pub fn assemble_group(&self, signals: Vec<Value>) -> Value {
        let mut group = Map::new();

        // Port of `FacetModel.assembleGroup`: for nested facets, provide the
        // number of columns to the parent group mark.
        if self.is_facet_model() && self.parent_type == Some(SpecType::Facet) {
            if let Some(column_def) = self.facet_field_def("column") {
                let field = channeldef::vg_field_with(
                    &column_def,
                    &FieldRefOption {
                        prefix: Some("distinct".to_string()),
                        ..Default::default()
                    },
                );
                group.insert(
                    "encode".to_string(),
                    json!({"update": {"columns": {"field": field}}}),
                );
            }
        }

        let mut signals = signals;
        signals.extend(self.assemble_signals());

        if !signals.is_empty() {
            group.insert("signals".to_string(), Value::Array(signals));
        }

        if let Some(layout) = self.assemble_layout() {
            group.insert("layout".to_string(), layout);
        }

        let mut marks = self.assemble_header_marks();
        marks.extend(self.assemble_marks());
        group.insert("marks".to_string(), Value::Array(marks));

        let include_scales = matches!(self.parent_type, None | Some(SpecType::Facet));
        if include_scales {
            let scales = super::scale::assemble::assemble_scales(self);
            if !scales.is_empty() {
                group.insert("scales".to_string(), Value::Array(scales));
            }
        }

        let axes = self.assemble_axes();
        if !axes.is_empty() {
            group.insert("axes".to_string(), Value::Array(axes));
        }

        let legends = self.assemble_legends();
        if !legends.is_empty() {
            group.insert("legends".to_string(), Value::Array(legends));
        }

        Value::Object(group)
    }

    // -- assembly dispatchers ----------------------------------------------

    /// Mark the output node for `type_` as required by the optimizer.
    pub fn mark_output_required(&mut self, type_: &str) {
        let name = self.get_data_name(type_);
        if let Some(component) = &self.data_flow {
            if let Some(node) = component.output_nodes.get(&name) {
                super::data::dataflow::OutputNode::mark_required(node);
            }
        }
    }

    pub fn assemble_signals(&self) -> Vec<Value> {
        match self.type_ {
            SpecType::Unit => {
                let mut signals = super::axis::assemble::assemble_axis_signals(self);
                signals = super::selection::assemble::assemble_unit_selection_signals(
                    self,
                    signals,
                );
                signals
            }
            SpecType::Layer => {
                let mut signals = super::axis::assemble::assemble_axis_signals(self);
                for child in &self.children {
                    signals.extend(child.assemble_signals());
                }
                signals
            }
            SpecType::Concat => {
                for child in &self.children {
                    let _ = child.assemble_signals();
                }
                Vec::new()
            }
            SpecType::Facet => {
                if let Some(child) = self.children.first() {
                    let _ = child.assemble_signals();
                }
                Vec::new()
            }
        }
    }

    pub fn assemble_layout_signals(&self) -> Vec<Value> {
        match self.type_ {
            SpecType::Unit => super::layoutsize::assemble::assemble_layout_signals(self),
            SpecType::Layer => {
                let mut signals = super::layoutsize::assemble::assemble_layout_signals(self);
                for child in &self.children {
                    signals.extend(child.assemble_layout_signals());
                }
                signals
            }
            SpecType::Concat => {
                let mut signals = super::layoutsize::assemble::assemble_layout_signals(self);
                for child in &self.children {
                    signals.extend(child.assemble_layout_signals());
                }
                signals
            }
            SpecType::Facet => self
                .children
                .first()
                .map(|child| child.assemble_layout_signals())
                .unwrap_or_default(),
        }
    }

    pub fn assemble_selection_top_level_signals(&self, signals: Vec<Value>) -> Vec<Value> {
        match self.type_ {
            SpecType::Unit => {
                super::selection::assemble::assemble_top_level_signals(self, signals)
            }
            SpecType::Layer | SpecType::Concat => self.children.iter().fold(signals, |sg, child| {
                child.assemble_selection_top_level_signals(sg)
            }),
            SpecType::Facet => match self.children.first() {
                Some(child) => child.assemble_selection_top_level_signals(signals),
                None => signals,
            },
        }
    }

    pub fn assemble_selection_data(&self, data: Vec<Value>) -> Vec<Value> {
        match self.type_ {
            SpecType::Unit => {
                super::selection::assemble::assemble_unit_selection_data(self, data)
            }
            SpecType::Layer | SpecType::Concat => self
                .children
                .iter()
                .fold(data, |db, child| child.assemble_selection_data(db)),
            SpecType::Facet => match self.children.first() {
                Some(child) => child.assemble_selection_data(data),
                None => data,
            },
        }
    }

    pub fn assemble_group_style(&self) -> Option<Value> {
        match self.type_ {
            SpecType::Unit => {
                if let Some(style) = self.view.as_ref().and_then(|v| v.get("style")) {
                    if !style.is_null() {
                        return Some(style.clone());
                    }
                }
                if let Some(encoding) = self.encoding() {
                    if encoding.get("x").is_some() || encoding.get("y").is_some() {
                        return Some(Value::String("cell".to_string()));
                    }
                }
                Some(Value::String("view".to_string()))
            }
            SpecType::Layer => {
                let mut styles: Vec<Value> = Vec::new();
                for child in &self.children {
                    let child_style = child.assemble_group_style();
                    let items = match child_style {
                        Some(Value::Array(items)) => items,
                        Some(other) => vec![other],
                        None => Vec::new(),
                    };
                    for item in items {
                        if !styles.iter().any(|s| s == &item) {
                            styles.push(item);
                        }
                    }
                }
                if styles.len() > 1 {
                    Some(Value::Array(styles))
                } else if styles.len() == 1 {
                    Some(styles.remove(0))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn assemble_marks(&self) -> Vec<Value> {
        match self.type_ {
            SpecType::Unit => {
                let mut marks = self.component.mark.clone();
                if self.parent_type != Some(SpecType::Layer) {
                    marks = super::selection::assemble::assemble_unit_selection_marks(self, marks);
                }
                marks
                    .into_iter()
                    .map(|mark| self.correct_data_names(mark))
                    .collect()
            }
            SpecType::Layer => {
                let mut marks: Vec<Value> = Vec::new();
                for child in &self.children {
                    marks.extend(child.assemble_marks());
                }
                super::selection::assemble::assemble_layer_selection_marks(self, marks)
            }
            SpecType::Concat => self
                .children
                .iter()
                .map(|child| {
                    let title = child.assemble_title();
                    let style = child.assemble_group_style();
                    let encode_entry = child.assemble_group_encode_entry(false);

                    let mut group = Map::new();
                    group.insert("type".to_string(), Value::String("group".to_string()));
                    group.insert(
                        "name".to_string(),
                        Value::String(child.get_name("group")),
                    );
                    if let Some(title) = title {
                        group.insert("title".to_string(), title);
                    }
                    if let Some(style) = style {
                        group.insert("style".to_string(), style);
                    }
                    if let Some(encode_entry) = encode_entry {
                        group.insert(
                            "encode".to_string(),
                            serde_json::json!({"update": encode_entry}),
                        );
                    }
                    if let Value::Object(inner) = child.assemble_group(Vec::new()) {
                        for (k, v) in inner {
                            group.insert(k, v);
                        }
                    }
                    Value::Object(group)
                })
                .collect(),
            SpecType::Facet => self.assemble_facet_marks(),
        }
    }

    /// Port of `UnitModel.correctDataNames`.
    fn correct_data_names(&self, mut mark: Value) -> Value {
        if let Some(data) = mark
            .get_mut("from")
            .and_then(|f| f.get_mut("data"))
            .and_then(|d| d.as_str().map(str::to_string))
        {
            let mut name = self.lookup_data_source(&data);
            if self
                .encoding()
                .map(|e| e.get("time").is_some())
                .unwrap_or(false)
            {
                name.push_str("_curr");
            }
            mark["from"]["data"] = Value::String(name);
        }
        if let Some(facet_data) = mark
            .get_mut("from")
            .and_then(|f| f.get_mut("facet"))
            .and_then(|f| f.get_mut("data"))
            .and_then(|d| d.as_str().map(str::to_string))
        {
            let name = self.lookup_data_source(&facet_data);
            mark["from"]["facet"]["data"] = Value::String(name);
        }
        mark
    }

    pub fn has_axis_orient_signal_ref(&self) -> bool {
        let index = super::axis::component::axis_index_from_value(&self.component.axes);
        index.get("x").map(|axes| axes.iter().any(|a| a.has_orient_signal_ref())).unwrap_or(false)
            || index.get("y").map(|axes| axes.iter().any(|a| a.has_orient_signal_ref())).unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Dataflow support trait
// ---------------------------------------------------------------------------

impl crate::compile::data::support::Model for Model {
    fn name(&self, text: &str) -> String {
        self.get_name(text)
    }

    fn selection_extent(&self, name: &str, extent: &Value) -> String {
        super::selection::parse::parse_selection_extent(self, name, extent)
    }
}

// ---------------------------------------------------------------------------
// Free type guards, mirroring the exports of `compile/model.ts`.
// ---------------------------------------------------------------------------

pub fn is_unit_model(model: &Model) -> bool {
    model.type_ == SpecType::Unit
}

pub fn is_facet_model(model: &Model) -> bool {
    model.type_ == SpecType::Facet
}

pub fn is_layer_model(model: &Model) -> bool {
    model.type_ == SpecType::Layer
}

pub fn is_concat_model(model: &Model) -> bool {
    model.type_ == SpecType::Concat
}

/// Find the current source of an output node named `name` in a dataflow tree.
fn find_output_source(node: &super::data::dataflow::NodeRef, name: &str) -> Option<String> {
    use super::data::dataflow::{children, with_node, OutputNode};
    if let Some(source) = with_node::<OutputNode, _>(node, |n| {
        if n.name == name {
            Some(n.source.clone())
        } else {
            None
        }
    })
    .flatten()
    {
        return Some(source);
    }
    for child in children(node) {
        if let Some(source) = find_output_source(&child, name) {
            return Some(source);
        }
    }
    None
}

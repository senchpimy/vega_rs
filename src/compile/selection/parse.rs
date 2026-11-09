//! Port of `src/compile/selection/parse.ts`.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use crate::compile::model::Model;
use crate::log;
use crate::util::{array, replace_path_in_field, string_value, var_name};

use super::component::{
    get_selection_component, is_timer_selection, merge_selection_defaults, selection_compilers,
    set_selection_components, SelectionComponent,
};
use super::message;
use super::selector::parse_selector;

fn known_key(key: &str) -> bool {
    matches!(
        key,
        "type"
            | "init"
            | "events"
            | "bind"
            | "resolve"
            | "mark"
            | "toggle"
            | "translate"
            | "zoom"
            | "nearest"
            | "clear"
    )
}

/// Port of `parseUnitSelection`.
pub fn parse_unit_selection(
    model: &mut Model,
    sel_defs: &[Value],
) -> BTreeMap<String, SelectionComponent> {
    let mut sel_cmpts: BTreeMap<String, SelectionComponent> = BTreeMap::new();
    let selection_config = model
        .config
        .get("selection")
        .cloned()
        .unwrap_or(Value::Object(Map::new()));

    if sel_defs.is_empty() {
        return sel_cmpts;
    }

    let mut n_timer_selections = 0;

    for def in sel_defs {
        let name = var_name(def.get("name").and_then(Value::as_str).unwrap_or_default());
        let (defaults, type_) = merge_selection_defaults(def, &selection_config);

        let mut component = SelectionComponent {
            name: name.clone(),
            order: sel_cmpts.len(),
            inherited: false,
            type_: type_.clone(),
            init: def.get("value").cloned(),
            bind: def.get("bind").cloned(),
            resolve: defaults
                .get("resolve")
                .and_then(Value::as_str)
                .unwrap_or("global")
                .to_string(),
            mark: defaults.get("mark").cloned(),
            toggle: defaults.get("toggle").cloned(),
            translate: defaults.get("translate").cloned(),
            zoom: defaults.get("zoom").cloned(),
            nearest: defaults.get("nearest").cloned(),
            clear: defaults.get("clear").cloned(),
            events: defaults.get("on").map(|on| {
                if let Some(s) = on.as_str() {
                    parse_selector(s, "scope")
                } else {
                    array(on)
                }
            }),
            project: Default::default(),
            scales: None,
            extra: Map::new(),
        };

        for (k, v) in &defaults {
            if !known_key(k) {
                component.extra.insert(k.clone(), v.clone());
            }
        }

        if is_timer_selection(&component) {
            n_timer_selections += 1;
            if n_timer_selections > 1 {
                continue;
            }
        }

        let mut def_ = def.clone();
        for c in selection_compilers() {
            if (c.defined)(&component) {
                if let Some(parse) = c.parse {
                    parse(model, &mut component, &mut def_);
                }
            }
        }

        sel_cmpts.insert(name, component);
    }

    if n_timer_selections > 1 {
        log::warn(message::MULTIPLE_TIMER_ANIMATION_SELECTION);
    }

    sel_cmpts
}

/// Convenience wrapper: parse and store selections on the model.
pub fn parse_and_set_unit_selection(
    model: &mut Model,
    sel_defs: &[Value],
) -> BTreeMap<String, SelectionComponent> {
    let components = parse_unit_selection(model, sel_defs);
    set_selection_components(model, &components);
    components
}

/// Port of `parseSelectionPredicate`.
pub fn parse_selection_predicate(model: &Model, pred: &Value, datum: &str) -> String {
    let name = pred.as_str().map(str::to_string).unwrap_or_else(|| {
        pred.get("param")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    });
    let vname = var_name(&name);
    let store = string_value(&json!(format!("{}{}", vname, super::component::STORE)));

    let sel_cmpt = match get_selection_component(model, &vname, &name) {
        Some(c) => c,
        None => return format!("!!{vname}"),
    };

    let fn_name = if sel_cmpt.project.has_selection_id {
        "vlSelectionIdTest("
    } else {
        "vlSelectionTest("
    };
    let resolve = if sel_cmpt.resolve == "global" {
        ")".to_string()
    } else {
        format!(", {})", string_value(&json!(sel_cmpt.resolve)))
    };
    let test = format!("{fn_name}{store}, {datum}{resolve}");
    let length = format!("length(data({store}))");

    if pred.get("empty").and_then(Value::as_bool) == Some(false) {
        format!("{length} && {test}")
    } else {
        format!("!{length} || {test}")
    }
}

/// Collect the `TimeUnitNode` components required by selection predicates in
/// `filter` (upstream `parseSelectionPredicate` inserts them into the dataflow).
pub fn selection_predicate_time_units(model: &Model, filter: &Value) -> Vec<Value> {
    fn walk(model: &Model, op: &Value, out: &mut Vec<Value>) {
        if let Some(items) = op.get("and").and_then(Value::as_array) {
            for item in items {
                walk(model, item, out);
            }
        } else if let Some(items) = op.get("or").and_then(Value::as_array) {
            for item in items {
                walk(model, item, out);
            }
        } else if let Some(item) = op.get("not") {
            walk(model, item, out);
        } else if let Some(name) = op.get("param").and_then(Value::as_str) {
            let vname = var_name(name);
            if let Some(sel_cmpt) = get_selection_component(model, &vname, name) {
                // Inherited selections are defined in a sibling/ancestor unit
                // whose own data pipeline already materializes the time unit.
                if let Some(Value::Object(map)) = &sel_cmpt.project.time_unit {
                    for component in map.values() {
                        out.push(component.clone());
                    }
                }
            }
        }
    }

    let mut out = Vec::new();
    walk(model, filter, &mut out);
    out
}

/// Port of `parseSelectionExtent`.
pub fn parse_selection_extent(model: &Model, name: &str, extent: &Value) -> String {
    let vname = var_name(name);
    let encoding = extent.get("encoding").and_then(Value::as_str);
    let mut field = extent
        .get("field")
        .and_then(Value::as_str)
        .map(str::to_string);

    let sel_cmpt = match get_selection_component(model, &vname, name) {
        Some(c) => c,
        None => return vname,
    };

    if encoding.is_none() && field.is_none() {
        field = sel_cmpt.project.items.first().map(|p| p.field.clone());
        if sel_cmpt.project.items.len() > 1 {
            log::warn(message::selection_as_scale_domain_without_field(
                field.as_deref().unwrap_or_default(),
            ));
        }
    } else if let Some(encoding) = encoding {
        if field.is_none() {
            let encodings: Vec<_> = sel_cmpt
                .project
                .items
                .iter()
                .filter(|p| p.channel.as_deref() == Some(encoding))
                .cloned()
                .collect();
            if encodings.len() != 1 {
                field = sel_cmpt.project.items.first().map(|p| p.field.clone());
                log::warn(message::selection_as_scale_domain_wrong_encodings(
                    &encodings,
                    encoding,
                    extent,
                    field.as_deref().unwrap_or_default(),
                ));
            } else {
                field = Some(encodings[0].field.clone());
            }
        }
    }

    let field = field.unwrap_or_default();
    format!(
        "{}[{}]",
        sel_cmpt.name,
        string_value(&json!(replace_path_in_field(&field)))
    )
}

/// Port of `materializeSelections`.
///
/// The dataflow is represented by JSON placeholders in this port; each
/// materialized selection becomes a lookup output node whose descriptor carries
/// the filter parameter.
pub fn materialize_selections(model: &mut Model, main: &str) {
    let components = super::component::selection_components(model);
    for (selection, _sel_cmpt) in components {
        let lookup_name = model.get_name(&format!("lookup_{selection}"));
        model.component.data.output_nodes.insert(
            lookup_name.clone(),
            json!({
                "name": lookup_name,
                "type": "filter",
                "param": selection,
                "source": main,
            }),
        );
    }
}

/// Port of the per-model `parseSelections` methods (unit/facet/layer/concat).
///
/// Selections are merged up the hierarchy; animation in a multi-view display
/// is reported as an error.
pub fn parse_selections(model: &mut Model) {
    match model.type_ {
        crate::compile::model::SpecType::Unit => {
            let defs = model.unit.selection.clone();
            let components = parse_unit_selection(model, &defs);
            set_selection_components(model, &components);
        }
        crate::compile::model::SpecType::Facet => {
            if let Some(child) = model.children.get_mut(0) {
                parse_selections(child);
                model.component.selection = child.component.selection.clone();
            }
            let suffix = facet_unit_name_suffix(model);
            if !suffix.is_empty() {
                for child in &mut model.children {
                    apply_facet_suffix(child, &suffix);
                }
            }
            propagate_selection_extents(model);
            warn_if_animated(model);
        }
        _ => {
            let mut merged = Map::new();
            for child in &mut model.children {
                parse_selections(child);
                if let Value::Object(obj) = &child.component.selection {
                    for (k, v) in obj {
                        merged.insert(k.clone(), v.clone());
                    }
                }
            }
            model.component.selection = Value::Object(merged);
            let inherited = super::component::selection_components(model);
            for child in &mut model.children {
                inherit_into(child, &inherited);
            }
            propagate_selection_extents(model);
            warn_if_animated(model);
        }
    }
}

/// Selection unit-name suffix for units nested in a facet, mirroring
/// `unitName` in `src/compile/selection/index.ts`.
pub const FACET_SUFFIX_KEY: &str = "__facetSuffix";

fn facet_unit_name_suffix(model: &Model) -> String {
    let mut suffix = String::new();
    for channel in crate::channel::FACET_CHANNELS {
        if let Some(field_def) = model.facet_data.facet.get(channel) {
            if !crate::channeldef_support::truthy(field_def) {
                continue;
            }
            let field = model.vg_field(channel);
            suffix.push_str(&format!(
                " + '__facet_{channel}_' + (facet[{}])",
                string_value(&json!(field))
            ));
        }
    }
    suffix
}

fn apply_facet_suffix(model: &mut Model, suffix: &str) {
    if model.is_unit_model() {
        if let Value::Object(components) = &mut model.component.selection {
            for component in components.values_mut() {
                if let Value::Object(map) = component {
                    map.insert(
                        FACET_SUFFIX_KEY.to_string(),
                        Value::String(suffix.to_string()),
                    );
                }
            }
        }
        return;
    }
    if model.type_ == crate::compile::model::SpecType::Facet {
        // The nearest facet supplies the suffix (upstream stops at the first
        // facet ancestor), so nested facet children are handled by their own
        // facet model.
        return;
    }
    for child in &mut model.children {
        apply_facet_suffix(child, suffix);
    }
}

/// Propagate `selectionExtent` set on merged child scales up to the parent
/// scale component (upstream children share the parent's component object,
/// while this port keeps a snapshot on each child).
fn propagate_selection_extents(model: &mut Model) {
    for child in &model.children {
        for (channel, child_cmpt) in &child.component.scales {
            if !child_cmpt.merged {
                continue;
            }
            let Some(extent) = child_cmpt.get("selectionExtent") else {
                continue;
            };
            if let Some(parent) = model.component.scales.get_mut(channel) {
                if parent.get("selectionExtent").is_none() {
                    parent.set("selectionExtent", extent, true);
                }
            }
        }
    }
    // Push the parent extent back down to merged children: upstream shares the
    // scale component object, so every child sees the extent added by a
    // sibling's bound interval selection.
    let parent_extents: Vec<(String, Value)> = model
        .component
        .scales
        .iter()
        .filter_map(|(channel, cmpt)| {
            cmpt.get("selectionExtent")
                .map(|extent| (channel.clone(), extent))
        })
        .collect();
    for child in &mut model.children {
        for (channel, extent) in &parent_extents {
            if let Some(child_cmpt) = child.component.scales.get_mut(channel) {
                if child_cmpt.merged && child_cmpt.get("selectionExtent").is_none() {
                    child_cmpt.set("selectionExtent", extent.clone(), true);
                }
            }
            if let Some(merged) = child.merged_scales.get_mut(channel) {
                if merged.get("selectionExtent").is_none() {
                    std::rc::Rc::make_mut(merged).set("selectionExtent", extent.clone(), true);
                }
            }
        }
    }
    for child in &mut model.children {
        propagate_selection_extents(child);
    }
}

/// Make ancestor/sibling selections available to descendants for lookups,
/// mirroring `Model.getSelectionComponent`'s walk up the model tree.
fn inherit_into(model: &mut Model, inherited: &BTreeMap<String, SelectionComponent>) {
    let mut current = super::component::selection_components(model);
    for (name, component) in inherited {
        if !current.contains_key(name) {
            let mut copy = component.clone();
            copy.inherited = true;
            current.insert(name.clone(), copy);
        }
    }
    super::component::set_selection_components(model, &current);
    let all = current;
    for child in &mut model.children {
        inherit_into(child, &all);
    }
}

fn warn_if_animated(model: &Model) {
    let components = super::component::selection_components(model);
    if components.values().any(is_timer_selection) {
        log::warn(message::MULTI_VIEW_ANIMATION_UNSUPPORTED);
    }
}

//! Port of `src/compile/data/formatparse.ts`.

use std::any::Any;

use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use crate::channeldef::{
    get_field_def, is_field_or_datum_def_for_time_format, is_scale_field_def,
    is_typed_field_def,
};
use crate::logical::for_each_leaf;
use crate::predicate::{
    is_field_equal_predicate, is_field_gt_predicate, is_field_gte_predicate,
    is_field_lt_predicate, is_field_lte_predicate, is_field_one_of_predicate, is_field_predicate,
    is_field_range_predicate,
};
use crate::util::{
    access_path_depth, access_path_with_datum, duplicate, hash, remove_path_from_field,
};

use super::dataflow::{with_node, with_node_mut, DataFlowNode, FieldSet, NodeBehavior, NodeRef};
use super::index::AncestorParse;
use super::support::{hash_str, is_date_time};
use crate::compile::model::Model;

/// Port of `Parse` (an ordered `Dict<string>`).
pub type Parse = IndexMap<String, Value>;

/// Remove quotes from a string.
fn unquote(pattern: &str) -> String {
    let bytes = pattern.as_bytes();
    if bytes.len() >= 2
        && ((pattern.starts_with('\'') && pattern.ends_with('\''))
            || (pattern.starts_with('"') && pattern.ends_with('"')))
    {
        return pattern[1..pattern.len() - 1].to_string();
    }
    pattern.to_string()
}

/// Port of `parseExpression`.
pub fn parse_expression(field: &str, parse: &str) -> Option<String> {
    let f = access_path_with_datum(field, "datum");
    if parse == "number" {
        Some(format!("toNumber({f})"))
    } else if parse == "boolean" {
        Some(format!("toBoolean({f})"))
    } else if parse == "string" {
        Some(format!("toString({f})"))
    } else if parse == "date" {
        Some(format!("toDate({f})"))
    } else if parse == "flatten" {
        Some(f)
    } else if let Some(rest) = parse.strip_prefix("date:") {
        let specifier = unquote(rest);
        Some(format!("timeParse({f},'{specifier}')"))
    } else if let Some(rest) = parse.strip_prefix("utc:") {
        let specifier = unquote(rest);
        Some(format!("utcParse({f},'{specifier}')"))
    } else {
        crate::log::warn(format!(
            "Unrecognized parse type: {}",
            crate::util::stringify(&Value::String(parse.to_string()))
        ));
        None
    }
}

/// Port of `getImplicitFromFilterTransform`.
pub fn get_implicit_from_filter_transform(transform: &Value) -> Parse {
    let mut implicit = Parse::new();
    let filter = transform.get("filter").cloned().unwrap_or(Value::Null);

    for_each_leaf(&filter, &mut |filter| {
        if is_field_predicate(filter) {
            let mut val = Value::Null;

            if is_field_equal_predicate(filter) {
                val = filter.get("equal").cloned().unwrap_or(Value::Null);
            } else if is_field_lte_predicate(filter) {
                val = filter.get("lte").cloned().unwrap_or(Value::Null);
            } else if is_field_lt_predicate(filter) {
                val = filter.get("lt").cloned().unwrap_or(Value::Null);
            } else if is_field_gt_predicate(filter) {
                val = filter.get("gt").cloned().unwrap_or(Value::Null);
            } else if is_field_gte_predicate(filter) {
                val = filter.get("gte").cloned().unwrap_or(Value::Null);
            } else if is_field_range_predicate(filter) {
                val = filter
                    .get("range")
                    .and_then(|r| r.as_array())
                    .and_then(|a| a.first())
                    .cloned()
                    .unwrap_or(Value::Null);
            } else if is_field_one_of_predicate(filter) {
                val = filter
                    .get("oneOf")
                    .or_else(|| filter.get("in"))
                    .and_then(|r| r.as_array())
                    .and_then(|a| a.first())
                    .cloned()
                    .unwrap_or(Value::Null);
            }

            let field = filter
                .get("field")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();

            if !val.is_null()
                && !(val.is_object() && (val.get("expr").is_some() || val.get("signal").is_some()))
            {
                if is_date_time(&val) {
                    implicit.insert(field.clone(), json!("date"));
                } else if val.is_number() {
                    implicit.insert(field.clone(), json!("number"));
                } else if val.is_string() {
                    implicit.insert(field.clone(), json!("string"));
                }
            }

            if filter.get("timeUnit").map(|v| !v.is_null()).unwrap_or(false) {
                implicit.insert(field, json!("date"));
            }
        }
    });

    implicit
}

/// Iterate over `(fieldDef, channel)` of a model, like `forEachFieldDef`.
fn for_each_field_def(model: &Model, f: &mut impl FnMut(&Value, &str)) {
    let Some(mapping) = model.encoding() else {
        return;
    };
    let Some(obj) = mapping.as_object() else {
        return;
    };
    for (channel, channel_def) in obj {
        if let Some(field_def) = get_field_def(channel_def) {
            f(&field_def, channel);
        }
    }
}

/// Port of `getImplicitFromEncoding`.
pub fn get_implicit_from_encoding(model: &Model) -> Parse {
    let mut implicit = Parse::new();

    fn add(field_def: &Value, implicit: &mut Parse) {
        let field = field_def
            .get("field")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_default();

        if is_field_or_datum_def_for_time_format(field_def) {
            implicit.insert(field, json!("date"));
        } else if field_def.get("type").and_then(Value::as_str) == Some("quantitative")
            && field_def
                .get("aggregate")
                .map(|a| match a {
                    Value::String(s) => crate::aggregate::is_min_max_op_name(s),
                    Value::Object(o) => o
                        .get("op")
                        .and_then(Value::as_str)
                        .map(crate::aggregate::is_min_max_op_name)
                        .unwrap_or(false),
                    _ => false,
                })
                .unwrap_or(false)
        {
            // we need to parse numbers to support correct min and max
            implicit.insert(field, json!("number"));
        } else if access_path_depth(&field) > 1 {
            // For non-date/non-number (strings and booleans), derive a flattened
            // field for a referenced nested field.
            if !implicit.contains_key(&field) {
                implicit.insert(field, json!("flatten"));
            }
        } else if is_scale_field_def(field_def) {
            if let Some(sort) = field_def.get("sort") {
                if is_sort_field(sort) {
                    let sort_field = sort
                        .get("field")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if access_path_depth(sort_field) > 1 && !implicit.contains_key(sort_field) {
                        implicit.insert(sort_field.to_string(), json!("flatten"));
                    }
                }
            }
        }
    }

    if model.is_unit_model() || model.is_facet_model() {
        for_each_field_def(model, &mut |field_def, channel| {
            if is_typed_field_def(field_def) {
                add(field_def, &mut implicit);
            } else {
                let main_channel = crate::channel::get_main_range_channel(channel);
                if let Some(main_field_def) = model.field_def(main_channel) {
                    let mut merged = field_def.clone();
                    if let Value::Object(ref mut o) = merged {
                        o.insert(
                            "type".into(),
                            main_field_def.get("type").cloned().unwrap_or(Value::Null),
                        );
                    }
                    add(&merged, &mut implicit);
                }
            }
        });
    }

    // Parse quantitative dimension fields of path marks as numbers so that we
    // sort them correctly.
    if model.is_unit_model() {
        let mark = model.mark_type();
        let encoding = model.encoding().cloned().unwrap_or_else(|| json!({}));
        if crate::mark::is_path_mark(&mark)
            && !encoding.get("order").map(|v| !v.is_null()).unwrap_or(false)
        {
            let orient = model
                .mark_def()
                .and_then(|m| m.get("orient"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let dimension_channel = if orient == "horizontal" { "y" } else { "x" };
            if let Some(dimension_channel_def) = encoding.get(dimension_channel) {
                if let Some(field_def) = get_field_def(dimension_channel_def) {
                    let field = field_def
                        .get("field")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if field_def.get("type").and_then(Value::as_str) == Some("quantitative")
                        && !implicit.contains_key(field)
                    {
                        implicit.insert(field.to_string(), json!("number"));
                    }
                }
            }
        }
    }

    implicit
}

/// Port of `getImplicitFromSelection`.
pub fn get_implicit_from_selection(model: &Model) -> Parse {
    let mut implicit = Parse::new();

    if model.is_unit_model() {
        if let Value::Object(selections) = &model.component.selection {
            for (_name, sel_cmpt) in selections {
                let items = sel_cmpt
                    .get("project")
                    .and_then(|p| p.get("items"))
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                for proj in items {
                    let field = proj
                        .get("field")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let has_channel = proj
                        .get("channel")
                        .map(|c| !c.is_null())
                        .unwrap_or(false);
                    if !has_channel && access_path_depth(field) > 1 {
                        implicit.insert(field.to_string(), json!("flatten"));
                    }
                }
            }
        }
    }

    implicit
}

/// Port of `isSortField` (`src/sort.ts`), limited to the dataflow needs.
fn is_sort_field(sort: &Value) -> bool {
    crate::normalize_support::is_sort_field(sort)
}

// ---------------------------------------------------------------------------
// ParseNode
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ParseNode {
    pub parse: Parse,
}

impl ParseNode {
    pub fn new(parent: Option<&NodeRef>, parse: Parse) -> NodeRef {
        DataFlowNode::new(Box::new(ParseNode { parse }), parent, None)
    }

    pub fn clone_parse_node(node: &NodeRef) -> NodeRef {
        with_node::<ParseNode, _>(node, |n| ParseNode::new(None, duplicate(&n.parse)))
            .unwrap_or_else(|| ParseNode::new(None, Parse::new()))
    }

    /// Creates a parse node from a `data.format.parse` and updates ancestorParse.
    pub fn make_explicit(
        parent: Option<&NodeRef>,
        model: &Model,
        ancestor_parse: &mut AncestorParse,
    ) -> Option<NodeRef> {
        let mut explicit = Parse::new();
        if let Some(data) = &model.data {
            if !super::source::is_generator(data) {
                if let Some(parse) = data
                    .get("format")
                    .and_then(|f| f.get("parse"))
                    .and_then(Value::as_object)
                {
                    for (k, v) in parse {
                        explicit.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        ParseNode::make_with_ancestors(parent, explicit, Parse::new(), ancestor_parse)
    }

    /// Creates a parse node from "explicit" parse and "implicit" parse and
    /// updates ancestorParse.
    pub fn make_with_ancestors(
        parent: Option<&NodeRef>,
        explicit: Parse,
        implicit: Parse,
        ancestor_parse: &mut AncestorParse,
    ) -> Option<NodeRef> {
        let mut explicit = explicit;
        let mut implicit = implicit;

        // We should not parse what has already been parsed in a parent.
        let implicit_keys: Vec<String> = implicit.keys().cloned().collect();
        for field in implicit_keys {
            let parsed_as = ancestor_parse.get_with_explicit(&field);
            if parsed_as.value.is_some() {
                let parsed_value = parsed_as.value.clone().unwrap_or(Value::Null);
                let implicit_value = implicit.get(&field).cloned().unwrap_or(Value::Null);
                if parsed_as.explicit
                    || parsed_value == implicit_value
                    || parsed_value == json!("derived")
                    || implicit_value == json!("flatten")
                {
                    implicit.shift_remove(&field);
                } else {
                    crate::log::warn(format!(
                        "Different parse types for field \"{}\": implicitly parsed as {}, but was already parsed as {}.",
                        field,
                        super::support::js_value_string(&implicit_value),
                        super::support::js_value_string(&parsed_value)
                    ));
                }
            }
        }

        let explicit_keys: Vec<String> = explicit.keys().cloned().collect();
        for field in explicit_keys {
            if let Some(parsed_as) = ancestor_parse.get(&field) {
                if parsed_as == explicit[&field] {
                    explicit.shift_remove(&field);
                } else {
                    crate::log::warn(format!(
                        "Different parse types for field \"{}\": explicitly parsed as {}, but was already parsed as {}.",
                        field,
                        super::support::js_value_string(&explicit[&field]),
                        super::support::js_value_string(&parsed_as)
                    ));
                }
            }
        }

        // Merge like `new Split(explicit, implicit)`: explicit wins.
        let mut combined: Parse = explicit.clone();
        for (k, v) in &implicit {
            combined.entry(k.clone()).or_insert_with(|| v.clone());
        }

        // add the format parse from this model so that children don't parse the
        // same field again
        {
            let split = crate::compile::split::Split {
                explicit: explicit
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
                implicit: implicit
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            };
            ancestor_parse.copy_all(&split);
        }

        // copy only non-null parses
        let mut p = Parse::new();
        for (key, val) in &combined {
            let v = if explicit.contains_key(key) {
                explicit[key].clone()
            } else {
                val.clone()
            };
            if !v.is_null() {
                p.insert(key.clone(), v);
            }
        }

        if p.is_empty() || ancestor_parse.parse_nothing {
            return None;
        }

        Some(ParseNode::new(parent, p))
    }

    pub fn merge(node: &NodeRef, other: &NodeRef) {
        let other_parse = with_node::<ParseNode, _>(other, |n| n.parse.clone()).unwrap_or_default();
        with_node_mut::<ParseNode, _>(node, |n| {
            for (k, v) in other_parse {
                n.parse.insert(k, v);
            }
        });
        super::dataflow::remove(other);
    }

    /// Assemble an object for Vega's `format.parse` property.
    pub fn assemble_format_parse(&self) -> Value {
        let mut format_parse = Map::new();
        for (field, p) in &self.parse {
            if access_path_depth(field) == 1 {
                format_parse.insert(field.clone(), p.clone());
            }
        }
        Value::Object(format_parse)
    }

    pub fn assemble_transforms(&self, only_nested: bool) -> Vec<Value> {
        let mut out = Vec::new();
        for (field, p) in &self.parse {
            if only_nested && access_path_depth(field) <= 1 {
                continue;
            }
            let parse = p.as_str().unwrap_or("");
            let Some(expr) = parse_expression(field, parse) else {
                continue;
            };
            out.push(json!({
                "type": "formula",
                "expr": expr,
                "as": remove_path_from_field(field),
            }));
        }
        out
    }
}

impl NodeBehavior for ParseNode {
    fn clone_box(&self) -> Box<dyn NodeBehavior> {
        Box::new(ParseNode {
            parse: duplicate(&self.parse),
        })
    }

    fn hash(&self) -> Value {
        Value::String(format!("Parse {}", hash_str(&hash(&json!(self.parse)))))
    }

    fn dependent_fields(&self) -> FieldSet {
        self.parse.keys().cloned().collect()
    }

    fn produced_fields(&self) -> Option<FieldSet> {
        Some(self.parse.keys().cloned().collect())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

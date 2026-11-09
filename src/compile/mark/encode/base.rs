//! Port of `src/compile/mark/encode/base.ts`.

use serde_json::{json, Map, Value};

use crate::channeldef_support::truthy;
use crate::compile::model::Model;
use crate::compile_common::signal_or_value_ref;
use crate::util::has_property;

use super::aria::aria;
use super::color::color;
use super::nonposition::{non_position, NonPositionOpt};
use super::text::text;
use super::tooltip::tooltip;
use super::zindex::zindex;

pub const VG_MARK_CONFIGS: [&str; 57] = [
    "aria",
    "description",
    "ariaRole",
    "ariaRoleDescription",
    "blend",
    "opacity",
    "fill",
    "fillOpacity",
    "stroke",
    "strokeCap",
    "strokeWidth",
    "strokeOpacity",
    "strokeDash",
    "strokeDashOffset",
    "strokeJoin",
    "strokeOffset",
    "strokeMiterLimit",
    "startAngle",
    "endAngle",
    "padAngle",
    "innerRadius",
    "outerRadius",
    "size",
    "shape",
    "interpolate",
    "tension",
    "orient",
    "align",
    "baseline",
    "text",
    "dir",
    "dx",
    "dy",
    "ellipsis",
    "limit",
    "radius",
    "theta",
    "angle",
    "font",
    "fontSize",
    "fontWeight",
    "fontStyle",
    "lineBreak",
    "lineHeight",
    "cursor",
    "href",
    "tooltip",
    "cornerRadius",
    "cornerRadiusTopLeft",
    "cornerRadiusTopRight",
    "cornerRadiusBottomLeft",
    "cornerRadiusBottomRight",
    "aspect",
    "width",
    "height",
    "url",
    "smooth",
];

const ALWAYS_IGNORE: [&str; 3] = ["aria", "width", "height"];

#[derive(Debug, Clone, Copy, Default)]
pub struct Ignore {
    pub color: &'static str,
    pub size: &'static str,
    pub orient: &'static str,
    pub align: &'static str,
    pub baseline: &'static str,
    pub theta: &'static str,
}

impl Ignore {
    pub fn is_ignored(&self, prop: &str) -> bool {
        match prop {
            "color" => self.color == "ignore",
            "size" => self.size == "ignore",
            "orient" => self.orient == "ignore",
            "align" => self.align == "ignore",
            "baseline" => self.baseline == "ignore",
            "theta" => self.theta == "ignore",
            _ => false,
        }
    }
}

pub fn base_encode_entry(model: &Model, ignore: Ignore) -> Value {
    let mark_def = &model.unit.mark_def;

    let mut out = mark_def_properties(mark_def, ignore);

    if ignore.color == "include" {
        if let Value::Object(color_entry) = color(model, None) {
            for (k, v) in color_entry {
                out.insert(k, v);
            }
        }
    }

    for (channel, opt) in [
        ("opacity", NonPositionOpt::default()),
        ("fillOpacity", NonPositionOpt::default()),
        ("strokeOpacity", NonPositionOpt::default()),
        ("strokeWidth", NonPositionOpt::default()),
        ("strokeDash", NonPositionOpt::default()),
    ] {
        merge(&mut out, non_position(channel, model, opt));
    }

    merge(&mut out, zindex(model));
    merge(&mut out, tooltip(model));
    merge(&mut out, text(model, "href"));
    merge(&mut out, aria(model));

    Value::Object(out)
}

fn mark_def_properties(mark_def: &Value, ignore: Ignore) -> Map<String, Value> {
    let mut out = Map::new();
    for prop in VG_MARK_CONFIGS {
        if ALWAYS_IGNORE.contains(&prop) {
            continue;
        }
        if !has_property(mark_def, prop) {
            continue;
        }
        if ignore.is_ignored(prop) {
            continue;
        }
        if prop == "tooltip" {
            let tooltip = &mark_def["tooltip"];
            if tooltip.as_bool() == Some(true)
                || (tooltip.is_object() && has_property(tooltip, "content"))
            {
                continue;
            }
        }
        out.insert(prop.to_string(), signal_or_value_ref(&mark_def[prop]));
    }
    out
}

fn merge(dest: &mut Map<String, Value>, src: Value) {
    if let Value::Object(src) = src {
        for (k, v) in src {
            dest.insert(k, v);
        }
    }
}

#[allow(dead_code)]
fn _unused(v: &Value) -> Value {
    if truthy(v) {
        json!(null)
    } else {
        Value::Null
    }
}

/// Merge encode entries in order (later entries win).
pub fn merge_entries(entries: Vec<Value>) -> Map<String, Value> {
    let mut out = Map::new();
    for entry in entries {
        if let Value::Object(map) = entry {
            for (k, v) in map {
                out.insert(k, v);
            }
        }
    }
    out
}

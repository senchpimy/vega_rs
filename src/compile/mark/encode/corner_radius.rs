//! Port of `src/compile/mark/encode/corner-radius.ts`.

use serde_json::{json, Map, Value};

use crate::compile::model::Model;
use crate::compile_common::signal_or_value_ref;
use crate::mark::get_mark_prop_or_config;
use crate::util::{flat_access_with_datum, string_value};

pub fn corner_radius_end(model: &Model, encode_entry: &Value) -> Value {
    let mark_def = &model.unit.mark_def;
    let config = &model.config;
    let encoding = &model.unit.encoding;
    let radius = get_mark_prop_or_config("cornerRadiusEnd", mark_def, config, None, false);
    let mark_type = mark_def.get("type").and_then(Value::as_str).unwrap_or("");
    let orient = mark_def.get("orient").and_then(Value::as_str);

    let Some(radius) = radius else {
        return Value::Object(Map::new());
    };
    if mark_type != "bar" || orient.is_none() {
        return Value::Object(Map::new());
    }
    let orient = orient.unwrap();

    let is_ranged = if orient == "horizontal" {
        encoding.get("x2").map(|v| !v.is_null()).unwrap_or(false)
    } else {
        encoding.get("y2").map(|v| !v.is_null()).unwrap_or(false)
    };

    if is_ranged {
        if mark_def.get("cornerRadiusEnd").is_none()
            && mark_def.get("cornerRadius").is_some()
        {
            return Value::Object(Map::new());
        }
        return json!({"cornerRadius": signal_or_value_ref(&radius)});
    }

    if orient == "horizontal" {
        let x = position_ref_to_expr(encode_entry.get("x"));
        let x2 = position_ref_to_expr(encode_entry.get("x2"));
        if let (Some(x), Some(x2)) = (x, x2) {
            horizontal_corner_radius(model, &radius, &format!("{x} > {x2}"), &format!("{x} < {x2}"))
        } else {
            Value::Object(Map::new())
        }
    } else {
        let y = position_ref_to_expr(encode_entry.get("y"));
        let y2 = position_ref_to_expr(encode_entry.get("y2"));
        if let (Some(y), Some(y2)) = (y, y2) {
            vertical_corner_radius(model, &radius, &format!("{y} < {y2}"), &format!("{y} > {y2}"))
        } else {
            Value::Object(Map::new())
        }
    }
}

pub fn corner_radius_end_for_stacked_bar(
    model: &Model,
    radius: Option<&Value>,
    scale_name: &str,
    fields: (&str, &str, &str, &str),
) -> Value {
    let Some(radius) = radius else {
        return Value::Object(Map::new());
    };
    let (min_start, max_start, min_end, max_end) = fields;
    let min_start = scaled_field_expr(scale_name, min_start);
    let max_start = scaled_field_expr(scale_name, max_start);
    let min_end = scaled_field_expr(scale_name, min_end);
    let max_end = scaled_field_expr(scale_name, max_end);

    let field_channel = super::super::mark::stack_of(model)
        .and_then(|s| s.get("fieldChannel").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_default();

    if field_channel == "x" {
        horizontal_corner_radius(
            model,
            radius,
            &format!("{max_end} > {max_start}"),
            &format!("{min_end} < {min_start}"),
        )
    } else {
        vertical_corner_radius(
            model,
            radius,
            &format!("{max_end} < {max_start}"),
            &format!("{min_end} > {min_start}"),
        )
    }
}

fn horizontal_corner_radius(model: &Model, radius: &Value, right_test: &str, left_test: &str) -> Value {
    let mut out = Map::new();
    merge(
        &mut out,
        conditional_corner_radius(
            model,
            &["cornerRadiusTopRight", "cornerRadiusBottomRight"],
            right_test,
            radius,
        ),
    );
    merge(
        &mut out,
        conditional_corner_radius(
            model,
            &["cornerRadiusTopLeft", "cornerRadiusBottomLeft"],
            left_test,
            radius,
        ),
    );
    Value::Object(out)
}

fn vertical_corner_radius(model: &Model, radius: &Value, top_test: &str, bottom_test: &str) -> Value {
    let mut out = Map::new();
    merge(
        &mut out,
        conditional_corner_radius(
            model,
            &["cornerRadiusTopLeft", "cornerRadiusTopRight"],
            top_test,
            radius,
        ),
    );
    merge(
        &mut out,
        conditional_corner_radius(
            model,
            &["cornerRadiusBottomLeft", "cornerRadiusBottomRight"],
            bottom_test,
            radius,
        ),
    );
    Value::Object(out)
}

fn conditional_corner_radius(
    model: &Model,
    corners: &[&str],
    test: &str,
    radius: &Value,
) -> Value {
    let mark_def = &model.unit.mark_def;
    let config = &model.config;
    let value_ref = signal_or_value_ref(radius);
    let specified_end = mark_def.get("cornerRadiusEnd").is_some();

    let mut out = Map::new();
    for corner in corners {
        if !specified_end
            && (mark_def.get(*corner).is_some() || mark_def.get("cornerRadius").is_some())
        {
            continue;
        }
        let fallback = get_mark_prop_or_config(corner, mark_def, config, None, false)
            .or_else(|| get_mark_prop_or_config("cornerRadius", mark_def, config, None, false))
            .unwrap_or(json!(0));
        let mut conditional = value_ref.as_object().cloned().unwrap_or_default();
        conditional.insert("test".to_string(), Value::String(test.to_string()));
        out.insert(
            corner.to_string(),
            Value::Array(vec![
                Value::Object(conditional),
                signal_or_value_ref(&fallback),
            ]),
        );
    }
    Value::Object(out)
}

fn position_ref_to_expr(reference: Option<&Value>) -> Option<String> {
    let reference = reference?;
    let reference = match reference {
        Value::Array(arr) => arr.last()?,
        other => other,
    };
    value_ref_to_expr(reference)
}

fn value_ref_to_expr(reference: &Value) -> Option<String> {
    let mut expr: Option<String> = None;

    if let Some(scale) = reference.get("scale") {
        let scaled_value = scaled_value_ref_to_expr(reference)?;
        expr = Some(format!(
            "scale({}, {scaled_value})",
            string_value(scale)
        ));
    } else if let Some(signal) = reference.get("signal") {
        expr = Some(format!("({})", signal.as_str().unwrap_or("")));
    } else if let Some(field) = reference.get("field").filter(|f| !f.is_null()) {
        expr = field_ref_to_expr(field);
    } else if let Some(value) = reference.get("value") {
        expr = Some(string_value(value));
    }

    let mut expr = expr?;

    if let Some(mult) = reference.get("mult").filter(|v| !v.is_null()) {
        expr = format!("{} * ({expr})", string_value(mult));
    }

    if let Some(offset) = reference.get("offset").filter(|v| !v.is_null()) {
        let offset = if offset.is_number() {
            string_value(offset)
        } else {
            value_ref_to_expr(offset)?
        };
        if !offset.is_empty() {
            expr = format!("{expr} + {offset}");
        }
    }

    Some(expr)
}

fn scaled_value_ref_to_expr(reference: &Value) -> Option<String> {
    if let Some(signal) = reference.get("signal") {
        Some(signal.as_str().unwrap_or("").to_string())
    } else if let Some(field) = reference.get("field").filter(|f| !f.is_null()) {
        field_ref_to_expr(field)
    } else { reference.get("value").map(string_value) }
}

fn field_ref_to_expr(field: &Value) -> Option<String> {
    if let Some(field) = field.as_str() {
        Some(flat_access_with_datum(field, "datum"))
    } else if let Some(datum) = field.get("datum") {
        Some(flat_access_with_datum(
            datum.as_str().unwrap_or(""),
            "datum",
        ))
    } else { field.get("parent").map(|parent| flat_access_with_datum(
            parent.as_str().unwrap_or(""),
            "parent",
        )) }
}

fn scaled_field_expr(scale_name: &str, field: &str) -> String {
    format!(
        "scale({}, {})",
        string_value(&Value::String(scale_name.to_string())),
        flat_access_with_datum(field, "datum")
    )
}

fn merge(dest: &mut Map<String, Value>, src: Value) {
    if let Value::Object(src) = src {
        for (k, v) in src {
            dest.insert(k, v);
        }
    }
}

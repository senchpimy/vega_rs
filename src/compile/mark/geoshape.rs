//! Port of `src/compile/mark/geoshape.ts`.

use serde_json::{json, Value};

use crate::channeldef::{is_field_def, vg_field_with, FieldRefOption};
use crate::compile::model::Model;

use super::encode::base::{base_encode_entry, Ignore};

pub fn encode_entry(model: &Model) -> Value {
    base_encode_entry(
        model,
        Ignore {
            align: "ignore",
            baseline: "ignore",
            color: "include",
            size: "ignore",
            orient: "ignore",
            theta: "ignore",
        },
    )
}

pub fn post_encoding_transform(model: &Model) -> Value {
    let shape_def = model.unit.encoding.get("shape").cloned().unwrap_or(Value::Null);
    let mut transform = json!({
        "type": "geoshape",
        "projection": model.projection_name(false),
    });

    if is_field_def(&shape_def)
        && shape_def.get("type").and_then(Value::as_str) == Some("geojson")
    {
        transform["field"] = Value::String(vg_field_with(
            &shape_def,
            &FieldRefOption {
                expr: Some("datum".to_string()),
                ..Default::default()
            },
        ));
    }
    json!([transform])
}

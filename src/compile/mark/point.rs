//! Port of `src/compile/mark/point.ts`.

use serde_json::{json, Map, Value};

use crate::compile::model::Model;

use super::encode::base::{base_encode_entry, merge_entries, Ignore};
use super::encode::nonposition::{non_position, NonPositionOpt};
use super::encode::position_point::point_position;

fn encode(model: &Model, fixed_shape: Option<&str>) -> Value {
    let mut entries = vec![
        base_encode_entry(
            model,
            Ignore {
                align: "ignore",
                baseline: "ignore",
                color: "include",
                size: "include",
                orient: "ignore",
                theta: "ignore",
            },
        ),
        point_position("x", model, Some("mid"), None),
        point_position("y", model, Some("mid"), None),
        non_position("size", model, NonPositionOpt::default()),
        non_position("angle", model, NonPositionOpt::default()),
    ];

    if let Some(fixed_shape) = fixed_shape {
        let mut map = Map::new();
        map.insert("shape".to_string(), json!({"value": fixed_shape}));
        entries.push(Value::Object(map));
    } else {
        entries.push(non_position("shape", model, NonPositionOpt::default()));
    }

    Value::Object(merge_entries(entries))
}

pub fn point_entry(model: &Model) -> Value {
    encode(model, None)
}

pub fn circle_entry(model: &Model) -> Value {
    encode(model, Some("circle"))
}

pub fn square_entry(model: &Model) -> Value {
    encode(model, Some("square"))
}

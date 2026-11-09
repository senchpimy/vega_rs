//! Port of `src/compile/mark/line.ts`.

use serde_json::Value;

use crate::compile::model::Model;

use super::encode::base::{base_encode_entry, merge_entries, Ignore};
use super::encode::defined::defined;
use super::encode::nonposition::{non_position, NonPositionOpt};
use super::encode::position_point::point_position;

pub fn encode_entry(model: &Model) -> Value {
    let out = merge_entries(vec![
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
        ),
        point_position("x", model, Some("mid"), None),
        point_position("y", model, Some("mid"), None),
        non_position(
            "size",
            model,
            NonPositionOpt {
                vg_channel: Some("strokeWidth".to_string()),
                ..Default::default()
            },
        ),
        defined(model),
    ]);
    Value::Object(out)
}

pub fn trail_entry(model: &Model) -> Value {
    let out = merge_entries(vec![
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
        defined(model),
    ]);
    Value::Object(out)
}

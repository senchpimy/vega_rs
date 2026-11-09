//! Port of `src/compile/mark/arc.ts`.

use serde_json::Value;

use crate::compile::model::Model;

use super::encode::base::{base_encode_entry, Ignore};
use super::encode::position_point::point_position;
use super::encode::position_rect::rect_position;

pub fn encode_entry(model: &Model) -> Value {
    let out = super::encode::base::merge_entries(vec![
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
        rect_position(model, "radius"),
        rect_position(model, "theta"),
    ]);
    Value::Object(out)
}

//! Port of `src/compile/mark/rect.ts`.

use serde_json::Value;

use crate::compile::model::Model;

use super::encode::base::{base_encode_entry, merge_entries, Ignore};
use super::encode::position_rect::rect_position;

pub fn encode_entry(model: &Model) -> Value {
    let out = merge_entries(vec![
        base_encode_entry(
            model,
            Ignore {
                align: "ignore",
                baseline: "ignore",
                color: "include",
                orient: "ignore",
                size: "ignore",
                theta: "ignore",
            },
        ),
        rect_position(model, "x"),
        rect_position(model, "y"),
    ]);
    Value::Object(out)
}

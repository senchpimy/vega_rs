//! Port of `src/compile/mark/rule.ts`.

use serde_json::Value;

use crate::compile::model::Model;

use super::encode::base::{base_encode_entry, merge_entries, Ignore};
use super::encode::nonposition::{non_position, NonPositionOpt};
use super::encode::position_range::point_or_range_position;

pub fn encode_entry(model: &Model) -> Value {
    let mark_def = &model.unit.mark_def;
    let encoding = &model.unit.encoding;
    let orient = mark_def.get("orient").and_then(Value::as_str).unwrap_or("");

    let has_x = encoding.get("x").map(|v| !v.is_null()).unwrap_or(false);
    let has_y = encoding.get("y").map(|v| !v.is_null()).unwrap_or(false);
    let has_lat = encoding.get("latitude").map(|v| !v.is_null()).unwrap_or(false);
    let has_long = encoding.get("longitude").map(|v| !v.is_null()).unwrap_or(false);
    if !has_x && !has_y && !has_lat && !has_long {
        return Value::Object(serde_json::Map::new());
    }

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
        point_or_range_position(
            "x",
            model,
            Some(if orient == "horizontal" { "zeroOrMax" } else { "mid" }),
            "zeroOrMin",
            orient != "vertical",
        ),
        point_or_range_position(
            "y",
            model,
            Some(if orient == "vertical" { "zeroOrMax" } else { "mid" }),
            "zeroOrMin",
            orient != "horizontal",
        ),
        non_position(
            "size",
            model,
            NonPositionOpt {
                vg_channel: Some("strokeWidth".to_string()),
                ..Default::default()
            },
        ),
    ]);
    Value::Object(out)
}

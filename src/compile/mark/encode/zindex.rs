//! Port of `src/compile/mark/encode/zindex.ts`.

use serde_json::{Map, Value};

use crate::channeldef::is_value_def;
use crate::compile::model::Model;
use crate::compile_common::signal_or_value_ref;
use crate::mark::is_path_mark;

use super::conditional::wrap_condition;

pub fn zindex(model: &Model) -> Value {
    let encoding = &model.unit.encoding;
    let mark = model.mark_type();
    let order = encoding.get("order").cloned();

    if !is_path_mark(mark.as_str()) && order.as_ref().map(is_value_def).unwrap_or(false) {
        let order = order.unwrap();
        return wrap_condition(
            model,
            Some(&order),
            "zindex",
            None,
            |cd| Some(signal_or_value_ref(&cd["value"])),
        );
    }
    Value::Object(Map::new())
}

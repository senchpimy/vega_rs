//! Port of `src/compile/header/common.ts`.

use serde_json::{Map, Value};

use crate::util::get_first_defined;

/// `getHeaderChannel`: the channel a header/title is laid out on.
pub fn get_header_channel(channel: &str, orient: Option<&Value>) -> &'static str {
    if let Some(orient) = orient.and_then(Value::as_str) {
        if orient == "top" || orient == "bottom" {
            return "column";
        }
        if orient == "left" || orient == "right" {
            return "row";
        }
    }
    if channel == "row" {
        "row"
    } else {
        "column"
    }
}

fn header_specific_config<'a>(config: &'a Value, channel: &str) -> &'a Value {
    let key = match channel {
        "row" => "headerRow",
        "column" => "headerColumn",
        _ => "headerFacet",
    };
    config.get(key).unwrap_or(&Value::Null)
}

/// `getHeaderProperty`: explicit header value, then channel-specific config,
/// then the generic header config.
pub fn get_header_property(
    prop: &str,
    header: Option<&Value>,
    config: &Value,
    channel: &str,
) -> Option<Value> {
    let explicit = header
        .and_then(|h| h.as_object())
        .and_then(|h| h.get(prop))
        .cloned();
    let specific = header_specific_config(config, channel)
        .as_object()
        .and_then(|c| c.get(prop))
        .cloned();
    let generic = config
        .get("header")
        .and_then(|h| h.as_object())
        .and_then(|h| h.get(prop))
        .cloned();

    get_first_defined(&[
        explicit.as_ref(),
        specific.as_ref(),
        generic.as_ref(),
    ])
    .cloned()
}

/// `getHeaderProperties`.
pub fn get_header_properties(
    properties: &[&str],
    header: Option<&Value>,
    config: &Value,
    channel: &str,
) -> Map<String, Value> {
    let mut props = Map::new();
    for prop in properties {
        if let Some(value) = get_header_property(prop, header, config, channel) {
            props.insert((*prop).to_string(), value);
        }
    }
    props
}

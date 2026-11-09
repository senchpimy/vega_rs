//! Port of `src/compile/mark/encode/position-align.ts`.

use serde_json::{json, Value};

use crate::channel::get_vg_position_channel;
use crate::mark::get_mark_prop_or_config;

pub fn vg_aligned_position_channel(
    channel: &str,
    mark_def: &Value,
    config: &Value,
    default_align: &str,
) -> String {
    if channel == "radius" || channel == "theta" {
        return get_vg_position_channel(channel).to_string();
    }
    let align_channel = if channel == "x" { "align" } else { "baseline" };
    let align = get_mark_prop_or_config(align_channel, mark_def, config, None, false);

    let align_is_signal = align
        .as_ref()
        .map(|a| a.get("signal").is_some() || a.get("expr").is_some())
        .unwrap_or(false);
    if align_is_signal {
        crate::log::warn(format!(
            "The {align_channel} for range marks cannot be an expression"
        ));
    }
    let align_excluding_signal = align
        .as_ref()
        .filter(|_a| !align_is_signal)
        .and_then(|a| a.as_str())
        .map(str::to_string);

    if channel == "x" {
        let key = align_excluding_signal.unwrap_or_else(|| {
            if default_align == "top" {
                "left".to_string()
            } else {
                "center".to_string()
            }
        });
        match key.as_str() {
            "left" => "x",
            "right" => "x2",
            _ => "xc",
        }
        .to_string()
    } else {
        let key = align_excluding_signal.unwrap_or_else(|| default_align.to_string());
        match key.as_str() {
            "top" => "y",
            "bottom" => "y2",
            _ => "yc",
        }
        .to_string()
    }
}

#[allow(dead_code)]
fn _unused() -> Value {
    json!(null)
}

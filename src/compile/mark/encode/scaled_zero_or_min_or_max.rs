//! Port of `src/compile/mark/encode/scaledZeroOrMinOrMax.ts`.

use serde_json::{json, Value};

use crate::compile::scale::component::ScaleComponent;

pub fn scaled_zero_or_min_or_max(
    scale_name: &str,
    scale: Option<&ScaleComponent>,
    mode: &str,
) -> Option<Value> {
    let scale = scale?;
    if scale_name.is_empty() {
        return None;
    }
    let domain = format!("domain('{scale_name}')");
    let min = format!("{domain}[0]");
    let max = format!("peek({domain})");

    match scale.domain_has_zero() {
        "definitely" => Some(json!({"scale": scale_name, "value": 0})),
        "maybe" => {
            let non_zero_value = if mode == "zeroOrMin" { min } else { max };
            Some(json!({
                "signal": format!(
                    "scale('{scale_name}', inrange(0, {domain}) ? 0 : {non_zero_value})"
                )
            }))
        }
        _ => {
            let value = if mode == "zeroOrMin" { min } else { max };
            Some(json!({"signal": format!("scale('{scale_name}', {value})")}))
        }
    }
}

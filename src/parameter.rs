//! Port of `src/parameter.ts`.

use serde_json::{Map, Value};

use crate::selection::is_selection_parameter;

pub type ParameterName = String;

/// Port of `assembleParameterSignals`.
pub fn assemble_parameter_signals(params: &[Value]) -> Vec<Value> {
    let mut signals: Vec<Value> = Vec::new();
    for param in params {
        // Selection parameters are handled separately.
        if is_selection_parameter(param) {
            continue;
        }

        let mut rest = param.as_object().cloned().unwrap_or_default();
        let expr = rest.remove("expr");
        let bind = rest.remove("bind");

        match (bind, expr) {
            (Some(bind), Some(expr)) => {
                let mut signal = rest;
                signal.insert("bind".to_string(), bind);
                signal.insert("init".to_string(), expr);
                signals.push(Value::Object(signal));
            }
            (bind, expr) => {
                let mut signal = rest;
                if let Some(expr) = expr {
                    signal.insert("update".to_string(), expr);
                }
                if let Some(bind) = bind {
                    signal.insert("bind".to_string(), bind);
                }
                signals.push(Value::Object(signal));
            }
        }
    }
    signals
}

#[allow(dead_code)]
fn _map(_: Map<String, Value>) {}

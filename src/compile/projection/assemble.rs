//! Port of `src/compile/projection/assemble.ts`.

use serde_json::{json, Map, Value};

use crate::util::contains;

use super::super::model::{is_concat_model, is_layer_model, Model};
use super::component::ProjectionComponent;

/// Port of `assembleProjections`.
pub fn assemble_projections(model: &Model) -> Vec<Value> {
    if is_layer_model(model) || is_concat_model(model) {
        assemble_projections_for_model_and_children(model)
    } else {
        assemble_projection_for_model(model)
    }
}

fn assemble_projections_for_model_and_children(model: &Model) -> Vec<Value> {
    let mut projections = assemble_projection_for_model(model);
    for child in &model.children {
        projections.extend(child.assemble_projections());
    }
    projections
}

fn assemble_projection_for_model(model: &Model) -> Vec<Value> {
    let component = match &model.component.projection {
        Some(c) if !c.merged => c,
        _ => return Vec::new(),
    };

    let projection = component.combine();
    let name = projection
        .get("name")
        .cloned()
        .unwrap_or(Value::Null);

    let mut out = Map::new();
    out.insert("name".to_string(), name);

    if component.data.is_none() {
        out.insert(
            "translate".to_string(),
            json!({"signal": "[width / 2, height / 2]"}),
        );
        for (k, v) in projection {
            if k == "name" {
                continue;
            }
            out.insert(k, v);
        }
        return vec![Value::Object(out)];
    }

    let size = component.size.clone().unwrap_or_default();
    let size_signals: Vec<String> = size
        .iter()
        .map(|r| {
            r.get("signal")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        })
        .collect();
    out.insert(
        "size".to_string(),
        json!({"signal": format!("[{}]", size_signals.join(", "))}),
    );

    let mut fits: Vec<String> = Vec::new();
    for data in component.data.clone().unwrap_or_default() {
        let source = if let Some(signal) = data.get("signal").and_then(Value::as_str) {
            signal.to_string()
        } else {
            let name = data.as_str().unwrap_or_default().to_string();
            format!("data('{}')", model.lookup_data_source(&name))
        };
        if !contains(&fits, &source) {
            fits.push(source);
        }
    }

    if fits.is_empty() {
        panic!("Projection's fit didn't find any data sources");
    }

    out.insert(
        "fit".to_string(),
        json!({"signal": if fits.len() > 1 { format!("[{}]", fits.join(", ")) } else { fits[0].clone() }}),
    );

    for (k, v) in projection {
        if k == "name" {
            continue;
        }
        out.insert(k, v);
    }
    vec![Value::Object(out)]
}

#[allow(dead_code)]
fn _unused(_: &ProjectionComponent) {}

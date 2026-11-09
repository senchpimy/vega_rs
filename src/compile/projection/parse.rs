//! Port of `src/compile/projection/parse.ts`.

use serde_json::{Map, Value};

use crate::channel::{LATITUDE, LATITUDE2, LONGITUDE, LONGITUDE2, SHAPE};
use crate::channeldef::{get_field_or_datum_def, is_field_def};
use crate::type_::Type;
use crate::util::{deep_equal, every, has_property};

use super::super::model::{is_unit_model, Model};
use super::component::ProjectionComponent;

pub const PROJECTION_PROPERTIES: [&str; 17] = [
    "type",
    "clipAngle",
    "clipExtent",
    "center",
    "rotate",
    "precision",
    "reflectX",
    "reflectY",
    "coefficient",
    "distance",
    "fraction",
    "lobes",
    "parallel",
    "radius",
    "ratio",
    "spacing",
    "tilt",
];

/// Port of `parseProjection`.
pub fn parse_projection(model: &mut Model) {
    model.component.projection = if is_unit_model(model) {
        parse_unit_projection(model)
    } else {
        parse_non_unit_projections(model)
    };
}

/// Port of `replaceExprRef` for a projection object: each top-level property
/// is passed through `signalRefOrValue`.
fn replace_expr_ref(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(crate::expr::replace_expr_ref(map, 0)),
        _ => Value::Object(Map::new()),
    }
}

fn parse_unit_projection(model: &mut Model) -> Option<ProjectionComponent> {
    if !model.has_projection() {
        return None;
    }
    let proj = replace_expr_ref(&model.unit.specified_projection.clone());
    let fit = !(proj.get("scale").map(|v| !v.is_null()).unwrap_or(false)
        || proj.get("translate").map(|v| !v.is_null()).unwrap_or(false));
    let size = if fit {
        Some(vec![model.get_size_signal_ref("width"), model.get_size_signal_ref("height")])
    } else {
        None
    };
    let data = if fit { Some(gather_fit_data(model)) } else { None };

    let name = model.projection_name(true).unwrap_or_default();
    let config_projection = model
        .config
        .get("projection")
        .cloned()
        .unwrap_or(Value::Null);
    let mut specified = replace_expr_ref(&config_projection);
    if let (Some(base), Some(map)) = (specified.as_object_mut(), proj.as_object()) {
        for (k, v) in map {
            base.insert(k.clone(), v.clone());
        }
    } else {
        specified = proj.clone();
    }

    let mut proj_comp = ProjectionComponent::new(&name, &specified, size, data);
    if proj_comp.get("type").map(|v| v.is_null()).unwrap_or(true) {
        proj_comp.set("type", Value::String("equalEarth".to_string()), false);
    }
    Some(proj_comp)
}

fn gather_fit_data(model: &mut Model) -> Vec<Value> {
    let mut data: Vec<Value> = Vec::new();
    let encoding = model.encoding().cloned().unwrap_or(Value::Null);

    for pair in [[LONGITUDE, LATITUDE], [LONGITUDE2, LATITUDE2]] {
        if get_field_or_datum_def(&encoding[pair[0]]).is_some()
            || get_field_or_datum_def(&encoding[pair[1]]).is_some()
        {
            let name = model.get_name(&format!("geojson_{}", data.len()));
            data.push(serde_json::json!({"signal": name}));
        }
    }

    if model.channel_has_field(SHAPE) {
        if let Some(field_def) = model.typed_field_def(SHAPE) {
            if field_def.get("type").and_then(Value::as_str) == Some(Type::Geojson.as_str()) {
                let name = model.get_name(&format!("geojson_{}", data.len()));
                data.push(serde_json::json!({"signal": name}));
            }
        }
    }

    if data.is_empty() {
        data.push(Value::String(model.request_data_name("main")));
    }

    data
}

fn merge_if_no_conflict(
    first: &ProjectionComponent,
    second: &ProjectionComponent,
) -> Option<ProjectionComponent> {
    let all_properties_shared = every(&PROJECTION_PROPERTIES, |prop, _| {
        let first_has = first.explicit.contains_key(*prop);
        let second_has = second.explicit.contains_key(*prop);
        if !first_has && !second_has {
            return true;
        }
        if first_has
            && second_has
            && deep_equal(
                &first.get(prop).unwrap_or(Value::Null),
                &second.get(prop).unwrap_or(Value::Null),
            )
        {
            return true;
        }
        false
    });

    let size = deep_equal(
        &first.size.clone().map(Value::Array).unwrap_or(Value::Null),
        &second.size.clone().map(Value::Array).unwrap_or(Value::Null),
    );
    if size {
        if all_properties_shared {
            return Some(first.clone());
        } else if first.explicit.is_empty() {
            return Some(second.clone());
        } else if second.explicit.is_empty() {
            return Some(first.clone());
        }
    }
    None
}

fn parse_non_unit_projections(model: &mut Model) -> Option<ProjectionComponent> {
    if model.children.is_empty() {
        return None;
    }
    let mut non_unit_projection: Option<ProjectionComponent> = None;

    for child in &mut model.children {
        parse_projection(child);
    }

    let mut mergable = true;
    for child in &model.children {
        let projection = &child.component.projection;
        match projection {
            None => {}
            Some(projection) => match &non_unit_projection {
                None => non_unit_projection = Some(projection.clone()),
                Some(existing) => match merge_if_no_conflict(existing, projection) {
                    Some(merged) => non_unit_projection = Some(merged),
                    None => mergable = false,
                },
            },
        }
    }

    if let Some(non_unit_projection) = non_unit_projection {
        if mergable {
            let name = model.projection_name(true).unwrap_or_default();
            let mut model_projection = ProjectionComponent::new(
                &name,
                &Value::Object(
                    non_unit_projection
                        .explicit
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect::<Map<String, Value>>(),
                ),
                non_unit_projection.size.clone(),
                non_unit_projection.data.clone(),
            );
            model_projection.set_with_explicit(
                "type",
                non_unit_projection.get_with_explicit("type"),
            );

            for child in &mut model.children {
                if let Some(child_projection) = child.component.projection.clone() {
                    if child_projection.is_fit() {
                        if let (Some(data), Some(child_data)) =
                            (&mut model_projection.data, &child_projection.data)
                        {
                            data.extend(child_data.clone());
                        }
                    }
                    if let Some(old_name) = child_projection.get("name") {
                        if let Some(old_name) = old_name.as_str() {
                            child.rename_projection(old_name, &name);
                        }
                    }
                    if let Some(p) = &mut child.component.projection {
                        p.merged = true;
                    }
                }
            }
            return Some(model_projection);
        }
    }

    None
}

#[allow(dead_code)]
fn _unused(_: &Value) -> bool {
    has_property(&Value::Null, "x") && is_field_def(&Value::Null)
}

//! Port of `src/compile/mark/init.ts`.

use serde_json::{Map, Value};

use crate::channeldef::{
    is_field_def, is_numeric_data_def, is_typed_field_def,
    is_unbinned_quantitative_field_or_datum_def,
};
use crate::encoding::{channel_has_quantitative_offset, is_aggregate};
use crate::mark::{LINE, POINT, RULE, TICK};
use crate::mark::{get_mark_prop_or_config};
use crate::util::get_first_defined;


pub const DEFAULT_REDUCED_OPACITY: f64 = 0.7;

/// Port of `initMarkdef`.
pub fn init_markdef(original_mark_def: &Value, encoding: &Value, config: &Value) -> Value {
    // `replaceExprRef` maps each top-level mark property through
    // `signalRefOrValue` (ExprRef -> SignalRef).
    let mut mark_def = match original_mark_def {
        Value::Object(map) => Value::Object(crate::expr::replace_expr_ref(map, 0)),
        other => other.clone(),
    };
    if !mark_def.is_object() {
        mark_def = Value::Object(Map::new());
    }
    let mark = mark_def
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let specified_orient = get_mark_prop_or_config("orient", &mark_def, config, None, false);
    let orient_value = orient(&mark, encoding, specified_orient.as_ref());
    set_or_remove(
        &mut mark_def,
        "orient",
        orient_value.clone().map(Value::String),
    );

    if let (Some(specified), Some(actual)) = (
        specified_orient.as_ref().and_then(Value::as_str),
        orient_value.as_deref(),
    ) {
        if specified != actual {
            crate::log::warn(format!(
                "Orient is overridden to be \"{actual}\" as the specified orient \"{specified}\" is not valid."
            ));
        }
    }

    let specified_opacity = get_mark_prop_or_config("opacity", &mark_def, config, None, false);
    let specified_fill_opacity =
        get_mark_prop_or_config("fillOpacity", &mark_def, config, None, false);
    if specified_opacity.is_none() && specified_fill_opacity.is_none() {
        set_or_remove(&mut mark_def, "opacity", opacity(&mark, encoding));
    }

    let specified_cursor = get_mark_prop_or_config("cursor", &mark_def, config, None, false);
    if specified_cursor.is_none() {
        let cursor_value = cursor(&mark_def, encoding, config);
        set_or_remove(&mut mark_def, "cursor", cursor_value);
    }

    mark_def
}

fn set_or_remove(mark_def: &mut Value, key: &str, value: Option<Value>) {
    if let Some(value) = value {
        if let Some(map) = mark_def.as_object_mut() {
            map.insert(key.to_string(), value);
        }
    } else if let Some(map) = mark_def.as_object_mut() {
        map.remove(key);
    }
}

/// Port of `cursor`.
fn cursor(mark_def: &Value, encoding: &Value, config: &Value) -> Option<Value> {
    if encoding.get("href").map(|v| !v.is_null()).unwrap_or(false)
        || mark_def.get("href").map(|v| !v.is_null()).unwrap_or(false)
        || get_mark_prop_or_config("href", mark_def, config, None, false).is_some()
    {
        return Some(Value::String("pointer".to_string()));
    }
    mark_def.get("cursor").cloned()
}

/// Port of `opacity`.
fn opacity(mark: &str, encoding: &Value) -> Option<Value> {
    if ["point", "tick", "circle", "square"].contains(&mark)
        && !is_aggregate(encoding)
    {
        return Some(serde_json::json!(DEFAULT_REDUCED_OPACITY));
    }
    None
}

/// Port of `defaultFilled`.
pub fn default_filled(
    mark_def: &Value,
    config: &Value,
    graticule: bool,
) -> bool {
    if graticule {
        return false;
    }
    let filled_config = get_mark_prop_or_config("filled", mark_def, config, None, false);
    let mark = mark_def.get("type").and_then(Value::as_str).unwrap_or("");
    let default = Value::Bool(mark != POINT.as_str() && mark != LINE.as_str() && mark != RULE.as_str());
    get_first_defined(&[filled_config.as_ref(), Some(&default)])
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn binned(field_def: &Value) -> bool {
    is_field_def(field_def) && crate::channeldef_support::is_binned(&field_def["bin"])
}

fn binning(field_def: &Value) -> bool {
    is_field_def(field_def) && crate::channeldef_support::is_binning(&field_def["bin"])
}

fn aggregate(field_def: &Value) -> bool {
    is_field_def(field_def)
        && crate::channeldef_support::truthy(&field_def["aggregate"])
}

/// Port of `orient`.
fn orient(mark: &str, encoding: &Value, specified_orient: Option<&Value>) -> Option<String> {
    let specified = specified_orient.and_then(Value::as_str);
    match mark {
        "point" | "circle" | "square" | "rect" | "image" => return None,
        _ => {}
    }

    let x = encoding.get("x").cloned().unwrap_or(Value::Null);
    let y = encoding.get("y").cloned().unwrap_or(Value::Null);
    let x2 = encoding.get("x2").cloned().unwrap_or(Value::Null);
    let y2 = encoding.get("y2").cloned().unwrap_or(Value::Null);
    let x_offset_is_measure = channel_has_quantitative_offset(encoding, "x");
    let y_offset_is_measure = channel_has_quantitative_offset(encoding, "y");

    let result = (|| -> Option<Option<String>> {
        match mark {
            "text" | "bar" => {
                if y.is_null() && y_offset_is_measure {
                    return Some(Some(specified.unwrap_or("vertical").to_string()));
                }
                if x.is_null() && x_offset_is_measure {
                    return Some(Some(specified.unwrap_or("horizontal").to_string()));
                }
                if binned(&x) || (is_field_def(&y) && aggregate(&y) && !aggregate(&x)) {
                    return Some(Some("vertical".to_string()));
                }
                if binned(&y) || (is_field_def(&x) && aggregate(&x) && !aggregate(&y)) {
                    return Some(Some("horizontal".to_string()));
                }
                if !y2.is_null() || !x2.is_null() {
                    if let Some(specified) = specified {
                        return Some(Some(specified.to_string()));
                    }
                    if x2.is_null() {
                        if ((is_field_def(&x)
                            && x.get("type").and_then(Value::as_str) == Some("quantitative")
                            && !binning(&x))
                            || is_numeric_data_def(&x))
                            && binned(&y) {
                                return Some(Some("horizontal".to_string()));
                            }
                        return Some(Some("vertical".to_string()));
                    }
                    if y2.is_null() {
                        if ((is_field_def(&y)
                            && y.get("type").and_then(Value::as_str) == Some("quantitative")
                            && !binning(&y))
                            || is_numeric_data_def(&y))
                            && binned(&x) {
                                return Some(Some("vertical".to_string()));
                            }
                        return Some(Some("horizontal".to_string()));
                    }
                }
            }
            _ => {}
        }

        match mark {
            "bar" | "rule" | "area" => {
                if !x2.is_null()
                    && !binned(&x)
                    && !y2.is_null()
                    && !binned(&y)
                {
                    return Some(None);
                }
                if y.is_null() && y_offset_is_measure {
                    return Some(Some(specified.unwrap_or("vertical").to_string()));
                }
                if x.is_null() && x_offset_is_measure {
                    return Some(Some(specified.unwrap_or("horizontal").to_string()));
                }
                if !y2.is_null() {
                    if binned(&y) {
                        return Some(Some("horizontal".to_string()));
                    }
                    return Some(Some("vertical".to_string()));
                } else if !x2.is_null() {
                    if binned(&x) {
                        return Some(Some("vertical".to_string()));
                    }
                    return Some(Some("horizontal".to_string()));
                } else if mark == "rule" {
                    if !x.is_null() && y.is_null() {
                        return Some(Some("vertical".to_string()));
                    } else if !y.is_null() && x.is_null() {
                        return Some(Some("horizontal".to_string()));
                    }
                }
            }
            _ => {}
        }

        {
            let x_is_measure = is_unbinned_quantitative_field_or_datum_def(&x);
            let y_is_measure = is_unbinned_quantitative_field_or_datum_def(&y);
            if let Some(specified) = specified {
                return Some(Some(specified.to_string()));
            } else if x_is_measure && !y_is_measure {
                return Some(Some(
                    if mark != TICK.as_str() { "horizontal" } else { "vertical" }.to_string(),
                ));
            } else if !x_is_measure && y_is_measure {
                return Some(Some(
                    if mark != TICK.as_str() { "vertical" } else { "horizontal" }.to_string(),
                ));
            } else if x_is_measure && y_is_measure {
                return Some(Some("vertical".to_string()));
            } else {
                let x_is_temporal = is_typed_field_def(&x)
                    && x.get("type").and_then(Value::as_str) == Some("temporal");
                let y_is_temporal = is_typed_field_def(&y)
                    && y.get("type").and_then(Value::as_str) == Some("temporal");
                if x_is_temporal && !y_is_temporal {
                    return Some(Some("vertical".to_string()));
                } else if !x_is_temporal && y_is_temporal {
                    return Some(Some("horizontal".to_string()));
                }
            }
            Some(None)
        }
    })();

    result.unwrap_or(Some("vertical".to_string()))
}

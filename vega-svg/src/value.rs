//! Loose JavaScript-like value helpers shared by the expression engine, data
//! transforms and encoders.

use serde_json::{Map, Number, Value};

use crate::format::parse_date;

/// Sentinel for JavaScript `undefined` (e.g. a missing object field). It is a
/// string internally so it survives the JSON-shaped value model, with the
/// usual coercions implemented below.
const UNDEFINED: &str = "\u{0}__vega_undefined__";

pub fn undefined() -> Value {
    Value::String(UNDEFINED.to_string())
}

pub fn is_undefined(v: &Value) -> bool {
    matches!(v, Value::String(s) if s == UNDEFINED)
}

pub fn null() -> Value {
    Value::Null
}

pub fn is_nullish(v: &Value) -> bool {
    v.is_null()
}

pub fn is_number(v: &Value) -> bool {
    v.is_number()
}

/// JavaScript `Number(value)` coercion, with Vega's ISO date parsing on top.
pub fn to_number(v: &Value) -> f64 {
    match v {
        Value::Null => 0.0,
        Value::Bool(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::String(s) => {
            if s == UNDEFINED {
                return f64::NAN;
            }
            let t = s.trim();
            if t.is_empty() {
                return 0.0;
            }
            if let Ok(n) = t.parse::<f64>() {
                return n;
            }
            if let Some(ms) = parse_date(t) {
                return ms;
            }
            f64::NAN
        }
        Value::Array(_) | Value::Object(_) => f64::NAN,
    }
}

pub fn to_int(v: &Value) -> i64 {
    let n = to_number(v);
    if n.is_finite() {
        n.trunc() as i64
    } else {
        0
    }
}

/// JavaScript `String(value)` coercion.
pub fn to_string_js(v: &Value) -> String {
    match v {
        Value::String(s) if s == UNDEFINED => "undefined".to_string(),
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => number_to_string(n),
        Value::String(s) => s.clone(),
        Value::Array(a) => a
            .iter()
            .map(|x| match x {
                Value::Null => String::new(),
                _ => to_string_js(x),
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

pub fn number_to_string(n: &Number) -> String {
    if let Some(i) = n.as_i64() {
        return i.to_string();
    }
    if let Some(u) = n.as_u64() {
        return u.to_string();
    }
    let f = n.as_f64().unwrap_or(f64::NAN);
    if f.is_nan() {
        return "NaN".to_string();
    }
    if f.is_infinite() {
        return if f > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    if f == f.trunc() && f.abs() < 1e21 {
        return format!("{}", f as i64);
    }
    let mut s = format!("{}", f);
    if s.contains('e') {
        s = format!("{:?}", f);
    }
    s
}

pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => {
            let f = n.as_f64().unwrap_or(f64::NAN);
            f != 0.0 && !f.is_nan()
        }
        Value::String(s) => !s.is_empty() && s != UNDEFINED,
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// Loose equality used by `==`/`!=` (a pragmatic subset of JavaScript).
pub fn loose_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Null, _) | (_, Value::Null) => is_undefined(a) || is_undefined(b),
        (Value::String(x), Value::String(y)) => {
            if x == UNDEFINED || y == UNDEFINED {
                x == UNDEFINED && y == UNDEFINED
            } else {
                x == y
            }
        }
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(a, b)| loose_eq(a, b))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).map(|w| loose_eq(v, w)).unwrap_or(false))
        }
        _ => {
            let x = to_number(a);
            let y = to_number(b);
            !x.is_nan() && !y.is_nan() && x == y
        }
    }
}

pub fn strict_eq(a: &Value, b: &Value) -> bool {
    a == b
}

/// JavaScript/`vega-util` comparison ordering.
pub fn compare(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x
            .as_f64()
            .unwrap_or(0.0)
            .partial_cmp(&y.as_f64().unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal),
        (Value::String(x), Value::String(y)) => x.cmp(y),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        (Value::Null, Value::Null) => std::cmp::Ordering::Equal,
        (Value::Null, _) => std::cmp::Ordering::Less,
        (_, Value::Null) => std::cmp::Ordering::Greater,
        _ => {
            let x = to_number(a);
            let y = to_number(b);
            x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal)
        }
    }
}

/// Splits a Vega access path (field reference) into its component parts,
/// mirroring upstream `vega-util`'s `splitAccessPath`.
#[allow(unused_assignments)]
pub fn split_access_path(p: &str) -> Vec<String> {
    let bytes = p.as_bytes();
    let n = bytes.len();
    let mut path = Vec::new();
    let mut quote: Option<u8> = None;
    let mut bracket: i64 = 0;
    let mut segment = String::new();
    let mut i = 0usize;
    let mut j = 0usize;

    macro_rules! push_segment {
        () => {{
            segment.push_str(&p[i..j]);
            path.push(std::mem::take(&mut segment));
            i = j + 1;
        }};
    }

    while j < n {
        let c = bytes[j];
        if c == b'\\' {
            segment.push_str(&p[i..j]);
            j += 1;
            if j < n {
                let escaped = p.as_bytes()[j] as char;
                segment.push(escaped);
                j += 1;
            }
            i = j;
        } else if Some(c) == quote {
            push_segment!();
            quote = None;
            bracket = -1;
        } else if quote.is_some() {
            // Skip quoted characters.
        } else if i as i64 == bracket && c == b'"' {
            i = j + 1;
            quote = Some(b'"');
        } else if i as i64 == bracket && c == b'\'' {
            i = j + 1;
            quote = Some(b'\'');
        } else if c == b'.' && bracket == 0 {
            if j > i {
                push_segment!();
            } else {
                i = j + 1;
            }
        } else if c == b'[' {
            if j > i {
                push_segment!();
            }
            bracket = (j + 1) as i64;
            i = j + 1;
        } else if c == b']' {
            if bracket > 0 {
                push_segment!();
            }
            bracket = 0;
            i = j + 1;
        }
        j += 1;
    }
    if j > i {
        // Upstream advances `j` before slicing; `substring` clamps to `n`.
        j += 1;
        let end = j.min(n);
        segment.push_str(&p[i..end.min(n)]);
        path.push(segment);
    }
    path
}

fn lookup_key<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    match v {
        Value::Array(items) => key.parse::<usize>().ok().and_then(|i| items.get(i)),
        Value::Object(map) => map.get(key),
        _ => None,
    }
}

/// Returns the parsed access path for `field`, reusing the previous parse.
///
/// Field references are evaluated once per data row, so re-parsing the same
/// string (and allocating a `Vec<String>`) dominates simple transforms. The
/// cache is thread-local and bounded; specs only contain a handful of distinct
/// field references.
fn access_path(field: &str) -> std::rc::Rc<Vec<String>> {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    thread_local! {
        static CACHE: RefCell<HashMap<String, Rc<Vec<String>>>> = RefCell::new(HashMap::new());
    }

    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(path) = cache.get(field) {
            return path.clone();
        }
        if cache.len() >= 4096 {
            cache.clear();
        }
        let path = Rc::new(split_access_path(field));
        cache.insert(field.to_string(), path.clone());
        path
    })
}

/// Field access that understands dotted and bracket paths as used by Vega
/// field refs (e.g. `a.b`, `a["b c"]`, `a[0]`).
pub fn get_field<'a>(v: &'a Value, field: &str) -> Option<&'a Value> {
    if field.is_empty() {
        return None;
    }
    // Fast path: the vast majority of field refs are a plain object key.
    if !field
        .as_bytes()
        .iter()
        .any(|b| matches!(b, b'.' | b'[' | b'\\'))
    {
        return lookup_key(v, field);
    }
    let path = access_path(field);
    let mut cur = v;
    for part in path.iter() {
        cur = lookup_key(cur, part)?;
    }
    Some(cur)
}

pub fn get_field_owned(v: &Value, field: &str) -> Value {
    get_field(v, field).cloned().unwrap_or(Value::Null)
}

pub fn set_field(obj: &mut Value, field: &str, value: Value) {
    if !obj.is_object() {
        *obj = Value::Object(Map::new());
    }
    if let Value::Object(map) = obj {
        map.insert(field.to_string(), value);
    }
}

/// Evaluates a Vega `field` reference: a string, or one of the special field
/// expression objects (`{"expr": ...}` etc. are handled by the caller).
pub fn field_value(datum: &Value, field: &Value) -> Value {
    match field {
        Value::String(s) => get_field_owned(datum, s),
        Value::Number(n) => datum
            .get(n.as_i64().unwrap_or(0) as usize)
            .cloned()
            .unwrap_or(Value::Null),
        Value::Array(parts) => {
            let mut cur = datum;
            for p in parts {
                let key = match p {
                    Value::String(s) => s.clone(),
                    other => to_string_js(other),
                };
                match cur.get(&key) {
                    Some(v) => cur = v,
                    None => return Value::Null,
                }
            }
            cur.clone()
        }
        _ => Value::Null,
    }
}

pub fn as_f64(v: &Value) -> Option<f64> {
    v.as_f64()
}

pub fn json_number(f: f64) -> Value {
    if f.is_nan() || f.is_infinite() {
        Value::Null
    } else if f == f.trunc() && f.abs() < 9_007_199_254_740_992.0 {
        Value::Number(Number::from(f as i64))
    } else {
        Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

pub fn as_object(v: &Value) -> Option<&Map<String, Value>> {
    v.as_object()
}

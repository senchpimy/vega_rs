//! Port of `src/util.ts`.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicI64, Ordering};

use serde_json::{Map, Value};

/// Split an access path (`foo.bar`, `foo["bar"].baz`, escaped dots, ...) into
/// its components. Port of `vega-util`'s `splitAccessPath`.
pub fn split_access_path(p: &str) -> Vec<String> {
    let chars: Vec<char> = p.chars().collect();
    let n = chars.len();
    let mut path: Vec<String> = Vec::new();
    let mut q: Option<char> = None;
    let mut b: isize = 0;
    let mut s = String::new();
    let mut i: usize = 0;

    fn push(
        path: &mut Vec<String>,
        s: &mut String,
        chars: &[char],
        i: &mut usize,
        j: usize,
    ) {
        let end = j.min(chars.len());
        let start = (*i).min(end);
        let sub: String = chars[start..end].iter().collect();
        path.push(format!("{s}{sub}"));
        s.clear();
        *i = j + 1;
    }

    let mut j: usize = 0;
    while j < n {
        let c = chars[j];
        if c == '\\' {
            let sub: String = chars[i..j].iter().collect();
            s.push_str(&sub);
            j += 1; // skip the escaped character
            i = j;
        } else if Some(c) == q {
            push(&mut path, &mut s, &chars, &mut i, j);
            q = None;
            b = -1;
        } else if q.is_some() {
            // inside quotes: keep scanning
        } else if i as isize == b && (c == '"' || c == '\'') {
            i = j + 1;
            q = Some(c);
        } else if c == '.' && b == 0 {
            if j > i {
                push(&mut path, &mut s, &chars, &mut i, j);
            } else {
                i = j + 1;
            }
        } else if c == '[' {
            if j > i {
                push(&mut path, &mut s, &chars, &mut i, j);
            }
            b = i as isize;
            i = j + 1;
        } else if c == ']' {
            if b == 0 {
                panic!("Access path missing open bracket: {p}");
            }
            if b > 0 {
                push(&mut path, &mut s, &chars, &mut i, j);
            }
            b = 0;
            i = j + 1;
        }
        j += 1;
    }
    if b != 0 {
        panic!("Access path missing closing bracket: {p}");
    }
    if q.is_some() {
        panic!("Access path missing closing quote: {p}");
    }
    if j > i {
        j += 1;
        push(&mut path, &mut s, &chars, &mut i, j);
    }
    path
}

pub fn array(v: &Value) -> Vec<Value> {
    match v {
        Value::Null => Vec::new(),
        Value::Array(a) => a.clone(),
        other => vec![other.clone()],
    }
}

/// Deep clone of a JSON value (mirrors `structuredClone`).
pub fn duplicate<T: Clone>(x: &T) -> T {
    x.clone()
}

pub fn never(message: &str) -> ! {
    panic!("{message}")
}

pub fn pick(obj: &Map<String, Value>, props: &[&str]) -> Map<String, Value> {
    let mut copy = Map::new();
    for prop in props {
        if let Some(v) = obj.get(*prop) {
            copy.insert((*prop).to_string(), v.clone());
        }
    }
    copy
}

pub fn omit(obj: &Map<String, Value>, props: &[&str]) -> Map<String, Value> {
    let mut copy = obj.clone();
    for prop in props {
        copy.remove(*prop);
    }
    copy
}

/// Converts any object to a string of limited size, or a number.
pub fn hash(a: &Value) -> Value {
    if let Value::Number(n) = a {
        return Value::Number(n.clone());
    }

    let str = match a {
        Value::String(s) => s.clone(),
        _ => stringify(a),
    };

    if str.encode_utf16().count() < 250 {
        return Value::String(str);
    }

    // JS implementation of Java's String.hashCode
    let mut h: i32 = 0;
    for unit in str.encode_utf16() {
        h = h.wrapping_shl(5).wrapping_sub(h).wrapping_add(unit as i32);
    }
    Value::Number(serde_json::Number::from(h))
}

pub fn is_null_or_false(x: &Value) -> bool {
    matches!(x, Value::Null) || x == &Value::Bool(false)
}

pub fn contains<T: PartialEq>(array: &[T], item: &T) -> bool {
    array.contains(item)
}

pub fn some<T>(arr: &[T], mut f: impl FnMut(&T, usize) -> bool) -> bool {
    arr.iter().enumerate().any(|(i, a)| f(a, i))
}

pub fn every<T>(arr: &[T], mut f: impl FnMut(&T, usize) -> bool) -> bool {
    arr.iter().enumerate().all(|(i, a)| f(a, i))
}

/// recursively merges `src` into `dest` (JSON objects are merged).
pub fn merge_deep(dest: &mut Value, src: &Value) {
    deep_merge_(dest, src);
}

fn deep_merge_(dest: &mut Value, src: &Value) {
    if let (Value::Object(d), Value::Object(s)) = (&mut *dest, src) {
        for (k, v) in s {
            match (d.get_mut(k), v) {
                (Some(Value::Object(_)), Value::Object(_)) => {
                    deep_merge_(d.get_mut(k).unwrap(), v);
                }
                _ => {
                    d.insert(k.clone(), v.clone());
                }
            }
        }
    } else {
        *dest = src.clone();
    }
}

pub fn unique(values: &[Value], f: impl Fn(&Value) -> Value) -> Vec<Value> {
    let mut results = Vec::new();
    let mut u = std::collections::HashSet::new();
    for val in values {
        let key = stringify(&f(val));
        if u.insert(key) {
            results.push(val.clone());
        }
    }
    results
}

pub type Dict = Map<String, Value>;

/// Returns true if the two dictionaries agree. Applies only to defined values.
pub fn is_equal(dict: &Map<String, Value>, other: &Map<String, Value>) -> bool {
    if dict.len() != other.len() {
        return false;
    }
    for (key, value) in dict {
        if other.get(key) != Some(value) {
            return false;
        }
    }
    true
}

pub fn set_equal<T: Ord>(a: &BTreeSet<T>, b: &BTreeSet<T>) -> bool {
    a == b
}

pub fn has_intersection<T: Ord>(a: &BTreeSet<T>, b: &BTreeSet<T>) -> bool {
    a.intersection(b).next().is_some()
}

pub fn prefix_generator(a: &BTreeSet<String>) -> BTreeSet<String> {
    let mut prefixes = BTreeSet::new();
    for x in a {
        let split_field = split_access_path(x);
        let wrapped: Vec<String> = split_field
            .iter()
            .enumerate()
            .map(|(i, y)| if i == 0 { y.clone() } else { format!("[{y}]") })
            .collect();
        for i in 0..wrapped.len() {
            prefixes.insert(wrapped[..=i].concat());
        }
    }
    prefixes
}

/// Returns true if a and b have an intersection. Also returns true if either is
/// `None` since this means we don't know what fields a node produces/depends on.
pub fn field_intersection(a: Option<&BTreeSet<String>>, b: Option<&BTreeSet<String>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => has_intersection(&prefix_generator(a), &prefix_generator(b)),
        _ => true,
    }
}

pub fn is_empty(obj: &Map<String, Value>) -> bool {
    obj.is_empty()
}

pub fn keys(obj: &Value) -> Vec<String> {
    match obj {
        Value::Object(map) => map.keys().cloned().collect(),
        _ => Vec::new(),
    }
}

pub fn vals(obj: &Value) -> Vec<Value> {
    match obj {
        Value::Object(map) => map.values().cloned().collect(),
        _ => Vec::new(),
    }
}

pub fn entries(obj: &Value) -> Vec<(String, Value)> {
    match obj {
        Value::Object(map) => map.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        _ => Vec::new(),
    }
}

pub fn is_boolean(b: &Value) -> bool {
    matches!(b, Value::Bool(_))
}

/// Returns true if the value is a primitive type.
pub fn is_primitive(v: &Value) -> bool {
    matches!(v, Value::String(_) | Value::Number(_) | Value::Bool(_))
}

/// Convert a string into a valid variable name.
pub fn var_name(s: &str) -> String {
    let all_valid = s
        .as_bytes()
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || *b == b'_');
    if all_valid {
        if s.as_bytes().first().map(|b| b.is_ascii_digit()).unwrap_or(false) {
            return format!("_{s}");
        }
        return s.to_string();
    }
    let alphanumeric: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    let leading_number = s.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false);
    if leading_number {
        format!("_{alphanumeric}")
    } else {
        alphanumeric
    }
}

pub fn is_logical_not(op: &Value) -> bool {
    has_property(op, "not")
}
pub fn is_logical_and(op: &Value) -> bool {
    has_property(op, "and")
}
pub fn is_logical_or(op: &Value) -> bool {
    has_property(op, "or")
}

pub fn logical_expr(op: &Value, cb: &mut impl FnMut(&Value) -> String) -> String {
    if is_logical_not(op) {
        format!("!({})", logical_expr(&op["not"], cb))
    } else if is_logical_and(op) {
        let parts: Vec<String> = op["and"]
            .as_array()
            .map(|a| a.iter().map(|x| logical_expr(x, cb)).collect())
            .unwrap_or_default();
        format!("({})", parts.join(") && ("))
    } else if is_logical_or(op) {
        let parts: Vec<String> = op["or"]
            .as_array()
            .map(|a| a.iter().map(|x| logical_expr(x, cb)).collect())
            .unwrap_or_default();
        format!("({})", parts.join(") || ("))
    } else {
        cb(op)
    }
}

/// Delete a nested property of an object, and delete the ancestors of the
/// property if they become empty.
pub fn delete_nested_property(obj: &mut Value, ordered_props: &[String]) -> bool {
    fn go(obj: &mut Value, props: &[String], i: usize) -> bool {
        if i == props.len() {
            return true;
        }
        let prop = &props[i];
        let mut should_delete = false;
        if let Value::Object(map) = obj {
            if let Some(child) = map.get_mut(prop) {
                if go(child, props, i + 1) {
                    should_delete = true;
                }
            }
            if should_delete {
                map.remove(prop);
            }
            map.is_empty()
        } else {
            false
        }
    }
    go(obj, ordered_props, 0)
}

pub fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Converts a path to an access path with datum.
pub fn access_path_with_datum(path: &str, datum: &str) -> String {
    let pieces = split_access_path(path);
    let mut prefixes = Vec::new();
    for i in 1..=pieces.len() {
        let prefix: Vec<String> = pieces[..i].iter().map(|p| string_value(&Value::String(p.clone()))).collect();
        prefixes.push(format!("{datum}[{}]", prefix.join("][")));
    }
    prefixes.join(" && ")
}

/// Return access with datum to the flattened field.
pub fn flat_access_with_datum(path: &str, datum: &str) -> String {
    let joined = split_access_path(path).join(".");
    format!("{datum}[{}]", string_value(&Value::String(joined)))
}

/// Return access with datum to an unescaped path.
pub fn access_with_datum_to_unescaped_path(unescaped_path: &str) -> String {
    let escaped = unescaped_path.replace('\'', "\\'");
    format!("datum['{escaped}']")
}

pub fn unescape_single_quote_and_path_dot(escaped_path: &str) -> String {
    escaped_path.replace("\\'", "'").replace("\\.", ".")
}

fn escape_path_access(string: &str) -> String {
    let mut out = String::new();
    for c in string.chars() {
        if matches!(c, '[' | ']' | '.' | '\'' | '"') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Replaces path accesses with access to non-nested field.
pub fn replace_path_in_field(path: &str) -> String {
    split_access_path(path)
        .iter()
        .map(|p| escape_path_access(p))
        .collect::<Vec<_>>()
        .join("\\.")
}

/// Replace all occurrences of a string with another string.
pub fn replace_all(string: &str, find: &str, replacement: &str) -> String {
    string.replace(find, replacement)
}

/// Remove path accesses with access from field.
pub fn remove_path_from_field(path: &str) -> String {
    split_access_path(path).join(".")
}

/// Count the depth of the path. Returns 1 for fields that are not nested.
pub fn access_path_depth(path: &str) -> usize {
    if path.is_empty() {
        return 0;
    }
    split_access_path(path).len()
}

/// Replacement for chained `||` that includes `0`.
pub fn get_first_defined<'a>(args: &[Option<&'a Value>]) -> Option<&'a Value> {
    args.iter().find_map(|a| *a)
}

static ID_COUNTER: AtomicI64 = AtomicI64::new(42);

/// Returns a new random id every time it gets called. Has side effect!
pub fn unique_id(prefix: Option<&str>) -> String {
    let id = ID_COUNTER.fetch_add(1, Ordering::SeqCst) + 1;
    match prefix {
        Some(p) => format!("{p}{id}"),
        None => id.to_string(),
    }
}

pub fn reset_id_counter() {
    ID_COUNTER.store(42, Ordering::SeqCst);
}

pub fn internal_field(name: &str) -> String {
    if is_internal_field(name) {
        name.to_string()
    } else {
        format!("__{name}")
    }
}

pub fn is_internal_field(name: &str) -> bool {
    name.starts_with("__")
}

/// Normalize angle to be within [0,360).
pub fn normalize_angle(angle: Option<f64>) -> Option<f64> {
    angle.map(|a| ((a % 360.0) + 360.0) % 360.0)
}

/// Returns whether the passed in value is a valid number.
pub fn is_numeric(value: &Value) -> bool {
    match value {
        Value::Number(_) => true,
        Value::String(s) => {
            let t = s.trim();
            !t.is_empty() && t.parse::<f64>().is_ok()
        }
        _ => false,
    }
}

/// Compares two values for equality, including arrays and objects.
pub fn deep_equal(a: &Value, b: &Value) -> bool {
    a == b
}

/// Converts any object to a stable string representation (sorted keys), mirroring
/// `fast-json-stable-stringify`.
pub fn stringify(data: &Value) -> String {
    fn go(node: &Value, out: &mut String) {
        match node {
            Value::Null => out.push_str("null"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Number(n) => {
                if let Some(f) = n.as_f64() {
                    if !f.is_finite() {
                        out.push_str("null");
                    } else {
                        out.push_str(&crate::util::js_number_to_string(f));
                    }
                } else {
                    out.push_str(&n.to_string());
                }
            }
            Value::String(s) => out.push_str(&serde_json::to_string(s).unwrap()),
            Value::Array(arr) => {
                out.push('[');
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    go(v, out);
                }
                out.push(']');
            }
            Value::Object(map) => {
                let mut ks: Vec<&String> = map.keys().collect();
                ks.sort();
                out.push('{');
                for (i, k) in ks.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&serde_json::to_string(*k).unwrap());
                    out.push(':');
                    go(&map[*k], out);
                }
                out.push('}');
            }
        }
    }
    let mut out = String::new();
    go(data, &mut out);
    out
}

/// Formats a number the way JavaScript's `String(number)` does for common cases.
pub fn js_number_to_string(f: f64) -> String {
    if f == f.trunc() && f.abs() < 1e21 {
        format!("{}", f as i64)
    } else {
        let s = format!("{f}");
        s
    }
}

/// Check if the input object has the property and it's not undefined.
pub fn has_property(obj: &Value, key: &str) -> bool {
    match obj {
        Value::Object(map) => map.get(key).map(|x| !x.is_null()).unwrap_or(false),
        _ => false,
    }
}

/// `JSON.stringify`-like string conversion used by vega-util's `stringValue`.
pub fn string_value(x: &Value) -> String {
    match x {
        Value::String(s) => {
            // JSON.stringify for strings, but vega's stringValue only quotes
            // strings when they are objects/strings? Actually it always
            // JSON-stringifies them.
            serde_json::to_string(s).unwrap()
        }
        _ => stringify(x),
    }
}

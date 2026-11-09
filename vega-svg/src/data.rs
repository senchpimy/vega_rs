//! Data loading and transform pipeline (a pragmatic port of
//! [vega-transforms]).
//!
//! [vega-transforms]: https://github.com/vega/vega/tree/main/packages/vega-transforms

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use crate::error::Error;
use crate::expression::{self, Environment};
use crate::format::parse_date;
use crate::value::*;

#[derive(Default)]
pub struct DataStore {
    pub sets: IndexMap<String, Vec<Value>>,
    pub signals: HashMap<String, Value>,
}

impl DataStore {
    pub fn get(&self, name: &str) -> Option<&Vec<Value>> {
        self.sets.get(name)
    }
    pub fn get_or_empty(&self, name: &str) -> Vec<Value> {
        self.sets.get(name).cloned().unwrap_or_default()
    }
}

struct DataEnv<'a> {
    signals: &'a HashMap<String, Value>,
    datasets: Option<&'a IndexMap<String, Vec<Value>>>,
}

impl<'a> Environment for DataEnv<'a> {
    fn signal(&self, name: &str) -> Option<Value> {
        self.signals.get(name).cloned()
    }
    fn dataset(&self, name: &str) -> Option<Vec<Value>> {
        self.datasets.and_then(|sets| sets.get(name).cloned())
    }
}

pub struct DataOptions<'a> {
    pub base: &'a Path,
    pub extra_dirs: Vec<PathBuf>,
}

impl<'a> Default for DataOptions<'a> {
    fn default() -> Self {
        Self {
            base: Path::new("."),
            extra_dirs: Vec::new(),
        }
    }
}

pub fn load(
    data: Option<&Vec<Value>>,
    opts: &DataOptions,
    initial_signals: &HashMap<String, Value>,
) -> Result<DataStore, Error> {
    let mut store = DataStore::default();
    store.signals.extend(initial_signals.clone());
    let entries = data.cloned().unwrap_or_default();
    for entry in &entries {
        let Some(name) = entry.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        let mut values = if let Some(values) = entry.get("values") {
            match values {
                Value::Array(a) => a.clone(),
                Value::String(s) => {
                    let ty = entry
                        .get("format")
                        .and_then(|f| f.get("type"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("json");
                    parse_text(s, ty, entry.get("format"))?
                }
                other => vec![other.clone()],
            }
        } else if let Some(url) = entry.get("url").and_then(|v| v.as_str()) {
            load_url(url, entry.get("format"), opts)?
        } else if let Some(source) = entry.get("source") {
            match source {
                Value::String(s) => store.get_or_empty(s),
                Value::Array(names) => {
                    let mut all = Vec::new();
                    for n in names {
                        if let Some(s) = n.as_str() {
                            all.extend(store.get_or_empty(s));
                        }
                    }
                    all
                }
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        };

        // `format.property` selects a nested array from JSON data (e.g. an
        // Elasticsearch `hits.hits` response).
        if let Some(prop) = entry
            .get("format")
            .and_then(|f| f.get("property"))
            .and_then(|p| p.as_str())
        {
            let mut selected: Option<Value> = None;
            if values.len() == 1 {
                selected = get_field(&values[0], prop).cloned();
            }
            values = match selected {
                Some(Value::Array(a)) => a,
                Some(Value::Object(_)) => vec![selected.unwrap()],
                Some(Value::Null) | None => Vec::new(),
                Some(other) => vec![other],
            };
        }

        // vega-dataflow's `ingest()` wraps primitive values in a `{data: ...}`
        // tuple (`datum === Object(datum) ? datum : {data: datum}`). This is
        // what makes specs such as arc_radial, whose inline values are plain
        // numbers, address them as `datum.data`.
        for v in values.iter_mut() {
            if !v.is_object() && !v.is_array() {
                let primitive = std::mem::take(v);
                *v = serde_json::json!({ "data": primitive });
            }
        }

        if let Some(parse) = entry
            .get("format")
            .and_then(|f| f.get("parse"))
            .and_then(|p| p.as_object())
        {
            parse_values(&mut values, parse);
        }

        if let Some(transforms) = entry.get("transform").and_then(|t| t.as_array()) {
            let DataStore { sets, signals } = &mut store;
            values = apply_transforms_with_sets(values, transforms, signals, Some(sets))?;
        }
        store.sets.insert(name.to_string(), values);
    }
    Ok(store)
}

/// Parse a date string the way JavaScript's `new Date(value)` does: ISO
/// date-only strings are UTC, but other strings (including RFC-style dates
/// such as `Jan 1 2000`) are interpreted in local time.
fn js_to_date_ms(text: &str) -> Option<f64> {
    let t = text.trim();
    if t.is_empty() {
        return parse_date(t);
    }
    let ms = parse_date(t)?;
    // Explicit timezone designator: already correct.
    let upper = t.to_ascii_uppercase();
    let has_zone = upper.ends_with('Z')
        || upper.ends_with("GMT")
        || upper.ends_with("UTC")
        || {
            let bytes: Vec<char> = t.chars().collect();
            let mut found = false;
            for i in 0..bytes.len().saturating_sub(5) {
                if (bytes[i] == '+' || bytes[i] == '-')
                    && bytes[i + 1].is_ascii_digit()
                    && bytes[i + 2].is_ascii_digit()
                    && bytes[i + 3] == ':'
                    && bytes[i + 4].is_ascii_digit()
                    && bytes[i + 5].is_ascii_digit()
                {
                    found = true;
                }
            }
            found
        };
    if has_zone {
        return Some(ms);
    }
    // Strict ISO date-only (`YYYY-MM-DD`) is parsed as UTC by ECMAScript.
    let is_iso_date_only = t.len() == 10
        && t.as_bytes()[4] == b'-'
        && t.as_bytes()[7] == b'-'
        && t.chars().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                c == '-'
            } else {
                c.is_ascii_digit()
            }
        });
    if is_iso_date_only {
        return Some(ms);
    }
    // Reinterpret the wall-clock components as local time.
    let (y, m, d, h, mi, s, milli) = ms_to_civil(ms);
    Some(local_civil_to_ms(y, m, d, h, mi, s, milli))
}

fn parse_values(values: &mut [Value], parse: &Map<String, Value>) {
    for v in values.iter_mut() {
        for (field, ty) in parse {
            let Some(target) = v.get_mut(field) else {
                continue;
            };
            let ty = match ty {
                Value::String(s) => s.as_str(),
                Value::Object(o) => o.get("type").and_then(|t| t.as_str()).unwrap_or(""),
                _ => "",
            };
            match ty {
                "date" => {
                    let ms = match &*target {
                        Value::String(s) => js_to_date_ms(s),
                        Value::Number(n) => n.as_f64(),
                        _ => None,
                    };
                    if let Some(ms) = ms {
                        *target = json_number(ms);
                    }
                }
                "utc" => {
                    let ms = match &*target {
                        Value::String(s) => parse_date(s),
                        Value::Number(n) => n.as_f64(),
                        _ => None,
                    };
                    if let Some(ms) = ms {
                        *target = json_number(ms);
                    }
                }
                t if t.starts_with("date:") || t.starts_with("utc:") => {
                    let ms = match &*target {
                        Value::String(s) => parse_date(s),
                        Value::Number(n) => n.as_f64(),
                        _ => None,
                    };
                    if let Some(ms) = ms {
                        *target = json_number(ms);
                    }
                }
                "string" => *target = Value::String(to_string_js(target)),
                "number" => {
                    // vega-util's toNumber maps null/"" to null (not 0).
                    if target.is_null() || target.as_str() == Some("") {
                        *target = Value::Null;
                    } else {
                        *target = json_number(to_number(target));
                    }
                }
                "boolean" => {
                    *target = Value::Bool(match &*target {
                        Value::String(s) => s == "true" || s == "1",
                        other => truthy(other),
                    })
                }
                _ => {}
            }
        }
    }
}

fn candidate_paths(url: &str, opts: &DataOptions) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let p = Path::new(url);
    if p.is_absolute() {
        out.push(p.to_path_buf());
    } else {
        out.push(opts.base.join(url));
        for d in &opts.extra_dirs {
            out.push(d.join(url));
        }
        out.push(PathBuf::from(url));
        // vega-datasets convention used by many examples
        if url.starts_with("data/") {
            out.push(PathBuf::from("/tmp/opencode").join(url));
        }
    }
    out
}

fn load_url(url: &str, format: Option<&Value>, opts: &DataOptions) -> Result<Vec<Value>, Error> {
    // Inline data URLs are rare but easy to support.
    if url.starts_with("data:") {
        let text = url.split_once(',').map(|(_, t)| t).unwrap_or("");
        return parse_text(text, "json", format);
    }
    let mut last_err = None;
    for path in candidate_paths(url, opts) {
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let ty = format
                    .and_then(|f| f.get("type"))
                    .and_then(|t| t.as_str())
                    .unwrap_or_else(|| {
                        if url.ends_with(".csv") {
                            "csv"
                        } else if url.ends_with(".tsv") {
                            "tsv"
                        } else if url.ends_with(".json") {
                            "json"
                        } else {
                            "json"
                        }
                    });
                return parse_text(&text, ty, format);
            }
            Err(e) => last_err = Some(e),
        }
    }
    Err(Error::data(format!(
        "could not load data url `{url}`: {}",
        last_err
            .map(|e| e.to_string())
            .unwrap_or_else(|| "not found".to_string())
    )))
}

fn parse_text(text: &str, ty: &str, format: Option<&Value>) -> Result<Vec<Value>, Error> {
    match ty {
        "csv" | "tsv" => {
            let delim = if ty == "tsv" { '\t' } else { ',' };
            let rows = parse_delimited(text, delim);
            if rows.is_empty() {
                return Ok(vec![]);
            }
            let header = &rows[0];
            let mut out = Vec::new();
            for row in &rows[1..] {
                if row.len() == 1 && row[0].is_empty() {
                    continue;
                }
                let mut map = Map::new();
                for (i, key) in header.iter().enumerate() {
                    let val = row.get(i).cloned().unwrap_or_default();
                    map.insert(key.clone(), Value::String(val));
                }
                out.push(Value::Object(map));
            }
            Ok(out)
        }
        "json" => {
            let v: Value = serde_json::from_str(text)
                .map_err(|e| Error::data(format!("invalid JSON data: {e}")))?;
            let values = match v {
                Value::Array(a) => a,
                Value::Object(mut m) => {
                    if let Some(Value::Array(a)) = m.remove("values") {
                        a
                    } else {
                        vec![Value::Object(m)]
                    }
                }
                other => vec![other],
            };
            let _ = format;
            Ok(values)
        }
        "topojson" => {
            // Decode TopoJSON arcs and geometry objects into GeoJSON features.
            let v: Value = serde_json::from_str(text)
                .map_err(|e| Error::data(format!("invalid TopoJSON data: {e}")))?;
            let features = topojson_features(&v, format);
            Ok(features)
        }
        _ => Err(Error::data(format!("unsupported data format `{ty}`"))),
    }
}

fn topojson_features(v: &Value, format: Option<&Value>) -> Vec<Value> {
    let topology = Topology::new(v);
    let mut out = Vec::new();
    let Some(objects) = v.get("objects").and_then(|o| o.as_object()) else {
        return out;
    };
    let selected = format
        .and_then(|f| f.get("feature"))
        .and_then(|f| f.as_str());
    if let Some(name) = selected {
        if let Some(obj) = objects.get(name) {
            topology.collect(obj, &mut out);
        }
    } else {
        for (_, obj) in objects {
            topology.collect(obj, &mut out);
        }
    }
    out
}

struct Topology<'a> {
    arcs: &'a [Value],
    scale: [f64; 2],
    translate: [f64; 2],
}

impl<'a> Topology<'a> {
    fn new(v: &'a Value) -> Self {
        let mut scale = [1.0, 1.0];
        let mut translate = [0.0, 0.0];
        if let Some(t) = v.get("transform") {
            if let Some(s) = t.get("scale").and_then(|s| s.as_array()) {
                scale = [
                    s.first().and_then(|v| v.as_f64()).unwrap_or(1.0),
                    s.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0),
                ];
            }
            if let Some(t) = t.get("translate").and_then(|s| s.as_array()) {
                translate = [
                    t.first().and_then(|v| v.as_f64()).unwrap_or(0.0),
                    t.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0),
                ];
            }
        }
        Self {
            arcs: v
                .get("arcs")
                .and_then(|a| a.as_array())
                .map(|a| a.as_slice())
                .unwrap_or(&[]),
            scale,
            translate,
        }
    }

    fn arc(&self, index: i64) -> Vec<Value> {
        let i = if index < 0 {
            (-index - 1) as usize
        } else {
            index as usize
        };
        let Some(arc) = self.arcs.get(i).and_then(|a| a.as_array()) else {
            return Vec::new();
        };
        let mut x = 0.0;
        let mut y = 0.0;
        let mut out: Vec<Value> = Vec::with_capacity(arc.len());
        for point in arc {
            let Some(p) = point.as_array() else { continue };
            x += p.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
            y += p.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
            out.push(json!([
                x * self.scale[0] + self.translate[0],
                y * self.scale[1] + self.translate[1]
            ]));
        }
        if index < 0 {
            out.reverse();
        }
        out
    }

    fn ring(&self, indexes: &Value) -> Vec<Value> {
        let Some(ids) = indexes.as_array() else {
            return Vec::new();
        };
        let mut out: Vec<Value> = Vec::new();
        for id in ids {
            let Some(id) = id.as_i64() else { continue };
            let arc = self.arc(id);
            if !out.is_empty() {
                out.pop();
            }
            out.extend(arc);
        }
        if out.len() < 4 {
            if let Some(first) = out.first().cloned() {
                while out.len() < 4 {
                    out.push(first.clone());
                }
            }
        }
        out
    }

    fn lines(&self, arcs: &Value) -> Vec<Value> {
        arcs.as_array()
            .map(|a| a.iter().map(|r| Value::Array(self.ring(r))).collect())
            .unwrap_or_default()
    }

    fn collect(&self, object: &Value, out: &mut Vec<Value>) {
        let Some(kind) = object.get("type").and_then(|t| t.as_str()) else {
            // TopoJSON uses `null` geometry types for deleted/empty areas;
            // topojson-client still emits a Feature with a null geometry.
            out.push(json!({
                "type": "Feature",
                "geometry": Value::Null,
                "properties": object.get("properties").cloned().unwrap_or(json!({})),
                "id": object.get("id").cloned().unwrap_or(Value::Null)
            }));
            return;
        };
        match kind {
            "GeometryCollection" => {
                if let Some(geoms) = object.get("geometries").and_then(|g| g.as_array()) {
                    for g in geoms {
                        self.collect(g, out);
                    }
                }
            }
            "Point" => {
                if let Some(c) = object.get("coordinates").and_then(|c| c.as_array()) {
                    let x = c.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let y = c.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    out.push(json!({
                        "type": "Feature",
                        "geometry": {"type": "Point", "coordinates": [
                            x * self.scale[0] + self.translate[0],
                            y * self.scale[1] + self.translate[1]
                        ]},
                        "properties": object.get("properties").cloned().unwrap_or(json!({})),
                        "id": object.get("id").cloned().unwrap_or(Value::Null)
                    }));
                }
            }
            "MultiPoint" => {
                let mut coords = Vec::new();
                if let Some(cs) = object.get("coordinates").and_then(|c| c.as_array()) {
                    for c in cs {
                        if let Some(c) = c.as_array() {
                            let x = c.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
                            let y = c.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
                            coords.push(json!([
                                x * self.scale[0] + self.translate[0],
                                y * self.scale[1] + self.translate[1]
                            ]));
                        }
                    }
                }
                out.push(feature("MultiPoint", Value::Array(coords), object));
            }
            "LineString" => {
                let coords = object
                    .get("arcs")
                    .map(|a| Value::Array(self.ring(a)))
                    .unwrap_or(json!([]));
                out.push(feature("LineString", coords, object));
            }
            "MultiLineString" => {
                let coords = object
                    .get("arcs")
                    .map(|a| Value::Array(self.lines(a)))
                    .unwrap_or(json!([]));
                out.push(feature("MultiLineString", coords, object));
            }
            "Polygon" => {
                let rings = object
                    .get("arcs")
                    .and_then(|a| a.as_array())
                    .map(|a| a.iter().map(|r| Value::Array(self.ring(r))).collect())
                    .unwrap_or_default();
                out.push(feature("Polygon", Value::Array(rings), object));
            }
            "MultiPolygon" => {
                let polys = object
                    .get("arcs")
                    .and_then(|a| a.as_array())
                    .map(|a| {
                        a.iter()
                            .map(|poly| {
                                let rings = poly
                                    .as_array()
                                    .map(|r| r.iter().map(|ring| Value::Array(self.ring(ring))).collect())
                                    .unwrap_or_default();
                                Value::Array(rings)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                out.push(feature("MultiPolygon", Value::Array(polys), object));
            }
            _ => {}
        }
    }
}

fn feature(kind: &str, coordinates: Value, object: &Value) -> Value {
    json!({
        "type": "Feature",
        "geometry": {"type": kind, "coordinates": coordinates},
        "properties": object.get("properties").cloned().unwrap_or(json!({})),
        "id": object.get("id").cloned().unwrap_or(Value::Null)
    })
}

/// RFC 4180-ish CSV parser (quoted fields, doubled quotes, embedded newlines).
fn parse_delimited(text: &str, delim: char) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(c);
            }
        } else if c == '"' {
            in_quotes = true;
        } else if c == delim {
            row.push(std::mem::take(&mut field));
        } else if c == '\n' {
            row.push(std::mem::take(&mut field));
            rows.push(std::mem::take(&mut row));
        } else if c == '\r' {
            // handled by \n
        } else {
            field.push(c);
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

pub fn apply_transforms(
    values: Vec<Value>,
    transforms: &[Value],
    signals: &mut HashMap<String, Value>,
) -> Result<Vec<Value>, Error> {
    apply_transforms_with_sets(values, transforms, signals, None)
}

fn apply_transforms_with_sets(
    mut values: Vec<Value>,
    transforms: &[Value],
    signals: &mut HashMap<String, Value>,
    datasets: Option<&IndexMap<String, Vec<Value>>>,
) -> Result<Vec<Value>, Error> {
    for t in transforms {
        let ty = t.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let env = DataEnv { signals, datasets };
        values = match ty {
            "filter" => {
                let expr = t.get("expr").and_then(|v| v.as_str()).unwrap_or("true");
                let mut out = Vec::new();
                for d in values {
                    if truthy(&expression::evaluate(expr, Some(&d), &env)?) {
                        out.push(d);
                    }
                }
                out
            }
            "formula" | "calculate" => {
                let expr = t.get("expr").and_then(|v| v.as_str()).unwrap_or("null");
                let as_ = t.get("as").and_then(|v| v.as_str()).unwrap_or("data");
                for d in values.iter_mut() {
                    let v = expression::evaluate(expr, Some(d), &env)?;
                    set_field(d, as_, v);
                }
                values
            }
            "collect" => {
                sort_values(&mut values, t.get("sort"));
                values
            }
            "aggregate" => aggregate(values, t)?,
            "joinaggregate" => joinaggregate(values, t)?,
            "stack" => stack(values, t)?,
            "window" => window(values, t)?,
            "extent" => {
                let field = t.get("field").and_then(|v| v.as_str()).unwrap_or("");
                let mut min = f64::INFINITY;
                let mut max = f64::NEG_INFINITY;
                for d in &values {
                    if let Some(n) = field_number(d, field) {
                        min = min.min(n);
                        max = max.max(n);
                    }
                }
                let extent = if min.is_finite() {
                    json!([min, max])
                } else {
                    json!([null, null])
                };
                if let Some(name) = t.get("signal").and_then(|v| v.as_str()) {
                    signals.insert(name.to_string(), extent);
                } else if let Some(name) = t.get("as").and_then(|v| v.as_str()) {
                    for d in values.iter_mut() {
                        set_field(d, name, extent.clone());
                    }
                }
                values
            }
            "bin" => {
                let (out, bins_signal) = bin(values, t, signals)?;
                if let Some(name) = t.get("signal").and_then(|v| v.as_str()) {
                    signals.insert(name.to_string(), bins_signal);
                }
                out
            }
            "kde" => kde(values, t)?,
            "density" => density(values, t, datasets)?,
            "timeunit" => timeunit(values, t, signals)?,
            "impute" => impute(values, t, signals, datasets)?,
            "fold" => fold(values, t)?,
            "flatten" => flatten(values, t)?,
            "project" => project(values, t)?,
            "lookup" => lookup(values, t, datasets)?,
            "sequence" => {
                let (vals, sig) = sequence(t, &env)?;
                if let Some((name, value)) = sig {
                    signals.insert(name, value);
                }
                vals
            }
            "identifier" => {
                let as_ = t.get("as").and_then(|v| v.as_str()).unwrap_or("_id");
                for (i, d) in values.iter_mut().enumerate() {
                    set_field(d, as_, json_number(i as f64 + 1.0));
                }
                values
            }
            "sample" => {
                let size = t.get("size").and_then(|v| v.as_f64()).unwrap_or(1000.0) as usize;
                values.truncate(size);
                values
            }
            "pivot" => pivot(values, t)?,
            "quantile" => {
                let field = t.get("field").and_then(|v| v.as_str()).unwrap_or("");
                let step = t.get("step").and_then(|v| v.as_f64()).unwrap_or(0.01);
                let probs: Vec<f64> = if let Some(probs) = t.get("probs").and_then(|p| p.as_array())
                {
                    probs.iter().filter_map(|p| p.as_f64()).collect()
                } else {
                    let mut out = Vec::new();
                    let mut p = step / 2.0;
                    while p < 1.0 - 1e-14 {
                        out.push(p);
                        p += step;
                    }
                    out
                };
                let as_ = t.get("as").and_then(|a| a.as_array());
                let prob_name = as_
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_str())
                    .unwrap_or("prob");
                let value_name = as_
                    .and_then(|a| a.get(1))
                    .and_then(|v| v.as_str())
                    .unwrap_or("value");
                let groupby: Vec<String> = t
                    .get("groupby")
                    .and_then(|v| v.as_array())
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                let mut groups: IndexMap<String, (Vec<Value>, Vec<f64>)> = IndexMap::new();
                for row in &values {
                    let Some(raw) = get_field(row, field) else {
                        continue;
                    };
                    let n = to_number(raw);
                    if !n.is_finite() {
                        continue;
                    }
                    let dims: Vec<Value> =
                        groupby.iter().map(|g| get_field_owned(row, g)).collect();
                    let key = serde_json::to_string(&dims).unwrap_or_default();
                    groups.entry(key).or_insert_with(|| (dims, Vec::new())).1.push(n);
                }
                let mut out = Vec::new();
                for (_, (dims, mut nums)) in groups {
                    nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                    for p in &probs {
                        let mut row = Map::new();
                        for (i, name) in groupby.iter().enumerate() {
                            row.insert(name.clone(), dims.get(i).cloned().unwrap_or(Value::Null));
                        }
                        row.insert(prob_name.to_string(), json_number(*p));
                        row.insert(value_name.to_string(), json_number(quantile(&nums, *p)));
                        out.push(Value::Object(row));
                    }
                }
                out
            }
            "geojson" => {
                let feature_collection = geojson_features(&values, t, &env);
                if let Some(name) = t.get("signal").and_then(|v| v.as_str()) {
                    signals.insert(name.to_string(), feature_collection);
                }
                values
            }
            "graticule" => vec![graticule_geometry(t)],
            // Transforms that need projections/statistics are accepted as no-ops
            // so that the rest of a specification still renders.
            "loess" => loess(values, t),
            "regression" => regression(values, t),
            "geopoint" | "force" | "crossfilter" | "voronoi"
            | "treemap" | "node" | "link" | "label" | "countpattern" | "parse" => values,
            _ => values,
        };
    }
    Ok(values)
}

fn sort_values(values: &mut [Value], sort: Option<&Value>) {
    let Some(sort) = sort else { return };
    let (fields, orders): (Vec<String>, Vec<bool>) = match sort {
        Value::String(f) => (vec![f.clone()], vec![false]),
        Value::Array(a) => {
            let fs = a
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            (fs, vec![])
        }
        Value::Object(o) => {
            let fields = o
                .get("field")
                .map(|f| match f {
                    Value::String(s) => vec![s.clone()],
                    Value::Array(a) => a
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect(),
                    _ => vec![],
                })
                .unwrap_or_default();
            let orders = o
                .get("order")
                .map(|f| match f {
                    Value::String(s) => vec![s == "descending"],
                    Value::Array(a) => a.iter().map(|v| v.as_str() == Some("descending")).collect(),
                    _ => vec![],
                })
                .unwrap_or_default();
            (fields, orders)
        }
        _ => return,
    };
    values.sort_by(|a, b| {
        for (i, f) in fields.iter().enumerate() {
            let av = get_field_owned(a, f);
            let bv = get_field_owned(b, f);
            let mut ord = compare(&av, &bv);
            if orders.get(i).copied().unwrap_or(false) {
                ord = ord.reverse();
            }
            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
        }
        std::cmp::Ordering::Equal
    });
}

fn aggregate(values: Vec<Value>, t: &Value) -> Result<Vec<Value>, Error> {
    let groupby: Vec<String> = t
        .get("groupby")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let ops: Vec<String> = t
        .get("ops")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let fields: Vec<String> = t
        .get("fields")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default();
    let as_: Vec<String> = t
        .get("as")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default();
    let params: Vec<Option<f64>> = t
        .get("aggregate_params")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().map(|v| v.as_f64()).collect())
        .unwrap_or_default();

    let mut groups: IndexMap<String, Vec<Value>> = IndexMap::new();
    for d in values {
        let key = groupby
            .iter()
            .map(|g| {
                format!(
                    "{}",
                    serde_json::to_string(&get_field_owned(&d, g)).unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join("\u{1f}");
        groups.entry(key).or_default().push(d);
    }
    if groups.is_empty() && groupby.is_empty() {
        groups.insert(String::new(), Vec::new());
    }

    let mut out = Vec::new();
    for (_, rows) in groups {
        let mut obj = Map::new();
        if let Some(first) = rows.first() {
            for g in &groupby {
                let name = crate::value::split_access_path(g).join(".");
                obj.insert(name, get_field_owned(first, g));
            }
        }
        for (i, op) in ops.iter().enumerate() {
            let field = fields.get(i).cloned().unwrap_or_default();
            let name = as_
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("{op}_{field}"));
            let value = aggregate_op_param(op, &field, &rows, params.get(i).copied().flatten());
            obj.insert(name, value);
        }
        out.push(Value::Object(obj));
    }
    Ok(out)
}

/// Numeric field accessor matching Vega's aggregate semantics: missing and
/// `null` values are invalid (not coerced to zero like JavaScript's `+null`).
fn field_number(d: &Value, field: &str) -> Option<f64> {
    match get_field(d, field) {
        Some(v) if !v.is_null() => {
            let n = to_number(v);
            if n.is_finite() {
                Some(n)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn numeric_values(rows: &[Value], field: &str) -> Vec<f64> {
    rows.iter()
        .filter_map(|d| field_number(d, field))
        .collect()
}

pub fn aggregate_op(op: &str, field: &str, rows: &[Value]) -> Value {
    aggregate_op_param(op, field, rows, None)
}

fn aggregate_op_param(op: &str, field: &str, rows: &[Value], param: Option<f64>) -> Value {
    match op {
        "ci0" | "ci1" => {
            let v = numeric_values(rows, field);
            if v.len() < 2 {
                Value::Null
            } else {
                let n = v.len() as f64;
                let mean = v.iter().sum::<f64>() / n;
                let ss: f64 = v.iter().map(|x| (x - mean).powi(2)).sum();
                let stderr = (ss / (n - 1.0)).sqrt() / n.sqrt();
                let z = 1.959_963_984_540_054;
                let q = if op == "ci0" { mean - z * stderr } else { mean + z * stderr };
                json_number(q)
            }
        }
        "exponential" | "exponentialb" => {
            let r = param.unwrap_or(0.5);
            let v = numeric_values(rows, field);
            if v.is_empty() || r == 1.0 {
                return Value::Null;
            }
            let mut exp = 0.0;
            for x in &v {
                exp = r * exp + x;
            }
            let n = v.len() as f64;
            let mut out = exp * (1.0 - r) / (1.0 - r.powf(n));
            if op == "exponentialb" {
                out = exp * (1.0 - r);
            }
            json_number(out)
        }
        _ => aggregate_op_inner(op, field, rows),
    }
}

fn aggregate_op_inner(op: &str, field: &str, rows: &[Value]) -> Value {
    match op {
        "count" => json_number(rows.len() as f64),
        "valid" => json_number(
            rows.iter()
                .filter(|d| get_field(d, field).map(|v| !v.is_null()).unwrap_or(false))
                .count() as f64,
        ),
        "missing" => json_number(
            rows.iter()
                .filter(|d| get_field(d, field).map(|v| v.is_null()).unwrap_or(true))
                .count() as f64,
        ),
        "distinct" => {
            let mut seen: Vec<String> = Vec::new();
            for d in rows {
                if let Some(v) = get_field(d, field) {
                    let k = serde_json::to_string(v).unwrap_or_default();
                    if !seen.contains(&k) {
                        seen.push(k);
                    }
                }
            }
            json_number(seen.len() as f64)
        }
        "sum" => json_number(numeric_values(rows, field).iter().sum()),
        "product" => json_number(numeric_values(rows, field).iter().product()),
        "mean" | "average" => {
            let v = numeric_values(rows, field);
            if v.is_empty() {
                Value::Null
            } else {
                json_number(v.iter().sum::<f64>() / v.len() as f64)
            }
        }
        "variance" | "variancep" => {
            let v = numeric_values(rows, field);
            if v.is_empty() {
                return Value::Null;
            }
            let mean = v.iter().sum::<f64>() / v.len() as f64;
            let ss: f64 = v.iter().map(|x| (x - mean).powi(2)).sum();
            let den = if op == "variancep" {
                v.len()
            } else {
                v.len().saturating_sub(1)
            };
            if den == 0 {
                Value::Null
            } else {
                json_number(ss / den as f64)
            }
        }
        "stdev" | "stdevp" => {
            let v = numeric_values(rows, field);
            if v.is_empty() {
                return Value::Null;
            }
            let mean = v.iter().sum::<f64>() / v.len() as f64;
            let ss: f64 = v.iter().map(|x| (x - mean).powi(2)).sum();
            let den = if op == "stdevp" {
                v.len()
            } else {
                v.len().saturating_sub(1)
            };
            if den == 0 {
                Value::Null
            } else {
                json_number((ss / den as f64).sqrt())
            }
        }
        "stderr" => {
            let v = numeric_values(rows, field);
            if v.len() < 2 {
                Value::Null
            } else {
                let mean = v.iter().sum::<f64>() / v.len() as f64;
                let ss: f64 = v.iter().map(|x| (x - mean).powi(2)).sum();
                json_number((ss / (v.len() - 1) as f64).sqrt() / (v.len() as f64).sqrt())
            }
        }
        "min" => {
            let mut v = numeric_values(rows, field);
            v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            v.first().map(|n| json_number(*n)).unwrap_or(Value::Null)
        }
        "max" => {
            let mut v = numeric_values(rows, field);
            v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            v.last().map(|n| json_number(*n)).unwrap_or(Value::Null)
        }
        "median" | "q1" | "q3" => {
            let mut v = numeric_values(rows, field);
            v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let p = match op {
                "q1" => 0.25,
                "q3" => 0.75,
                _ => 0.5,
            };
            if v.is_empty() {
                Value::Null
            } else {
                json_number(quantile(&v, p))
            }
        }
        "argmin" | "argmax" => {
            let mut best: Option<(f64, Value)> = None;
            for d in rows {
                if let Some(v) = get_field(d, field) {
                    let n = to_number(v);
                    if !n.is_finite() {
                        continue;
                    }
                    let better = match &best {
                        None => true,
                        Some((b, _)) => {
                            if op == "argmin" {
                                n < *b
                            } else {
                                n > *b
                            }
                        }
                    };
                    if better {
                        best = Some((n, d.clone()));
                    }
                }
            }
            best.map(|(_, d)| d).unwrap_or(Value::Null)
        }
        "values" => Value::Array(
            rows.iter()
                .filter_map(|d| get_field(d, field).cloned())
                .collect(),
        ),
        _ => Value::Null,
    }
}

pub fn quantile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let p = p.clamp(0.0, 1.0);
    let idx = p * (sorted.len() - 1) as f64;
    let lo = idx.floor() as usize;
    let hi = idx.ceil() as usize;
    let t = idx - lo as f64;
    sorted[lo] * (1.0 - t) + sorted[hi.min(sorted.len() - 1)] * t
}

fn stack(values: Vec<Value>, t: &Value) -> Result<Vec<Value>, Error> {
    let field = t.get("field").and_then(|v| v.as_str()).unwrap_or("");
    let groupby: Vec<String> = t
        .get("groupby")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let as_ = t
        .get("as")
        .and_then(|v| v.as_array())
        .map(|a| {
            (
                a.first()
                    .and_then(|v| v.as_str())
                    .unwrap_or("y0")
                    .to_string(),
                a.get(1)
                    .and_then(|v| v.as_str())
                    .unwrap_or("y1")
                    .to_string(),
            )
        })
        .unwrap_or_else(|| ("y0".to_string(), "y1".to_string()));
    let offset = t.get("offset").and_then(|v| v.as_str()).unwrap_or("zero");

    let mut groups: IndexMap<String, Vec<usize>> = IndexMap::new();
    for (i, d) in values.iter().enumerate() {
        let key = groupby
            .iter()
            .map(|g| serde_json::to_string(&get_field_owned(d, g)).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\u{1f}");
        groups.entry(key).or_default().push(i);
    }

    let mut result = values;
    // Vega's stack partition computes each group's sum of absolute values and
    // remembers the global maximum, which `center` uses as its baseline.
    let group_sums: Vec<f64> = groups
        .values()
        .map(|idxs| {
            idxs.iter()
                .map(|&i| {
                    let n = get_field(&result[i], field).map(to_number).unwrap_or(0.0);
                    if n.is_finite() {
                        n.abs()
                    } else {
                        0.0
                    }
                })
                .sum::<f64>()
        })
        .collect();
    let global_max = group_sums.iter().copied().fold(0.0f64, f64::max);
    for ((_, mut idxs), abs_total) in groups.into_iter().zip(group_sums) {
        if let Some(sort) = t.get("sort") {
            let fields: Vec<String> = match sort.get("field") {
                Some(Value::String(s)) => vec![s.clone()],
                Some(Value::Array(a)) => a
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect(),
                _ => vec![],
            };
            let orders: Vec<bool> = match sort.get("order") {
                Some(Value::String(s)) => vec![s == "descending"],
                Some(Value::Array(a)) => {
                    a.iter().map(|v| v.as_str() == Some("descending")).collect()
                }
                _ => vec![],
            };
            idxs.sort_by(|&a, &b| {
                for (i, f) in fields.iter().enumerate() {
                    let mut ord = compare(
                        &get_field_owned(&result[a], f),
                        &get_field_owned(&result[b], f),
                    );
                    if orders.get(i).copied().unwrap_or(false) {
                        ord = ord.reverse();
                    }
                    if ord != std::cmp::Ordering::Equal {
                        return ord;
                    }
                }
                a.cmp(&b)
            });
        }
        let mut _total: f64 = 0.0;
        let mut _min: f64 = 0.0;
        let mut _max: f64 = 0.0;
        let mut _running: f64 = 0.0;
        for &i in &idxs {
            let n = get_field(&result[i], field).map(to_number).unwrap_or(0.0);
            if n.is_finite() {
                _total += n;
                _min = _min.min(n);
                _max = _max.max(n);
            }
        }
        let mut neg = 0.0;
        let mut pos = 0.0;
        let mut center_last = (global_max - abs_total) / 2.0;
        let mut norm_last = 0.0;
        let mut norm_v = 0.0;
        let norm_scale = if abs_total != 0.0 { 1.0 / abs_total } else { 0.0 };
        for &i in &idxs {
            let n = get_field(&result[i], field).map(to_number).unwrap_or(0.0);
            let n = if n.is_finite() { n } else { 0.0 };
            let (start, end) = match offset {
                "normalize" => {
                    let s = norm_last;
                    norm_v += n.abs();
                    norm_last = norm_scale * norm_v;
                    (s, norm_last)
                }
                "center" => {
                    let s = center_last;
                    center_last += n.abs();
                    (s, center_last)
                }
                _ => {
                    if n >= 0.0 {
                        let s = pos;
                        pos += n;
                        (s, pos)
                    } else {
                        let s = neg;
                        neg += n;
                        (s, neg)
                    }
                }
            };
            let _ = (_min, _max, _running);
            _running += n;
            if let Value::Object(map) = &mut result[i] {
                map.insert(as_.0.clone(), json_number(start));
                map.insert(as_.1.clone(), json_number(end));
            }
        }
    }
    Ok(result)
}

fn joinaggregate(values: Vec<Value>, t: &Value) -> Result<Vec<Value>, Error> {
    let groupby: Vec<String> = t
        .get("groupby")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let ops: Vec<String> = t
        .get("ops")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let fields: Vec<String> = t
        .get("fields")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default();
    let as_: Vec<String> = t
        .get("as")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default();

    let mut groups: IndexMap<String, Vec<usize>> = IndexMap::new();
    for (i, d) in values.iter().enumerate() {
        let key = groupby
            .iter()
            .map(|g| serde_json::to_string(&get_field_owned(d, g)).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\u{1f}");
        groups.entry(key).or_default().push(i);
    }
    let mut result = values;
    for (_, idxs) in groups {
        let rows: Vec<Value> = idxs.iter().map(|&i| result[i].clone()).collect();
        for (j, op) in ops.iter().enumerate() {
            let field = fields.get(j).cloned().unwrap_or_default();
            let name = as_
                .get(j)
                .cloned()
                .unwrap_or_else(|| format!("{op}_{field}"));
            let v = aggregate_op(op, &field, &rows);
            for &i in &idxs {
                if let Value::Object(map) = &mut result[i] {
                    map.insert(name.clone(), v.clone());
                }
            }
        }
    }
    Ok(result)
}

fn window(values: Vec<Value>, t: &Value) -> Result<Vec<Value>, Error> {
    let groupby: Vec<String> = t
        .get("groupby")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let ops: Vec<String> = t
        .get("ops")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let fields: Vec<String> = t
        .get("fields")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default();
    let as_: Vec<String> = t
        .get("as")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default();
    let params: Vec<Option<f64>> = t
        .get("params")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().map(|v| v.as_f64()).collect())
        .unwrap_or_default();
    let frame: Option<(Option<i64>, Option<i64>)> = if t.get("frame").is_none() {
        Some((None, Some(0)))
    } else {
        t.get("frame").and_then(|f| f.as_array()).and_then(|a| {
            if a.len() < 2 {
                None
            } else {
                let bound = |v: &Value| -> Option<i64> {
                    if v.is_null() {
                        None
                    } else {
                        v.as_f64().map(|n| n as i64)
                    }
                };
                Some((bound(&a[0]), bound(&a[1])))
            }
        })
    };

    let mut groups: IndexMap<String, Vec<usize>> = IndexMap::new();
    for (i, d) in values.iter().enumerate() {
        let key = groupby
            .iter()
            .map(|g| serde_json::to_string(&get_field_owned(d, g)).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\u{1f}");
        groups.entry(key).or_default().push(i);
    }
    let mut result = values;
    for (_, mut idxs) in groups {
        if let Some(sort) = t.get("sort") {
            let fields: Vec<String> = match sort.get("field") {
                Some(Value::String(s)) => vec![s.clone()],
                Some(Value::Array(a)) => a
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect(),
                _ => vec![],
            };
            let orders: Vec<bool> = match sort.get("order") {
                Some(Value::String(s)) => vec![s == "descending"],
                Some(Value::Array(a)) => {
                    a.iter().map(|v| v.as_str() == Some("descending")).collect()
                }
                _ => vec![],
            };
            idxs.sort_by(|&a, &b| {
                for (i, f) in fields.iter().enumerate() {
                    let mut ord = compare(
                        &get_field_owned(&result[a], f),
                        &get_field_owned(&result[b], f),
                    );
                    if orders.get(i).copied().unwrap_or(false) {
                        ord = ord.reverse();
                    }
                    if ord != std::cmp::Ordering::Equal {
                        return ord;
                    }
                }
                a.cmp(&b)
            });
        }
        let sort_fields: Vec<String> = t
            .get("sort")
            .and_then(|sort| match sort.get("field") {
                Some(Value::String(s)) => Some(vec![s.clone()]),
                Some(Value::Array(a)) => Some(
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect(),
                ),
                _ => None,
            })
            .unwrap_or_default();
        let sort_orders: Vec<bool> = t
            .get("sort")
            .and_then(|sort| match sort.get("order") {
                Some(Value::String(s)) => Some(vec![s == "descending"]),
                Some(Value::Array(a)) => {
                    Some(a.iter().map(|v| v.as_str() == Some("descending")).collect())
                }
                _ => None,
            })
            .unwrap_or_default();
        let rows: Vec<Value> = idxs.iter().map(|&i| result[i].clone()).collect();
        for (j, op) in ops.iter().enumerate() {
            let field = fields.get(j).cloned().unwrap_or_default();
            let name = as_
                .get(j)
                .cloned()
                .unwrap_or_else(|| format!("{op}_{field}"));
            for (pos, &i) in idxs.iter().enumerate() {
                let scoped;
                let scope: &[Value] = if let (Some((f0, f1)), true) = (frame, !matches!(op.as_str(), "row_number" | "rank" | "dense_rank" | "percent_rank" | "cume_dist" | "ntile" | "lag" | "lead")) {
                    let resolve = |v: Option<i64>, default: i64| -> usize {
                        let n = rows.len() as i64;
                        let abs = match v {
                            None => default,
                            Some(k) => pos as i64 + k,
                        };
                        abs.clamp(0, n) as usize
                    };
                    let mut start = resolve(f0, 0).min(pos + 1);
                    let mut end =
                        resolve(f1, rows.len() as i64 - 1).min(rows.len().saturating_sub(1));
                    // `ignorePeers: false` (the default) extends the frame to
                    // whole peer groups that share the sort key.
                    if !t
                        .get("ignorePeers")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                        && !sort_fields.is_empty()
                        && end < rows.len()
                    {
                        let peer = |a: usize, b: usize| {
                            compare_sort_rows(&rows[a], &rows[b], &sort_fields, &sort_orders)
                                == std::cmp::Ordering::Equal
                        };
                        if f0.is_some() {
                            while start > 0 && peer(start - 1, start) {
                                start -= 1;
                            }
                        }
                        if f1.is_some() {
                            while end + 1 < rows.len() && peer(end + 1, end) {
                                end += 1;
                            }
                        }
                    }
                    if start > end {
                        scoped = Vec::new();
                    } else {
                        scoped = rows[start..=end].to_vec();
                    }
                    &scoped
                } else {
                    &rows
                };
                let value = window_op(
                    op,
                    &field,
                    scope,
                    pos,
                    params.get(j).copied().flatten(),
                    &sort_fields,
                    &sort_orders,
                );
                if let Value::Object(map) = &mut result[i] {
                    map.insert(name.clone(), value);
                }
            }
        }
    }
    Ok(result)
}

fn compare_sort_rows(
    a: &Value,
    b: &Value,
    fields: &[String],
    orders: &[bool],
) -> std::cmp::Ordering {
    for (i, f) in fields.iter().enumerate() {
        let mut ord = compare(&get_field_owned(a, f), &get_field_owned(b, f));
        if orders.get(i).copied().unwrap_or(false) {
            ord = ord.reverse();
        }
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
    }
    std::cmp::Ordering::Equal
}

fn window_op(
    op: &str,
    field: &str,
    rows: &[Value],
    pos: usize,
    param: Option<f64>,
    sort_fields: &[String],
    sort_orders: &[bool],
) -> Value {
    let n = rows.len();
    match op {
        "row_number" => json_number((pos + 1) as f64),
        "rank" => {
            if sort_fields.is_empty() {
                return json_number((pos + 1) as f64);
            }
            let mut rank = 1;
            for i in 1..=pos {
                if compare_sort_rows(&rows[i - 1], &rows[i], sort_fields, sort_orders)
                    != std::cmp::Ordering::Equal
                {
                    rank = i + 1;
                }
            }
            json_number(rank as f64)
        }
        "dense_rank" => {
            if sort_fields.is_empty() {
                return json_number((pos + 1) as f64);
            }
            let mut count = 1;
            for i in 1..=pos {
                if compare_sort_rows(&rows[i - 1], &rows[i], sort_fields, sort_orders)
                    != std::cmp::Ordering::Equal
                {
                    count += 1;
                }
            }
            json_number(count as f64)
        }
        "percent_rank" => {
            let v = window_op("rank", field, rows, pos, None, sort_fields, sort_orders);
            let r = to_number(&v);
            json_number(if n <= 1 {
                0.0
            } else {
                (r - 1.0) / (n as f64 - 1.0)
            })
        }
        "cume_dist" => json_number((pos + 1) as f64 / n as f64),
        "ntile" => {
            let buckets = param.unwrap_or(1.0).max(1.0);
            json_number(((pos as f64) * buckets / n as f64).floor() + 1.0)
        }
        "lag" | "lead" => {
            let delta = param.unwrap_or(1.0) as i64;
            let target = if op == "lag" {
                pos as i64 - delta
            } else {
                pos as i64 + delta
            };
            if target >= 0 && (target as usize) < n {
                get_field_owned(&rows[target as usize], field)
            } else {
                Value::Null
            }
        }
        "first_value" => get_field_owned(&rows[0], field),
        "last_value" => rows
            .last()
            .map(|r| get_field_owned(r, field))
            .unwrap_or(Value::Null),
        "nth_value" => {
            let k = param.unwrap_or(1.0).max(1.0) as usize;
            rows.get(k - 1)
                .map(|r| get_field_owned(r, field))
                .unwrap_or(Value::Null)
        }
        "count" => json_number(n as f64),
        "sum" => json_number(numeric_values(rows, field).iter().sum()),
        "mean" | "average" => {
            let v = numeric_values(rows, field);
            if v.is_empty() {
                Value::Null
            } else {
                json_number(v.iter().sum::<f64>() / v.len() as f64)
            }
        }
        "min" => aggregate_op("min", field, rows),
        "max" => aggregate_op("max", field, rows),
        "median" => aggregate_op("median", field, rows),
        "distinct" => {
            let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
            for row in rows {
                let v = get_field_owned(row, field);
                if !v.is_null() {
                    seen.insert(serde_json::to_string(&v).unwrap_or_default());
                }
            }
            json_number(seen.len() as f64)
        }
        _ => Value::Null,
    }
}

// ---------------------------------------------------------------------------
// KDE (vega-transforms KDE + vega-statistics bandwidth/sampleCurve)
// ---------------------------------------------------------------------------

fn kde_bandwidth(support: &[f64], n: usize) -> f64 {
    if n == 0 {
        return 0.0;
    }
    // d3-array's `variance`: Welford accumulation, sample denominator.
    let mut count = 0usize;
    let mut wmean = 0.0f64;
    let mut wsum = 0.0f64;
    for &x in support {
        if x.is_nan() {
            continue;
        }
        count += 1;
        let delta = x - wmean;
        wmean += delta / count as f64;
        wsum += delta * (x - wmean);
    }
    let deviation = if count > 1 {
        (wsum / (count - 1) as f64).sqrt()
    } else {
        0.0
    };
    let mut sorted = support.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let q1 = quantile(&sorted, 0.25);
    let q3 = quantile(&sorted, 0.75);
    let h = (q3 - q1) / 1.34;
    let v = if deviation.is_finite() && deviation != 0.0 && deviation < h {
        deviation
    } else if deviation.is_finite() && deviation != 0.0 {
        deviation
    } else if h.is_finite() && h != 0.0 {
        h
    } else if q1.abs() > 0.0 {
        q1.abs()
    } else {
        1.0
    };
    let v = if h.is_finite() && h != 0.0 {
        deviation.min(h)
    } else {
        v
    };
    let v = if v.is_finite() && v != 0.0 {
        v
    } else if deviation.is_finite() && deviation != 0.0 {
        deviation
    } else if q1.abs() > 0.0 {
        q1.abs()
    } else {
        1.0
    };
    1.06 * v * (n as f64).powf(-0.2)
}

fn normal_pdf(z: f64) -> f64 {
    (-0.5 * z * z).exp() / (2.0 * std::f64::consts::PI).sqrt()
}

fn normal_cdf(z: f64) -> f64 {
    // Abramowitz & Stegun 7.1.26 via erf approximation is sufficient here.
    let x = z / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.3275911 * x.abs());
    let poly = t
        * (0.254829592
            + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027 + t * 1.061405429))));
    let erf = 1.0 - poly * (-x * x).exp();
    if z >= 0.0 {
        0.5 * (1.0 + erf)
    } else {
        0.5 * (1.0 - erf)
    }
}

fn kde_pdf(support: &[f64], bandwidth: f64, x: f64) -> f64 {
    if support.is_empty() || bandwidth == 0.0 {
        return f64::NAN;
    }
    let y: f64 = support
        .iter()
        .map(|v| normal_pdf((x - v) / bandwidth))
        .sum();
    y / bandwidth / support.len() as f64
}

fn kde_cdf(support: &[f64], bandwidth: f64, x: f64) -> f64 {
    if support.is_empty() {
        return f64::NAN;
    }
    let y: f64 = support
        .iter()
        .map(|v| normal_cdf((x - v) / bandwidth))
        .sum();
    y / support.len() as f64
}

/// Port of vega-statistics `sampleCurve` (adaptive curve sampling).
fn sample_curve<F>(f: F, extent: (f64, f64), minsteps: f64, maxsteps: f64) -> Vec<(f64, f64)>
where
    F: Fn(f64) -> f64,
{
    let minsteps = if minsteps > 0.0 { minsteps } else { 25.0 };
    let maxsteps = maxsteps.max(minsteps).max(1.0);
    let (min_x, max_x) = extent;
    let span = max_x - min_x;
    let stop = span / maxsteps;
    let point = |x: f64| (x, f(x));
    let mut prev = vec![point(min_x)];
    if (minsteps - maxsteps).abs() < f64::EPSILON {
        let steps = maxsteps as i64;
        for i in 1..steps {
            prev.push(point(min_x + (i as f64 / minsteps) * span));
        }
        prev.push(point(max_x));
        return prev;
    }
    let mut next = vec![point(max_x)];
    let mut i = minsteps as i64;
    while i > 0 {
        i -= 1;
        next.push(point(min_x + (i as f64 / minsteps) * span));
    }
    let mut p0 = prev[0];
    let mut p1 = *next.last().unwrap();
    let sx = if span != 0.0 { 1.0 / span } else { 0.0 };
    let init = p0.1;
    let (mut ymin, mut ymax) = (init, init);
    for p in &next {
        ymin = ymin.min(p.1);
        ymax = ymax.max(p.1);
    }
    let sy = if ymax != ymin { 1.0 / (ymax - ymin) } else { 0.0 };
    loop {
        let pm = point((p0.0 + p1.0) / 2.0);
        let dx = pm.0 - p0.0 >= stop;
        let angle_delta = {
            let a0 = ((sy * (p1.1 - p0.1)).atan2(sx * (p1.0 - p0.0))).abs();
            let a1 = ((sy * (pm.1 - p0.1)).atan2(sx * (pm.0 - p0.0))).abs();
            (a0 - a1).abs()
        };
        const MIN_RADIANS: f64 = 0.5 * std::f64::consts::PI / 180.0;
        if dx && angle_delta > MIN_RADIANS {
            next.push(pm);
        } else {
            p0 = p1;
            prev.push(p1);
            next.pop();
            if next.is_empty() {
                break;
            }
        }
        p1 = *next.last().unwrap();
    }
    prev
}

fn kde(values: Vec<Value>, t: &Value) -> Result<Vec<Value>, Error> {
    let field = t.get("field").and_then(|v| v.as_str()).unwrap_or("");
    let groupby: Vec<String> = t
        .get("groupby")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let as_ = t
        .get("as")
        .and_then(|v| v.as_array())
        .map(|a| {
            (
                a.first()
                    .and_then(|v| v.as_str())
                    .unwrap_or("value")
                    .to_string(),
                a.get(1)
                    .and_then(|v| v.as_str())
                    .unwrap_or("density")
                    .to_string(),
            )
        })
        .unwrap_or_else(|| ("value".to_string(), "density".to_string()));
    let bandwidth = t
        .get("bandwidth")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let counts = t.get("counts").and_then(|v| v.as_bool()).unwrap_or(false);
    let cumulative = t
        .get("cumulative")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let shared = t.get("resolve").and_then(|v| v.as_str()) == Some("shared");
    let extent = t.get("extent").and_then(|v| v.as_array()).and_then(|a| {
        if a.len() == 2 {
            let lo = to_number(&a[0]);
            let hi = to_number(&a[1]);
            if lo.is_finite() && hi.is_finite() {
                return Some((lo, hi));
            }
        }
        None
    });
    let steps = t.get("steps").and_then(|v| v.as_f64());
    let mut minsteps = steps
        .or_else(|| t.get("minsteps").and_then(|v| v.as_f64()))
        .unwrap_or(25.0);
    let maxsteps = steps
        .or_else(|| t.get("maxsteps").and_then(|v| v.as_f64()))
        .unwrap_or(200.0);

    let mut groups: IndexMap<String, (Vec<Value>, Vec<f64>, Vec<f64>)> = IndexMap::new();
    for d in &values {
        let dims: Vec<Value> = groupby.iter().map(|g| get_field_owned(d, g)).collect();
        let key = dims
            .iter()
            .map(|v| serde_json::to_string(v).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\u{1f}");
        let entry = groups
            .entry(key)
            .or_insert_with(|| (dims, Vec::new(), Vec::new()));
        match get_field(d, field) {
            // JS arithmetic coerces null to 0 inside the KDE kernel.
            Some(v) if v.is_null() => entry.1.push(0.0),
            Some(v) => {
                let n = to_number(v);
                if n.is_finite() {
                    entry.1.push(n);
                    entry.2.push(n);
                }
            }
            None => entry.1.push(f64::NAN),
        }
    }

    let shared_extent = if shared {
        let mut mn = f64::INFINITY;
        let mut mx = f64::NEG_INFINITY;
        for d in &values {
            if let Some(n) = field_number(d, field) {
                mn = mn.min(n);
                mx = mx.max(n);
            }
        }
        Some(extent.unwrap_or((mn, mx)))
    } else {
        None
    };
    if shared {
        minsteps = maxsteps;
    }

    let mut out = Vec::new();
    for (_, (dims, support, valid)) in groups {
        let local = shared_extent.or(extent).unwrap_or_else(|| {
            let mn = support.iter().fold(f64::INFINITY, |a, b| a.min(*b));
            let mx = support.iter().fold(f64::NEG_INFINITY, |a, b| a.max(*b));
            (mn, mx)
        });
        if !local.0.is_finite() || !local.1.is_finite() {
            continue;
        }
        let bw = if bandwidth > 0.0 {
            bandwidth
        } else {
            kde_bandwidth(&valid, support.len())
        };
        let scale = if counts { support.len() as f64 } else { 1.0 };
        let samples = if cumulative {
            sample_curve(|x| kde_cdf(&support, bw, x), local, minsteps, maxsteps)
        } else {
            sample_curve(|x| kde_pdf(&support, bw, x), local, minsteps, maxsteps)
        };
        for (x, y) in samples {
            let mut obj = Map::new();
            for (i, g) in groupby.iter().enumerate() {
                obj.insert(g.clone(), dims.get(i).cloned().unwrap_or(Value::Null));
            }
            obj.insert(as_.0.clone(), json_number(x));
            obj.insert(as_.1.clone(), json_number(y * scale));
            out.push(Value::Object(obj));
        }
    }
    Ok(out)
}

fn density(
    values: Vec<Value>,
    t: &Value,
    datasets: Option<&IndexMap<String, Vec<Value>>>,
) -> Result<Vec<Value>, Error> {
    let dist = t.get("distribution").cloned().unwrap_or(Value::Null);
    let func = dist
        .get("function")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let method = t.get("method").and_then(|v| v.as_str()).unwrap_or("pdf");
    let as_ = t
        .get("as")
        .and_then(|v| v.as_array())
        .map(|a| {
            (
                a.first()
                    .and_then(|v| v.as_str())
                    .unwrap_or("value")
                    .to_string(),
                a.get(1)
                    .and_then(|v| v.as_str())
                    .unwrap_or("density")
                    .to_string(),
            )
        })
        .unwrap_or_else(|| ("value".to_string(), "density".to_string()));
    let steps = t.get("steps").and_then(|v| v.as_f64());
    let minsteps = steps
        .or_else(|| t.get("minsteps").and_then(|v| v.as_f64()))
        .unwrap_or(25.0);
    let maxsteps = steps
        .or_else(|| t.get("maxsteps").and_then(|v| v.as_f64()))
        .unwrap_or(200.0);
    let extent = t.get("extent").and_then(|v| v.as_array()).and_then(|a| {
        if a.len() == 2 {
            let lo = to_number(&a[0]);
            let hi = to_number(&a[1]);
            if lo.is_finite() && hi.is_finite() {
                return Some((lo, hi));
            }
        }
        None
    });
    let param = |name: &str, default: f64| {
        dist.get(name).and_then(|v| v.as_f64()).unwrap_or(default)
    };
    let mean = param("mean", 0.0);
    let stdev = param("stdev", 1.0);
    let umin = param("min", 0.0);
    let umax = param("max", 1.0);
    let (support, valid_support): (Vec<f64>, Vec<f64>) = if func == "kde" {
        let field = dist.get("field").and_then(|v| v.as_str()).unwrap_or("");
        let rows: Vec<Value> = match dist.get("from").and_then(|v| v.as_str()) {
            Some(name) => datasets.and_then(|sets| sets.get(name)).cloned(),
            None => Some(values.clone()),
        }
        .unwrap_or_default();
        let mut support = Vec::new();
        let mut valid = Vec::new();
        for d in &rows {
            match get_field(d, field) {
                // Upstream coerces null samples to 0 via JS arithmetic
                // (`x - null`), so KDE uses them as zero-valued support
                // points while still dividing by the full sample count.
                Some(v) if v.is_null() => support.push(0.0),
                Some(v) => {
                    let n = to_number(v);
                    support.push(n);
                    if n.is_finite() {
                        valid.push(n);
                    }
                }
                None => support.push(f64::NAN),
            }
        }
        (support, valid)
    } else {
        (Vec::new(), Vec::new())
    };
    let bw = if func == "kde" {
        let b = param("bandwidth", 0.0);
        if b > 0.0 {
            b
        } else {
            kde_bandwidth(&valid_support, support.len())
        }
    } else {
        0.0
    };
    let domain = extent.or_else(|| {
        if support.is_empty() {
            None
        } else {
            let mn = support.iter().fold(f64::INFINITY, |a, b| a.min(*b));
            let mx = support.iter().fold(f64::NEG_INFINITY, |a, b| a.max(*b));
            Some((mn, mx))
        }
    });
    let Some(domain) = domain else {
        return Ok(vec![]);
    };
    let cumulative = method == "cdf";
    let f = |x: f64| -> f64 {
        match func {
            "normal" => {
                if cumulative {
                    normal_cdf((x - mean) / stdev)
                } else {
                    normal_pdf((x - mean) / stdev) / stdev
                }
            }
            "lognormal" => {
                if x <= 0.0 {
                    if cumulative {
                        0.0
                    } else {
                        0.0
                    }
                } else if cumulative {
                    normal_cdf((x.ln() - mean) / stdev)
                } else {
                    normal_pdf((x.ln() - mean) / stdev) / (x * stdev)
                }
            }
            "uniform" => {
                let span = umax - umin;
                if span == 0.0 {
                    f64::NAN
                } else if cumulative {
                    ((x - umin) / span).clamp(0.0, 1.0)
                } else if x < umin || x > umax {
                    0.0
                } else {
                    1.0 / span
                }
            }
            "kde" => {
                if cumulative {
                    kde_cdf(&support, bw, x)
                } else {
                    kde_pdf(&support, bw, x)
                }
            }
            _ => f64::NAN,
        }
    };
    let samples = sample_curve(f, domain, minsteps, maxsteps);
    Ok(samples
        .into_iter()
        .map(|(x, y)| {
            let mut obj = Map::new();
            obj.insert(as_.0.clone(), json_number(x));
            obj.insert(as_.1.clone(), json_number(y));
            Value::Object(obj)
        })
        .collect())
}

/// Port of `vega-statistics`' `bin`: determines the bin start/stop/step for an
/// extent, honouring `maxbins`, `base`, `divide`, `minstep`, `steps`, `step`,
/// `nice`, `span` and `anchor` just like d3/bin.
fn bin_params(t: &Value, extent: (f64, f64)) -> (f64, f64, f64) {
    let maxb = t.get("maxbins").and_then(|v| v.as_f64()).unwrap_or(20.0);
    let base = t
        .get("base")
        .and_then(|v| v.as_f64())
        .filter(|b| *b > 0.0)
        .unwrap_or(10.0);
    let logb = base.ln();
    let divide: Vec<f64> = t
        .get("divide")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_f64()).collect())
        .filter(|v: &Vec<f64>| !v.is_empty())
        .unwrap_or_else(|| vec![5.0, 2.0]);
    let (mut min, mut max) = extent;
    let span_opt = t.get("span").and_then(|v| v.as_f64()).filter(|v| *v != 0.0);
    let span = span_opt
        .filter(|v| v.is_finite())
        .unwrap_or_else(|| {
            let s = max - min;
            if s != 0.0 {
                s
            } else if min != 0.0 {
                min.abs()
            } else {
                1.0
            }
        });
    let minstep = t.get("minstep").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let step;
    if let Some(s) = t.get("step").and_then(|v| v.as_f64()).filter(|s| *s != 0.0) {
        step = s;
    } else if let Some(steps) = t.get("steps").and_then(|v| v.as_array()) {
        let v = span / maxb;
        let mut i = 0;
        while i < steps.len() && steps[i].as_f64().unwrap_or(f64::NAN) < v {
            i += 1;
        }
        step = steps
            .get(i.saturating_sub(1))
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);
    } else {
        let level = (maxb.max(1.0).ln() / logb).ceil();
        let mut s = (base.powf((span.ln() / logb).round() - level)).max(minstep);
        if !s.is_finite() || s <= 0.0 {
            s = minstep.max(1.0);
        }
        while (span / s).ceil() > maxb {
            s *= base;
        }
        for d in &divide {
            if *d <= 0.0 {
                continue;
            }
            let v = s / d;
            if v >= minstep && span / v <= maxb {
                s = v;
            }
        }
        step = s;
    }
    let nice = t.get("nice").and_then(|v| v.as_bool()).unwrap_or(true);
    if nice && step > 0.0 {
        let lstep = step.ln();
        let precision = if lstep >= 0.0 {
            0.0
        } else {
            (-lstep / logb).trunc() + 1.0
        };
        let eps = base.powf(-precision - 1.0);
        let v = (min / step + eps).floor() * step;
        min = if min < v { v - step } else { v };
        max = (max / step).ceil() * step;
    }
    let mut start = min;
    let mut stop = if max == min { min + step } else { max };
    if let Some(anchor) = t.get("anchor").and_then(|v| v.as_f64()) {
        let d = anchor - (start + step * ((anchor - start) / step).floor());
        start += d;
        stop += d;
    }
    // Bin.js rounds the stop up to a whole number of steps.
    stop = start + ((stop - start) / step).ceil() * step;
    (start, stop, step)
}

fn bin(
    values: Vec<Value>,
    t: &Value,
    signals: &HashMap<String, Value>,
) -> Result<(Vec<Value>, Value), Error> {
    let field = t.get("field").and_then(|v| v.as_str()).unwrap_or("");
    let as_ = t
        .get("as")
        .and_then(|v| v.as_array())
        .map(|a| {
            (
                a.first()
                    .and_then(|v| v.as_str())
                    .unwrap_or("bin0")
                    .to_string(),
                a.get(1)
                    .and_then(|v| v.as_str())
                    .unwrap_or("bin1")
                    .to_string(),
            )
        })
        .unwrap_or_else(|| ("bin0".to_string(), "bin1".to_string()));
    let extent = t
        .get("extent")
        .and_then(|e| match e {
            Value::Array(a) if a.len() == 2 => Some((to_number(&a[0]), to_number(&a[1]))),
            other => other
                .get("signal")
                .and_then(|s| s.as_str())
                .and_then(|name| signals.get(name))
                .and_then(|v| v.as_array())
                .filter(|a| a.len() == 2)
                .map(|a| (to_number(&a[0]), to_number(&a[1]))),
        })
        .unwrap_or_else(|| {
            let mut mn = f64::INFINITY;
            let mut mx = f64::NEG_INFINITY;
            for d in &values {
                if let Some(n) = field_number(d, field) {
                    mn = mn.min(n);
                    mx = mx.max(n);
                }
            }
            (mn, mx)
        });
    if !extent.0.is_finite() || !extent.1.is_finite() {
        return Ok((values, json!([])));
    }
    let (start, stop, step) = bin_params(t, extent);
    let band = t.get("interval").and_then(|v| v.as_bool()).unwrap_or(true);
    let count = (((stop - start) / step).round().max(0.0) as usize).max(0);
    let starts: Vec<Value> = (0..count)
        .map(|i| json_number(start + i as f64 * step))
        .collect();
    let bin_signal = json!({
        "start": start,
        "stop": stop,
        "step": step,
        "bins": starts,
    });
    // epsilon bias to offset floating point error (vega/vega#1737)
    const EPSILON: f64 = 1e-14;
    let mut out = values;
    for d in out.iter_mut() {
        let Some(n) = field_number(d, field) else {
            continue;
        };
        let v = if n < start {
            f64::NEG_INFINITY
        } else if n > stop {
            f64::INFINITY
        } else {
            let clamped = n.max(start).min(stop - step);
            start + step * (EPSILON + (clamped - start) / step).floor()
        };
        if let Value::Object(map) = d {
            map.insert(as_.0.clone(), json_number(v));
            if band {
                // Use the same convoluted math as Vega for floating point parity.
                let end = if v.is_finite() {
                    start + step * (1.0 + (v - start) / step)
                } else {
                    v
                };
                map.insert(as_.1.clone(), json_number(end));
            }
        }
    }
    Ok((out, bin_signal))
}

const MONTHS: [i64; 13] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334, 365];

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        2 => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

pub fn civil_to_ms(y: i64, m: i64, d: i64, h: i64, mi: i64, s: i64, ms: i64) -> f64 {
    // m is 1-based.
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = if y2 >= 0 { y2 } else { y2 - 399 } / 400;
    let yoe = y2 - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    days as f64 * 86_400_000.0
        + h as f64 * 3_600_000.0
        + mi as f64 * 60_000.0
        + s as f64 * 1000.0
        + ms as f64
}

// ---------------------------------------------------------------------------
// Local time support.
//
// Vega's `timeunit` (and the unqualified time expressions) operate in local
// time, while date parsing (`Date.parse`) and `utc*` helpers use UTC. The Rust
// renderer has no timezone crate, so ask libc for the system offset (honouring
// `TZ`) at a given instant.
// ---------------------------------------------------------------------------

#[cfg(unix)]
mod localtime {
    #[repr(C)]
    struct Tm {
        tm_sec: i32,
        tm_min: i32,
        tm_hour: i32,
        tm_mday: i32,
        tm_mon: i32,
        tm_year: i32,
        tm_wday: i32,
        tm_yday: i32,
        tm_isdst: i32,
        tm_gmtoff: i64,
        tm_zone: *const std::os::raw::c_char,
    }

    extern "C" {
        fn localtime_r(timep: *const i64, result: *mut Tm) -> *mut Tm;
    }

    /// Offset in minutes east of UTC at the given instant.
    pub fn offset_minutes(ms: f64) -> i64 {
        let secs = (ms / 1000.0).floor() as i64;
        let mut tm = Tm {
            tm_sec: 0,
            tm_min: 0,
            tm_hour: 0,
            tm_mday: 0,
            tm_mon: 0,
            tm_year: 0,
            tm_wday: 0,
            tm_yday: 0,
            tm_isdst: 0,
            tm_gmtoff: 0,
            tm_zone: std::ptr::null(),
        };
        let ok = unsafe { !localtime_r(&secs, &mut tm).is_null() };
        if ok {
            tm.tm_gmtoff / 60
        } else {
            0
        }
    }
}

#[cfg(not(unix))]
mod localtime {
    pub fn offset_minutes(_ms: f64) -> i64 {
        0
    }
}

pub(crate) fn local_offset_minutes(ms: f64) -> f64 {
    localtime::offset_minutes(ms) as f64
}

/// Civil date/time components in the local timezone.
fn ms_to_local_civil(ms: f64) -> (i64, i64, i64, i64, i64, i64, i64) {
    let offset = local_offset_minutes(ms);
    ms_to_civil(ms + offset * 60_000.0)
}

fn ms_to_local_wall(ms: f64) -> f64 {
    ms + local_offset_minutes(ms) * 60_000.0
}

/// Convert local wall-clock components back to an epoch timestamp.
pub(crate) fn local_civil_to_ms(y: i64, m: i64, d: i64, h: i64, mi: i64, s: i64, ms: i64) -> f64 {
    let wall = civil_to_ms(y, m, d, h, mi, s, ms);
    // Resolve the offset at the target instant (handles DST boundaries).
    let guess = wall - local_offset_minutes(wall) * 60_000.0;
    wall - local_offset_minutes(guess) * 60_000.0
}

pub fn ms_to_civil(ms: f64) -> (i64, i64, i64, i64, i64, i64, i64) {
    let days = (ms / 86_400_000.0).floor() as i64;
    let rem = ms - days as f64 * 86_400_000.0;
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let y = if m <= 2 { y + 1 } else { y };
    let h = (rem / 3_600_000.0).floor() as i64;
    let mi = ((rem % 3_600_000.0) / 60_000.0).floor() as i64;
    let s = ((rem % 60_000.0) / 1000.0).floor() as i64;
    let milli = (rem % 1000.0).floor() as i64;
    (y, m, d, h, mi, s, milli)
}

fn timeunit(
    values: Vec<Value>,
    t: &Value,
    _signals: &HashMap<String, Value>,
) -> Result<Vec<Value>, Error> {
    let field = t.get("field").and_then(|v| v.as_str()).unwrap_or("");
    let units: Vec<String> = t
        .get("units")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let as_ = t
        .get("as")
        .and_then(|v| v.as_array())
        .map(|a| {
            (
                a.first()
                    .and_then(|v| v.as_str())
                    .unwrap_or("unit0")
                    .to_string(),
                a.get(1)
                    .and_then(|v| v.as_str())
                    .unwrap_or("unit1")
                    .to_string(),
            )
        })
        .unwrap_or_else(|| ("unit0".to_string(), "unit1".to_string()));
    let unit_step = t.get("step").and_then(|v| v.as_f64()).unwrap_or(1.0).max(1.0);
    let mut out = values;
    let has_year = units.iter().any(|u| u == "year");
    let has_quarter = units.iter().any(|u| u == "quarter");
    let has_month = units.iter().any(|u| u == "month");
    let has_week = units.iter().any(|u| u == "week");
    let has_isoweek = units.iter().any(|u| u == "isoweek");
    let has_day = units.iter().any(|u| u == "day");
    let has_date_only = units.iter().any(|u| u == "date");
    let has_date = has_date_only || has_day;
    let has_hours = units.iter().any(|u| u == "hours");
    let has_minutes = units.iter().any(|u| u == "minutes");
    let has_seconds = units.iter().any(|u| u == "seconds");
    let has_ms = units.iter().any(|u| u == "milliseconds");
    let has_calendar = has_year || has_quarter || has_month || has_date_only || has_week;
    for d in out.iter_mut() {
        let raw = get_field(d, field).cloned().unwrap_or(Value::Null);
        if raw.is_null() {
            continue;
        }
        let ms = to_number(&raw);
        if !ms.is_finite() {
            continue;
        }
        let (mut y, mut m, mut day, mut h, mut mi, mut s, mut milli) = ms_to_civil(ms);
        let (ly, lm, lday, lh, lmi, ls, lmilli) =
            ms_to_local_civil(ms);
        // Vega's calendar units operate on local wall-clock time. The result is
        // emitted on the same UTC timeline that `timeFormat` (UTC) expects, so
        // that a local midnight is rendered as the corresponding local date.
        if has_calendar {
            y = ly;
            m = lm;
            day = lday;
            h = lh;
            mi = lmi;
            s = ls;
            milli = lmilli;
        }
        let local_ms = if has_calendar {
            // Wall-clock representation of the local instant.
            let off = local_offset_minutes(ms);
            ms + off * 60_000.0
        } else {
            ms
        };
        let mut end = ms;
        if has_week || has_isoweek {
            // Vega's week floor is week-number based (not simply the start of
            // the containing week): the result is the Sunday of the same week
            // number within the target year.
            let wall = if has_calendar {
                local_ms
            } else {
                ms
            };
            let days = (wall / 86_400_000.0).floor() as i64;
            let (input_year, ..) = ms_to_civil(wall);
            let target_year = if has_year { input_year } else { 2012 };
            let jan1 = civil_to_ms(input_year, 1, 1, 0, 0, 0, 0) / 86_400_000.0;
            let jan1_day = jan1 as i64;
            let first_day = (jan1_day + 4).rem_euclid(7);
            let first_sunday = jan1_day + (7 - first_day) % 7;
            let week_num = if days < first_sunday {
                0
            } else {
                (days - first_sunday) / 7 + 1
            };
            let first_day_target = {
                let j = civil_to_ms(target_year, 1, 1, 0, 0, 0, 0) / 86_400_000.0;
                (j as i64 + 4).rem_euclid(7)
            };
            let date_value = if has_isoweek {
                // ISO weeks start Monday and are numbered 1..53.
                let monday_first = (first_day + 6) % 7;
                let first_monday = jan1_day + (7 - monday_first) % 7;
                let iso_week = if days < first_monday {
                    0
                } else {
                    (days - first_monday) / 7 + 1
                };
                iso_week * 7 - ((first_day_target + 6) % 7)
            } else {
                week_num * 7 - (first_day_target + 6) % 7
            };
            let start = civil_to_ms(target_year, 1, 1, 0, 0, 0, 0)
                + (date_value - 1) as f64 * 86_400_000.0;
            end = start + 7.0 * 86_400_000.0;
            if let Value::Object(map) = d {
                map.insert(as_.0.clone(), json_number(start));
                map.insert(as_.1.clone(), json_number(end));
            }
            continue;
        }
        if has_day
            && !has_year
            && !has_quarter
            && !has_month
            && !has_week
            && !has_isoweek
            && !has_date_only
            && !has_hours
            && !has_minutes
            && !has_seconds
            && !has_ms
        {
            // Day-of-week is normalized into the reference year 2012, matching
            // Vega's timeunit defaults (2012-01-01 was a Sunday).
            let local_days = (ms_to_local_wall(ms) / 86_400_000.0).floor() as i64;
            let weekday = (local_days + 4).rem_euclid(7);
            let start = local_civil_to_ms(2012, 1, weekday + 1, 0, 0, 0, 0);
            let end = start + 86_400_000.0;
            if let Value::Object(map) = d {
                map.insert(as_.0.clone(), json_number(start));
                map.insert(as_.1.clone(), json_number(end));
            }
            continue;
        }
        if has_year || has_quarter || has_month || has_date {
            if !has_year {
                // Vega's timeunit uses 2012 as the reference year when no
                // explicit year unit is present.
                y = 2012;
            }
            if has_year && !has_month && !has_date && !has_quarter {
                y = ((y as f64) / unit_step).floor() as i64 * unit_step as i64;
            }
            if has_quarter && !has_month {
                // Quarter start: months 1, 4, 7, 10.
                m = ((m - 1) / 3) * 3 + 1;
            } else if !has_month {
                m = 1;
            }
            if !has_date {
                day = 1;
            }
            if !has_hours {
                h = 0;
            }
            if !has_minutes {
                mi = 0;
            }
            if !has_seconds {
                s = 0;
            }
            if !has_ms {
                milli = 0;
            }
            let start = local_civil_to_ms(y, m, day, h, mi, s, milli);
            let sub_day_step = if has_ms {
                1.0
            } else if has_seconds {
                1000.0
            } else if has_minutes {
                60_000.0
            } else if has_hours {
                3_600_000.0
            } else {
                0.0
            };
            let end_ms = if has_quarter && sub_day_step == 0.0 {
                local_civil_to_ms(
                    y + if m > 9 { 1 } else { 0 },
                    if m > 9 { m - 9 } else { m + 3 },
                    1,
                    0,
                    0,
                    0,
                    0,
                )
            } else if sub_day_step > 0.0 {
                start + sub_day_step
            } else if has_date {
                start + 86_400_000.0
            } else if has_month {
                let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
                local_civil_to_ms(ny, nm, 1, 0, 0, 0, 0)
            } else if has_year {
                local_civil_to_ms(y + unit_step as i64, 1, 1, 0, 0, 0, 0)
            } else {
                start + 86_400_000.0
            };
            end = end_ms;
            let _ = MONTHS;
            if let Value::Object(map) = d {
                map.insert(as_.0.clone(), json_number(start));
                map.insert(as_.1.clone(), json_number(end));
            }
            continue;
        } else if has_hours || has_minutes || has_seconds || has_ms {
            // Sub-day units without year/month/date are normalized into the
            // reference year 2012 (Vega's timeunit default).
            y = 2012;
            m = 1;
            day = 1;
            h = lh;
            mi = lmi;
            s = ls;
            milli = lmilli;
            if !has_hours {
                h = 0;
            }
            if !has_minutes {
                mi = 0;
            }
            if !has_seconds {
                s = 0;
            }
            if !has_ms {
                milli = 0;
            }
            // `step` floors the smallest active unit (e.g. minutes step 5
            // snaps 00:07 to 00:05).
            let k = unit_step as i64;
            if has_ms {
                milli -= milli.rem_euclid(k);
            } else if has_seconds {
                s -= s.rem_euclid(k);
            } else if has_minutes {
                mi -= mi.rem_euclid(k);
            } else if has_hours {
                h -= h.rem_euclid(k);
            }
            let start = local_civil_to_ms(y, m, day, h, mi, s, milli);
            let step = if has_hours {
                3_600_000.0
            } else if has_minutes {
                60_000.0
            } else if has_seconds {
                1000.0
            } else {
                1.0
            } * unit_step;
            end = start + step;
            let _ = start;
            let _ = has_seconds;
            if let Value::Object(map) = d {
                map.insert(as_.0.clone(), json_number(start));
                map.insert(as_.1.clone(), json_number(end));
            }
            continue;
        }
        if let Value::Object(map) = d {
            // Units without a calendar component leave the timestamp as-is.
            map.insert(as_.0.clone(), json_number(ms));
            map.insert(as_.1.clone(), json_number(end));
        }
    }
    let _ = days_in_month;
    Ok(out)
}

fn impute(
    values: Vec<Value>,
    t: &Value,
    signals: &HashMap<String, Value>,
    datasets: Option<&IndexMap<String, Vec<Value>>>,
) -> Result<Vec<Value>, Error> {
    let field = t.get("field").and_then(|v| v.as_str()).unwrap_or("");
    let key = t.get("key").and_then(|v| v.as_str()).unwrap_or("");
    let groupby: Vec<String> = t
        .get("groupby")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let method = t.get("method").and_then(|v| v.as_str()).unwrap_or("value");
    let value = t.get("value").cloned().unwrap_or(Value::Null);

    let mut all_keys: Vec<Value> = Vec::new();
    for d in &values {
        if let Some(k) = get_field(d, key) {
            if !all_keys.iter().any(|x| x == k) {
                all_keys.push(k.clone());
            }
        }
    }
    if let Some(keyvals) = t.get("keyvals") {
        let explicit = match keyvals {
            Value::Array(a) => Some(a.clone()),
            Value::Object(o) => o
                .get("signal")
                .and_then(|s| s.as_str())
                .and_then(|sig| {
                    let env = DataEnv { signals, datasets };
                    expression::evaluate(sig, None, &env).ok()
                })
                .and_then(|v| v.as_array().cloned()),
            _ => None,
        };
        if let Some(explicit) = explicit {
            for k in explicit {
                if !all_keys.iter().any(|x| loose_eq(x, &k)) {
                    all_keys.push(k);
                }
            }
        }
    }
    let mut groups: IndexMap<String, Vec<Value>> = IndexMap::new();
    for d in values {
        let gkey = groupby
            .iter()
            .map(|g| serde_json::to_string(&get_field_owned(&d, g)).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\u{1f}");
        groups.entry(gkey).or_default().push(d);
    }
    let mut out = Vec::new();
    for (_, rows) in groups {
        let mut imputed: Vec<Value> = Vec::new();
        let mut present: Vec<Value> = Vec::new();
        for d in &rows {
            if let Some(k) = get_field(d, key) {
                present.push(k.clone());
            }
        }
        for k in &all_keys {
            if !present.iter().any(|x| x == k) {
                let mut obj = Map::new();
                if let Some(first) = rows.first() {
                    for g in &groupby {
                        obj.insert(g.clone(), get_field_owned(first, g));
                    }
                }
                obj.insert(key.to_string(), k.clone());
                let fill = if method == "value" {
                    value.clone()
                } else {
                    aggregate_op(method, field, &rows)
                };
                obj.insert(field.to_string(), fill);
                imputed.push(Value::Object(obj));
            }
        }
        out.extend(rows);
        out.extend(imputed);
    }
    Ok(out)
}

fn fold(values: Vec<Value>, t: &Value) -> Result<Vec<Value>, Error> {
    let fields: Vec<String> = t
        .get("fields")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let as_ = t
        .get("as")
        .and_then(|v| v.as_array())
        .map(|a| {
            (
                a.first()
                    .and_then(|v| v.as_str())
                    .unwrap_or("key")
                    .to_string(),
                a.get(1)
                    .and_then(|v| v.as_str())
                    .unwrap_or("value")
                    .to_string(),
            )
        })
        .unwrap_or_else(|| ("key".to_string(), "value".to_string()));
    let mut out = Vec::new();
    for d in values {
        for f in &fields {
            let mut obj = d.clone();
            set_field(&mut obj, &as_.0, Value::String(f.clone()));
            let v = get_field_owned(&d, f);
            set_field(&mut obj, &as_.1, v);
            out.push(obj);
        }
    }
    Ok(out)
}

fn flatten(values: Vec<Value>, t: &Value) -> Result<Vec<Value>, Error> {
    let fields: Vec<String> = t
        .get("fields")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let as_: Vec<String> = t
        .get("as")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default();
    let mut out = Vec::new();
    for d in values {
        let mut idx = 0;
        loop {
            let mut any = false;
            let mut obj = d.clone();
            for (i, f) in fields.iter().enumerate() {
                if let Some(Value::Array(a)) = get_field(&d, f) {
                    if idx < a.len() {
                        any = true;
                        let name = as_.get(i).cloned().unwrap_or_else(|| f.clone());
                        set_field(&mut obj, &name, a[idx].clone());
                    } else {
                        set_field(
                            &mut obj,
                            &as_.get(i).cloned().unwrap_or_else(|| f.clone()),
                            Value::Null,
                        );
                    }
                }
            }
            if !any {
                break;
            }
            out.push(obj);
            idx += 1;
        }
    }
    Ok(out)
}

fn transform_field(datum: &Value, field: &Value, env: &DataEnv) -> Value {
    if let Some(expr) = field.get("expr").and_then(|v| v.as_str()) {
        expression::evaluate(expr, Some(datum), env).unwrap_or(Value::Null)
    } else {
        field_value(datum, field)
    }
}

fn geojson_features(values: &[Value], t: &Value, env: &DataEnv) -> Value {
    let mut features: Vec<Value> = Vec::new();
    if let Some(field) = t.get("geojson").and_then(|v| v.as_str()) {
        for d in values {
            let value = field_value(d, &json!(field));
            if !value.is_null() {
                features.push(value);
            }
        }
    }
    if let Some(fields) = t.get("fields").and_then(|v| v.as_array()) {
        let lon = fields.first().cloned().unwrap_or(Value::Null);
        let lat = fields.get(1).cloned().unwrap_or(Value::Null);
        let mut points: Vec<Value> = Vec::new();
        for d in values {
            let x = to_number(&transform_field(d, &lon, env));
            let y = to_number(&transform_field(d, &lat, env));
            if x.is_finite() && y.is_finite() {
                points.push(json!([x, y]));
            }
        }
        features.push(json!({
            "type": "Feature",
            "geometry": {"type": "MultiPoint", "coordinates": points}
        }));
    }
    json!({"type": "FeatureCollection", "features": features})
}

fn graticule_geometry(t: &Value) -> Value {
    let major = t
        .get("extentMajor")
        .or_else(|| t.get("extent"))
        .and_then(|v| v.as_array());
    let minor = t
        .get("extentMinor")
        .or_else(|| t.get("extent"))
        .and_then(|v| v.as_array());
    let pair = |v: &Value| -> Option<[f64; 2]> {
        let a = v.as_array()?;
        Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?])
    };
    let extent = |v: Option<&Vec<Value>>| -> Option<[[f64; 2]; 2]> {
        let a = v?;
        Some([pair(a.first()?)?, pair(a.get(1)?)?])
    };
    let (x0, x1, y0, y1) = {
        let e = extent(minor);
        (
            e.map(|e| e[0][0]).unwrap_or(-180.0),
            e.map(|e| e[1][0]).unwrap_or(180.0),
            e.map(|e| e[0][1]).unwrap_or(-80.0),
            e.map(|e| e[1][1]).unwrap_or(80.0),
        )
    };
    let (mx0, mx1, my0, my1) = {
        let e = extent(major);
        (
            e.map(|e| e[0][0]).unwrap_or(-180.0),
            e.map(|e| e[1][0]).unwrap_or(180.0),
            e.map(|e| e[0][1]).unwrap_or(-90.0),
            e.map(|e| e[1][1]).unwrap_or(90.0),
        )
    };
    // Minor extents default to [-180,-80-eps] [180,80+eps] upstream.
    let (y0, y1) = if minor.is_some() {
        (y0, y1)
    } else {
        (-80.0 - 1e-6, 80.0 + 1e-6)
    };
    let (my0, my1) = if major.is_some() {
        (my0, my1)
    } else {
        (-90.0 + 1e-6, 90.0 - 1e-6)
    };
    let step = t.get("step").and_then(|v| v.as_array()).map(|a| {
        (
            a.first().and_then(|v| v.as_f64()).unwrap_or(10.0),
            a.get(1).and_then(|v| v.as_f64()).unwrap_or(10.0),
        )
    });
    let (dx, dy) = t
        .get("stepMinor")
        .and_then(|v| v.as_array())
        .map(|a| {
            (
                a.first().and_then(|v| v.as_f64()).unwrap_or(10.0),
                a.get(1).and_then(|v| v.as_f64()).unwrap_or(10.0),
            )
        })
        .or(step)
        .unwrap_or((10.0, 10.0));
    let (dx_major, dy_major) = t
        .get("stepMajor")
        .and_then(|v| v.as_array())
        .map(|a| {
            (
                a.first().and_then(|v| v.as_f64()).unwrap_or(90.0),
                a.get(1).and_then(|v| v.as_f64()).unwrap_or(360.0),
            )
        })
        .or(step)
        .unwrap_or((90.0, 360.0));
    let precision = t.get("precision").and_then(|v| v.as_f64()).unwrap_or(2.5);

    let seq = |start: f64, stop: f64, step: f64| -> Vec<f64> {
        let mut out = Vec::new();
        let mut v = (start / step).ceil() * step;
        while v < stop - 1e-9 {
            out.push(v);
            v += step;
        }
        out
    };
    let mut lines: Vec<Value> = Vec::new();
    let meridian = |x: f64, ys: &[f64]| -> Value {
        Value::Array(ys.iter().map(|y| json!([x, *y])).collect())
    };
    let parallel = |y: f64, xs: &[f64]| -> Value {
        Value::Array(xs.iter().map(|x| json!([*x, y])).collect())
    };
    // d3-array `range` starts at the raw endpoint (not a step multiple); the
    // line positions above use the ceil-quantized starting points instead.
    let range = |start: f64, stop: f64, step: f64| -> Vec<f64> {
        let mut out = Vec::new();
        let mut v = start;
        while v < stop {
            out.push(v);
            v += step;
        }
        out
    };
    let with_end = |mut values: Vec<f64>, end: f64| {
        values.push(end);
        values
    };
    let major_y = with_end(range(my0, my1 - 1e-6, 90.0), my1);
    let major_x = with_end(range(mx0, mx1 - 1e-6, precision), mx1);
    let minor_ys = with_end(range(y0, y1 - 1e-6, 90.0), y1);
    let minor_xs = with_end(range(x0, x1 - 1e-6, precision), x1);
    let _ = dx;
    for x in seq(mx0, mx1, dx_major) {
        lines.push(meridian(x, &major_y));
    }
    for y in seq(my0, my1, dy_major) {
        lines.push(parallel(y, &major_x));
    }
    for x in seq(x0, x1, dx).into_iter().filter(|x| (x % dx_major).abs() > 1e-6) {
        lines.push(meridian(x, &minor_ys));
    }
    for y in seq(y0, y1, dy).into_iter().filter(|y| (y % dy_major).abs() > 1e-6) {
        lines.push(parallel(y, &minor_xs));
    }
    json!({"type": "MultiLineString", "coordinates": lines})
}

/// Port of vega-regression's `Regression` transform (linear/constant fits).
fn regression(values: Vec<Value>, t: &Value) -> Vec<Value> {
    let x_field = t.get("x").and_then(|v| v.as_str()).unwrap_or("x");
    let y_field = t.get("y").and_then(|v| v.as_str()).unwrap_or("y");
    let method = t
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("linear");
    let want_params = t.get("params").and_then(|v| v.as_bool()).unwrap_or(false);
    let as_ = t
        .get("as")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_else(|| vec![json!(x_field), json!(y_field)]);
    let x_name = as_.first().and_then(|v| v.as_str()).unwrap_or(x_field);
    let y_name = as_.get(1).and_then(|v| v.as_str()).unwrap_or(y_field);
    let groupby: Vec<String> = t
        .get("groupby")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let mut groups: IndexMap<String, (Vec<Value>, Vec<(f64, f64)>)> = IndexMap::new();
    for row in values {
        let raw_x = get_field(&row, x_field).filter(|v| !v.is_null()).map(to_number);
        let raw_y = get_field(&row, y_field).filter(|v| !v.is_null()).map(to_number);
        let (Some(x), Some(y)) = (raw_x, raw_y) else {
            continue;
        };
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        let dims: Vec<Value> = groupby.iter().map(|g| get_field_owned(&row, g)).collect();
        let key = serde_json::to_string(&dims).unwrap_or_default();
        let entry = groups.entry(key).or_insert_with(|| (dims, Vec::new()));
        entry.1.push((x, y));
    }

    let extent = t.get("extent").and_then(|e| e.as_array());
    let mut out = Vec::new();
    for (_, (dims, points)) in groups {
        let n = points.len();
        if n <= 1 {
            continue;
        }
        let ux = points.iter().map(|p| p.0).sum::<f64>() / n as f64;
        let uy = points.iter().map(|p| p.1).sum::<f64>() / n as f64;
        let uxy = points.iter().map(|p| p.0 * p.1).sum::<f64>() / n as f64;
        let ux2 = points.iter().map(|p| p.0 * p.0).sum::<f64>() / n as f64;
        let (intercept, slope) = if method == "constant" {
            (uy, 0.0)
        } else {
            ols(ux, uy, uxy, ux2)
        };
        let predict = |x: f64| intercept + slope * x;
        let (mut sse, mut sst) = (0.0, 0.0);
        for (x, y) in &points {
            let d = y - predict(*x);
            sse += d * d;
            let d = y - uy;
            sst += d * d;
        }
        let r_squared = if sst == 0.0 { 1.0 - sse / sst } else { 1.0 - sse / sst };

        if want_params {
            let mut row = Map::new();
            row.insert("keys".to_string(), Value::Array(dims.clone()));
            row.insert("coef".to_string(), json!([intercept, slope]));
            row.insert("rSquared".to_string(), json_number(r_squared));
            out.push(Value::Object(row));
            continue;
        }

        let (lo, hi) = if let Some(e) = extent.filter(|e| e.len() >= 2) {
            (to_number(&e[0]), to_number(&e[1]))
        } else {
            let lo = points.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
            let hi = points.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
            (lo, hi)
        };
        for x in [lo, hi] {
            let mut row = Map::new();
            for (i, name) in groupby.iter().enumerate() {
                row.insert(name.clone(), dims.get(i).cloned().unwrap_or(Value::Null));
            }
            row.insert(x_name.to_string(), json_number(x));
            row.insert(y_name.to_string(), json_number(predict(x)));
            out.push(Value::Object(row));
        }
    }
    out
}

fn loess(values: Vec<Value>, t: &Value) -> Vec<Value> {
    let x_field = t.get("x").and_then(|v| v.as_str()).unwrap_or("x");
    let y_field = t.get("y").and_then(|v| v.as_str()).unwrap_or("y");
    let bandwidth = t.get("bandwidth").and_then(|v| v.as_f64()).unwrap_or(0.3);
    let as_ = t
        .get("as")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_else(|| vec![json!(x_field), json!(y_field)]);
    let x_name = as_.first().and_then(|v| v.as_str()).unwrap_or(x_field);
    let y_name = as_.get(1).and_then(|v| v.as_str()).unwrap_or(y_field);
    let groupby: Vec<String> = t
        .get("groupby")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let mut groups: IndexMap<String, Vec<(f64, f64, Value)>> = IndexMap::new();
    for row in values {
        let raw_x = field_value(&row, &json!(x_field));
        let raw_y = field_value(&row, &json!(y_field));
        if raw_x.is_null() || raw_y.is_null() {
            continue;
        }
        let x = to_number(&raw_x);
        let y = to_number(&raw_y);
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        let key: String = groupby
            .iter()
            .map(|f| serde_json::to_string(&get_field_owned(&row, f)).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\u{1f}");
        groups.entry(key).or_default().push((x, y, row));
    }
    let mut out = Vec::new();
    for (_, mut pts) in groups {
        pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let n = pts.len();
        if n == 0 {
            continue;
        }
        let ux = pts.iter().map(|p| p.0).sum::<f64>() / n as f64;
        let uy = pts.iter().map(|p| p.1).sum::<f64>() / n as f64;
        let xv: Vec<f64> = pts.iter().map(|p| p.0 - ux).collect();
        let yv: Vec<f64> = pts.iter().map(|p| p.1 - uy).collect();
        let bw = ((bandwidth * n as f64) as i64).max(2) as usize;
        let bw = bw.min(n);
        let mut yhat = vec![0.0; n];
        let mut residuals = vec![0.0; n];
        let mut robust = vec![1.0; n];
        for iter in 0..=2 {
            let mut interval = (0usize, bw.saturating_sub(1));
            for i in 0..n {
                let dx = xv[i];
                let (i0, i1) = interval;
                let edge = if (dx - xv[i0]) > (xv[i1] - dx) { i0 } else { i1 };
                let dist = (xv[edge] - dx).abs();
                let denom = 1.0 / if dist == 0.0 { 1.0 } else { dist };
                let (mut wsum, mut xs, mut ys, mut xys, mut x2s) = (0.0, 0.0, 0.0, 0.0, 0.0);
                for k in i0..=i1 {
                    let xk = xv[k];
                    let yk = yv[k];
                    let w = tricube((dx - xk).abs() * denom) * robust[k];
                    let xkw = xk * w;
                    wsum += w;
                    xs += xkw;
                    ys += yk * w;
                    xys += yk * xkw;
                    x2s += xk * xkw;
                }
                let (a, b) = ols(xs / wsum, ys / wsum, xys / wsum, x2s / wsum);
                yhat[i] = a + b * dx;
                residuals[i] = (yv[i] - yhat[i]).abs();
                update_interval(&xv, i + 1, &mut interval);
            }
            if iter == 2 {
                break;
            }
            let mut med = residuals.clone();
            med.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let median_residual = median_sorted(&med);
            if median_residual.abs() < 1e-12 {
                break;
            }
            for i in 0..n {
                let arg = residuals[i] / (6.0 * median_residual);
                robust[i] = if arg >= 1.0 { 1e-12 } else { let w = 1.0 - arg * arg; w * w };
            }
        }
        // Average repeated x values and add the centering offsets back,
        // mirroring vega-statistics' `output()` online update.
        let mut raw: Vec<(f64, f64, Value)> = Vec::new();
        let mut prev: Option<(f64, f64)> = None;
        let mut cnt = 0.0;
        for i in 0..n {
            let v = xv[i] + ux;
            if let Some((px, py)) = prev {
                if px == v {
                    cnt += 1.0;
                    prev = Some((px, py + (yhat[i] - py) / cnt));
                    continue;
                }
                raw.push((px, py + uy, pts[i - 1].2.clone()));
            }
            cnt = 0.0;
            prev = Some((v, yhat[i]));
        }
        if let Some((px, py)) = prev {
            raw.push((px, py + uy, pts[n - 1].2.clone()));
        }
        out.extend(raw);
    }
    out.into_iter()
        .map(|(x, y, row)| {
            let mut obj = Map::new();
            for f in &groupby {
                obj.insert(f.clone(), get_field_owned(&row, f));
            }
            obj.insert(x_name.to_string(), json_number(x));
            obj.insert(y_name.to_string(), json_number(y));
            Value::Object(obj)
        })
        .collect()
}

fn tricube(x: f64) -> f64 {
    let x = 1.0 - x * x * x;
    x * x * x
}

fn ols(ux: f64, uy: f64, uxy: f64, ux2: f64) -> (f64, f64) {
    let delta = ux2 - ux * ux;
    let slope = if delta.abs() < 1e-24 { 0.0 } else { (uxy - ux * uy) / delta };
    (uy - slope * ux, slope)
}

fn update_interval(xv: &[f64], i: usize, interval: &mut (usize, usize)) {
    if i >= xv.len() {
        return;
    }
    let val = xv[i];
    let mut left = interval.0;
    let mut right = interval.1 + 1;
    if right >= xv.len() {
        return;
    }
    while i > left && (xv[right] - val) <= (val - xv[left]) {
        left += 1;
        interval.0 = left;
        interval.1 = right;
        right += 1;
        if right >= xv.len() {
            break;
        }
    }
}

fn median_sorted(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    }
}

fn project(values: Vec<Value>, t: &Value) -> Result<Vec<Value>, Error> {
    let fields = t
        .get("fields")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let as_ = t
        .get("as")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    for d in values {
        let mut obj = Map::new();
        for (i, f) in fields.iter().enumerate() {
            let name = as_
                .get(i)
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| f.as_str().unwrap_or(""))
                .to_string();
            obj.insert(name, field_value(&d, f));
        }
        out.push(Value::Object(obj));
    }
    Ok(out)
}

/// Hashable key with JavaScript `Map` (SameValueZero) equality.
#[derive(Hash, PartialEq, Eq)]
enum LookupKey {
    Null,
    Bool(bool),
    Number(u64),
    String(String),
    Other(String),
}

impl LookupKey {
    fn from_value(value: &Value) -> Self {
        match value {
            Value::Null => LookupKey::Null,
            Value::Bool(b) => LookupKey::Bool(*b),
            Value::Number(n) => {
                let f = n.as_f64().unwrap_or(f64::NAN);
                let f = if f == 0.0 {
                    0.0
                } else if f.is_nan() {
                    f64::NAN
                } else {
                    f
                };
                LookupKey::Number(f.to_bits())
            }
            Value::String(s) => LookupKey::String(s.clone()),
            other => LookupKey::Other(
                serde_json::to_string(other).unwrap_or_else(|_| "null".to_string()),
            ),
        }
    }
}

fn lookup(
    values: Vec<Value>,
    t: &Value,
    datasets: Option<&IndexMap<String, Vec<Value>>>,
) -> Result<Vec<Value>, Error> {
    let as_: Vec<String> = t
        .get("as")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let from = t.get("from").and_then(|v| v.as_str()).unwrap_or("");
    let key = t.get("key").and_then(|v| v.as_str()).unwrap_or("");
    let fields: Vec<String> = t
        .get("fields")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let value_fields: Vec<String> = t
        .get("values")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let default = t.get("default").cloned().unwrap_or(Value::Null);
    let target = datasets.and_then(|sets| sets.get(from));
    // Upstream builds a hash index once; scanning the source rows for every
    // input row is O(n*m) and dominates large lookups (county maps, flights).
    let index: Option<HashMap<LookupKey, &Value>> = target.map(|rows| {
        let mut index = HashMap::with_capacity(rows.len());
        for row in rows {
            if let Some(value) = get_field(row, key) {
                index.insert(LookupKey::from_value(value), row);
            }
        }
        index
    });
    let mut out = values;
    for d in out.iter_mut() {
        for (i, f) in fields.iter().enumerate() {
            let probe = get_field_owned(d, f);
            // Vega's lookup index is last-write-wins when the source data has
            // duplicate keys (some TopoJSON county sets contain duplicates).
            let found = index
                .as_ref()
                .and_then(|index| index.get(&LookupKey::from_value(&probe)).copied())
                .filter(|r| loose_eq(&get_field_owned(r, key), &probe))
                .or_else(|| {
                    // Fall back for loose matches the exact index cannot see
                    // (for example the number 1 vs the string "1").
                    target.and_then(|rows| {
                        rows.iter()
                            .rev()
                            .find(|r| loose_eq(&get_field_owned(r, key), &probe))
                    })
                });
            if value_fields.is_empty() {
                let name = as_.get(i).cloned().unwrap_or_else(|| "_lookup".to_string());
                let value = found.cloned().unwrap_or_else(|| default.clone());
                set_field(d, &name, value);
            } else {
                for (j, vf) in value_fields.iter().enumerate() {
                    let name = as_.get(j).cloned().unwrap_or_else(|| vf.clone());
                    let value = found
                        .map(|r| get_field_owned(r, vf))
                        .filter(|v| !v.is_null())
                        .unwrap_or_else(|| default.clone());
                    set_field(d, &name, value);
                }
            }
        }
    }
    Ok(out)
}

fn sequence(t: &Value, env: &DataEnv) -> Result<(Vec<Value>, Option<(String, Value)>), Error> {
    let eval = |v: Option<&Value>, default: f64| -> f64 {
        match v {
            Some(Value::Number(n)) => n.as_f64().unwrap_or(default),
            Some(Value::Object(o)) => o
                .get("signal")
                .and_then(|s| s.as_str())
                .and_then(|s| expression::evaluate(s, None, env).ok())
                .map(|v| to_number(&v))
                .unwrap_or(default),
            _ => default,
        }
    };
    let start = eval(t.get("start"), 0.0);
    let stop = eval(t.get("stop"), 0.0);
    let step = eval(t.get("step"), 1.0);
    let as_ = t.get("as").and_then(|v| v.as_str()).unwrap_or("data");
    let mut out = Vec::new();
    if step != 0.0 {
        let mut x = start;
        while (step > 0.0 && x < stop) || (step < 0.0 && x > stop) {
            out.push(json!({ as_: x }));
            x += step;
            if out.len() > 1_000_000 {
                break;
            }
        }
    }
    let sig = t.get("signal").and_then(|v| v.as_str()).map(|name| {
        (
            name.to_string(),
            json!({ "start": start, "stop": stop, "step": step }),
        )
    });
    Ok((out, sig))
}

fn pivot(values: Vec<Value>, t: &Value) -> Result<Vec<Value>, Error> {
    let field = t.get("field").and_then(|v| v.as_str()).unwrap_or("");
    let value = t.get("value").and_then(|v| v.as_str()).unwrap_or("");
    let groupby: Vec<String> = t
        .get("groupby")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let mut groups: IndexMap<String, Vec<Value>> = IndexMap::new();
    for d in values {
        let key = groupby
            .iter()
            .map(|g| serde_json::to_string(&get_field_owned(&d, g)).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\u{1f}");
        groups.entry(key).or_default().push(d);
    }
    let mut out = Vec::new();
    for (_, rows) in groups {
        let mut obj = Map::new();
        if let Some(first) = rows.first() {
            for g in &groupby {
                let name = crate::value::split_access_path(g).join(".");
                obj.insert(name, get_field_owned(first, g));
            }
        }
        for r in &rows {
            let key = to_string_js(&get_field_owned(r, field));
            obj.insert(key, get_field_owned(r, value));
        }
        out.push(Value::Object(obj));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_parse_quotes() {
        let rows = parse_delimited("a,b\n\"x, y\",2\n", ',');
        assert_eq!(rows[1][0], "x, y");
        assert_eq!(rows[1][1], "2");
    }

    #[test]
    fn aggregate_counts() {
        let values = vec![json!({"a": "x", "b": 1}), json!({"a": "x", "b": 3})];
        let out = aggregate(
            values,
            &json!({"groupby": ["a"], "ops": ["sum", "count"], "fields": ["b", null], "as": ["s", "n"]}),
        )
        .unwrap();
        assert_eq!(out[0]["s"], json!(4));
        assert_eq!(out[0]["n"], json!(2));
    }

    #[test]
    fn stack_zero() {
        let values = vec![json!({"g": "a", "v": 2}), json!({"g": "a", "v": 3})];
        let out = stack(
            values,
            &json!({"groupby": ["g"], "field": "v", "as": ["v0", "v1"]}),
        )
        .unwrap();
        assert_eq!(out[0]["v0"], json!(0));
        assert_eq!(out[0]["v1"], json!(2));
        assert_eq!(out[1]["v1"], json!(5));
    }

    #[test]
    fn load_wraps_primitive_values_as_data_tuples() {
        let spec = vec![json!({"name": "d", "values": [12, 23]})];
        let store = load(
            Some(&spec),
            &DataOptions::default(),
            &HashMap::new(),
        )
        .unwrap();
        let rows = store.get("d").unwrap();
        assert_eq!(rows[0]["data"], json!(12));
        assert_eq!(rows[1]["data"], json!(23));
    }

    #[test]
    fn stack_center_uses_global_max_group_sum() {
        // Vega centers each group using the *largest* group sum, not the
        // group's own sum.
        let values = vec![
            json!({"g": "a", "v": 1}),
            json!({"g": "b", "v": 3}),
        ];
        let out = stack(
            values,
            &json!({
                "groupby": ["g"], "field": "v", "offset": "center",
                "as": ["v0", "v1"]
            }),
        )
        .unwrap();
        assert_eq!(out[0]["v0"], json!(1));
        assert_eq!(out[0]["v1"], json!(2));
        assert_eq!(out[1]["v0"], json!(0));
        assert_eq!(out[1]["v1"], json!(3));
    }
}

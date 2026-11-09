//! Runtime: signal resolution, scales, scenegraph construction, guides and
//! layout. This is the equivalent of the static subset of `vega-runtime`,
//! `vega-parser` and `vega-view`.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use crate::data::{self, DataOptions, DataStore};
use crate::error::Error;
use crate::expression::{self, Environment};
use crate::projection::{self, Projection};
use crate::scale::{self, Scale};
use crate::bounds::text_width;
use crate::scenegraph::Item;
use crate::value::*;

pub const DEFAULT_FONT: &str = "sans-serif";
pub const DEFAULT_COLOR: &str = "#4c78a8";

#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Base directory for relative `data.url` references.
    pub data_base: PathBuf,
    /// Additional directories searched for relative data URLs.
    pub data_dirs: Vec<PathBuf>,
    /// Optional output scale factor (sets `width`/`height` multipliers).
    pub scale: f64,
    /// Override the spec background.
    pub background: Option<String>,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            data_base: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            data_dirs: Vec::new(),
            scale: 1.0,
            background: None,
        }
    }
}

pub struct Env<'a> {
    pub data: &'a DataStore,
    pub signals: &'a HashMap<String, Value>,
    pub scales: &'a HashMap<String, Scale>,
    /// Group-local datasets (notably a facet's partitioned data).
    pub local: Option<&'a HashMap<String, Vec<Value>>>,
    pub group_width: f64,
    pub group_height: f64,
    pub local_scales: Option<&'a HashMap<String, Scale>>,
}

impl<'a> Env<'a> {
    fn get_scale(&self, name: &str) -> Option<&Scale> {
        self.local_scales
            .and_then(|m| m.get(name))
            .or_else(|| self.scales.get(name))
    }
}

impl<'a> Environment for Env<'a> {
    fn signal(&self, name: &str) -> Option<Value> {
        // Nested group scopes shadow the view-level width/height signals, but
        // the root scope must keep the signal values used by axes/layout.
        if self.local.is_some() {
            if name == "width" {
                let base = self.signals.get("width").and_then(|v| v.as_f64());
                if base != Some(self.group_width) {
                    return Some(json_number(self.group_width));
                }
            }
            if name == "height" {
                let base = self.signals.get("height").and_then(|v| v.as_f64());
                if base != Some(self.group_height) {
                    return Some(json_number(self.group_height));
                }
            }
        }
        self.signals.get(name).cloned()
    }
    fn dataset(&self, name: &str) -> Option<Vec<Value>> {
        self.local
            .and_then(|l| l.get(name).cloned())
            .or_else(|| self.data.get(name).cloned())
    }
    fn scale(&self, name: &str, value: &Value) -> Option<Value> {
        let s = self.get_scale(name)?;
        let v = s.scale(value);
        if v.is_null() {
            None
        } else {
            Some(v)
        }
    }
    fn scale_domain(&self, name: &str) -> Option<Vec<Value>> {
        self.get_scale(name).map(|s| s.domain.clone())
    }
    fn scale_range(&self, name: &str) -> Option<Vec<Value>> {
        self.get_scale(name).map(|s| s.range_numeric())
    }
    fn bandwidth(&self, name: &str) -> Option<f64> {
        self.get_scale(name).map(|s| s.bandwidth)
    }
    fn invert(&self, name: &str, value: f64) -> Option<Value> {
        self.get_scale(name).map(|s| s.invert(value))
    }
}

/// Resolves top-level signals lazily, with cycle detection.
struct SignalResolver<'a> {
    specs: &'a HashMap<String, Value>,
    base: &'a HashMap<String, Value>,
    data: &'a DataStore,
    domains: &'a HashMap<String, Vec<Value>>,
    scales: &'a HashMap<String, Scale>,
    values: RefCell<HashMap<String, Value>>,
    active: RefCell<HashSet<String>>,
}

impl<'a> Environment for SignalResolver<'a> {
    fn signal(&self, name: &str) -> Option<Value> {
        self.resolve(name)
    }
    fn dataset(&self, name: &str) -> Option<Vec<Value>> {
        self.data.get(name).cloned()
    }
    fn scale(&self, name: &str, value: &Value) -> Option<Value> {
        self.scales.get(name).and_then(|s| {
            let v = s.scale(value);
            if v.is_null() {
                None
            } else {
                Some(v)
            }
        })
    }
    fn scale_domain(&self, name: &str) -> Option<Vec<Value>> {
        self.domains
            .get(name)
            .cloned()
            .or_else(|| self.scales.get(name).map(|s| s.domain.clone()))
    }
    fn scale_range(&self, name: &str) -> Option<Vec<Value>> {
        self.scales.get(name).map(|s| s.range_numeric())
    }
    fn bandwidth(&self, name: &str) -> Option<f64> {
        self.scales.get(name).map(|s| s.bandwidth)
    }
    fn invert(&self, name: &str, value: f64) -> Option<Value> {
        self.scales.get(name).map(|s| s.invert(value))
    }
}

impl<'a> SignalResolver<'a> {
    fn resolve(&self, name: &str) -> Option<Value> {
        if let Some(v) = self.values.borrow().get(name) {
            return Some(v.clone());
        }
        let Some(spec) = self.specs.get(name) else {
            return self.base.get(name).cloned();
        };
        if !self.active.borrow_mut().insert(name.to_string()) {
            return self.base.get(name).cloned();
        }
        let value = self.eval_spec(spec);
        self.active.borrow_mut().remove(name);
        self.values
            .borrow_mut()
            .insert(name.to_string(), value.clone());
        Some(value)
    }

    fn eval_spec(&self, spec: &Value) -> Value {
        let expr = spec
            .get("update")
            .and_then(|v| v.as_str())
            .or_else(|| spec.get("init").and_then(|v| v.as_str()));
        if let Some(expr) = expr {
            match expression::evaluate(expr, None, self) {
                Ok(v) => return v,
                Err(_) => {
                    if let Some(v) = spec.get("value") {
                        return v.clone();
                    }
                    return Value::Null;
                }
            }
        }
        if let Some(value) = spec.get("value") {
            if let Some(sig) = value.get("signal").and_then(|v| v.as_str()) {
                return expression::evaluate(sig, None, self).unwrap_or(Value::Null);
            }
            return value.clone();
        }
        if let Some(value) = spec.get("init") {
            return value.clone();
        }
        Value::Null
    }

    fn resolve_all(&self) -> HashMap<String, Value> {
        let mut names: Vec<String> = self.specs.keys().cloned().collect();
        for name in self.base.keys() {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
        for name in names {
            self.resolve(&name);
        }
        self.values.borrow().clone()
    }
}

fn signal_specs(spec: &Value) -> HashMap<String, Value> {
    let mut out = HashMap::new();
    if let Some(signals) = spec.get("signals").and_then(|s| s.as_array()) {
        for s in signals {
            if let Some(name) = s.get("name").and_then(|n| n.as_str()) {
                out.insert(name.to_string(), s.clone());
            }
        }
    }
    out
}

fn constant_signal_values(specs: &HashMap<String, Value>) -> HashMap<String, Value> {
    let mut out = HashMap::new();
    for (name, s) in specs {
        if let Some(v) = s.get("value") {
            if v.get("signal").is_none() {
                out.insert(name.clone(), v.clone());
            }
        }
    }
    out
}

fn scale_specs(spec: &Value) -> IndexMap<String, Value> {
    let mut out = IndexMap::new();
    if let Some(scales) = spec.get("scales").and_then(|s| s.as_array()) {
        for s in scales {
            if let Some(name) = s.get("name").and_then(|n| n.as_str()) {
                out.insert(name.to_string(), s.clone());
            }
        }
    }
    out
}

/// Hashable equivalent of [`crate::value::loose_eq`] for domain values.
#[derive(Hash, PartialEq, Eq)]
enum DomainKey {
    Null,
    Number(u64),
    String(String),
    Other(String),
}

impl DomainKey {
    fn from_value(value: &Value) -> Self {
        match value {
            Value::Null => DomainKey::Null,
            Value::Bool(b) => DomainKey::Number((if *b { 1.0f64 } else { 0.0 }).to_bits()),
            Value::Number(n) => {
                let f = n.as_f64().unwrap_or(f64::NAN);
                DomainKey::Number(if f == 0.0 { 0.0f64 } else { f }.to_bits())
            }
            Value::String(s) => {
                let t = s.trim();
                if !t.is_empty() {
                    if let Ok(f) = t.parse::<f64>() {
                        return DomainKey::Number(if f == 0.0 { 0.0f64 } else { f }.to_bits());
                    }
                    if let Some(ms) = crate::format::parse_date(t) {
                        return DomainKey::Number(ms.to_bits());
                    }
                }
                DomainKey::String(s.clone())
            }
            other => DomainKey::Other(
                serde_json::to_string(other).unwrap_or_else(|_| "null".to_string()),
            ),
        }
    }
}

fn compute_domains(
    specs: &IndexMap<String, Value>,
    data: &DataStore,
    signals: &HashMap<String, Value>,
) -> HashMap<String, Vec<Value>> {
    let mut out = HashMap::new();
    for (name, s) in specs {
        if let Ok(mut d) = scale::resolve_domain(s.get("domain"), data, signals) {
            let kind = Scale::kind_from_spec(s);
            if kind.is_discrete() {
                // Deduplicate with a hash set: domains can contain thousands
                // of categories and the previous quadratic scan dominated
                // `Runtime::new` for high-cardinality band/ordinal scales.
                let mut seen: std::collections::HashSet<DomainKey> =
                    std::collections::HashSet::with_capacity(d.len());
                let mut unique: Vec<Value> = Vec::with_capacity(d.len());
                for v in d {
                    if seen.insert(DomainKey::from_value(&v)) {
                        unique.push(v);
                    }
                }
                d = unique;
            }
            out.insert(name.clone(), d);
        }
    }
    out
}

fn eval_param(value: &Value, env: &Env) -> Value {
    match value {
        Value::Object(o) if o.get("signal").and_then(|s| s.as_str()).is_some() => {
            let expr = o.get("signal").and_then(|s| s.as_str()).unwrap_or("");
            expression::evaluate(expr, None, env).unwrap_or(Value::Null)
        }
        Value::Array(a) => Value::Array(a.iter().map(|v| eval_param(v, env)).collect()),
        other => other.clone(),
    }
}

fn featurize(value: &Value) -> Vec<Value> {
    match value {
        Value::Array(items) => items.iter().flat_map(featurize).collect(),
        Value::Object(o) => {
            if o.get("type").and_then(|t| t.as_str()) == Some("FeatureCollection") {
                o.get("features")
                    .and_then(|f| f.as_array())
                    .cloned()
                    .unwrap_or_default()
            } else if o.get("type").and_then(|t| t.as_str()) == Some("Feature") {
                vec![value.clone()]
            } else if o.contains_key("type") && o.contains_key("coordinates") {
                vec![json!({"type": "Feature", "geometry": value.clone()})]
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

fn collect_geojson(value: &Value) -> Value {
    let items: Vec<Value> = match value {
        Value::Array(a) => a.clone(),
        other => vec![other.clone()],
    };
    if items.len() == 1 {
        return items[0].clone();
    }
    let mut features = Vec::new();
    for item in items {
        features.extend(featurize(&item));
    }
    json!({"type": "FeatureCollection", "features": features})
}

fn build_projections(
    spec: &Value,
    store: &DataStore,
    signals: &HashMap<String, Value>,
    scales: &HashMap<String, Scale>,
) -> HashMap<String, Projection> {
    let mut out = HashMap::new();
    let width = number_signal(signals, "width", spec.get("width"));
    let height = number_signal(signals, "height", spec.get("height"));
    let env = Env {
        data: store,
        signals,
        scales,
        local: None,
        group_width: width,
        group_height: height,
        local_scales: None,
    };
    let Some(projections) = spec.get("projections").and_then(|p| p.as_array()) else {
        return out;
    };
    for def in projections {
        let Some(name) = def.get("name").and_then(|n| n.as_str()) else {
            continue;
        };
        let type_name = def
            .get("type")
            .map(|v| eval_param(v, &env))
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "mercator".to_string());
        let Some(mut projection) = Projection::from_type(&type_name) else {
            continue;
        };
        for key in [
            "clipAngle",
            "clipExtent",
            "scale",
            "translate",
            "center",
            "rotate",
            "parallels",
            "precision",
            "reflectX",
            "reflectY",
        ] {
            let Some(raw) = def.get(key) else { continue };
            let value = eval_param(raw, &env);
            match key {
                "clipAngle" => {
                    let angle = value.as_f64().unwrap_or(0.0);
                    projection.clip_angle = if angle != 0.0 { Some(angle) } else { None };
                }
                "clipExtent" => projection.set_clip_extent(&value),
                "scale" => {
                    if let Some(scale) = value.as_f64() {
                        projection.scale = scale;
                    }
                }
                "translate" => projection.set_translate(&value),
                "center" => projection.set_center(&value),
                "rotate" => projection.set_rotate(&value),
                "parallels" => projection.set_parallels(&value),
                "precision" => {
                    if let Some(precision) = value.as_f64() {
                        projection.precision = precision;
                    }
                }
                "reflectX" => {
                    projection.reflect_x = value.as_bool().unwrap_or(false);
                }
                "reflectY" => {
                    projection.reflect_y = value.as_bool().unwrap_or(false);
                }
                _ => {}
            }
        }
        if let Some(raw_fit) = def.get("fit") {
            let fit_value = eval_param(raw_fit, &env);
            let object = collect_geojson(&fit_value);
            if let Some(raw_extent) = def.get("extent") {
                let extent = eval_param(raw_extent, &env);
                if let Some(e) = extent_from_value(&extent) {
                    projection.fit_extent(e, &object);
                }
            } else if let Some(raw_size) = def.get("size") {
                let size = eval_param(raw_size, &env);
                if let Some(s) = size.as_array() {
                    let w = s.first().and_then(|v| v.as_f64()).unwrap_or(width);
                    let h = s.get(1).and_then(|v| v.as_f64()).unwrap_or(height);
                    projection.fit_size([w, h], &object);
                }
            }
        }
        out.insert(name.to_string(), projection);
    }
    out
}

fn extent_from_value(value: &Value) -> Option<[[f64; 2]; 2]> {
    let a = value.as_array()?;
    let p0 = a.first()?.as_array()?;
    let p1 = a.get(1)?.as_array()?;
    Some([
        [
            p0.first()?.as_f64()?,
            p0.get(1)?.as_f64()?,
        ],
        [
            p1.first()?.as_f64()?,
            p1.get(1)?.as_f64()?,
        ],
    ])
}

fn apply_geopoints(
    spec: &Value,
    store: &mut DataStore,
    projections: &HashMap<String, Projection>,
    signals: &HashMap<String, Value>,
) {
    let Some(entries) = spec.get("data").and_then(|d| d.as_array()) else {
        return;
    };
    for entry in entries {
        let Some(transforms) = entry.get("transform").and_then(|t| t.as_array()) else {
            continue;
        };
        for transform in transforms {
            if transform.get("type").and_then(|t| t.as_str()) != Some("geopoint") {
                continue;
            }
            let Some(name) = transform.get("projection").and_then(|p| p.as_str()) else {
                continue;
            };
            let Some(projection) = projections.get(name) else {
                continue;
            };
            let Some(fields) = transform.get("fields").and_then(|f| f.as_array()) else {
                continue;
            };
            let lon = fields.first().cloned().unwrap_or(Value::Null);
            let lat = fields.get(1).cloned().unwrap_or(Value::Null);
            let scales = HashMap::new();
            let empty_store = DataStore::default();
            let env = Env {
                data: &empty_store,
                signals,
                scales: &scales,
                local: None,
                group_width: number_signal(signals, "width", spec.get("width")),
                group_height: number_signal(signals, "height", spec.get("height")),
                local_scales: None,
            };
            let as_ = transform
                .get("as")
                .and_then(|a| a.as_array())
                .cloned()
                .unwrap_or_else(|| vec![json!("x"), json!("y")]);
            let x_name = as_.first().and_then(|v| v.as_str()).unwrap_or("x");
            let y_name = as_.get(1).and_then(|v| v.as_str()).unwrap_or("y");
            for values in store.sets.values_mut() {
                for row in values.iter_mut() {
                    let lon_value = if let Some(expr) = lon.get("expr").and_then(|v| v.as_str()) {
                        expression::evaluate(expr, Some(row), &env).unwrap_or(Value::Null)
                    } else {
                        field_value(row, &lon)
                    };
                    let lat_value = if let Some(expr) = lat.get("expr").and_then(|v| v.as_str()) {
                        expression::evaluate(expr, Some(row), &env).unwrap_or(Value::Null)
                    } else {
                        field_value(row, &lat)
                    };
                    let lon = crate::value::to_number(&lon_value);
                    let lat = crate::value::to_number(&lat_value);
                    if lon.is_finite() && lat.is_finite() {
                        if let Some(p) = projection.project(lon, lat) {
                            set_field(row, x_name, json_number(p[0]));
                            set_field(row, y_name, json_number(p[1]));
                        } else {
                            set_field(row, x_name, Value::Null);
                            set_field(row, y_name, Value::Null);
                        }
                    }
                }
            }
        }
    }
}

fn build_scales(
    specs: &IndexMap<String, Value>,
    data: &DataStore,
    signals: &HashMap<String, Value>,
) -> Result<HashMap<String, Scale>, Error> {
    let empty = HashMap::new();
    let mut out = HashMap::new();
    for (name, s) in specs {
        // Scales may depend on earlier scales through signals such as
        // `bandwidth('y')`, so expose the ones already built to the resolver.
        let env = Env {
            data,
            signals,
            scales: if out.is_empty() { &empty } else { &out },
            local: None,
            group_width: 0.0,
            group_height: 0.0,
            local_scales: None,
        };
        let scale = scale::build_scale(name, s, data, signals, &env)?;
        out.insert(name.clone(), scale);
    }
    Ok(out)
}

fn number_signal(signals: &HashMap<String, Value>, name: &str, spec_value: Option<&Value>) -> f64 {
    if let Some(Value::Number(n)) = spec_value {
        if let Some(f) = n.as_f64() {
            return f;
        }
    }
    signals.get(name).and_then(|v| v.as_f64()).unwrap_or(0.0)
}

#[derive(Debug, Clone, Default)]
struct Padding {
    left: f64,
    right: f64,
    top: f64,
    bottom: f64,
}

fn parse_padding(spec: &Value, config: &Value) -> Padding {
    let p = spec
        .get("padding")
        .or_else(|| config.get("padding"))
        .cloned()
        .unwrap_or(Value::Null);
    match p {
        Value::Number(n) => {
            let v = n.as_f64().unwrap_or(0.0);
            Padding {
                left: v,
                right: v,
                top: v,
                bottom: v,
            }
        }
        Value::Object(m) => Padding {
            left: m.get("left").and_then(|v| v.as_f64()).unwrap_or(0.0),
            right: m.get("right").and_then(|v| v.as_f64()).unwrap_or(0.0),
            top: m.get("top").and_then(|v| v.as_f64()).unwrap_or(0.0),
            bottom: m.get("bottom").and_then(|v| v.as_f64()).unwrap_or(0.0),
        },
        _ => Padding::default(),
    }
}

fn config_value(config: &Value, section: &str, key: &str) -> Option<Value> {
    config.get(section).and_then(|s| s.get(key)).cloned()
}

fn cfg_str(config: &Value, section: &str, key: &str, default: &str) -> String {
    config_value(config, section, key)
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| default.to_string())
}

fn cfg_num(config: &Value, section: &str, key: &str, default: f64) -> f64 {
    config_value(config, section, key)
        .and_then(|v| v.as_f64())
        .unwrap_or(default)
}

fn cfg_bool(config: &Value, section: &str, key: &str, default: bool) -> bool {
    config_value(config, section, key)
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}

fn title_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Array(a) => Some(a.iter().map(to_string_js).collect::<Vec<_>>().join(" ")),
        Value::Object(o) => o.get("text").and_then(title_text),
        _ => None,
    }
}

struct AxisInfo {
    spec: Value,
    orient: String,
    scale_name: String,
    grid: bool,
    grid_scale: Option<String>,
    show_domain: bool,
    show_ticks: bool,
    show_labels: bool,
    ticks: Vec<Value>,
    labels: Vec<String>,
    title: Option<String>,
    label_angle: f64,
    label_align: Option<String>,
    label_baseline: Option<String>,
    label_flush_threshold: Option<f64>,
    tick_size: f64,
    label_padding: f64,
    title_padding: f64,
    offset: f64,
    min_extent: f64,
    max_extent: f64,
    font_size: f64,
    title_font_size: f64,
    title_limit: f64,
    label_overlap: bool,
    label_overlap_method: String,
    label_separation: f64,
    label_limit: f64,
    tick_offset: f64,
    label_offset: f64,
    title_x: Option<f64>,
    title_y: Option<f64>,
    title_angle: Option<f64>,
    title_align: Option<String>,
    title_baseline: Option<String>,
    bounds: crate::bounds::Bounds,
    axis_s: f64,
    axis_range: f64,
    axis_mid: f64,
    band_position: f64,
    grid_color: String,
    tick_color: String,
    domain_color: String,
    zindex: f64,
}

fn eval_signal_value(v: &Value, env: &Env) -> Option<Value> {
    match v {
        Value::Object(o) => {
            if let Some(sig) = o.get("signal").and_then(|s| s.as_str()) {
                expression::evaluate(sig, None, env).ok()
            } else if let Some(value) = o.get("value") {
                Some(value.clone())
            } else {
                None
            }
        }
        other => Some(other.clone()),
    }
}

fn axis_local_bounds(info: &AxisInfo, scale: &Scale) -> (crate::bounds::Bounds, f64, f64) {
    use crate::bounds::{rule_bounds, text_bounds, Bounds};
    let mut b = Bounds::empty();
    let mut out_s = 0.0;
    let mut out_range = 0.0;
    if !info.grid {
        if info.show_ticks {
            for v in &info.ticks {
                let p = tick_pos(scale, v);
                let r = if info.orient == "left" || info.orient == "right" {
                    rule_bounds(
                        0.0,
                        p,
                        -if info.orient == "left" {
                            info.tick_size
                        } else {
                            -info.tick_size
                        },
                        p,
                        1.0,
                    )
                } else {
                    let sign = if info.orient == "top" { -1.0 } else { 1.0 };
                    rule_bounds(p, 0.0, p, sign * info.tick_size, 1.0)
                };
                b.union(r);
            }
        }
        if info.show_labels {
            for (i, v) in info.ticks.iter().enumerate() {
                let Some(label) = info.labels.get(i) else {
                    continue;
                };
                let p = tick_pos(scale, v);
                let sign = if info.orient == "left" || info.orient == "top" {
                    -1.0
                } else {
                    1.0
                };
                let mut align = info.label_align.clone().unwrap_or_else(|| {
                    if info.orient == "left" || info.orient == "right" {
                        if info.orient == "left" {
                            "right".to_string()
                        } else {
                            "left".to_string()
                        }
                    } else {
                        "center".to_string()
                    }
                });
                let baseline =
                    info.label_baseline
                        .clone()
                        .unwrap_or_else(|| match info.orient.as_str() {
                            "top" => "bottom".to_string(),
                            "bottom" => "top".to_string(),
                            _ => "middle".to_string(),
                        });
                if let Some(threshold) = info.label_flush_threshold {
                    if (info.orient == "bottom" || info.orient == "top")
                        && info.label_align.is_none()
                    {
                        align = flush_align(scale, p, threshold)
                            .unwrap_or_else(|| "center".to_string());
                    }
                }
                let _ = i;
                let (x, y) = if info.orient == "left" || info.orient == "right" {
                    (sign * (info.tick_size + info.label_padding), p)
                } else {
                    (p, sign * (info.tick_size + info.label_padding))
                };
                let label = truncate_text(label, info.font_size, info.label_limit, "\u{2026}");
                b.union(text_bounds(
                    x,
                    y,
                    &label,
                    info.font_size,
                    Some(&align),
                    Some(&baseline),
                    0.0,
                    0.0,
                    info.label_angle,
                ));
            }
        }
        let range = scale.range_numeric();
        let r0 = range.first().map(to_number).unwrap_or(0.0);
        let r1 = range.last().map(to_number).unwrap_or(0.0);
        let range = (r1 - r0).abs();
        out_range = range;
        let mid = (r0 + r1) / 2.0;
        let clamp = |v: f64| v.max(info.min_extent).min(info.max_extent);
        let (s, title_y, title_x, title_baseline) = match info.orient.as_str() {
            "bottom" => {
                let s = clamp(b.y2.max(0.0));
                b.add(0.0, 0.0);
                b.add(range, s);
                (s, s + info.title_padding, mid, "top")
            }
            "top" => {
                let s = clamp((-b.y1).max(0.0));
                b.add(0.0, -s);
                b.add(range, 0.0);
                (
                    s,
                    -(s + info.title_padding + info.title_font_size),
                    mid,
                    "top",
                )
            }
            "left" => {
                let s = clamp((-b.x1).max(0.0));
                b.add(-s, 0.0);
                b.add(0.0, range);
                (s, mid, -(s + info.title_padding), "bottom")
            }
            _ => {
                let s = clamp(b.x2.max(0.0));
                b.add(0.0, 0.0);
                b.add(s, range);
                (s, mid, s + info.title_padding, "bottom")
            }
        };
        out_s = s;
        if let Some(title) = &info.title {
            let angle = match info.orient.as_str() {
                "left" => -90.0,
                "right" => 90.0,
                _ => 0.0,
            };
            b.union(text_bounds(
                title_x,
                title_y,
                title,
                info.title_font_size,
                Some("center"),
                Some(title_baseline),
                0.0,
                0.0,
                angle,
            ));
        }
    }
    (b, out_s, out_range)
}

fn build_axis_info(
    spec: &Value,
    config: &Value,
    env: &Env,
    width: f64,
    height: f64,
) -> Option<AxisInfo> {
    let scale_name = spec.get("scale").and_then(|s| s.as_str())?.to_string();
    let scale = env.get_scale(&scale_name)?;
    let orient = spec
        .get("orient")
        .and_then(|v| v.as_str())
        .unwrap_or("bottom")
        .to_string();
    let grid = spec
        .get("grid")
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| cfg_bool(config, "axis", "grid", false));
    let grid_scale = spec
        .get("gridScale")
        .and_then(|v| v.as_str())
        .map(String::from);
    let domain = spec
        .get("domain")
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| cfg_bool(config, "axis", "domain", true));
    let labels_on = spec
        .get("labels")
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| cfg_bool(config, "axis", "labels", true));
    let ticks_on = spec
        .get("ticks")
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| cfg_bool(config, "axis", "ticks", true));
    let count = spec
        .get("tickCount")
        .and_then(|v| eval_signal_value(v, env))
        .and_then(|v| v.as_f64())
        .unwrap_or(10.0);
    let min_step = spec
        .get("tickMinStep")
        .and_then(|v| eval_signal_value(v, env))
        .and_then(|v| v.as_f64())
        .or_else(|| config_value(config, "axis", "tickMinStep").and_then(|v| v.as_f64()));
    let count = if let Some(min_step) = min_step.filter(|v| *v > 0.0) {
        let lo = scale.d0.min(scale.d1);
        let hi = scale.d0.max(scale.d1);
        let n = ((hi - lo) / min_step).floor().max(1.0) + 1.0;
        count.min(n)
    } else {
        count
    };
    let mut ticks = if let Some(values) = spec.get("values").and_then(|v| v.as_array()) {
        values.clone()
    } else {
        scale.ticks(count)
    };
    // d3 time intervals are half-open: a tick exactly at the domain maximum
    // is not generated. Mirror that so axes stop one interval earlier.
    if matches!(
        scale.kind,
        crate::scale::ScaleKind::Time | crate::scale::ScaleKind::Utc
    ) && ticks.len() > 1
    {
        let last = ticks
            .last()
            .map(crate::value::to_number)
            .unwrap_or(f64::NAN);
        if (last - scale.d1).abs() < 1e-6 && scale.d1 > scale.d0 {
            ticks.pop();
        }
    }
    let format = spec
        .get("format")
        .and_then(|v| eval_signal_value(v, env))
        .and_then(|v| match v {
            Value::String(s) => Some(s),
            Value::Null => None,
            other => Some(to_string_js(&other)),
        });
    let format_type = spec
        .get("formatType")
        .and_then(|v| eval_signal_value(v, env))
        .and_then(|v| match v {
            Value::String(s) => Some(s),
            Value::Null => None,
            other => Some(to_string_js(&other)),
        });
    let inferred_format: Option<String> = if format.is_none()
        && format_type.is_none()
        && !matches!(
            scale.kind,
            crate::scale::ScaleKind::Time
                | crate::scale::ScaleKind::Utc
                | crate::scale::ScaleKind::Log
        )
        && !scale.is_discrete()
    {
        let mut nums: Vec<f64> = ticks
            .iter()
            .map(crate::value::to_number)
            .filter(|v| v.is_finite())
            .collect();
        nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let step = nums
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .filter(|d| *d > 0.0)
            .fold(f64::INFINITY, f64::min);
        if step.is_finite() && step < 1.0 {
            let decimals = (-step.log10()).ceil().max(0.0) as usize;
            Some(format!(",.{}f", decimals.min(6)))
        } else {
            Some(",d".to_string())
        }
    } else {
        None
    };
    let label_format = format.or(inferred_format);
    let mut labels: Vec<String> = if labels_on {
        ticks
            .iter()
            .map(|v| scale.label(v, label_format.as_deref(), format_type.as_deref()))
            .collect()
    } else {
        vec![]
    };
    if let Some(text_enc) = spec
        .get("encode")
        .and_then(|e| e.get("labels"))
        .and_then(|l| l.get("update"))
        .and_then(|u| u.get("text"))
    {
        if let Some(signal) = text_enc.get("signal").and_then(|s| s.as_str()) {
            for (i, label) in labels.iter_mut().enumerate() {
                let value = ticks.get(i).cloned().unwrap_or(Value::Null);
                let datum = json!({"value": value, "label": *label, "index": i});
                if let Ok(v) = expression::evaluate(signal, Some(&datum), env) {
                    if let Some(s) = v.as_str() {
                        *label = s.to_string();
                    } else if !v.is_null() {
                        *label = to_string_js(&v);
                    }
                }
            }
        }
    }
    let title = spec.get("title").and_then(title_text);
    let label_angle = spec
        .get("labelAngle")
        .and_then(|v| v.as_f64())
        .unwrap_or_else(|| cfg_num(config, "axis", "labelAngle", 0.0));
    let font_size = spec
        .get("labelFontSize")
        .and_then(|v| v.as_f64())
        .unwrap_or_else(|| {
            config_value(config, "style", "guide-label")
                .and_then(|s| s.get("fontSize").and_then(|v| v.as_f64()))
                .unwrap_or(10.0)
        });
    let title_font_size = spec
        .get("titleFontSize")
        .and_then(|v| v.as_f64())
        .unwrap_or_else(|| {
            config_value(config, "style", "guide-title")
                .and_then(|s| s.get("fontSize").and_then(|v| v.as_f64()))
                .unwrap_or(11.0)
        });
    let axis_section = if orient == "top" || orient == "bottom" {
        "axisX"
    } else {
        "axisY"
    };
    // Vega's axis config has no default `titleLimit` (only legends do), so an
    // axis title is only truncated when the limit is set explicitly.
    let title_limit = spec
        .get("titleLimit")
        .and_then(|v| v.as_f64())
        .or_else(|| config_value(config, axis_section, "titleLimit").and_then(|v| v.as_f64()))
        .or_else(|| config_value(config, "axis", "titleLimit").and_then(|v| v.as_f64()))
        .unwrap_or(0.0);
    let title = title.map(|t| truncate_text(&t, title_font_size, title_limit, "\u{2026}"));
    let tick_size = if ticks_on {
        spec.get("tickSize")
            .and_then(|v| v.as_f64())
            .unwrap_or_else(|| cfg_num(config, "axis", "tickSize", 5.0))
    } else {
        0.0
    };
    let label_padding = spec
        .get("labelPadding")
        .and_then(|v| v.as_f64())
        .unwrap_or_else(|| cfg_num(config, "axis", "labelPadding", 2.0));
    let title_padding = spec
        .get("titlePadding")
        .and_then(|v| v.as_f64())
        .unwrap_or_else(|| cfg_num(config, "axis", "titlePadding", 4.0));
    let offset = spec.get("offset").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let min_extent = spec
        .get("minExtent")
        .and_then(|v| v.as_f64())
        .unwrap_or_else(|| cfg_num(config, "axis", "minExtent", 0.0));
    let max_extent = spec
        .get("maxExtent")
        .and_then(|v| v.as_f64())
        .unwrap_or_else(|| cfg_num(config, "axis", "maxExtent", 200.0));
    let flush_value = spec
        .get("labelFlush")
        .cloned()
        .or_else(|| config_value(config, "axis", "labelFlush"));
    let label_flush_threshold = match flush_value {
        Some(Value::Bool(true)) => Some(1.0),
        Some(Value::Number(n)) => n.as_f64(),
        Some(v) if truthy(&v) => Some(1.0),
        _ => None,
    };
    let overlap_value = spec
        .get("labelOverlap")
        .cloned()
        .or_else(|| config_value(config, "axis", "labelOverlap"));
    let label_overlap = overlap_value.as_ref().map(truthy).unwrap_or(false);
    let label_overlap_method = match overlap_value.as_ref() {
        Some(Value::String(s)) => s.clone(),
        _ => "parity".to_string(),
    };
    let label_separation = spec
        .get("labelSeparation")
        .and_then(|v| v.as_f64())
        .or_else(|| config_value(config, "axis", "labelSeparation").and_then(|v| v.as_f64()))
        .unwrap_or(0.0);
    let label_limit = spec
        .get("labelLimit")
        .and_then(|v| v.as_f64())
        .or_else(|| config_value(config, "axis", "labelLimit").and_then(|v| v.as_f64()))
        .unwrap_or(180.0);
    let tick_offset = spec
        .get("tickOffset")
        .and_then(|v| v.as_f64())
        .unwrap_or_else(|| {
            if scale.kind == crate::scale::ScaleKind::Band {
                cfg_num(
                    config,
                    "axisBand",
                    "tickOffset",
                    cfg_num(config, "axis", "tickOffset", 0.0),
                )
            } else {
                cfg_num(config, "axis", "tickOffset", 0.0)
            }
        });
    let label_offset = spec
        .get("labelOffset")
        .and_then(|v| v.as_f64())
        .or_else(|| config_value(config, "axis", "labelOffset").and_then(|v| v.as_f64()))
        .unwrap_or(0.0);
    let title_x = spec.get("titleX").and_then(|v| v.as_f64());
    let title_y = spec.get("titleY").and_then(|v| v.as_f64());
    let title_angle = spec.get("titleAngle").and_then(|v| v.as_f64());
    let title_align = spec
        .get("titleAlign")
        .and_then(|v| v.as_str())
        .map(String::from);
    let title_baseline = spec
        .get("titleBaseline")
        .and_then(|v| v.as_str())
        .map(String::from);
    let label_align = spec
        .get("labelAlign")
        .and_then(|v| v.as_str())
        .map(String::from);
    let label_baseline = spec
        .get("labelBaseline")
        .and_then(|v| v.as_str())
        .map(String::from);

    let _ = (width, height, domain);
    let mut axis_info = AxisInfo {
        spec: spec.clone(),
        orient,
        scale_name: scale_name.clone(),
        grid,
        grid_scale,
        show_domain: domain,
        show_ticks: ticks_on,
        show_labels: labels_on,
        ticks,
        labels,
        title,
        label_angle,
        label_align,
        label_baseline,
        label_flush_threshold,
        tick_size,
        label_padding,
        title_padding,
        offset,
        min_extent,
        max_extent,
        font_size,
        title_font_size,
        title_limit,
        label_overlap,
        label_overlap_method,
        label_separation,
        label_limit,
        tick_offset,
        label_offset,
        title_x,
        title_y,
        title_angle,
        title_align,
        title_baseline,
        bounds: crate::bounds::Bounds::default(),
        axis_s: 0.0,
        axis_range: 0.0,
        axis_mid: 0.0,
        zindex: spec.get("zindex").and_then(|v| v.as_f64()).unwrap_or(0.0),
        grid_color: spec
            .get("gridColor")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| cfg_str(config, "axis", "gridColor", "#ddd")),
        tick_color: spec
            .get("tickColor")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| cfg_str(config, "axis", "tickColor", "#888")),
        domain_color: spec
            .get("domainColor")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| cfg_str(config, "axis", "domainColor", "#888")),
        band_position: {
            // `tickBand: "extent"` pins grid/ticks to band ends and emits the
            // extra boundary tick that closes a band axis.
            let tick_band = spec
                .get("tickBand")
                .cloned()
                .or_else(|| config_value(config, "axis", "tickBand"));
            if tick_band.as_ref().and_then(|v| v.as_str()) == Some("extent")
                && scale.is_discrete()
            {
                1.0
            } else {
                config_value(config, "axisBand", "bandPosition")
                    .or_else(|| config_value(config, "axis", "bandPosition"))
                    .or_else(|| spec.get("bandPosition").cloned())
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.5)
            }
        },
    };
    if let Some(scale) = env.get_scale(&scale_name) {
        let (b, s, range) = axis_local_bounds(&axis_info, scale);
        axis_info.bounds = b;
        axis_info.axis_s = s;
        axis_info.axis_range = range;
        let rn = scale::range_pair(scale);
        axis_info.axis_mid = (rn.0 + rn.1) / 2.0;
    }
    Some(axis_info)
}

#[derive(Clone)]
struct LegendEntry {
    label: String,
    fill: Value,
    stroke: Value,
    size: f64,
    shape: String,
    opacity: Option<f64>,
}

struct LegendInfo {
    orient: String,
    title: Option<Vec<String>>,
    entries: Vec<LegendEntry>,
    gradient: Option<Vec<String>>,
    width: f64,
    height: f64,
    gradient_length: f64,
    gradient_thickness: f64,
    font_size: f64,
    title_font_size: f64,
    /// Properties from `legend.encode.symbols.update` applied to every symbol.
    symbol_props: Map<String, Value>,
    /// Labels of a continuous gradient legend.
    gradient_labels: Vec<String>,
    gradient_label_perc: Vec<f64>,
    /// Discrete band legend (bin-ordinal scales).
    band_legend: bool,
    /// Legend symbols are line segments (`symbolType: "stroke"`).
    stroke_symbol: bool,
    /// Width/height of a single symbol legend entry row.
    entry_width: f64,
    entry_height: f64,
    /// Per-entry widths (horizontal top/bottom legends position entries).
    entry_widths: Vec<f64>,
    /// True for top/bottom legends laid out horizontally.
    horizontal: bool,
    /// Size legends use per-entry rows with variable symbol diameters.
    size_legend: bool,
    entry_ys: Vec<f64>,
    entry_heights: Vec<f64>,
    entry_symbol_ys: Vec<f64>,
    size_symbol_x: f64,
    size_label_x: f64,
    size_symbol_xs: Vec<f64>,
    size_label_xs: Vec<f64>,
    entry_xs: Vec<f64>,
    stroke_width_legend: bool,
    /// Vertical space reserved above the entries when a title is present.
    title_height: f64,
    /// Horizontal legends place the title beside the entries (`titleOrient: left`).
    title_on_side: bool,
    /// Space reserved for the title when it is beside the entries.
    title_width: f64,
    /// Corner legend offset from the view bounds.
    offset: f64,
    /// Explicit `legendX` / `legendY` position (legend orient `none`).
    legend_x: Option<f64>,
    legend_y: Option<f64>,
}

/// Port of vega-scenegraph's `truncate()` helper (uses the estimator width).
pub fn truncate_text(text: &str, font_size: f64, limit: f64, ellipsis: &str) -> String {
    if limit <= 0.0 {
        return text.to_string();
    }
    let ellipsis_w = text_width(ellipsis, font_size);
    if text_width(text, font_size) < limit {
        return text.to_string();
    }
    let mut budget = limit - ellipsis_w;
    let chars: Vec<char> = text.chars().collect();
    let mut lo = 0usize;
    let mut hi = chars.len();
    while lo < hi {
        let mid = 1 + (lo + hi) / 2;
        let prefix: String = chars[..mid].iter().collect();
        if text_width(&prefix, font_size) < budget {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let _ = &mut budget;
    let prefix: String = chars[..lo].iter().collect();
    format!("{prefix}{ellipsis}")
}

fn numeric_legend_label(scale: &Scale, value: &Value, label: String) -> String {
    use crate::scale::ScaleKind;
    if matches!(scale.kind, ScaleKind::Time | ScaleKind::Utc | ScaleKind::Band | ScaleKind::Point | ScaleKind::Ordinal) {
        return label;
    }
    let is_plain_int = !label.is_empty()
        && label.len() >= 4
        && label
            .chars()
            .all(|c| c.is_ascii_digit() || c == '-');
    if is_plain_int {
        crate::format::format_number(to_number(value), ",d")
    } else {
        label
    }
}

fn legend_key(spec: &Value) -> Option<(&'static str, String)> {
    if let Some(v) = spec.get("strokeWidth") {
        let name = match v {
            Value::String(s) => Some(s.clone()),
            Value::Object(o) => o.get("signal").and_then(|s| s.as_str()).map(String::from),
            _ => None,
        };
        if let Some(name) = name {
            return Some(("size", name));
        }
    }
    for key in [
        "fill",
        "stroke",
        "size",
        "opacity",
        "shape",
        "strokeDash",
        "gradient",
    ] {
        if let Some(v) = spec.get(key) {
            let name = match v {
                Value::String(s) => Some(s.clone()),
                Value::Object(o) => o.get("signal").and_then(|s| s.as_str()).map(String::from),
                _ => None,
            };
            if let Some(name) = name {
                return Some((key, name));
            }
        }
    }
    None
}

fn build_legend_info(spec: &Value, config: &Value, env: &Env) -> Option<LegendInfo> {
    let (key, scale_name) = legend_key(spec)?;
    let scale = env.get_scale(&scale_name)?;
    let stroke_width_legend = spec.get("strokeWidth").is_some();
    let orient = spec
        .get("orient")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| cfg_str(config, "legend", "orient", "right"));
    let title: Option<Vec<String>> = spec.get("title").and_then(|t| {
        let limit = cfg_num(config, "legend", "titleLimit", 180.0);
        let mut lines: Vec<String> = match t {
            Value::Array(a) => a.iter().filter_map(title_text).collect(),
            other => title_text(other).into_iter().collect(),
        };
        lines.retain(|l| !l.is_empty());
        if lines.is_empty() {
            None
        } else {
            Some(
                lines
                    .into_iter()
                    .map(|t| truncate_text(&t, 11.0, limit, "\u{2026}"))
                    .collect(),
            )
        }
    });
    let legend_x = spec
        .get("legendX")
        .and_then(|v| eval_signal_value(v, env))
        .and_then(|v| v.as_f64());
    let legend_y = spec
        .get("legendY")
        .and_then(|v| eval_signal_value(v, env))
        .and_then(|v| v.as_f64());
    let legend_offset = spec
        .get("offset")
        .and_then(|v| eval_signal_value(v, env))
        .and_then(|v| v.as_f64())
        .unwrap_or(18.0);
    let label_limit = spec
        .get("labelLimit")
        .and_then(|v| v.as_f64())
        .unwrap_or_else(|| cfg_num(config, "legend", "labelLimit", 160.0));
    let legend_format = spec
        .get("format")
        .and_then(|v| eval_signal_value(v, env))
        .and_then(|v| v.as_str().map(String::from));
    let legend_format_type = spec
        .get("formatType")
        .and_then(|v| v.as_str())
        .map(String::from);
    let font_size = config_value(config, "style", "guide-label")
        .and_then(|s| s.get("fontSize").and_then(|v| v.as_f64()))
        .unwrap_or(10.0);
    let title_font_size = config_value(config, "style", "guide-title")
        .and_then(|s| s.get("fontSize").and_then(|v| v.as_f64()))
        .unwrap_or(11.0);
    let symbol_size = spec
        .get("symbolSize")
        .and_then(|v| eval_signal_value(v, env))
        .and_then(|v| v.as_f64())
        .unwrap_or_else(|| cfg_num(config, "legend", "symbolSize", 100.0));
    let stroke_symbol = spec
        .get("symbolType")
        .and_then(|v| v.as_str())
        == Some("stroke");
    let symbol_type = spec
        .get("symbolType")
        .and_then(|v| v.as_str())
        .unwrap_or("circle")
        .to_string();
    let gradient_length = spec
        .get("gradientLength")
        .and_then(|v| eval_signal_value(v, env))
        .and_then(|v| v.as_f64())
        .unwrap_or_else(|| cfg_num(config, "legend", "gradientLength", 200.0));
    let gradient_thickness = spec
        .get("gradientThickness")
        .and_then(|v| eval_signal_value(v, env))
        .and_then(|v| v.as_f64())
        .unwrap_or_else(|| cfg_num(config, "legend", "gradientThickness", 16.0));
    // Legend `encode.symbols` overrides (typically fill/stroke/opacity) are
    // applied to every generated symbol.
    let mut symbol_props: Map<String, Value> = Map::new();
    if let Some(enc) = spec
        .get("encode")
        .and_then(|e| e.get("symbols"))
        .and_then(|s| s.get("update"))
        .and_then(|u| u.as_object())
    {
        let datum = Value::Null;
        let item = Value::Null;
        for (prop, encoder) in enc {
            if let Some(v) = encoder_value(encoder, &datum, &item, env) {
                symbol_props.insert(prop.clone(), v);
            }
        }
    }

    if scale.is_discrete() {
        let values = spec
            .get("values")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_else(|| scale.domain.clone());
        let entries: Vec<LegendEntry> = values
            .iter()
            .take(30)
            .map(|v| {
                let mapped = scale.scale(v);
                let fill = if let Some(name) =
                    spec.get("fill").and_then(|v| v.as_str())
                {
                    env.get_scale(name)
                        .map(|s| s.scale(v))
                        .unwrap_or_else(|| json!("transparent"))
                } else {
                    json!("transparent")
                };
                let stroke = if let Some(name) =
                    spec.get("stroke").and_then(|v| v.as_str())
                {
                    env.get_scale(name)
                        .map(|s| s.scale(v))
                        .unwrap_or(Value::Null)
                } else {
                    json!("#888")
                };
                let size = if key == "size" {
                    mapped.as_f64().unwrap_or(symbol_size)
                } else {
                    symbol_size
                };
                let shape = if key == "shape" {
                    mapped.as_str().unwrap_or(&symbol_type).to_string()
                } else {
                    symbol_type.clone()
                };
                LegendEntry {
                    label: truncate_text(
                        &numeric_legend_label(
                            scale,
                            v,
                            scale.label(
                                v,
                                legend_format.as_deref(),
                                legend_format_type.as_deref(),
                            ),
                        ),
                        font_size,
                        label_limit,
                        "\u{2026}",
                    ),
                    fill,
                    stroke,
                    size,
                    shape,
                    opacity: if key == "opacity" {
                        Some(to_number(&scale.scale(v)))
                    } else {
                        None
                    },
                }
            })
            .collect();
        // Symbol legend geometry mirrors vega-view-transforms + the parser's
        // symbol groups: a `sqrt(size)` symbol centered at `side/2 + 1`, a
        // label starting 10px further right, 11px rows with a 2px gap.
        let mut entry_width = 0.0f64;
        let mut entry_height = 11.0f64;
        let mut entry_widths = Vec::new();
        for e in &entries {
            let side = (e.size.max(0.0)).sqrt().max(1.0);
            let label_x = side / 2.0 + 11.0;
            let w = label_x + text_width(&e.label, font_size);
            entry_width = entry_width.max(w);
            entry_widths.push(w);
            let stroke_extra = if e.stroke.is_null() || stroke_symbol { 0.0 } else { 3.0 };
            entry_height = entry_height.max(side + 1.0 + stroke_extra);
        }
        let title_w = title
            .as_ref()
            .map(|lines| {
                lines
                    .iter()
                    .map(|t| text_width(t, title_font_size))
                    .fold(0.0, f64::max)
            })
            .unwrap_or(0.0);
        let title_height = title
            .as_ref()
            .map(|lines| {
                title_font_size
                    + 5.0
                    + (lines.len().saturating_sub(1) as f64) * (title_font_size + 2.0)
            })
            .unwrap_or(0.0);
        let direction = spec
            .get("direction")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| cfg_str(config, "legend", "direction", "vertical"));
        let horizontal =
            orient == "top" || orient == "bottom" || direction == "horizontal";
        let title_orient = spec
            .get("titleOrient")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| cfg_str(config, "legend", "titleOrient", "top"));
        let title_on_side =
            horizontal && title.is_some() && title_orient == "left";
        let title_width = if title_on_side { title_w + 5.0 } else { 0.0 };
        let rows = entries.len() as f64;
        let row_gap = if stroke_symbol {
            2.0
        } else if entries.iter().any(|e| !e.stroke.is_null()) {
            4.0
        } else {
            2.0
        };
        let (width, height) = if entries.is_empty() {
            (entry_width.max(title_w), title_height)
        } else if horizontal {
            let total = entry_widths.iter().sum::<f64>() + (rows - 1.0) * 10.0;
            if title_on_side {
                (title_width + total, entry_height)
            } else {
                (total.max(title_w), title_height + entry_height)
            }
        } else {
            (
                entry_width.max(title_w),
                title_height + rows * entry_height + (rows - 1.0) * row_gap,
            )
        };
        return Some(LegendInfo {
            orient,
            title,
            entries,
            gradient: None,
            width,
            height,
            entry_width,
            entry_height,
            entry_widths,
            horizontal,
            size_legend: false,
            entry_ys: vec![],
            entry_heights: vec![],
            entry_symbol_ys: vec![],
            size_symbol_x: 0.0,
            size_label_x: 0.0,
            size_symbol_xs: Vec::new(),
            size_label_xs: Vec::new(),
            entry_xs: Vec::new(),
            stroke_width_legend,
            title_height,
            title_on_side,
            title_width,
            offset: legend_offset,
            legend_x,
            legend_y,
            gradient_length,
            gradient_thickness,
            font_size,
            title_font_size,
            symbol_props,
            gradient_labels: Vec::new(),
            gradient_label_perc: Vec::new(),
            band_legend: false,
            stroke_symbol,
        });
    }

    // Continuous size/opacity/shape legends are symbol legends. A fill or
    // stroke legend becomes a symbol legend when it also carries size / symbol
    // encode properties (e.g. a merged quantize color+size legend).
    let legend_scale_count = ["fill", "stroke", "size", "opacity", "shape", "strokeDash"]
        .iter()
        .filter(|k| spec.get(*k).is_some())
        .count();
    let symbol_legend = (!matches!(key, "fill" | "stroke" | "gradient")
        || legend_scale_count > 1
        || spec.get("size").is_some()
        || spec.get("symbolType").is_some()
        || spec.get("encode").and_then(|e| e.get("symbols")).is_some())
        && (scale.bins.is_empty()
            || !matches!(key, "fill" | "stroke" | "gradient"));
    if symbol_legend {
        let fill_scale = spec
            .get("fill")
            .and_then(|v| v.as_str())
            .and_then(|n| env.get_scale(n));
        let stroke_scale = spec
            .get("stroke")
            .and_then(|v| v.as_str())
            .and_then(|n| env.get_scale(n));
        let size_scale = spec
            .get("size")
            .and_then(|v| v.as_str())
            .and_then(|n| env.get_scale(n));
        let entry_scale = size_scale
            .or(fill_scale)
            .or(stroke_scale)
            .unwrap_or(scale);
        let quantize_values: Option<Vec<Value>> = if entry_scale.kind == crate::scale::ScaleKind::Quantize {
            let n = if size_scale.is_some() {
                entry_scale.range.len().max(1)
            } else {
                entry_scale.colors.len().max(1)
            };
            let mut vals = vec![json!(f64::NEG_INFINITY)];
            for i in 1..n {
                vals.push(json_number(
                    entry_scale.d0 + (entry_scale.d1 - entry_scale.d0) * i as f64 / n as f64,
                ));
            }
            Some(vals)
        } else if matches!(
            entry_scale.kind,
            crate::scale::ScaleKind::Quantile | crate::scale::ScaleKind::Threshold
        ) {
            let mut vals = vec![json!(f64::NEG_INFINITY)];
            vals.extend(entry_scale.stops.iter().map(|s| json_number(*s)));
            Some(vals)
        } else {
            None
        };
        let binned = scale.bins.len() >= 2;
        let values: Vec<Value> = spec
            .get("values")
            .and_then(|v| v.as_array())
            .cloned()
            .or(quantize_values)
            .unwrap_or_else(|| {
                if binned {
                    scale.bins[..scale.bins.len() - 1].to_vec()
                } else {
                    entry_scale.ticks(5.0)
                }
            });
        let entries: Vec<LegendEntry> = values
            .iter()
            .enumerate()
            .map(|(idx, v)| {
                let label = if binned && idx + 1 < scale.bins.len() {
                    format!(
                        "{} – {}",
                        crate::format::format_number(to_number(&scale.bins[idx]), ""),
                        crate::format::format_number(to_number(&scale.bins[idx + 1]), "")
                    )
                } else if matches!(
                    entry_scale.kind,
                    crate::scale::ScaleKind::Quantize
                        | crate::scale::ScaleKind::Quantile
                        | crate::scale::ScaleKind::Threshold
                ) {
                    let n = values.len();
                    let fmt = |x: &Value| {
                        let text = crate::format::format_number(to_number(x), "");
                        if entry_scale.kind == crate::scale::ScaleKind::Quantile {
                            crate::format::format_number(to_number(x), ".1f")
                        } else {
                            text.trim_end_matches(".0").to_string()
                        }
                    };
                    if idx == 0 {
                        format!("< {}", fmt(&values[1]))
                    } else if idx + 1 == n {
                        format!("≥ {}", fmt(v))
                    } else {
                        format!("{} – {}", fmt(v), fmt(&values[idx + 1]))
                    }
                } else {
                    numeric_legend_label(
                        entry_scale,
                        v,
                        entry_scale.label(
                            v,
                            legend_format.as_deref(),
                            legend_format_type.as_deref(),
                        ),
                    )
                };
                // A legend attached to a `stroke` channel paints symbols with
                // a colored stroke and no fill (line/donut legends), while a
                // `fill` legend paints the fill.
                let (fill, stroke) = if let Some(fs) = fill_scale {
                    (
                        fs.scale(v),
                        stroke_scale
                            .map(|ss| ss.scale(v))
                            .unwrap_or(Value::Null),
                    )
                } else if let Some(ss) = stroke_scale {
                    (Value::Null, ss.scale(v))
                } else {
                    (json!("transparent"), json!("#888"))
                };
                let size = if let Some(ss) = size_scale {
                    ss.scale(v).as_f64().unwrap_or(symbol_size)
                } else if key == "size" {
                    scale.scale(v).as_f64().unwrap_or(symbol_size)
                } else {
                    symbol_size
                };
                LegendEntry {
                    label: truncate_text(&label, font_size, label_limit, "\u{2026}"),
                    fill,
                    stroke,
                    size,
                    shape: symbol_type.clone(),
                    opacity: if key == "opacity" {
                        Some(to_number(&scale.scale(v)))
                    } else {
                        None
                    },
                }
            })
            .collect();
        let title_w = title
            .as_ref()
            .map(|lines| {
                lines
                    .iter()
                    .map(|t| text_width(t, title_font_size))
                    .fold(0.0, f64::max)
            })
            .unwrap_or(0.0);
        let title_height = title
            .as_ref()
            .map(|lines| {
                title_font_size
                    + 5.0
                    + (lines.len().saturating_sub(1) as f64) * (title_font_size + 2.0)
            })
            .unwrap_or(0.0);
        if size_scale.is_some() || stroke_width_legend {
            // vega-parser's `sizeExpression`: max(ceil(sqrt(area)+strokeWidth), fontSize).
            let base_sw = symbol_props
                .get("strokeWidth")
                .and_then(|v| v.as_f64())
                .unwrap_or(1.5);
            let direction = spec
                .get("direction")
                .and_then(|v| v.as_str())
                .map(String::from)
                .unwrap_or_else(|| cfg_str(config, "legend", "direction", "vertical"));
            let horizontal =
                orient == "top" || orient == "bottom" || direction == "horizontal";
            let label_off = crate::bounds::baseline_offset(Some("middle"), font_size);
            let label_shift = label_off - (0.8 * font_size).round();
            let mut entry_widths: Vec<f64> = Vec::new();
            let mut entry_heights: Vec<f64> = Vec::new();
            let mut entry_symbol_ys: Vec<f64> = Vec::new();
            let mut entry_width_max = 0.0f64;
            let mut size_symbol_xs: Vec<f64> = Vec::new();
            let mut size_label_xs: Vec<f64> = Vec::new();
            let regular_metric = if stroke_width_legend {
                0.0
            } else {
                entries
                    .iter()
                    .map(|e| (e.size.max(0.0).sqrt() + base_sw).ceil().max(font_size))
                    .fold(0.0f64, f64::max)
            };
            for e in &entries {
                let (area, sw) = if stroke_width_legend {
                    (symbol_size, e.size.max(0.0))
                } else {
                    (e.size.max(0.0), base_sw)
                };
                let entry_metric = (area.sqrt() + sw).ceil().max(font_size);
                let metric = if stroke_width_legend {
                    entry_metric
                } else {
                    regular_metric
                };
                let symbol_y = entry_metric / 2.0;
                let label_x = metric + 4.0;
                let h = if stroke_width_legend {
                    symbol_y + 2.0 * sw
                } else {
                    let r = area.sqrt() / 2.0;
                    let label_top = symbol_y + label_shift;
                    let label_bottom = label_top + font_size;
                    (symbol_y + r).max(label_bottom)
                };
                entry_symbol_ys.push(symbol_y);
                entry_heights.push(h);
                entry_widths.push(label_x + text_width(&e.label, font_size));
                size_symbol_xs.push(metric / 2.0);
                size_label_xs.push(label_x);
                entry_width_max = entry_width_max.max(h);
            }
            let entry_width = entry_widths.iter().copied().fold(0.0, f64::max);
            let max_h = entry_heights.iter().copied().fold(0.0f64, f64::max);
            let mut entry_ys = vec![0.0; entries.len()];
            let mut entry_xs = vec![0.0; entries.len()];
            let (width, height);
            if horizontal {
                let slot = entry_width + 20.0;
                for i in 0..entries.len() {
                    entry_xs[i] = i as f64 * slot;
                }
                width = if entries.is_empty() {
                    title_w
                } else {
                    let last = (entries.len() - 1) as f64 * slot
                        + entry_widths.last().copied().unwrap_or(0.0);
                    last.max(title_w)
                };
                height = if entries.is_empty() {
                    title_height
                } else {
                    (title_height + max_h).ceil()
                };
                let row_off = (height - title_height - max_h) / 2.0;
                for i in 0..entries.len() {
                    entry_ys[i] = row_off + (max_h - entry_heights[i]) / 2.0;
                }
            } else {
                let mut y = 0.0f64;
                for i in 0..entries.len() {
                    if i > 0 {
                        y = (entry_ys[i - 1] + entry_heights[i - 1] + 2.0).ceil();
                    }
                    entry_ys[i] = y;
                }
                width = entry_width.max(title_w);
                height = if entries.is_empty() {
                    title_height
                } else {
                    let last_y = *entry_ys.last().unwrap();
                    let last_h = *entry_heights.last().unwrap();
                    (title_height + last_y + last_h).ceil()
                };
            }
            return Some(LegendInfo {
                orient,
                title,
                entries,
                gradient: None,
                width,
                height,
                entry_width,
                entry_height: entry_heights.first().copied().unwrap_or(10.0),
                entry_widths,
                horizontal,
                size_legend: true,
                entry_ys,
                entry_heights,
                entry_symbol_ys,
                title_on_side: false,
                title_width: 0.0,
                size_symbol_x: entry_width_max / 2.0,
                size_label_x: entry_width_max + 4.0,
                size_symbol_xs,
                size_label_xs,
                entry_xs,
                stroke_width_legend,
                title_height,
                offset: legend_offset,
                legend_x,
                legend_y,
                gradient_length,
                gradient_thickness,
                font_size,
                title_font_size,
                symbol_props,
                gradient_labels: Vec::new(),
                gradient_label_perc: Vec::new(),
                band_legend: false,
                stroke_symbol,
            });
        }
        let mut entry_width = 0.0f64;
        for e in &entries {
            let side = (e.size.max(0.0)).sqrt().max(1.0);
            entry_width = entry_width.max(side / 2.0 + 11.0 + text_width(&e.label, font_size));
        }
        let width = entry_width.max(title_w);
        let rows = entries.len() as f64;
        let height = if entries.is_empty() {
            title_height
        } else {
            title_height + rows * 14.0 + (rows - 1.0) * 4.0
        };
        return Some(LegendInfo {
            orient,
            title,
            entries,
            gradient: None,
            width,
            height,
            entry_width,
            entry_height: 14.0,
            entry_widths: vec![],
            horizontal: false,
            size_legend: false,
            entry_ys: vec![],
            entry_heights: vec![],
            entry_symbol_ys: vec![],
            size_symbol_x: 0.0,
            size_label_x: 0.0,
            size_symbol_xs: Vec::new(),
            size_label_xs: Vec::new(),
            entry_xs: Vec::new(),
            stroke_width_legend,
            title_height,
            title_on_side: false,
            title_width: 0.0,
            offset: legend_offset,
            legend_x,
            legend_y,
            gradient_length,
            gradient_thickness,
            font_size,
            title_font_size,
            symbol_props,
            gradient_labels: Vec::new(),
            gradient_label_perc: Vec::new(),
            band_legend: false,
            stroke_symbol,
        });
    }

    let palette = if scale.colors.is_empty() {
        crate::color::continuous_scheme("blues", 256)
    } else {
        scale.colors.clone()
    };
    let band_legend = !scale.bins.is_empty();
    // Gradient legends use generated tick values (d3's labelValues), not just
    // the domain endpoints.
    let count = (2.0 * (gradient_length / 100.0).floor()).max(2.0);
    let (lo, hi) = (scale.d0, scale.d1);
    let mut values = if scale.bins.len() > 2 {
        scale.bins[1..scale.bins.len() - 1].to_vec()
    } else {
        scale.ticks(count)
    };
    values.retain(|v| {
        let n = to_number(v);
        n >= lo.min(hi) && n <= lo.max(hi)
    });
    if values.is_empty() || (values.len() < 3 && scale.bins.is_empty()) {
        values = vec![
            scale.domain.first().cloned().unwrap_or(json!(0)),
            scale.domain.last().cloned().unwrap_or(json!(0)),
        ];
    }
    let labels: Vec<String> = values
        .iter()
        .map(|v| {
            numeric_legend_label(
                scale,
                v,
                scale.label(v, legend_format.as_deref(), legend_format_type.as_deref()),
            )
        })
        .collect();
    let span = (hi - lo).abs();
    let mut gradient_label_perc: Vec<f64> = values
        .iter()
        .map(|v| {
            if span == 0.0 {
                0.0
            } else {
                ((to_number(v) - lo) / (hi - lo)).clamp(0.0, 1.0)
            }
        })
        .collect();
    let mut labels = labels;
    if band_legend {
        labels.insert(0, String::new());
        gradient_label_perc.insert(0, 0.0);
    }
    let max_label = labels
        .iter()
        .map(|l| text_width(l, font_size))
        .fold(0.0, f64::max);
    let title_w = title
        .as_ref()
        .map(|lines| {
            lines
                .iter()
                .map(|t| text_width(t, title_font_size))
                .fold(0.0, f64::max)
        })
        .unwrap_or(0.0);
    let direction = spec
        .get("direction")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| cfg_str(config, "legend", "direction", "vertical"));
    let horizontal =
        direction == "horizontal" || orient == "top" || orient == "bottom";
    let (width, height) = if horizontal {
        (
            (gradient_length + 20.0).max(title_w),
            title.as_ref().map(|_| title_font_size + 5.0).unwrap_or(0.0)
                + gradient_thickness
                + 12.0,
        )
    } else {
        (
            (gradient_thickness + 2.0 + max_label).max(title_w),
            gradient_length
                + title
                    .as_ref()
                    .map(|lines| {
                        title_font_size
                            + 5.0
                            + (lines.len().saturating_sub(1) as f64) * (title_font_size + 2.0)
                    })
                    .unwrap_or(0.0),
        )
    };
    let title_height = title
        .as_ref()
        .map(|lines| {
            title_font_size
                + 5.0
                + (lines.len().saturating_sub(1) as f64) * (title_font_size + 2.0)
        })
        .unwrap_or(0.0);
    Some(LegendInfo {
        orient,
        title,
        entries: Vec::new(),
        gradient: Some(palette),
        width,
        height,
        entry_width: 0.0,
        entry_height: 0.0,
        title_on_side: false,
        title_width: 0.0,
        entry_widths: vec![],
        horizontal,
        size_legend: false,
        entry_ys: vec![],
        entry_heights: vec![],
        entry_symbol_ys: vec![],
        size_symbol_x: 0.0,
        size_label_x: 0.0,
        size_symbol_xs: Vec::new(),
        size_label_xs: Vec::new(),
        entry_xs: Vec::new(),
        stroke_width_legend,
        title_height,
        offset: legend_offset,
        legend_x,
        legend_y,
        gradient_length,
        gradient_thickness,
        font_size,
        title_font_size,
        symbol_props,
        gradient_labels: labels,
        gradient_label_perc,
        band_legend,
        stroke_symbol,
    })
}

#[derive(Default)]
struct GroupLayout {
    /// Total extents including axes and legends.
    left: f64,
    right: f64,
    top: f64,
    bottom: f64,
    title_top: f64,
    width: f64,
    height: f64,
    /// Axis-only extents (used to place legends).
    axis_left: f64,
    axis_right: f64,
    axis_top: f64,
    axis_bottom: f64,
}

impl GroupLayout {
    fn total_width(&self) -> f64 {
        self.left + self.width + self.right
    }
    fn total_height(&self) -> f64 {
        self.title_top + self.top + self.height + self.bottom
    }
}

pub struct Runtime<'a> {
    spec: &'a Value,
    config: Value,
    data: DataStore,
    signals: HashMap<String, Value>,
    scales: HashMap<String, Scale>,
    projections: HashMap<String, Projection>,
    width: f64,
    height: f64,
}

/// Vega's built-in config defaults for the styles used by layout. The spec's
/// own config is deep-merged over these.
fn default_config() -> Value {
    json!({
        "axis": {
            "tickOffset": 0,
            "labelLimit": 180
        },
        "axisBand": {
            "tickOffset": -0.5
        },
        "style": {
            "guide-label": {"fill": "#000", "font": DEFAULT_FONT, "fontSize": 10},
            "guide-title": {"fill": "#000", "font": DEFAULT_FONT, "fontSize": 11, "fontWeight": "bold"},
            "group-title": {"fill": "#000", "font": DEFAULT_FONT, "fontSize": 13, "fontWeight": "bold"},
            "group-subtitle": {"fill": "#000", "font": DEFAULT_FONT, "fontSize": 12},
            "cell": {"fill": "transparent", "stroke": "#ddd"},
            "view": {"fill": "transparent"}
        }
    })
}

fn merge_config(base: Value, top: Option<&Value>) -> Value {
    let mut out = base;
    let Some(Value::Object(t)) = top else {
        return out;
    };
    if let Value::Object(o) = &mut out {
        for (k, v) in t {
            merge_value(o.entry(k.clone()).or_insert(Value::Null), v);
        }
    }
    out
}

fn merge_value(dst: &mut Value, src: &Value) {
    match (dst, src) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in b {
                merge_value(a.entry(k.clone()).or_insert(Value::Null), v);
            }
        }
        (a, b) => *a = b.clone(),
    }
}

impl<'a> Runtime<'a> {
    pub fn new(spec: &'a Value, opts: &RenderOptions) -> Result<Self, Error> {
        if !spec.is_object() {
            return Err(Error::spec("Vega specification must be a JSON object"));
        }
        let config = merge_config(default_config(), spec.get("config"));
        let specs = signal_specs(spec);
        let mut constants = constant_signal_values(&specs);
        if let Some(v) = spec.get("width") {
            if v.is_number() {
                constants.insert("width".to_string(), v.clone());
            }
        }
        if let Some(v) = spec.get("height") {
            if v.is_number() {
                constants.insert("height".to_string(), v.clone());
            }
        }
        let data_opts = DataOptions {
            base: &opts.data_base,
            extra_dirs: opts.data_dirs.clone(),
        };
        let store = data::load(
            spec.get("data").and_then(|d| d.as_array()),
            &data_opts,
            &constants,
        )?;
        let scale_specs = scale_specs(spec);
        let mut signals = store.signals.clone();
        for (k, v) in &constants {
            if !signals.contains_key(k) {
                signals.insert(k.clone(), v.clone());
            }
        }
        // Cartographic projections are resolved before scales: `geopoint`
        // transforms write x/y fields that scales and marks consume, and the
        // projection `fit` only depends on data and signals.
        let mut store = store;
        let initial_scales: HashMap<String, Scale> = HashMap::new();
        let mut projections = build_projections(spec, &store, &signals, &initial_scales);
        apply_geopoints(spec, &mut store, &projections, &signals);
        let mut scales: HashMap<String, Scale> = HashMap::new();

        // Iterate the signal/scale dataflow a few times to settle
        // domain-dependent signals (notably Vega-Lite's width/height signals)
        // and range-dependent signals.
        for _ in 0..3 {
            let domains = compute_domains(&scale_specs, &store, &signals);
            let resolver = SignalResolver {
                specs: &specs,
                base: &signals,
                data: &store,
                domains: &domains,
                scales: &scales,
                values: RefCell::new(HashMap::new()),
                active: RefCell::new(HashSet::new()),
            };
            let mut next = resolver.resolve_all();
            for (k, v) in &signals {
                next.entry(k.clone()).or_insert_with(|| v.clone());
            }
            if let Some(w) = spec.get("width").and_then(|v| v.as_f64()) {
                next.insert("width".to_string(), json_number(w));
            }
            if let Some(h) = spec.get("height").and_then(|v| v.as_f64()) {
                next.insert("height".to_string(), json_number(h));
            }
            let next_scales = build_scales(&scale_specs, &store, &next)?;
            let stable = next == signals && !scales.is_empty();
            signals = next;
            scales = next_scales;
            if stable {
                break;
            }
        }

        let width = number_signal(&signals, "width", spec.get("width"));
        let height = number_signal(&signals, "height", spec.get("height"));
        // Rebuild projections now that scales/signals have settled (projection
        // parameters may reference domain signals); fit data is unchanged.
        projections = build_projections(spec, &store, &signals, &scales);
        Ok(Self {
            spec,
            config,
            data: store,
            signals,
            scales,
            projections,
            width,
            height,
        })
    }

    pub fn signals(&self) -> &HashMap<String, Value> {
        &self.signals
    }

    pub fn scales(&self) -> &HashMap<String, Scale> {
        &self.scales
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn height(&self) -> f64 {
        self.height
    }

    fn env(&self) -> Env<'_> {
        Env {
            data: &self.data,
            signals: &self.signals,
            scales: &self.scales,
            local: None,
            group_width: self.width,
            group_height: self.height,
            local_scales: None,
        }
    }

    fn env_with_dims<'b>(
        &'b self,
        local: &'b HashMap<String, Vec<Value>>,
        width: f64,
        height: f64,
    ) -> Env<'b> {
        Env {
            data: &self.data,
            signals: &self.signals,
            scales: &self.scales,
            local: Some(local),
            group_width: width,
            group_height: height,
            local_scales: None,
        }
    }

    fn env_full<'b>(
        &'b self,
        local: &'b HashMap<String, Vec<Value>>,
        local_scales: &'b HashMap<String, Scale>,
        width: f64,
        height: f64,
    ) -> Env<'b> {
        Env {
            data: &self.data,
            signals: &self.signals,
            scales: &self.scales,
            local: Some(local),
            group_width: width,
            group_height: height,
            local_scales: Some(local_scales),
        }
    }

    fn mark_defaults(&self, kind: &str) -> Map<String, Value> {
        let mut m = Map::new();
        let base = match kind {
            "arc" | "area" | "rect" => {
                m.insert("fill".into(), json!(DEFAULT_COLOR));
                None
            }
            "line" => {
                m.insert("stroke".into(), json!(DEFAULT_COLOR));
                m.insert("strokeWidth".into(), json!(2));
                None
            }
            "path" | "shape" => {
                m.insert("stroke".into(), json!(DEFAULT_COLOR));
                m.insert("strokeWidth".into(), json!(1));
                None
            }
            "rule" => {
                m.insert("stroke".into(), json!("#000"));
                m.insert("strokeWidth".into(), json!(1));
                None
            }
            "symbol" => {
                m.insert("fill".into(), json!(DEFAULT_COLOR));
                m.insert("size".into(), json!(64));
                Some("symbol")
            }
            "text" => {
                m.insert("fill".into(), json!("#000"));
                m.insert("font".into(), json!(DEFAULT_FONT));
                m.insert("fontSize".into(), json!(11));
                None
            }
            "trail" => {
                m.insert("fill".into(), json!(DEFAULT_COLOR));
                m.insert("size".into(), json!(2));
                None
            }
            "image" => {
                m.insert("width".into(), json!(100));
                m.insert("height".into(), json!(100));
                None
            }
            _ => None,
        };
        if let Some(Value::Object(o)) = self.config.get("mark") {
            for (k, v) in o {
                m.insert(k.clone(), v.clone());
            }
        }
        if let Some(Value::Object(o)) = self.config.get(kind) {
            for (k, v) in o {
                m.insert(k.clone(), v.clone());
            }
        }
        // The "point"/"circle"/"square" styles in Vega configs.
        if let Some(style) = base {
            if let Some(Value::Object(o)) = self.config.get("style").and_then(|s| s.get(style)) {
                for (k, v) in o {
                    m.insert(k.clone(), v.clone());
                }
            }
        }
        m
    }

    fn style_props(&self, style: &Value, out: &mut Map<String, Value>) {
        let names: Vec<String> = match style {
            Value::String(s) => vec![s.clone()],
            Value::Array(a) => a
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            _ => vec![],
        };
        for name in names {
            // Built-in Vega style defaults (vega-parser's `config.style`).
            // The compiled specs only carry user overrides, so these must be
            // supplied by the renderer.
            let built_in: &[(&str, Value)] = match name.as_str() {
                "point" => &[
                    ("size", json!(30)),
                    ("strokeWidth", json!(2)),
                    ("shape", json!("circle")),
                ],
                "circle" => &[("size", json!(30)), ("strokeWidth", json!(2))],
                "square" => &[
                    ("size", json!(30)),
                    ("strokeWidth", json!(2)),
                    ("shape", json!("square")),
                ],
                "cell" => &[
                    ("fill", json!("transparent")),
                    ("stroke", json!("#ddd")),
                ],
                "view" => &[("fill", json!("transparent"))],
                _ => &[],
            };
            for (k, v) in built_in {
                out.insert((*k).to_string(), v.clone());
            }
            if let Some(Value::Object(o)) = self.config.get("style").and_then(|s| s.get(&name)) {
                for (k, v) in o {
                    out.insert(k.clone(), v.clone());
                }
            }
        }
    }

    fn data_for_mark(&self, mark: &Value, local: &HashMap<String, Vec<Value>>) -> Vec<Value> {
        let from = mark.get("from");
        let name = from
            .and_then(|f| f.get("data"))
            .and_then(|d| d.as_str())
            .or_else(|| {
                from.and_then(|f| f.get("facet"))
                    .and_then(|f| f.get("data"))
                    .and_then(|d| d.as_str())
            });
        let mut values = name
            .and_then(|n| local.get(n).cloned().or_else(|| self.data.get(n).cloned()))
            .unwrap_or_default();
        if let Some(sort) = mark.get("sort") {
            sort_data(&mut values, sort);
        }
        values
    }

    fn build_mark(
        &self,
        mark: &Value,
        local: &HashMap<String, Vec<Value>>,
        local_scales: &HashMap<String, Scale>,
        group_width: f64,
        group_height: f64,
    ) -> Vec<Item> {
        let kind = mark.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let mut defaults = self.mark_defaults(kind);
        if let Some(style) = mark.get("style") {
            self.style_props(style, &mut defaults);
        }
        if kind == "group" {
            return self.build_group_mark(
                mark,
                &defaults,
                local,
                local_scales,
                group_width,
                group_height,
            );
        }
        let mut values = self.data_for_mark(mark, local);
        // A mark without a `from` data reference still renders a single item
        // (selection brushes, background rects, etc.), even though there is no
        // datum to bind properties to.
        if values.is_empty() && mark.get("from").is_none() {
            values.push(Value::Null);
        }
        let mut container = Item::new("group");
        container.class.push(format!("mark-{kind}"));
        container.class.push("role-mark".to_string());
        if let Some(style) = mark.get("style") {
            if let Some(s) = style.as_str() {
                container.class.push(s.to_string());
            } else if let Some(a) = style.as_array() {
                for s in a {
                    if let Some(s) = s.as_str() {
                        container.class.push(s.to_string());
                    }
                }
            }
        }
        let mut items = Vec::new();

        if matches!(kind, "line" | "area" | "trail") {
            let first_props = values
                .first()
                .map(|d| {
                    self.encode_props(
                        mark,
                        &defaults,
                        d,
                        local,
                        local_scales,
                        group_width,
                        group_height,
                    )
                })
                .unwrap_or_default();
            let mut path = Item::new(kind);
            for (k, v) in first_props {
                path.props.insert(k, v);
            }
            let mut pairs: Vec<(Item, Value)> = Vec::new();
            for datum in values.iter() {
                let props = self.encode_props(
                    mark,
                    &defaults,
                    datum,
                    local,
                    local_scales,
                    group_width,
                    group_height,
                );
                let mut v = Item::new("vertex");
                copy_prop(&props, &mut v, "x");
                copy_prop(&props, &mut v, "y");
                copy_prop(&props, &mut v, "x2");
                copy_prop(&props, &mut v, "y2");
                copy_prop(&props, &mut v, "defined");
                if kind == "trail" {
                    if let Some(size) = props.get("size") {
                        v.set("size", size.clone());
                    }
                }
                pairs.push((v, datum.clone()));
            }
            // Vega sorts line/area/trail marks by *encoded item properties*
            // (the mark `sort` field refers to e.g. the computed `x`), not by
            // raw datum fields. In particular area_temperature_range has
            // `"sort": {"field": "x"}` and relies on this for path order.
            if let Some(sort) = mark.get("sort") {
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
                if !fields.is_empty() {
                    pairs.sort_by(|a, b| {
                        for (i, field) in fields.iter().enumerate() {
                            let field = field
                                .trim_start_matches("datum[")
                                .trim_end_matches(']')
                                .trim_matches('"');
                            let left = a.0.f64(field).map(Value::from).unwrap_or_else(|| {
                                crate::value::get_field_owned(&a.1, field)
                            });
                            let right = b.0.f64(field).map(Value::from).unwrap_or_else(|| {
                                crate::value::get_field_owned(&b.1, field)
                            });
                            let mut ord = compare(&left, &right);
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
            }
            path.items = pairs.into_iter().map(|(v, _)| v).collect();
            container.items.push(path);
        } else {
            let geoshape = mark
            .get("transform")
            .and_then(|t| t.as_array())
            .and_then(|t| t.iter().find(|t| t.get("type").and_then(|v| v.as_str()) == Some("geoshape")))
            .cloned();
        for datum in &values {
                let mut props = self.encode_props(
                    mark,
                    &defaults,
                    datum,
                    local,
                    local_scales,
                    group_width,
                    group_height,
                );
                if let Some(transform) = &geoshape {
                    if let Some(path) = self.geoshape_path(transform, datum, local, local_scales, group_width, group_height) {
                        props.insert("path".into(), json!(path));
                    }
                    let encoded = mark
                        .get("encode")
                        .and_then(|e| e.get("update"))
                        .and_then(|e| e.as_object());
                    if encoded.and_then(|e| e.get("stroke")).is_none() {
                        props.remove("stroke");
                        props.remove("strokeWidth");
                    }
                }
                let mut item = Item::new(kind);
                if let Some(desc) = props.remove("description") {
                    if let Some(s) = desc.as_str() {
                        item.set("description", json!(s));
                    }
                }
                for (k, v) in props {
                    item.props.insert(k, v);
                }
                normalize_item(&mut item, kind);
                items.push(item);
            }
            container.items = items;
        }
        if mark.get("clip").and_then(|v| v.as_bool()).unwrap_or(false) {
            container.set("_clip", json!([0.0, 0.0, group_width, group_height]));
        }
        vec![container]
    }

    fn geoshape_path(
        &self,
        transform: &Value,
        datum: &Value,
        local: &HashMap<String, Vec<Value>>,
        local_scales: &HashMap<String, Scale>,
        group_width: f64,
        group_height: f64,
    ) -> Option<String> {
        let name = transform.get("projection").and_then(|p| p.as_str())?;
        let projection = self.projections.get(name)?;
        let object = if let Some(field) = transform.get("field") {
            if let Some(expr) = field.as_str() {
                if expr.starts_with("datum") {
                    expression::evaluate(
                        expr,
                        Some(datum),
                        &self.env_full(local, local_scales, group_width, group_height),
                    )
                    .unwrap_or(Value::Null)
                } else {
                    field_value(datum, field)
                }
            } else {
                field_value(datum, field)
            }
        } else {
            datum.clone()
        };
        let point_radius = transform.get("pointRadius").map(|v| {
            if v.is_number() {
                v.as_f64().unwrap_or(4.5)
            } else {
                expression::evaluate(
                    v.get("signal").and_then(|s| s.as_str()).unwrap_or(""),
                    Some(datum),
                    &self.env_full(local, local_scales, group_width, group_height),
                )
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(4.5)
            }
        });
        Some(projection::project_path(projection, &object, point_radius))
    }

    fn encode_props(
        &self,
        mark: &Value,
        defaults: &Map<String, Value>,
        datum: &Value,
        local: &HashMap<String, Vec<Value>>,
        local_scales: &HashMap<String, Scale>,
        group_width: f64,
        group_height: f64,
    ) -> Map<String, Value> {
        let mut out = defaults.clone();
        let env = self.env_full(local, local_scales, group_width, group_height);
        let encode = mark.get("encode");
        for phase in ["enter", "update"] {
            if let Some(enc) = encode
                .and_then(|e| e.get(phase))
                .and_then(|e| e.as_object())
            {
                // `item` is only visible to encoders that mention it. Building
                // the snapshot clones every property for every mark item, so
                // skip it entirely for the common case.
                let needs_item = enc.values().any(mentions_item);
                let item_snapshot = if needs_item {
                    Value::Object(out.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                } else {
                    Value::Null
                };
                for (prop, encoder) in enc {
                    if let Some(v) = encoder_value(encoder, datum, &item_snapshot, &env) {
                        out.insert(prop.clone(), v);
                    }
                }
            }
        }
        out
    }

    fn build_group_mark(
        &self,
        mark: &Value,
        defaults: &Map<String, Value>,
        local: &HashMap<String, Vec<Value>>,
        local_scales: &HashMap<String, Scale>,
        group_width: f64,
        group_height: f64,
    ) -> Vec<Item> {
        let mut out = Vec::new();
        if let Some(facet) = mark.get("from").and_then(|f| f.get("facet")) {
            let mut groups = self.facet_groups(mark, facet, local);
            // Facet group marks can carry a `sort` (e.g. trellis row order).
            if let Some(sort) = mark.get("sort") {
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
                groups.sort_by(|(a, _), (b, _)| {
                    for (i, f) in fields.iter().enumerate() {
                        let field = f
                            .trim_start_matches("datum[")
                            .trim_end_matches(']')
                            .trim_matches('"');
                        let mut ord =
                            compare(&get_field_owned(a, field), &get_field_owned(b, field));
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
            for (datum, rows) in groups {
                if let Some(group) = self.build_group_instance(
                    mark,
                    defaults,
                    &datum,
                    local,
                    local_scales,
                    Some(rows),
                    group_width,
                    group_height,
                ) {
                    out.push(group);
                }
            }
            return out;
        }
        let values = self.data_for_mark(mark, local);
        if values.is_empty() {
            if mark.get("from").is_none() {
                let datum = local
                    .get("__group_props")
                    .and_then(|v| v.first())
                    .cloned()
                    .unwrap_or(Value::Null);
                if let Some(group) = self.build_group_instance(
                    mark,
                    defaults,
                    &datum,
                    local,
                    local_scales,
                    None,
                    group_width,
                    group_height,
                ) {
                    out.push(group);
                }
            }
            return out;
        }
        for datum in &values {
            if let Some(group) = self.build_group_instance(
                mark,
                defaults,
                datum,
                local,
                local_scales,
                None,
                group_width,
                group_height,
            ) {
                out.push(group);
            }
        }
        out
    }

    fn derive_local_data(
        &self,
        defs: &[Value],
        local: &HashMap<String, Vec<Value>>,
    ) -> Vec<(String, Vec<Value>)> {
        use std::collections::HashSet;
        let mut needed: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let visit_source = |v: &Value, needed: &mut Vec<String>, seen: &mut HashSet<String>| {
            match v {
                Value::String(n) => {
                    if seen.insert(n.clone()) {
                        needed.push(n.clone());
                    }
                }
                Value::Array(a) => {
                    for n in a {
                        if let Some(n) = n.as_str() {
                            if seen.insert(n.to_string()) {
                                needed.push(n.to_string());
                            }
                        }
                    }
                }
                _ => {}
            }
        };
        for def in defs {
            if let Some(source) = def.get("source") {
                visit_source(source, &mut needed, &mut seen);
            }
        }
        let mut entries: Vec<Value> = Vec::new();
        for name in &needed {
            if let Some(rows) = local.get(name).or_else(|| self.data.get(name)) {
                entries.push(json!({"name": name, "values": rows}));
            }
        }
        entries.extend(defs.iter().cloned());
        let opts = DataOptions {
            base: &std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            extra_dirs: Vec::new(),
        };
        match data::load(Some(&entries), &opts, &self.signals) {
            Ok(store) => defs
                .iter()
                .filter_map(|d| d.get("name").and_then(|n| n.as_str()))
                .filter_map(|n| store.get(n).map(|rows| (n.to_string(), rows.clone())))
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    fn facet_groups(
        &self,
        mark: &Value,
        facet: &Value,
        local: &HashMap<String, Vec<Value>>,
    ) -> Vec<(Value, Vec<Value>)> {
        let data_name = facet.get("data").and_then(|d| d.as_str()).unwrap_or("");
        let rows = local
            .get(data_name)
            .cloned()
            .or_else(|| self.data.get(data_name).cloned())
            .unwrap_or_default();
        let groupby: Vec<String> = facet
            .get("groupby")
            .and_then(|g| g.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let mut groups: IndexMap<String, (Value, Vec<Value>)> = IndexMap::new();
        if groupby.is_empty() {
            groups.insert(String::new(), (Value::Object(Map::new()), rows.clone()));
        } else {
            for row in &rows {
                let mut key = Map::new();
                let mut values = Vec::new();
                for g in &groupby {
                    let v = get_field_owned(&row, g);
                    values.push(v.clone());
                    key.insert(g.clone(), v);
                }
                let id = serde_json::to_string(&values).unwrap_or_default();
                groups
                    .entry(id)
                    .or_insert_with(|| (Value::Object(key), Vec::new()))
                    .1
                    .push(row.clone());
            }
        }
        // `aggregate.cross` facets enumerate the full cross product of the
        // groupby field values, including empty combinations.
        let cross = facet
            .get("aggregate")
            .and_then(|a| a.get("cross"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if cross && !groupby.is_empty() {
            let mut domains: Vec<Vec<Value>> = Vec::new();
            for g in &groupby {
                let mut vals: Vec<Value> = Vec::new();
                for row in &rows {
                    let v = get_field_owned(row, g);
                    if !vals.iter().any(|x| x == &v) {
                        vals.push(v);
                    }
                }
                domains.push(vals);
            }
            groups.clear();
            let mut idx = vec![0usize; groupby.len()];
            loop {
                let mut key = Map::new();
                let mut values = Vec::new();
                for (k, g) in groupby.iter().enumerate() {
                    let v = domains[k].get(idx[k]).cloned().unwrap_or(Value::Null);
                    values.push(v.clone());
                    key.insert(g.clone(), v);
                }
                let id = serde_json::to_string(&values).unwrap_or_default();
                let matched: Vec<Value> = rows
                    .iter()
                    .filter(|row| {
                        groupby
                            .iter()
                            .enumerate()
                            .all(|(k, g)| get_field_owned(row, g) == values[k])
                    })
                    .cloned()
                    .collect();
                groups.insert(id, (Value::Object(key), matched));
                // Next combination (last field varies fastest).
                let mut carry = groupby.len();
                while carry > 0 {
                    carry -= 1;
                    idx[carry] += 1;
                    if idx[carry] < domains[carry].len() {
                        break;
                    }
                    idx[carry] = 0;
                }
                if carry == 0 && idx[0] == 0 {
                    break;
                }
            }
        }
        let mut out: Vec<(Value, Vec<Value>)> = groups.into_values().collect();
        if let Some(agg) = facet.get("aggregate").and_then(|a| a.as_object()) {
            let fields: Vec<String> = agg
                .get("fields")
                .and_then(|f| f.as_array())
                .map(|a| {
                    a.iter()
                        .map(|v| v.as_str().unwrap_or("").to_string())
                        .collect()
                })
                .unwrap_or_default();
            let ops: Vec<String> = agg
                .get("ops")
                .and_then(|f| f.as_array())
                .map(|a| {
                    a.iter()
                        .map(|v| v.as_str().unwrap_or("").to_string())
                        .collect()
                })
                .unwrap_or_default();
            let as_: Vec<String> = agg
                .get("as")
                .and_then(|f| f.as_array())
                .map(|a| {
                    a.iter()
                        .map(|v| v.as_str().unwrap_or("").to_string())
                        .collect()
                })
                .unwrap_or_default();
            for (datum, rows) in out.iter_mut() {
                if let Value::Object(map) = datum {
                    for (i, op) in ops.iter().enumerate() {
                        let field = fields.get(i).cloned().unwrap_or_default();
                        let name = as_
                            .get(i)
                            .cloned()
                            .unwrap_or_else(|| format!("{op}_{field}"));
                        map.insert(name, crate::data::aggregate_op(op, &field, rows));
                    }
                }
            }
        }
        if let Some(sort) = mark.get("sort") {
            let fields: Vec<Value> = match sort.get("field") {
                Some(Value::String(s)) => vec![Value::String(s.clone())],
                Some(Value::Array(a)) => a.clone(),
                _ => vec![],
            };
            let orders: Vec<bool> = match sort.get("order") {
                Some(Value::String(s)) => vec![s == "descending"],
                Some(Value::Array(a)) => {
                    a.iter().map(|v| v.as_str() == Some("descending")).collect()
                }
                _ => vec![],
            };
            let env = self.env();
            out.sort_by(|a, b| {
                for (i, f) in fields.iter().enumerate() {
                    let av = facet_sort_value(f, &a.0, &env);
                    let bv = facet_sort_value(f, &b.0, &env);
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
        out
    }

    fn build_group_instance(
        &self,
        mark: &Value,
        defaults: &Map<String, Value>,
        datum: &Value,
        local: &HashMap<String, Vec<Value>>,
        local_scales: &HashMap<String, Scale>,
        facet_rows: Option<Vec<Value>>,
        group_width: f64,
        group_height: f64,
    ) -> Option<Item> {
        let props = self.encode_props(
            mark,
            defaults,
            datum,
            local,
            local_scales,
            group_width,
            group_height,
        );
        let mut child_local = local.clone();
        // Nested groups without a data source use the parent group item as
        // their datum (`{"field": {"group": "y"}}` and similar encodes).
        child_local.insert(
            "__group_props".to_string(),
            vec![Value::Object(props.clone())],
        );
        if let Some(facet) = mark.get("from").and_then(|f| f.get("facet")) {
            let name = facet
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("facet");
            if let Some(rows) = &facet_rows {
                child_local.insert(name.to_string(), rows.clone());
            }
        }
        // Local data definitions scoped to this group (e.g. a facet's
        // `child_column_domain` derived from the partitioned data).
        if let Some(defs) = mark.get("data").and_then(|d| d.as_array()) {
            let local_sets = self.derive_local_data(defs, &child_local);
            for (name, rows) in local_sets {
                child_local.insert(name, rows);
            }
        }
        let mut child_scales = local_scales.clone();
        if let Some(scale_defs) = mark.get("scales").and_then(|s| s.as_array()) {
            // Only copy the datasets these scales can reach. Cloning the whole
            // store for every facet/group instance is O(groups * data).
            let mut needed: std::collections::HashSet<&str> = std::collections::HashSet::new();
            for def in scale_defs {
                collect_dataset_refs(def, &self.data.sets, &child_local, &mut needed);
            }
            let mut local_store = DataStore::default();
            for name in needed {
                if let Some(rows) = child_local.get(name) {
                    local_store.sets.insert(name.to_string(), rows.clone());
                } else if let Some(rows) = self.data.sets.get(name) {
                    local_store.sets.insert(name.to_string(), rows.clone());
                }
            }
            local_store.signals = self.signals.clone();
            let env = self.env_with_dims(&child_local, group_width, group_height);
            let mut built = Vec::new();
            for s in scale_defs {
                if let Some(name) = s.get("name").and_then(|n| n.as_str()) {
                    if let Ok(scale) =
                        scale::build_scale(name, s, &local_store, &self.signals, &env)
                    {
                        built.push((name.to_string(), scale));
                    }
                }
            }
            for (name, scale) in built {
                child_scales.insert(name, scale);
            }
        }
        let mut gx = props.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let mut gy = props.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let mut width = props.get("width").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let mut height = props.get("height").and_then(|v| v.as_f64()).unwrap_or(0.0);
        if let Some(x2) = props.get("x2").and_then(|v| v.as_f64()) {
            if width == 0.0 {
                width = x2 - gx;
            }
        }
        if let Some(y2) = props.get("y2").and_then(|v| v.as_f64()) {
            if height == 0.0 {
                height = y2 - gy;
            }
        }
        if width < 0.0 {
            gx += width;
            width = -width;
        }
        if height < 0.0 {
            gy += height;
            height = -height;
        }
        // Empty facet groups still produce a group item (with its encoded
        // position/size) but no child marks, axes, legends or titles.
        if facet_rows.as_ref().map(|r| r.is_empty()).unwrap_or(false) {
            let mut group = Item::new("group");
            group.class.push("mark-group".to_string());
            group.class.push("role-mark".to_string());
            if let Some(name) = mark.get("name").and_then(|n| n.as_str()) {
                group.set("_mark_name", json!(name));
            }
            if let Some(role) = mark.get("role").and_then(|r| r.as_str()) {
                group.set("_role", json!(role));
            }
            group.set("x", json_number(gx));
            group.set("y", json_number(gy));
            group.set("width", json_number(width));
            group.set("height", json_number(height));
            if let Some(style) = mark.get("style") {
                let mut group_props = Map::new();
                self.style_props(style, &mut group_props);
                for (k, v) in group_props {
                    if matches!(k.as_str(), "fill" | "stroke" | "strokeWidth" | "opacity") {
                        group.props.insert(k, v);
                    }
                }
            }
            return Some(group);
        }
        let axes: Vec<Value> = mark
            .get("axes")
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default();
        let legends: Vec<Value> = mark
            .get("legends")
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default();
        let child_marks: Vec<Value> = mark
            .get("marks")
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default();
        let env = self.env_full(&child_local, &child_scales, group_width, group_height);
        let group_title_spec = mark.get("title");
        let group_title = group_title_spec.and_then(|t| title_value(t, datum, &env));
        let layout = self.group_layout(&axes, &legends, group_title_spec, width, height, &env);
        let mut children = Vec::new();
        let mut axis_infos = Vec::new();
        for a in &axes {
            if let Some(info) = build_axis_info(a, &self.config, &env, width, height) {
                axis_infos.push(info);
            }
        }
        let mut legend_infos = Vec::new();
        for l in &legends {
            if let Some(info) = build_legend_info(l, &self.config, &env) {
                legend_infos.push(info);
            }
        }
        // Grid axes first, then marks, then regular axes.
        let mut high_grid_axes = Vec::new();
        for info in axis_infos.iter().filter(|a| a.grid) {
            if info.zindex > 0.0 {
                high_grid_axes.push(info);
            } else {
                children.push(axis_item(info, width, height, &env, true));
            }
        }
        let mut mark_children = Vec::new();
        for m in &child_marks {
            for item in self.build_mark(m, &child_local, &child_scales, width, height) {
                mark_children.push(item);
            }
        }
        // A `layout` on a group mark arranges the group's child marks (cf.
        // vega-view-transforms' ViewLayout, which runs trellisLayout on every
        // item of the mark and therefore its `items`).
        if let Some(spec) = mark.get("layout") {
            layout_groups(&mut mark_children, spec, &env);
        }
        children.extend(mark_children);
        for info in high_grid_axes {
            children.push(axis_item(info, width, height, &env, true));
        }
        let legend_anchor = legend_anchor_bounds(&children, width, height);
        for info in axis_infos.iter().filter(|a| !a.grid) {
            children.push(axis_item(info, width, height, &env, true));
        }
        let mut legend_y = 0.0;
        let mut top_legend_y = 0.0;
        let mut bottom_legend_y = 0.0;
        let mut bottom_legend_x = 0.0;
        for info in &legend_infos {
            let item = legend_item(info, &env);
            let (x, y) = match info.orient.as_str() {
                "left" => (
                    legend_anchor.x1.min(0.0).floor() - info.offset - info.width,
                    legend_y,
                ),
                "right" => (legend_anchor.x2.max(width).ceil() + info.offset, legend_y),
                "top" => (
                    0.0,
                    legend_anchor.y1.min(0.0).floor() - info.offset - top_legend_y - info.height,
                ),
                "bottom" => (
                    if info.horizontal { bottom_legend_x } else { 0.0 },
                    legend_anchor.y2.max(height).ceil() + info.offset + bottom_legend_y,
                ),
                "bottom-right" => (
                    width - info.width - info.offset,
                    height - info.height - info.offset,
                ),
                "top-right" => (
                    legend_anchor.x2.max(width).ceil() - info.width - info.offset,
                    legend_anchor.y1.min(0.0).floor() + info.offset,
                ),
                "bottom-left" => (
                    legend_anchor.x1.min(0.0).floor() + info.offset,
                    legend_anchor.y2.max(height).ceil() - info.height - info.offset,
                ),
                "top-left" => (
                    legend_anchor.x1.min(0.0).floor(),
                    legend_anchor.y1.min(0.0).floor(),
                ),
                "none" => (
                    info.legend_x
                        .unwrap_or(legend_anchor.x2.max(width) + 18.0),
                    info.legend_y.unwrap_or(legend_y),
                ),
                _ => (legend_anchor.x2.max(width).ceil() + info.offset, legend_y),
            };
            let mut placed = item;
            placed.set("x", json_number(x));
            placed.set("y", json_number(y));
            children.push(placed);
            // Gradient endpoint labels are emitted as sibling text items in
            // the enclosing scope (the SVG renderer builds the legend label
            // group itself).
            if info.gradient.is_some() && !info.gradient_labels.is_empty() {
                let top = y + info.title_height;
                let count = info.gradient_labels.len();
                for (i, label) in info.gradient_labels.iter().enumerate() {
                    let mut text = Item::new("text");
                    text.set("text", json!(label));
                    text.set("font", json!(DEFAULT_FONT));
                    text.set("fontSize", json_number(info.font_size));
                    text.set("fill", json!("#000"));
                    let perc = info
                        .gradient_label_perc
                        .get(i)
                        .copied()
                        .unwrap_or(if i + 1 == count { 1.0 } else { 0.0 });
                    if info.horizontal {
                        text.set("x", json_number(x + perc * info.gradient_length));
                        text.set(
                            "y",
                            json_number(top + info.gradient_thickness + 10.0),
                        );
                        text.set("align", json!("center"));
                    } else {
                        text.set("x", json_number(x + info.gradient_thickness + 2.0));
                        let ty = top + (1.0 - perc) * info.gradient_length;
                        text.set("y", json_number(ty));
                        text.set(
                            "baseline",
                            json!(if perc <= 0.0 {
                                "bottom"
                            } else if perc >= 1.0 {
                                "top"
                            } else {
                                "middle"
                            }),
                        );
                    }
                    children.push(text);
                }
            }
            match info.orient.as_str() {
                "top" => {
                    if info.horizontal {
                        top_legend_y += info.height + 8.0;
                    } else {
                        top_legend_y += info.height + 8.0;
                    }
                }
                "bottom" => {
                    if info.horizontal {
                        bottom_legend_x += info.width + 8.0;
                    } else {
                        bottom_legend_y += info.height + 8.0;
                    }
                }
                _ => legend_y += info.height + 10.0,
            }
        }
        if let Some(title) = group_title {
            let anchor_start = group_title_spec
                .and_then(|t| t.get("anchor"))
                .and_then(|a| a.as_str())
                == Some("start");
            let pre_bounds = if anchor_start {
                Some(scene_bounds(&children))
            } else {
                None
            };
            let item = title_item(
                &title,
                group_title_spec,
                width,
                height,
                &layout,
                pre_bounds,
                &self.config,
                &env,
            );
            children.push(item);
        }
        let base_x = gx;
        let base_y = gy;
        let mut group = Item::new("group");
        group.class.push("mark-group".to_string());
        group.class.push("role-mark".to_string());
        if let Some(name) = mark.get("name").and_then(|n| n.as_str()) {
            group.set("_mark_name", json!(name));
        }
        if let Some(role) = mark.get("role").and_then(|r| r.as_str()) {
            group.set("_role", json!(role));
        }
        group.set("x", json_number(base_x));
        group.set("y", json_number(base_y));
        group.set("width", json_number(width));
        group.set("height", json_number(height));
        group.set("_layout_width", json_number(layout.total_width()));
        group.set("_layout_height", json_number(layout.total_height()));
        // Style: cell background.
        if let Some(style) = mark.get("style") {
            let mut group_props = Map::new();
            self.style_props(style, &mut group_props);
            for (k, v) in group_props {
                if matches!(k.as_str(), "fill" | "stroke" | "strokeWidth" | "opacity") {
                    group.props.insert(k, v);
                }
            }
        }
        // Carry encoded group properties (clip, corner radii, paint) onto the
        // group item so the SVG renderer can emit backgrounds and clip paths.
        for (k, v) in &props {
            if !matches!(k.as_str(), "x" | "y" | "width" | "height" | "x2" | "y2") {
                group.set(k.as_str(), v.clone());
            }
        }
        group.items = children;
        let clip = props
            .get("clip")
            .and_then(|v| v.as_bool())
            .or_else(|| mark.get("clip").and_then(|v| v.as_bool()))
            .unwrap_or(false);
        if clip {
            group.set("_clip", json!([0.0, 0.0, width, height]));
        }
        Some(group)
    }

    fn group_layout(
        &self,
        axes: &[Value],
        legends: &[Value],
        title_spec: Option<&Value>,
        width: f64,
        height: f64,
        env: &Env,
    ) -> GroupLayout {
        let mut layout = GroupLayout {
            width,
            height,
            ..Default::default()
        };
        for a in axes {
            if let Some(info) = build_axis_info(a, &self.config, env, width, height) {
                if info.grid {
                    continue;
                }
                let (ox, oy) = match info.orient.as_str() {
                    "bottom" => (0.0, height + info.offset),
                    "top" => (0.0, -info.offset),
                    "left" => (-info.offset, 0.0),
                    "right" => (width + info.offset, 0.0),
                    _ => (0.0, height),
                };
                let b = info.bounds.translate(ox, oy);
                let r = |v: f64| (v * 1_000_000.0).round() / 1_000_000.0;
                // View extents union every axis.
                layout.left = layout.left.max(r(-b.x1));
                layout.right = layout.right.max(r(b.x2 - width)).max(0.0);
                layout.top = layout.top.max(r(-b.y1));
                layout.bottom = layout.bottom.max(r(b.y2 - height)).max(0.0);
                // Legend anchoring uses only the axis extents on that side:
                // right/top legends sit beyond the right/top axes, ignoring
                // overflow from bottom/top tick labels.
                match info.orient.as_str() {
                    "left" => layout.axis_left = layout.axis_left.max(r(-b.x1)),
                    "right" => {
                        layout.axis_right = layout.axis_right.max(r(b.x2 - width)).max(0.0)
                    }
                    "top" => layout.axis_top = layout.axis_top.max(r(-b.y1)),
                    "bottom" => {
                        layout.axis_bottom = layout.axis_bottom.max(r(b.y2 - height)).max(0.0)
                    }
                    _ => {}
                }
            }
        }
        if std::env::var("VEGA_SVG_DEBUG").is_ok() {
            eprintln!(
                "layout L{} R{} T{} B{} w{} h{}",
                layout.left, layout.right, layout.top, layout.bottom, width, height
            );
        }
        if let Some(t) = title_spec {
            let orient = t.get("orient").and_then(|o| o.as_str()).unwrap_or("top");
            let style = t
                .get("style")
                .and_then(|s| s.as_str())
                .unwrap_or("group-title");
            let fs = t
                .get("fontSize")
                .and_then(|v| v.as_f64())
                .or_else(|| {
                    config_value(&self.config, "style", style)
                        .and_then(|s| s.get("fontSize").and_then(|v| v.as_f64()))
                })
                .unwrap_or(match style {
                    "guide-label" => 10.0,
                    "guide-title" => 11.0,
                    "group-subtitle" => 12.0,
                    _ => 13.0,
                });
            let offset = t.get("offset").and_then(|o| o.as_f64()).unwrap_or(4.0);
            if orient == "left" || orient == "right" {
                layout.left = layout.left.max(layout.axis_left + fs + offset);
                layout.axis_left = layout.axis_left.max(fs + offset);
            } else if orient == "bottom" {
                layout.bottom += fs + offset;
            } else {
                let lines = t
                    .get("text")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(1)
                    .max(1);
                let line_height = t
                    .get("lineHeight")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(fs + 2.0);
                layout.title_top = fs
                    + offset
                    + (lines.saturating_sub(1) as f64) * line_height
                    + if lines > 1 { 11.0 } else { 0.0 };
            }
        }
        let mut top_legends: f64 = 0.0;
        let mut bottom_legends: f64 = 0.0;
        for l in legends {
            if let Some(info) = build_legend_info(l, &self.config, env) {
                match info.orient.as_str() {
                    "left" => layout.left = layout.left.max(layout.axis_left + info.width + 18.0),
                    "right" => {
                        layout.right = layout.right.max(layout.axis_right + info.width + 18.0)
                    }
                    "top" => {
                        if info.horizontal {
                            top_legends = top_legends.max(info.height + 8.0);
                        } else {
                            top_legends += info.height + 8.0;
                        }
                    }
                    "bottom" => {
                        if info.horizontal {
                            bottom_legends = bottom_legends.max(info.height + 8.0);
                        } else {
                            bottom_legends += info.height + 8.0;
                        }
                    }
                    // An unanchored (`orient: none`) legend participates in
                    // the scene bounds through its explicit x/y position but
                    // does not extend the axis margins. Corner orientations
                    // (e.g. bottom-right) are anchored inside the view.
                    "none" | "bottom-right" | "top-right" | "bottom-left" | "top-left" => {}
                    _ => layout.right = layout.right.max(layout.axis_right + info.width + 18.0),
                }
            }
        }
        layout.top += top_legends;
        layout.bottom += bottom_legends;
        layout.left = layout.left.floor();
        layout.right = layout.right.floor();
        layout.top = layout.top.floor();
        layout.bottom = layout.bottom.floor();
        layout.title_top = layout.title_top.floor();
        layout.axis_left = layout.axis_left.floor();
        layout.axis_right = layout.axis_right.floor();
        layout.axis_top = layout.axis_top.floor();
        layout.axis_bottom = layout.axis_bottom.floor();
        layout
    }

    pub fn build(&self, opts: &RenderOptions) -> Result<Item, Error> {
        // `autosize: fit*` resizes the data rectangle so that the *rendered*
        // view keeps the requested outer size. Upstream does this with a
        // resize/reflow cycle; emulate it with a probe pass at the target view
        // size followed by a final pass at the fitted plot size.
        let (auto_type, contains_padding) = match self.spec.get("autosize") {
            Some(Value::String(s)) => (s.clone(), false),
            Some(Value::Object(o)) => (
                o.get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("pad")
                    .to_string(),
                o.get("contains").and_then(|v| v.as_str()) == Some("padding"),
            ),
            _ => ("pad".to_string(), false),
        };
        if auto_type == "fit" {
            let padding = parse_padding(self.spec, &self.config);
            let target_w = self.width
                - if contains_padding {
                    padding.left + padding.right
                } else {
                    0.0
                };
            let target_h = self.height
                - if contains_padding {
                    padding.top + padding.bottom
                } else {
                    0.0
                };
            let mut probe_spec = self.spec.clone();
            if let Value::Object(o) = &mut probe_spec {
                o.insert("width".into(), json_number(target_w));
                o.insert("height".into(), json_number(target_h));
                o.insert("autosize".into(), json!("pad"));
            }
            let probe = Runtime::new(&probe_spec, opts)?.build_inner(opts)?;
            let left = probe.f64("_extent_left").unwrap_or(0.0);
            let right = probe.f64("_extent_right").unwrap_or(0.0);
            let top = probe.f64("_extent_top").unwrap_or(0.0);
            let bottom = probe.f64("_extent_bottom").unwrap_or(0.0);
            let fit_w = if auto_type == "fit" || auto_type == "fit-x" {
                (target_w - left - right).max(0.0)
            } else {
                target_w
            };
            let fit_h = if auto_type == "fit" || auto_type == "fit-y" {
                (target_h - top - bottom).max(0.0)
            } else {
                target_h
            };
            let mut final_spec = self.spec.clone();
            if let Value::Object(o) = &mut final_spec {
                o.insert("width".into(), json_number(fit_w));
                o.insert("height".into(), json_number(fit_h));
                o.insert("autosize".into(), json!("pad"));
            }
            return Runtime::new(&final_spec, opts)?.build_inner(opts);
        }
        self.build_inner(opts)
    }

    fn build_inner(&self, opts: &RenderOptions) -> Result<Item, Error> {
        let padding = parse_padding(self.spec, &self.config);
        let env = self.env();
        let axes: Vec<Value> = self
            .spec
            .get("axes")
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default();
        let legends: Vec<Value> = self
            .spec
            .get("legends")
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default();
        let title_spec = self.spec.get("title");
        let title = title_spec
            .and_then(|t| title_value(t, &Value::Null, &env))
            .filter(|t| !t.is_empty());
        let mut layout =
            self.group_layout(&axes, &legends, title_spec, self.width, self.height, &env);

        let axis_infos: Vec<AxisInfo> = axes
            .iter()
            .filter_map(|a| build_axis_info(a, &self.config, &env, self.width, self.height))
            .collect();
        let legend_infos: Vec<LegendInfo> = legends
            .iter()
            .filter_map(|l| build_legend_info(l, &self.config, &env))
            .collect();

        let mut children = Vec::new();
        let mut high_grid_axes = Vec::new();
        for info in axis_infos.iter().filter(|a| a.grid) {
            if info.zindex > 0.0 {
                high_grid_axes.push(info);
            } else {
                children.push(axis_item(info, self.width, self.height, &env, false));
            }
        }

        let marks: Vec<Value> = self
            .spec
            .get("marks")
            .and_then(|m| m.as_array())
            .cloned()
            .unwrap_or_default();
        let mut mark_items = Vec::new();
        let empty_local = HashMap::new();
        for m in &marks {
            for item in self.build_mark(m, &empty_local, &HashMap::new(), self.width, self.height) {
                mark_items.push(item);
            }
        }
        if let Some(layout_spec) = self.spec.get("layout") {
            layout_groups(&mut mark_items, layout_spec, &self.env());
        }
        let mut content_width: f64 = 0.0;
        let mut content_height: f64 = 0.0;
        for item in &mark_items {
            if item.kind == "group" {
                let x = item.f64("x").unwrap_or(0.0);
                let y = item.f64("y").unwrap_or(0.0);
                let w = item
                    .f64("_layout_width")
                    .unwrap_or(item.f64("width").unwrap_or(0.0));
                let h = item
                    .f64("_layout_height")
                    .unwrap_or(item.f64("height").unwrap_or(0.0));
                content_width = content_width.max(x + w);
                content_height = content_height.max(y + h);
            }
        }
        children.extend(mark_items);
        for info in high_grid_axes {
            children.push(axis_item(info, self.width, self.height, &env, false));
        }

        // Legend anchors derive from marks and scope groups, not from axes.
        let legend_anchor = legend_anchor_bounds(&children, self.width, self.height);

        for info in axis_infos.iter().filter(|a| !a.grid) {
            children.push(axis_item(info, self.width, self.height, &env, false));
        }
        let mut legend_y = 0.0;
        let mut top_legend_y = 0.0;
        let mut bottom_legend_y = 0.0;
        let mut bottom_legend_x = 0.0;
        for info in &legend_infos {
            let mut item = legend_item(info, &env);
            let (x, y) = match info.orient.as_str() {
                "left" => (
                    legend_anchor.x1.min(0.0).floor() - info.offset - info.width,
                    legend_y,
                ),
                "right" => (legend_anchor.x2.max(self.width).ceil() + info.offset, legend_y),
                "top" => (
                    0.0,
                    legend_anchor.y1.min(0.0).floor() - info.offset - top_legend_y - info.height,
                ),
                "bottom" => (
                    if info.horizontal { bottom_legend_x } else { 0.0 },
                    legend_anchor.y2.max(self.height).ceil() + info.offset + bottom_legend_y,
                ),
                "bottom-right" => (
                    self.width - info.width - info.offset,
                    self.height - info.height - info.offset,
                ),
                "top-right" => (
                    legend_anchor.x2.max(self.width).ceil() - info.width - info.offset,
                    legend_anchor.y1.min(0.0).floor() + info.offset,
                ),
                "bottom-left" => (
                    legend_anchor.x1.min(0.0).floor() + info.offset,
                    legend_anchor.y2.max(self.height).ceil() - info.height - info.offset,
                ),
                "top-left" => (
                    legend_anchor.x1.min(0.0).floor(),
                    legend_anchor.y1.min(0.0).floor(),
                ),
                "none" => (
                    info.legend_x
                        .unwrap_or(legend_anchor.x2.max(self.width) + 18.0),
                    info.legend_y.unwrap_or(legend_y),
                ),
                _ => (legend_anchor.x2.max(self.width).ceil() + info.offset, legend_y),
            };
            item.set("x", json_number(x));
            item.set("y", json_number(y));
            children.push(item);
            if info.gradient.is_some() && !info.gradient_labels.is_empty() {
                let top = y + info.title_height;
                let count = info.gradient_labels.len();
                for (i, label) in info.gradient_labels.iter().enumerate() {
                    let mut text = Item::new("text");
                    text.set("text", json!(label));
                    text.set("font", json!(DEFAULT_FONT));
                    text.set("fontSize", json_number(info.font_size));
                    text.set("fill", json!("#000"));
                    let perc = info
                        .gradient_label_perc
                        .get(i)
                        .copied()
                        .unwrap_or(if i + 1 == count { 1.0 } else { 0.0 });
                    if info.horizontal {
                        text.set("x", json_number(x + perc * info.gradient_length));
                        text.set(
                            "y",
                            json_number(top + info.gradient_thickness + 10.0),
                        );
                        text.set("align", json!("center"));
                    } else {
                        text.set("x", json_number(x + info.gradient_thickness + 2.0));
                        let ty = top + (1.0 - perc) * info.gradient_length;
                        text.set("y", json_number(ty));
                        text.set(
                            "baseline",
                            json!(if perc <= 0.0 {
                                "bottom"
                            } else if perc >= 1.0 {
                                "top"
                            } else {
                                "middle"
                            }),
                        );
                    }
                    children.push(text);
                }
            }
            match info.orient.as_str() {
                "top" => {
                    if info.horizontal {
                        top_legend_y += info.height + 8.0;
                    } else {
                        top_legend_y += info.height + 8.0;
                    }
                }
                "bottom" => {
                    if info.horizontal {
                        bottom_legend_x += info.width + 8.0;
                    } else {
                        bottom_legend_y += info.height + 8.0;
                    }
                }
                _ => legend_y += info.height + 10.0,
            }
        }
        if let Some(title) = title {
            let pre_bounds = scene_bounds(&children);
            let item = title_item(
                &title,
                title_spec,
                self.width,
                self.height,
                &layout,
                Some(pre_bounds),
                &self.config,
                &env,
            );
            children.push(item);
        }

        let mut content_bounds = scene_bounds(&children);
        content_bounds.add(0.0, 0.0);
        content_bounds.add(self.width, self.height);
        // For a top title the group offset is already reflected in the
        // title's own bounds; do not add it a second time below.
        let title_orient = title_spec
            .and_then(|t| t.get("orient"))
            .and_then(|o| o.as_str())
            .unwrap_or("top");
        if title_spec.is_some() && !matches!(title_orient, "bottom" | "left" | "right") {
            layout.title_top = 0.0;
        }
        let r6 = |v: f64| (v * 1_000_000.0).round() / 1_000_000.0;
        let base_width = self.width.max(0.0);
        let base_height = self.height.max(0.0);
        let mut left = r6(-content_bounds.x1).max(0.0).ceil();
        let mut top = r6(-content_bounds.y1).max(0.0).ceil();
        let right = r6(content_bounds.x2 - base_width).max(0.0).ceil();
        let bottom = r6(content_bounds.y2 - base_height).max(0.0).ceil();

        // Autosize (vega-view-transforms' viewSizeLayout).
        let (auto_type, contains_padding) = match self.spec.get("autosize") {
            Some(Value::String(s)) => (s.clone(), false),
            Some(Value::Object(o)) => (
                o.get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("pad")
                    .to_string(),
                o.get("contains").and_then(|v| v.as_str()) == Some("padding"),
            ),
            _ => ("pad".to_string(), false),
        };
        let mut view_width = base_width;
        let mut view_height = base_height;
        if contains_padding {
            view_width -= padding.left + padding.right;
            view_height -= padding.top + padding.bottom;
        }
        let mut root_width = base_width;
        let mut root_height = base_height;
        match auto_type.as_str() {
            "none" => {
                left = 0.0;
                top = 0.0;
                root_width = view_width;
                root_height = view_height;
            }
            "fit" => {
                root_width = (view_width - left - right).max(0.0);
                root_height = (view_height - top - bottom).max(0.0);
            }
            "fit-x" => {
                root_width = (view_width - left - right).max(0.0);
                view_height = base_height + top + bottom;
            }
            "fit-y" => {
                view_width = base_width + left + right;
                root_height = (view_height - top - bottom).max(0.0);
            }
            _ => {
                view_width = base_width + left + right;
                view_height = base_height + top + bottom;
            }
        }
        layout.left = left;
        layout.top = top;
        layout.right = right;
        layout.bottom = bottom;

        let mut root = Item::new("group");
        root.class.push("mark-group".to_string());
        root.class.push("role-frame".to_string());
        root.class.push("root".to_string());
        root.set("x", json_number(padding.left + layout.left));
        root.set(
            "y",
            json_number(padding.top + layout.title_top + layout.top),
        );
        root.set("width", json_number(root_width));
        root.set("height", json_number(root_height));
        root.set("_extent_left", json_number(left));
        root.set("_extent_right", json_number(right));
        root.set("_extent_top", json_number(top));
        root.set("_extent_bottom", json_number(bottom));
        let mut root_props = Map::new();
        if let Some(style) = self.spec.get("style") {
            self.style_props(style, &mut root_props);
        }
        // Top-level `encode.update` paints the view background (e.g. the
        // `view: {fill: ...}` configuration compiled into the root group).
        if let Some(update) = self
            .spec
            .get("encode")
            .and_then(|e| e.get("update"))
            .and_then(|u| u.as_object())
        {
            for (prop, enc) in update {
                if !matches!(prop.as_str(), "fill" | "stroke" | "strokeWidth" | "opacity") {
                    continue;
                }
                if let Some(v) = encoder_value(enc, &Value::Null, &Value::Null, &env) {
                    root_props.insert(prop.clone(), v);
                }
            }
        }
        for (k, v) in root_props {
            if matches!(k.as_str(), "fill" | "stroke" | "strokeWidth" | "opacity") {
                root.props.insert(k, v);
            }
        }
        let mut content = Item::new("group");
        content.items = children;
        let mut inner = Item::new("group");
        inner.items.push(content);
        root.items = vec![inner];

        // SVG dimensions include padding, axes and legends.
        let total_width = view_width + padding.left + padding.right;
        let total_height = view_height + padding.top + padding.bottom;
        root.set("_svg_width", json_number(total_width * opts.scale));
        root.set("_svg_height", json_number(total_height * opts.scale));
        root.set("_view_width", json_number(total_width));
        root.set("_view_height", json_number(total_height));
        Ok(root)
    }

    pub fn background(&self, opts: &RenderOptions) -> Option<String> {
        if let Some(bg) = &opts.background {
            return Some(bg.clone());
        }
        if let Some(Value::String(bg)) = self.spec.get("background") {
            return Some(bg.clone());
        }
        None
    }
}

fn item_stroke_half(item: &Item) -> f64 {
    if item.get("stroke").map(|v| !v.is_null()).unwrap_or(false) {
        let sw = item.f64("strokeWidth").unwrap_or(1.0);
        let cap = item.str("strokeCap");
        let join = item.str("strokeJoin");
        let mut e = if cap == Some("square") {
            std::f64::consts::SQRT_2 * sw / 2.0
        } else {
            sw / 2.0
        };
        if join.is_none() || join == Some("miter") {
            let limit = item.f64("strokeMiterLimit").unwrap_or(4.0);
            e = e.max(limit * sw / 2.0);
        }
        e
    } else {
        0.0
    }
}

fn item_bounds(item: &Item) -> crate::bounds::Bounds {
    let b = item_bounds_plain(item);
    // Marks with `clip: true` are intersected with their enclosing group's
    // dimensions by vega-scenegraph's boundClip().
    if let Some(v) = item.get("_clip").and_then(|v| v.as_array()) {
        if v.len() == 4 {
            let mut clip = crate::bounds::Bounds::new(
                to_number(&v[0]),
                to_number(&v[1]),
                to_number(&v[2]),
                to_number(&v[3]),
            );
            if item.kind == "group" {
                clip = clip.translate(
                    item.f64("x").unwrap_or(0.0),
                    item.f64("y").unwrap_or(0.0),
                );
            }
            let mut out = b;
            out.intersect(clip);
            return out;
        }
    }
    b
}

fn item_bounds_plain(item: &Item) -> crate::bounds::Bounds {
    use crate::bounds::{text_bounds, Bounds};
    match item.kind.as_str() {
        "group" => {
            if let Some(v) = item.get("_bounds").and_then(|v| v.as_array()) {
                if v.len() == 4 {
                    let b = Bounds::new(
                        to_number(&v[0]),
                        to_number(&v[1]),
                        to_number(&v[2]),
                        to_number(&v[3]),
                    );
                    if item.class.iter().any(|c| c == "role-axis") {
                        return match item.str("_axis_orient") {
                            Some("left") | Some("right") => {
                                b.translate(item.f64("x").unwrap_or(0.0), 0.0)
                            }
                            _ => b.translate(0.0, item.f64("y").unwrap_or(0.0)),
                        };
                    }
                    return b.translate(item.f64("x").unwrap_or(0.0), item.f64("y").unwrap_or(0.0));
                }
            }
            let mut b = Bounds::empty();
            for c in &item.items {
                b.union(item_bounds(c));
            }
            let w = item.f64("width").unwrap_or(0.0);
            let h = item.f64("height").unwrap_or(0.0);
            if w != 0.0 || h != 0.0 {
                b.add(0.0, 0.0);
                b.add(w, h);
            }
            // Group bounds use a plain centered stroke (no miter extension).
            let e = if item.get("stroke").map(|v| !v.is_null()).unwrap_or(false) {
                item.f64("strokeWidth").unwrap_or(1.0) / 2.0
            } else {
                0.0
            };
            if e > 0.0 {
                b.x1 -= e;
                b.y1 -= e;
                b.x2 += e;
                b.y2 += e;
            }
            b.translate(item.f64("x").unwrap_or(0.0), item.f64("y").unwrap_or(0.0))
        }
        "rect" => {
            let x = item.f64("x").unwrap_or(0.0);
            let y = item.f64("y").unwrap_or(0.0);
            let w = item.f64("width").unwrap_or(0.0);
            let h = item.f64("height").unwrap_or(0.0);
            let mut b = Bounds::new(x, y, x + w, y + h);
            let e = item.f64("strokeWidth").unwrap_or(1.0) / 2.0;
            if item.get("stroke").is_some() {
                b.x1 -= e;
                b.y1 -= e;
                b.x2 += e;
                b.y2 += e;
            }
            b
        }
        "rule" => {
            let x = item.f64("x").unwrap_or(0.0);
            let y = item.f64("y").unwrap_or(0.0);
            let x2 = item.f64("x2").unwrap_or(x);
            let y2 = item.f64("y2").unwrap_or(y);
            let sw = item.f64("strokeWidth").unwrap_or(1.0);
            crate::bounds::rule_bounds(x, y, x2, y2, sw)
        }
        "symbol" => {
            let x = item.f64("x").unwrap_or(0.0);
            let y = item.f64("y").unwrap_or(0.0);
            let size = item.f64("size").unwrap_or(64.0);
            // d3 symbols size themselves from r = sqrt(size) / 2.
            let r = size.max(0.0).sqrt() / 2.0;
            let e = item_stroke_half(item);
            Bounds::new(x - r - e, y - r - e, x + r + e, y + r + e)
        }
        "image" => {
            let x = item.f64("x").unwrap_or(0.0);
            let y = item.f64("y").unwrap_or(0.0);
            let w = item.f64("width").unwrap_or(0.0);
            let h = item.f64("height").unwrap_or(0.0);
            Bounds::new(x, y, x + w, y + h)
        }
        "text" => {
            let x = item.f64("x").unwrap_or(0.0);
            let y = item.f64("y").unwrap_or(0.0);
            let fs = item.f64("fontSize").unwrap_or(11.0);
            let raw_text = item.get("text");
            let (text, extra_lines) = match raw_text {
                Some(Value::Array(lines)) if lines.len() > 1 => {
                    let best = lines
                        .iter()
                        .map(|l| to_string_js(l))
                        .max_by(|a, b| {
                            text_width(a, fs)
                                .partial_cmp(&text_width(b, fs))
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .unwrap_or_default();
                    let lh = item.f64("lineHeight").unwrap_or(fs + 2.0);
                    (best, lh * (lines.len() - 1) as f64)
                }
                Some(v) => (to_string_js(v), 0.0),
                None => (String::new(), 0.0),
            };
            let mut b = text_bounds(
                x,
                y,
                &text,
                fs,
                item.str("align"),
                item.str("baseline"),
                item.f64("dx").unwrap_or(0.0),
                item.f64("dy").unwrap_or(0.0),
                item.f64("angle").unwrap_or(0.0),
            );
            if extra_lines != 0.0 {
                b.y2 += extra_lines;
            }
            if let Some(r) = item.f64("radius") {
                if r != 0.0 {
                    let t = item.f64("theta").unwrap_or(0.0) - std::f64::consts::FRAC_PI_2;
                    b = b.translate(r * t.cos(), r * t.sin());
                }
            }
            b
        }
        "arc" => {
            let x = item.f64("x").unwrap_or(0.0);
            let y = item.f64("y").unwrap_or(0.0);
            let a0 = item.f64("startAngle").unwrap_or(0.0);
            let a1 = item.f64("endAngle").unwrap_or(0.0);
            let mut ir = item.f64("innerRadius").unwrap_or(0.0).max(0.0);
            let mut or = item.f64("outerRadius").unwrap_or(0.0).max(0.0);
            if or < ir {
                std::mem::swap(&mut or, &mut ir);
            }
            let lo = a0.min(a1);
            let hi = a0.max(a1);
            let mut b = Bounds::empty();
            // Endpoints of the outer/inner arcs plus any cardinal-angle
            // extrema crossed by the sweep.
            for radius in [or, ir] {
                if radius <= 0.0 {
                    continue;
                }
                for &angle in &[lo, hi] {
                    let pt = (x + radius * angle.sin(), y - radius * angle.cos());
                    b.add(pt.0, pt.1);
                }
                let step = std::f64::consts::FRAC_PI_2;
                let start = (lo / step).ceil() as i64;
                let end = (hi / step).floor() as i64;
                for k in start..=end {
                    let angle = k as f64 * step;
                    let pt = (x + radius * angle.sin(), y - radius * angle.cos());
                    b.add(pt.0, pt.1);
                }
            }
            if ir <= 0.0 {
                b.add(x, y);
            }
            let e = item_stroke_half(item);
            b.x1 -= e;
            b.y1 -= e;
            b.x2 += e;
            b.y2 += e;
            b
        }
        "path" | "shape" => {
            let x = item.f64("x").unwrap_or(0.0);
            let y = item.f64("y").unwrap_or(0.0);
            let d = item.str("path").or_else(|| item.str("shape")).unwrap_or("");
            let mut b = crate::bounds::path_bounds(d);
            if b.is_empty() {
                b = Bounds::new(x, y, x, y);
            } else {
                let e = item_stroke_half(item);
                b.x1 = b.x1.min(x) - e;
                b.y1 = b.y1.min(y) - e;
                b.x2 = b.x2.max(x) + e;
                b.y2 = b.y2.max(y) + e;
            }
            b
        }
        "line" | "area" | "trail" => {
            let mut b = Bounds::empty();
            for v in &item.items {
                if v.get("defined")
                    .map(|d| d.as_bool().unwrap_or(true))
                    .unwrap_or(true)
                    == false
                {
                    continue;
                }
                let vx = v.f64("x").unwrap_or(0.0);
                let vy = v.f64("y").unwrap_or(0.0);
                if item.kind == "trail" {
                    let r = v.f64("size").unwrap_or(2.0) / 2.0;
                    b.add(vx - r, vy - r);
                    b.add(vx + r, vy + r);
                } else {
                    b.add(vx, vy);
                }
                if let Some(y2) = v.f64("y2") {
                    b.add(v.f64("x").unwrap_or(0.0), y2);
                }
                if let Some(x2) = v.f64("x2") {
                    b.add(x2, v.f64("y").unwrap_or(0.0));
                }
            }
            if !b.is_empty() {
                let e = item_stroke_half(item);
                if e > 0.0 {
                    b.x1 -= e;
                    b.y1 -= e;
                    b.x2 += e;
                    b.y2 += e;
                }
            }
            b
        }
        _ => Bounds::empty(),
    }
}

fn legend_anchor_bounds(items: &[Item], width: f64, height: f64) -> crate::bounds::Bounds {
    let mut b = crate::bounds::Bounds::new(0.0, 0.0, width, height);
    for it in items {
        if it.get("_grid").and_then(|v| v.as_bool()).unwrap_or(false) {
            continue;
        }
        let role = it.get("_role").and_then(|v| v.as_str()).unwrap_or("");
        let is_scope_group = it.kind == "group"
            && it.class.iter().any(|c| c == "mark-group")
            && !matches!(
                role,
                "axis" | "legend" | "title" | "row-title" | "column-title"
            );
        if matches!(role, "scope" | "frame") || is_scope_group {
            b.union(item_bounds(it));
        }
    }
    b
}

fn facet_sort_value(field: &Value, datum: &Value, env: &Env) -> Value {
    if let Some(expr) = field.get("expr").and_then(|v| v.as_str()) {
        let wrapped = match datum {
            Value::Object(map) => {
                let mut m = map.clone();
                m.insert("datum".to_string(), datum.clone());
                Value::Object(m)
            }
            _ => json!({ "datum": datum.clone() }),
        };
        expression::evaluate(expr, Some(&wrapped), env).unwrap_or(Value::Null)
    } else if let Some(name) = field.as_str() {
        let path = name
            .trim_start_matches("datum[")
            .trim_end_matches(']')
            .trim_matches('"');
        get_field_owned(datum, path)
    } else {
        Value::Null
    }
}

fn scene_bounds(items: &[Item]) -> crate::bounds::Bounds {
    let mut b = crate::bounds::Bounds::empty();
    for it in items {
        if it.get("_grid").and_then(|v| v.as_bool()).unwrap_or(false) {
            continue;
        }
        b.union(item_bounds(it));
    }
    b
}

fn copy_prop(props: &Map<String, Value>, item: &mut Item, key: &str) {
    if let Some(v) = props.get(key) {
        item.set(key, v.clone());
    }
}

fn sort_data(values: &mut [Value], sort: &Value) {
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
        Some(Value::Array(a)) => a.iter().map(|v| v.as_str() == Some("descending")).collect(),
        _ => vec![],
    };
    values.sort_by(|a, b| {
        for (i, f) in fields.iter().enumerate() {
            let field = f
                .trim_start_matches("datum[")
                .trim_end_matches(']')
                .trim_matches('"');
            let mut ord = compare(&get_field_owned(a, field), &get_field_owned(b, field));
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

fn encoder_value(enc: &Value, datum: &Value, item: &Value, env: &Env) -> Option<Value> {
    match enc {
        Value::Array(rules) => {
            for rule in rules {
                if let Some(test) = rule.get("test") {
                    if !test_encoder(test, datum, item, env) {
                        continue;
                    }
                    return encoder_value(rule, datum, item, env);
                }
            }
            for rule in rules {
                if rule.get("test").is_none() {
                    return encoder_value(rule, datum, item, env);
                }
            }
            None
        }
        Value::Object(o) => {
            if let Some(test) = o.get("test") {
                if !test_encoder(test, datum, item, env) {
                    return None;
                }
            }
            let mut value = if let Some(sig) = o.get("signal").and_then(|s| s.as_str()) {
                expression::evaluate(sig, Some(datum), env).ok()?
            } else if let Some(field) = o.get("field") {
                if let Some(group) = field.get("group").and_then(|g| g.as_str()) {
                    match group {
                        "width" => json_number(env.group_width),
                        "height" => json_number(env.group_height),
                        other => get_field_owned(datum, other),
                    }
                } else {
                    field_value(datum, field)
                }
            } else if let Some(v) = o.get("value") {
                v.clone()
            } else {
                Value::Null
            };
            if let Some(scale_name) = o.get("scale").and_then(|s| s.as_str()) {
                if let Some(scale) = env.get_scale(scale_name) {
                    value = if let Some(band) = o.get("band").and_then(|b| b.as_f64()) {
                        scale.scale_band(&value, band)
                    } else {
                        scale.scale(&value)
                    };
                    if value.is_null() {
                        return None;
                    }
                    if let Some(mult) = o.get("mult") {
                        let m = encoder_number(mult, datum, item, env).unwrap_or(1.0);
                        value = json_number(to_number(&value) * m);
                    }
                }
            } else if let Some(mult) = o.get("mult") {
                let m = encoder_number(mult, datum, item, env).unwrap_or(1.0);
                value = json_number(to_number(&value) * m);
            }
            if let Some(offset) = o.get("offset") {
                let off = encoder_value(offset, datum, item, env);
                if let Some(off) = off {
                    value = if value.is_string() && off.is_string() {
                        Value::String(format!(
                            "{}{}",
                            value.as_str().unwrap_or(""),
                            off.as_str().unwrap_or("")
                        ))
                    } else {
                        json_number(to_number(&value) + to_number(&off))
                    };
                }
            }
            Some(value)
        }
        other => Some(other.clone()),
    }
}

fn encoder_number(enc: &Value, datum: &Value, item: &Value, env: &Env) -> Option<f64> {
    encoder_value(enc, datum, item, env).map(|v| to_number(&v))
}

fn test_encoder(test: &Value, datum: &Value, item: &Value, env: &Env) -> bool {
    let v = if let Some(sig) = test.as_str() {
        expression::evaluate(sig, Some(datum), env).unwrap_or(Value::Bool(true))
    } else if let Some(o) = test.as_object() {
        if let Some(sig) = o.get("signal").and_then(|s| s.as_str()) {
            expression::evaluate(sig, Some(datum), env).unwrap_or(Value::Bool(true))
        } else if let Some(v) = o.get("value") {
            v.clone()
        } else {
            Value::Bool(true)
        }
    } else {
        test.clone()
    };
    let _ = item;
    truthy(&v)
}

fn normalize_item(item: &mut Item, kind: &str) {
    match kind {
        "rect" => {
            let x = item.f64("x");
            let xc = item.f64("xc");
            let width = item.f64("width");
            let x2 = item.f64("x2");
            let (x, width) = match (x, xc, width, x2) {
                (Some(x), _, _, Some(x2)) => (x, x2 - x),
                (Some(x), _, Some(w), _) => (x, w),
                (None, Some(xc), Some(w), _) => (xc - w / 2.0, w),
                (None, None, Some(w), _) => (0.0, w),
                (None, None, None, Some(x2)) => (0.0, x2),
                (Some(x), None, None, None) => (x, 0.0),
                _ => (0.0, 0.0),
            };
            let (x, width) = if width < 0.0 {
                (x + width, -width)
            } else {
                (x, width)
            };
            let y = item.f64("y");
            let yc = item.f64("yc");
            let height = item.f64("height");
            let y2 = item.f64("y2");
            let (y, height) = match (y, yc, height, y2) {
                (Some(y), _, _, Some(y2)) => (y, y2 - y),
                (Some(y), _, Some(h), _) => (y, h),
                (None, Some(yc), Some(h), _) => (yc - h / 2.0, h),
                (None, None, Some(h), _) => (0.0, h),
                (None, None, None, Some(y2)) => (0.0, y2),
                (Some(y), None, None, None) => (y, 0.0),
                _ => (0.0, 0.0),
            };
            let (y, height) = if height < 0.0 {
                (y + height, -height)
            } else {
                (y, height)
            };
            item.set("x", json_number(x));
            item.set("y", json_number(y));
            item.set("width", json_number(width));
            item.set("height", json_number(height));
        }
        "rule" => {
            if item.get("x2").is_none() {
                item.set("x2", item.get("x").cloned().unwrap_or(json!(0)));
            }
            if item.get("y2").is_none() {
                item.set("y2", item.get("y").cloned().unwrap_or(json!(0)));
            }
        }
        "symbol" => {
            if item.get("x").is_none() {
                item.set("x", json!(0));
            }
            if item.get("y").is_none() {
                item.set("y", json!(0));
            }
            if item.get("size").is_none() {
                item.set("size", json!(64));
            }
        }
        "image" => {
            let width = item.f64("width").unwrap_or(0.0);
            let height = item.f64("height").unwrap_or(0.0);
            let x = match (item.f64("x"), item.f64("xc")) {
                (Some(x), _) => x,
                (None, Some(xc)) => xc - width / 2.0,
                _ => 0.0,
            };
            let y = match (item.f64("y"), item.f64("yc")) {
                (Some(y), _) => y,
                (None, Some(yc)) => yc - height / 2.0,
                _ => 0.0,
            };
            item.set("x", json_number(x));
            item.set("y", json_number(y));
            item.set("width", json_number(width));
            item.set("height", json_number(height));
        }
        "text" => {
            if item.get("x").is_none() {
                item.set("x", json!(0));
            }
            if item.get("y").is_none() {
                item.set("y", json!(0));
            }
            if item.get("font").is_none() {
                item.set("font", json!(DEFAULT_FONT));
            }
            if item.get("fontSize").is_none() {
                item.set("fontSize", json!(11));
            }
        }
        "path" | "shape" => {
            if item.get("x").is_none() {
                item.set("x", json!(0));
            }
            if item.get("y").is_none() {
                item.set("y", json!(0));
            }
        }
        _ => {}
    }
}

/// Apply per-axis `encode.<part>.update` overrides (grid/ticks/labels).
fn apply_axis_encode(
    item: &mut Item,
    info: &AxisInfo,
    part: &str,
    datum: &Value,
    env: &Env,
) {
    if let Some(update) = info
        .spec
        .get("encode")
        .and_then(|e| e.get(part))
        .and_then(|p| p.get("update"))
        .and_then(|u| u.as_object())
    {
        for (prop, enc) in update {
            if let Some(v) = encoder_value(enc, datum, &Value::Null, env) {
                item.set(prop, v);
            }
        }
    }
}

fn axis_item(info: &AxisInfo, width: f64, height: f64, env: &Env, nested: bool) -> Item {
    let mut group = Item::new("group");
    group.class.push("mark-group".to_string());
    group.class.push("role-axis".to_string());
    // vega-view-transforms' `axisLayout` nudges every positioned axis by the
    // default `translate` delta (0.5px) so 1px rules render crisply.
    let delta = if nested { 0.0 } else { 0.5 };
    let (ox, oy) = match info.orient.as_str() {
        "bottom" => (delta, height + info.offset + delta),
        "top" => (delta, -info.offset + delta),
        "left" => (-info.offset + delta, delta),
        "right" => (width + info.offset + delta, delta),
        _ => (0.0, height),
    };
    group.set("x", json_number(ox));
    group.set("y", json_number(oy));
    group.set("_axis_orient", json!(info.orient));
    group.set("_nested_axis", json!(nested));
    group.set(
        "_bounds",
        json!([
            info.bounds.x1 - delta,
            info.bounds.y1 - delta,
            info.bounds.x2 - delta,
            info.bounds.y2 - delta
        ]),
    );
    let scale = match env.get_scale(&info.scale_name) {
        Some(s) => s,
        None => return group,
    };
    let sign = if info.orient == "top" || info.orient == "left" {
        -1.0
    } else {
        1.0
    };
    let _len = if info.orient == "left" || info.orient == "right" {
        height
    } else {
        width
    };

    if info.grid {
        let mut grid_group = Item::new("group");
        grid_group.class.push("mark-rule".to_string());
        grid_group.class.push("role-axis-grid".to_string());
        let grid_sign = if info.orient == "left" || info.orient == "top" {
            1.0
        } else {
            -1.0
        };
        let grid_range = info
            .grid_scale
            .as_ref()
            .and_then(|n| env.get_scale(n))
            .map(|s| {
                let (a, b) = scale::range_pair(s);
                (a * grid_sign, b * grid_sign)
            })
            .unwrap_or_else(|| {
                let len = if info.orient == "left" || info.orient == "right" {
                    width
                } else {
                    height
                };
                (0.0, len * grid_sign)
            });
        for v in &info.ticks {
            let mut line = Item::new("rule");
            if info.orient == "left" || info.orient == "right" {
                let y = tick_pos_band(scale, v, info.band_position).round();
                line.set("x", json_number(grid_range.0));
                line.set("x2", json_number(grid_range.1));
                line.set("y", json_number(y));
                line.set("y2", json_number(y));
            } else {
                let x = tick_pos_band(scale, v, info.band_position).round();
                line.set("x", json_number(x));
                line.set("x2", json_number(x));
                line.set("y", json_number(grid_range.0));
                line.set("y2", json_number(grid_range.1));
            }
            line.set("stroke", json!(info.grid_color));
            line.set("strokeWidth", json_number(1.0));
            line.set("opacity", json_number(1.0));
            apply_axis_encode(&mut line, info, "grid", &json!({ "value": v }), env);
            grid_group.items.push(line);
        }
        // `tickBand: "extent"` adds the boundary tick at the start of the
        // first band (the regular ticks sit at the band ends).
        if info.band_position == 1.0 {
            if let Some(first) = info.ticks.first() {
                let mut line = Item::new("rule");
                if info.orient == "left" || info.orient == "right" {
                    let y = tick_pos_band(scale, first, 0.0).round();
                    line.set("x", json_number(grid_range.0));
                    line.set("x2", json_number(grid_range.1));
                    line.set("y", json_number(y));
                    line.set("y2", json_number(y));
                } else {
                    let x = tick_pos_band(scale, first, 0.0).round();
                    line.set("x", json_number(x));
                    line.set("x2", json_number(x));
                    line.set("y", json_number(grid_range.0));
                    line.set("y2", json_number(grid_range.1));
                }
                line.set("stroke", json!(info.grid_color));
                line.set("strokeWidth", json_number(1.0));
                line.set("opacity", json_number(1.0));
                apply_axis_encode(&mut line, info, "grid", &json!({ "value": first }), env);
                grid_group.items.push(line);
            }
        }
        group.items.push(grid_group);
        group.set("_grid", json!(true));
        let mut content = Item::new("group");
        content.items = std::mem::take(&mut group.items);
        let mut middle = Item::new("group");
        middle.items.push(content);
        group.items.push(middle);
        return group;
    }

    let domain = info.show_domain;
    let ticks_on = info.show_ticks;
    let labels_on = info.show_labels;

    if domain {
        let mut line = Item::new("rule");
        let (d0, d1) = scale::range_pair(scale);
        if info.orient == "left" || info.orient == "right" {
            line.set("x", json!(0));
            line.set("x2", json!(0));
            line.set("y", json_number(d0));
            line.set("y2", json_number(d1));
        } else {
            line.set("x", json_number(d0));
            line.set("x2", json_number(d1));
            line.set("y", json!(0));
            line.set("y2", json!(0));
        }
        line.set("stroke", json!(info.domain_color));
        line.set("strokeWidth", json_number(1.0));
        line.set("opacity", json_number(1.0));
        let mut domain_group = Item::new("group");
        domain_group.class.push("mark-rule".to_string());
        domain_group.class.push("role-axis-domain".to_string());
        domain_group.items.push(line);
        group.items.push(domain_group);
    }

    if ticks_on {
        let mut tick_group = Item::new("group");
        tick_group.class.push("mark-rule".to_string());
        tick_group.class.push("role-axis-tick".to_string());
        for v in &info.ticks {
            let raw = tick_pos_band(scale, v, info.band_position);
            let p = (raw + info.tick_offset).round();
            let mut line = Item::new("rule");
            if info.orient == "left" || info.orient == "right" {
                line.set("x", json!(0));
                line.set("x2", json_number(sign * info.tick_size));
                line.set("y", json_number(p));
                line.set("y2", json_number(p));
            } else {
                line.set("x", json_number(p));
                line.set("x2", json_number(p));
                line.set("y", json!(0));
                line.set("y2", json_number(sign * info.tick_size));
            }
            line.set("stroke", json!(info.tick_color));
            line.set("strokeWidth", json_number(1.0));
            line.set("opacity", json_number(1.0));
            apply_axis_encode(&mut line, info, "ticks", &json!({ "value": v }), env);
            tick_group.items.push(line);
        }
        if info.band_position == 1.0 {
            if let Some(first) = info.ticks.first() {
                let raw = tick_pos_band(scale, first, 0.0);
                let p = (raw + info.tick_offset).round();
                let mut line = Item::new("rule");
                if info.orient == "left" || info.orient == "right" {
                    line.set("x", json!(0));
                    line.set("x2", json_number(sign * info.tick_size));
                    line.set("y", json_number(p));
                    line.set("y2", json_number(p));
                } else {
                    line.set("x", json_number(p));
                    line.set("x2", json_number(p));
                    line.set("y", json!(0));
                    line.set("y2", json_number(sign * info.tick_size));
                }
                line.set("stroke", json!(info.tick_color));
                line.set("strokeWidth", json_number(1.0));
                line.set("opacity", json_number(1.0));
                apply_axis_encode(&mut line, info, "ticks", &json!({ "value": first }), env);
                tick_group.items.push(line);
            }
        }
        group.items.push(tick_group);
    }

    let mut label_visible_bounds: Option<crate::bounds::Bounds> = None;
    if labels_on {
        let mut label_group = Item::new("group");
        label_group.class.push("mark-text".to_string());
        label_group.class.push("role-axis-label".to_string());
        for (i, v) in info.ticks.iter().enumerate() {
            let Some(label) = info.labels.get(i) else {
                continue;
            };
            let label = &truncate_text(label, info.font_size, info.label_limit, "\u{2026}");
            let p = tick_pos(scale, v) + info.tick_offset + info.label_offset;
            let offset = sign * (info.tick_size + info.label_padding);
            let mut text = Item::new("text");
            text.set("text", json!(label));
            text.set("font", json!(DEFAULT_FONT));
            text.set("fontSize", json_number(info.font_size));
            text.set("fill", json!("#000"));
            let mut align = info.label_align.clone().unwrap_or_else(|| {
                if info.orient == "left" || info.orient == "right" {
                    if info.orient == "left" {
                        "right".to_string()
                    } else {
                        "left".to_string()
                    }
                } else {
                    "center".to_string()
                }
            });
            let baseline =
                info.label_baseline
                    .clone()
                    .unwrap_or_else(|| match info.orient.as_str() {
                        "top" => "bottom".to_string(),
                        "bottom" => "top".to_string(),
                        _ => "middle".to_string(),
                    });
            if let Some(threshold) = info.label_flush_threshold {
                if (info.orient == "bottom" || info.orient == "top")
                    && info.label_align.is_none()
                {
                    align = flush_align(scale, p, threshold).unwrap_or_else(|| "center".to_string());
                }
            }
            let _ = i;
            if info.orient == "left" || info.orient == "right" {
                text.set("x", json_number(offset));
                text.set("y", json_number(p));
            } else {
                text.set("x", json_number(p));
                text.set("y", json_number(offset));
            }
            text.set("align", json!(align));
            text.set("baseline", json!(baseline));
            if info.label_angle != 0.0 {
                text.set("angle", json_number(info.label_angle));
            }
            // Per-axis `encode.labels.update` overrides (e.g. bandPosition).
            if let Some(update) = info
                .spec
                .get("encode")
                .and_then(|e| e.get("labels"))
                .and_then(|l| l.get("update"))
                .and_then(|u| u.as_object())
            {
                let datum = json!({ "value": v.clone(), "label": label, "index": i });
                for (prop, enc) in update {
                    if let Some(val) = encoder_value(enc, &datum, &Value::Null, env) {
                        text.set(prop, val);
                    }
                }
            }
            label_group.items.push(text);
        }
        if info.label_overlap {
            apply_label_overlap(
                &mut label_group.items,
                &info.label_overlap_method,
                info.label_separation,
                false,
                !matches!(info.orient.as_str(), "left" | "right"),
                if matches!(info.orient.as_str(), "left" | "right") {
                    height
                } else {
                    width
                },
                scale.kind == crate::scale::ScaleKind::Log,
            );
        }
        // `Overlap` recomputes the label mark bounds from visible items only,
        // so hidden labels do not inflate autosize extents.
        let mut visible_bounds = crate::bounds::Bounds::empty();
        for text in &label_group.items {
            if text.f64("opacity").unwrap_or(1.0) != 0.0 {
                visible_bounds.union(item_bounds_plain(text));
            }
        }
        if !visible_bounds.is_empty() {
            label_visible_bounds = Some(visible_bounds);
            label_group.set(
                "_bounds",
                json!([
                    visible_bounds.x1,
                    visible_bounds.y1,
                    visible_bounds.x2,
                    visible_bounds.y2
                ]),
            );
        }
        group.items.push(label_group);
    }

    if let Some(title) = &info.title {
        let title = truncate_text(title, info.title_font_size, info.title_limit, "\u{2026}");
        let mut text = Item::new("text");
        text.set("text", json!(title));
        text.set("font", json!(DEFAULT_FONT));
        text.set("fontSize", json_number(info.title_font_size));
        text.set("fontWeight", json!("bold"));
        text.set("fill", json!("#000"));
        text.set("align", json!("center"));
        text.set("baseline", json!("top"));
        let mid = info.axis_mid;
        let s = if nested {
            info.axis_s
        } else {
            match (&label_visible_bounds, info.orient.as_str()) {
                (Some(b), "bottom") => b.y2,
                (Some(b), "top") => -b.y1,
                (Some(b), "left") => -b.x1,
                (Some(b), "right") => b.x2,
                _ => info.axis_s,
            }
        };
        match info.orient.as_str() {
            "bottom" => {
                text.set("x", json_number(mid));
                text.set("y", json_number(s + info.title_padding));
                text.set("baseline", json!("top"));
            }
            "top" => {
                text.set("x", json_number(mid));
                // Reserve the glyph height above the axis (the SVG renderer
                // converts `baseline: top` into a baseline offset).
                text.set(
                    "y",
                    json_number(-(s + info.title_padding + info.title_font_size)),
                );
                text.set("baseline", json!("top"));
            }
            "left" => {
                text.set("x", json_number(-(s + info.title_padding)));
                text.set("y", json_number(mid));
                text.set("angle", json_number(-90.0));
                text.set("baseline", json!("bottom"));
            }
            "right" => {
                text.set("x", json_number(s + info.title_padding));
                text.set("y", json_number(mid));
                text.set("angle", json_number(90.0));
                text.set("baseline", json!("bottom"));
            }
            _ => {}
        }
        if let Some(x) = info.title_x {
            text.set("x", json_number(x));
        }
        if let Some(y) = info.title_y {
            text.set("y", json_number(y));
        }
        if let Some(angle) = info.title_angle {
            text.set("angle", json_number(angle));
        }
        if let Some(align) = &info.title_align {
            text.set("align", json!(align));
        }
        if let Some(baseline) = &info.title_baseline {
            text.set("baseline", json!(baseline));
        }
        let _ = env;
        let mut title_group = Item::new("group");
        title_group.class.push("mark-text".to_string());
        title_group.class.push("role-axis-title".to_string());
        title_group.items.push(text);
        group.items.push(title_group);
    }
    let mut content = Item::new("group");
    content.items = std::mem::take(&mut group.items);
    let mut middle = Item::new("group");
    middle.items.push(content);
    group.items.push(middle);
    if true {
        // Recompute the guide extent from the actual (visible) labels/title,
        // matching `axisLayout`'s union of guide mark bounds. Only text marks
        // contribute (rules/ticks are replaced by the axis range below).
        fn union_text_bounds(item: &Item, b: &mut crate::bounds::Bounds) {
            if item.kind == "text" {
                b.union(item_bounds_plain(item));
                return;
            }
            if item.get("_bounds").and_then(|v| v.as_array()).map(|v| v.len() == 4).unwrap_or(false) {
                b.union(item_bounds_plain(item));
                return;
            }
            for child in &item.items {
                union_text_bounds(child, b);
            }
        }
        let mut text_bounds = crate::bounds::Bounds::empty();
        for child in &group.items {
            union_text_bounds(child, &mut text_bounds);
        }
        let range = info.axis_range.abs();
        if !text_bounds.is_empty() {
            let mut nb = info.bounds;
            nb.x1 = info.bounds.x1 - delta;
            nb.y1 = info.bounds.y1 - delta;
            nb.x2 = info.bounds.x2 - delta;
            nb.y2 = info.bounds.y2 - delta;
            if info.orient == "top" || info.orient == "bottom" {
                nb.x1 = text_bounds.x1.min(0.0)
                    - if info.label_angle == 0.0 { 0.5 } else { 0.0 };
                let tick_at_end = info.ticks.iter().any(|v| {
                    (tick_pos_band(scale, v, info.band_position) - range).abs() < 1e-6
                });
                nb.x2 =
                    text_bounds.x2.max(range) + if tick_at_end { 0.5 } else { 0.0 };
                nb.y1 = nb.y1.min(text_bounds.y1);
                nb.y2 = nb.y2.max(text_bounds.y2);
            } else {
                nb.x1 = text_bounds.x1.min(0.0);
                nb.x2 = text_bounds.x2.max(0.0);
                nb.y1 = text_bounds.y1.min(0.0);
                nb.y2 = text_bounds.y2.max(range);
            }
            group.set("_bounds", json!([nb.x1, nb.y1, nb.x2, nb.y2]));
        }
    }
    group
}

/// Port of `vega-view-transforms`' `Overlap` label culling (parity/greedy).
/// Collect names in `value` that refer to a dataset in either store.
fn collect_dataset_refs<'a>(
    value: &'a Value,
    global: &IndexMap<String, Vec<Value>>,
    local: &HashMap<String, Vec<Value>>,
    out: &mut std::collections::HashSet<&'a str>,
) {
    match value {
        Value::String(s) => {
            if global.contains_key(s) || local.contains_key(s) {
                out.insert(s.as_str());
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_dataset_refs(item, global, local, out);
            }
        }
        Value::Object(map) => {
            for item in map.values() {
                collect_dataset_refs(item, global, local, out);
            }
        }
        _ => {}
    }
}

/// Whether an encoder object references the (partially built) mark item.
fn mentions_item(value: &Value) -> bool {
    match value {
        Value::String(s) => s.contains("item"),
        Value::Array(items) => items.iter().any(mentions_item),
        Value::Object(map) => map.values().any(mentions_item),
        _ => false,
    }
}

fn apply_label_overlap(
    items: &mut [Item],
    method: &str,
    separation: f64,
    flush: bool,
    horizontal: bool,
    bound: f64,
    log_scale: bool,
) {
    let total = items.len();
    // A flush-aligned end label is exempt from overlap culling (Vega's axis
    // layout keeps the flushed endpoints even when they graze a neighbor).
    let n = if flush && total > 1 { total - 1 } else { total };
    if n == 0 {
        return;
    }
    for item in items.iter_mut() {
        item.set("opacity", json_number(1.0));
    }
    let mut visible: Vec<usize> = (0..n).collect();
    if method == "greedy" && log_scale {
        visible = items
            .iter()
            .take(n)
            .enumerate()
            .filter_map(|(i, it)| {
                let raw = it.str("text").unwrap_or("").replace(',', "");
                let v: f64 = raw.parse().ok()?;
                if v <= 0.0 {
                    return None;
                }
                let decade = 10f64.powf(v.log10().floor());
                let mantissa = v / decade;
                (mantissa <= 3.0001).then_some(i)
            })
            .collect();
    }
    let bounds: Vec<crate::bounds::Bounds> = items.iter().map(item_bounds_plain).collect();
    let intersects = |ai: usize, bi: usize, sep: f64| -> bool {
        let (a, b) = (bounds[ai], bounds[bi]);
        let gap = (b.x1 - a.x2).max(a.x1 - b.x2).max(b.y1 - a.y2).max(a.y1 - b.y2);
        sep > gap
    };
    let has_overlap = |items: &[usize], sep: f64| -> bool {
        items.windows(2).any(|w| intersects(w[0], w[1], sep))
    };
    if method == "greedy" && !log_scale {
        let mut kept: Vec<usize> = Vec::new();
        let mut last: Option<usize> = None;
        for i in 0..n {
            match last {
                Some(a) if intersects(a, i, separation) => {}
                _ => {
                    kept.push(i);
                    last = Some(i);
                }
            }
        }
        visible = kept;
    } else if n >= 3 && has_overlap(&visible, separation) {
        while visible.len() >= 3 && has_overlap(&visible, separation) {
            visible = visible
                .iter()
                .enumerate()
                .filter(|(i, _)| i % 2 == 0)
                .map(|(_, idx)| *idx)
                .collect();
        }
    }
    // Upstream preserves the final item when only its opacity was cleared.
    if !visible.contains(&(n - 1)) && visible.len() < 3 {
        if visible.len() > 1 {
            if let Some(&peek) = visible.last() {
                items[peek].set("opacity", json_number(0.0));
                visible.pop();
            }
        }
        visible.push(n - 1);
    }
    if flush && total > n {
        let lb = bounds[n];
        let inside = if horizontal {
            lb.x1 >= -0.5 && lb.x2 <= bound + 0.5
        } else {
            lb.y1 >= -0.5 && lb.y2 <= bound + 0.5
        };
        if inside && !visible.contains(&n) {
            visible.push(n);
        }
    }
    let visible_set: std::collections::HashSet<usize> = visible.into_iter().collect();
    for (i, item) in items.iter_mut().enumerate() {
        if !visible_set.contains(&i) {
            item.set("opacity", json_number(0.0));
            if method == "greedy" {
                item.set("text", json!(""));
            }
        }
    }
}

/// Port of `vega-util`'s `flush()` used by axis label flush alignment.
fn flush_align(scale: &Scale, p: f64, threshold: f64) -> Option<String> {
    let range = scale.range_numeric();
    let mut a = to_number(range.first()?);
    let mut b = to_number(range.last()?);
    if b < a {
        std::mem::swap(&mut a, &mut b);
    }
    let l = (p - a).abs();
    let r = (b - p).abs();
    if l < r && l <= threshold {
        Some("left".to_string())
    } else if r <= threshold {
        Some("right".to_string())
    } else {
        Some("center".to_string())
    }
}

fn tick_pos(scale: &Scale, v: &Value) -> f64 {
    tick_pos_band(scale, v, 0.5)
}

fn tick_pos_band(scale: &Scale, v: &Value, band: f64) -> f64 {
    if scale.is_discrete() {
        to_number(&scale.scale_band(v, band))
    } else {
        to_number(&scale.scale(v))
    }
}

fn legend_item(info: &LegendInfo, _env: &Env) -> Item {
    let mut group = Item::new("group");
    group.class.push("mark-group".to_string());
    group.class.push("role-legend".to_string());
    group.set(
        "_entry_y",
        json_number(if info.title.is_some() && !info.title_on_side {
            info.title_height
        } else {
            0.0
        }),
    );
    let mut y = 0.0;
    if let Some(title) = &info.title {
        let mut text = Item::new("text");
        if title.len() > 1 {
            text.set("text", json!(title));
        } else {
            text.set("text", json!(title.first().cloned().unwrap_or_default()));
        }
        text.set("font", json!(DEFAULT_FONT));
        text.set("fontSize", json_number(info.title_font_size));
        text.set("fontWeight", json!("bold"));
        text.set("fill", json!("#000"));
        text.set("x", json!(0));
        // Upstream uses `baseline: top` at y=0; the renderer's baseline
        // offset places the glyphs (round(0.79 * 11) = 9px) below the top.
        text.set("y", json!(0));
        text.set("baseline", json!("top"));
        group.items.push(text);
        if !info.title_on_side {
            y += info.title_height;
        }
    }
    if let Some(palette) = &info.gradient {
        if info.band_legend && !palette.is_empty() {
            let n = palette.len() as f64;
            let band_h = info.gradient_length / n;
            for (i, color) in palette.iter().enumerate() {
                let mut rect = Item::new("rect");
                rect.set("_entry_y", json_number(y));
                rect.set("x", json!(0));
                rect.set("y", json_number((n - 1.0 - i as f64) * band_h));
                rect.set("width", json_number(info.gradient_thickness));
                rect.set("height", json_number(band_h));
                rect.set("fill", json!(color));
                rect.set("stroke", json!("#ddd"));
                rect.set("strokeWidth", json_number(0.0));
                group.items.push(rect);
            }
            let _ = info.gradient_length;
        } else {
        let mut rect = Item::new("rect");
        rect.set("_entry_y", json_number(y));
        rect.set("x", json!(0));
        rect.set("y", json!(0));
        if info.horizontal {
            rect.set("width", json_number(info.gradient_length));
            rect.set("height", json_number(info.gradient_thickness));
        } else {
            rect.set("width", json_number(info.gradient_thickness));
            rect.set("height", json_number(info.gradient_length));
        }
        let stops: Vec<Value> = palette
            .iter()
            .enumerate()
            .map(|(i, c)| {
                json!({
                    "offset": i as f64 / (palette.len().max(2) - 1) as f64,
                    "color": c
                })
            })
            .collect();
        rect.set(
            "fill",
            if info.horizontal {
                json!({
                    "gradient": "linear",
                    "x1": 0, "y1": 0, "x2": 1, "y2": 0,
                    "stops": stops
                })
            } else {
                json!({
                    "gradient": "linear",
                    "x1": 0, "y1": 1, "x2": 0, "y2": 0,
                    "stops": stops
                })
            },
        );
        rect.set("stroke", json!("#ddd"));
        rect.set("strokeWidth", json_number(0.0));
        group.items.push(rect);
        let _ = info.gradient_length;
        }
    } else {
        let mut x_cursor = if info.title_on_side { info.title_width } else { 0.0 };
        let mut row_y = if info.title_on_side { 0.0 } else { 0.0 };
        for (entry_idx, entry) in info.entries.iter().enumerate() {
            let side = (entry.size.max(0.0)).sqrt().max(1.0);
            let (sym_x, sym_y) = if info.size_legend {
                (
                    info.size_symbol_xs
                        .get(entry_idx)
                        .copied()
                        .unwrap_or(info.size_symbol_x),
                    info.entry_symbol_ys
                        .get(entry_idx)
                        .copied()
                        .unwrap_or(side / 2.0 + 1.0),
                )
            } else {
                (side / 2.0 + 1.0, side / 2.0 + 1.0)
            };
            let mut symbol = Item::new("symbol");
            symbol.set("x", json_number(sym_x));
            symbol.set("y", json_number(sym_y));
            if info.stroke_width_legend {
                symbol.set("size", json_number(100.0));
            } else {
                symbol.set("size", json_number(entry.size));
            }
            symbol.set("shape", json!(entry.shape));
            symbol.set("fill", entry.fill.clone());
            symbol.set("stroke", entry.stroke.clone());
            symbol.set("strokeWidth", json_number(1.5));
            if let Some(opacity) = entry.opacity {
                symbol.set("opacity", json_number(opacity));
            }
            for (k, v) in &info.symbol_props {
                if !matches!(k.as_str(), "x" | "y" | "size") {
                    symbol.set(k.as_str(), v.clone());
                }
            }
            if info.stroke_width_legend {
                symbol.set("fill", Value::Null);
                symbol.set("stroke", json!("#888"));
                symbol.set("strokeWidth", json_number(entry.size));
            }
            let entry_w = info
                .entry_widths
                .get(entry_idx)
                .copied()
                .unwrap_or(info.entry_width);
            let mut entry_group = Item::new("group");
            entry_group.class.push("role-legend-entry".to_string());
            if info.size_legend {
                entry_group.set(
                    "x",
                    json_number(info.entry_xs.get(entry_idx).copied().unwrap_or(0.0)),
                );
                entry_group.set(
                    "y",
                    json_number(info.entry_ys.get(entry_idx).copied().unwrap_or(0.0)),
                );
            } else if info.horizontal {
                entry_group.set("x", json_number(x_cursor));
                entry_group.set("y", json_number(row_y));
            } else {
                entry_group.set("x", json!(0));
                entry_group.set("y", json_number(row_y));
            }
            entry_group.set(
                "width",
                json_number(if info.size_legend && info.stroke_width_legend {
                    entry_w
                } else if info.horizontal && !info.size_legend {
                    entry_w
                } else {
                    info.entry_width
                }),
            );
            entry_group.set(
                "height",
                json_number(if info.size_legend {
                    info.entry_heights.get(entry_idx).copied().unwrap_or(info.entry_height)
                } else {
                    info.entry_height
                }),
            );
            entry_group.items.push(symbol);
            let mut text = Item::new("text");
            text.set("text", json!(entry.label));
            text.set("font", json!(DEFAULT_FONT));
            text.set("fontSize", json_number(info.font_size));
            text.set("fill", json!("#000"));
            if info.size_legend {
                text.set(
                    "x",
                    json_number(
                        info.size_label_xs
                            .get(entry_idx)
                            .copied()
                            .unwrap_or(info.size_label_x),
                    ),
                );
                text.set("y", json_number(sym_y));
            } else {
                text.set("x", json_number(side / 2.0 + 11.0));
                text.set("y", json_number(side / 2.0 + 1.0));
            }
            text.set("align", json!("left"));
            text.set("baseline", json!("middle"));
            entry_group.items.push(text);
            group.items.push(entry_group);
            if info.horizontal && !info.size_legend {
                x_cursor += entry_w + 10.0;
            } else if !info.size_legend {
                row_y += info.entry_height
                    + if info.stroke_symbol {
                        2.0
                    } else if info.entries.iter().any(|e| !e.stroke.is_null()) {
                        4.0
                    } else {
                        2.0
                    };
            }
        }
    }
    group.set("_bounds", json!([0.0, 0.0, info.width, info.height]));
    group.set("width", json_number(info.width));
    group.set("height", json_number(info.height));
    let mut content = Item::new("group");
    content.items = std::mem::take(&mut group.items);
    group.items.push(content);
    group
}

fn title_value(v: &Value, datum: &Value, env: &Env) -> Option<String> {
    match v {
        Value::String(_) | Value::Array(_) => title_text(v),
        Value::Object(o) => {
            let text = o.get("text")?;
            if let Some(sig) = text.get("signal").and_then(|s| s.as_str()) {
                // A facet's title may reference `parent`, which upstream Vega
                // binds to the enclosing group's datum. In this renderer the
                // same datum is passed directly, so substitute it first.
                let alt = sig.replace("parent", "datum");
                let mut value = expression::evaluate(&alt, Some(datum), env).ok();
                if value.as_ref().map(|v| v.is_null()).unwrap_or(true) {
                    value = expression::evaluate(sig, Some(datum), env).ok();
                }
                value
                    .filter(|v| !v.is_null())
                    .map(|v| to_string_js(&v))
                    .filter(|s| !s.is_empty() && s != "null")
            } else {
                title_text(text)
            }
        }
        _ => None,
    }
}

fn title_item(
    text: &str,
    spec: Option<&Value>,
    width: f64,
    height: f64,
    layout: &GroupLayout,
    pre_bounds: Option<crate::bounds::Bounds>,
    config: &Value,
    _env: &Env,
) -> Item {
    let mut group = Item::new("group");
    group.class.push("mark-group".to_string());
    group.class.push("role-title".to_string());
    let mut t = Item::new("text");
    let orient = spec
        .and_then(|s| s.get("orient"))
        .and_then(|o| o.as_str())
        .unwrap_or("top");
    let style = spec
        .and_then(|s| s.get("style"))
        .and_then(|o| o.as_str())
        .unwrap_or("group-title");
    let fs = spec
        .and_then(|s| s.get("fontSize"))
        .and_then(|v| v.as_f64())
        .or_else(|| {
            config_value(config, "style", style)
                .and_then(|s| s.get("fontSize").and_then(|v| v.as_f64()))
        })
        .unwrap_or(match style {
            "guide-label" => 10.0,
            "guide-title" => 11.0,
            "group-subtitle" => 12.0,
            _ => 13.0,
        });
    let title_offset = spec
        .and_then(|s| s.get("offset"))
        .and_then(|o| o.as_f64())
        .unwrap_or(4.0);
    let title_angle = spec
        .and_then(|s| s.get("angle"))
        .and_then(|o| o.as_f64());
    let font = spec
        .and_then(|s| s.get("font"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| {
            config_value(config, "style", style)
                .and_then(|s| s.get("font").and_then(|v| v.as_str()).map(String::from))
        })
        .unwrap_or_else(|| DEFAULT_FONT.to_string());
    let weight = spec
        .and_then(|s| s.get("fontWeight"))
        .cloned()
        .or_else(|| {
            config_value(config, "style", style).and_then(|s| s.get("fontWeight").cloned())
        });
    let title_text_value = spec
        .and_then(|s| s.get("text"))
        .filter(|v| matches!(v, Value::Array(a) if a.len() > 1))
        .cloned()
        .unwrap_or_else(|| json!(text));
    t.set("text", title_text_value);
    t.set("font", json!(font));
    t.set("fontSize", json_number(fs));
    if let Some(weight) = weight {
        t.set("fontWeight", weight);
    }
    let fill = spec
        .and_then(|s| s.get("color").or_else(|| s.get("fill")))
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| {
            config_value(config, "style", style)
                .and_then(|s| s.get("fill").and_then(|v| v.as_str()).map(String::from))
        })
        .unwrap_or_else(|| "#000".to_string());
    t.set("fill", json!(fill));
    // Offset of the rotated text box relative to the item anchor (see
    // `bounds::text_bounds`), used to align left/right titles to the layout.
    let rot_off = (0.3 * fs).round() - (0.8 * fs).round();
    match orient {
        "left" => {
            if title_angle == Some(0.0) {
                let width_text = crate::bounds::text_width(text, fs);
                t.set("x", json_number(-title_offset - width_text));
                t.set("y", json_number(height / 2.0));
                t.set("align", json!("left"));
                t.set("baseline", json!("middle"));
            } else {
                t.set("x", json_number(-layout.left - rot_off));
                t.set("y", json_number(height / 2.0));
                t.set("angle", json_number(-90.0));
                t.set("align", json!("center"));
                t.set("baseline", json!("middle"));
            }
        }
        "right" => {
            t.set("x", json_number(width + layout.right - rot_off - fs));
            t.set("y", json_number(height / 2.0));
            t.set("angle", json_number(90.0));
            t.set("align", json!("center"));
            t.set("baseline", json!("middle"));
        }
        "bottom" => {
            t.set("x", json_number(width / 2.0));
            t.set("y", json_number(height + layout.bottom - fs));
            t.set("align", json!("center"));
            t.set("baseline", json!("top"));
        }
        _ => {
            let anchor_start = spec
                .and_then(|s| s.get("anchor"))
                .and_then(|a| a.as_str())
                == Some("start");
            let frame_bounds = spec
                .and_then(|s| s.get("frame"))
                .and_then(|f| f.as_str())
                == Some("bounds");
            let pb = pre_bounds.unwrap_or_else(|| {
                crate::bounds::Bounds::new(0.0, 0.0, width, height)
            });
            let line_count = spec
                .and_then(|s| s.get("text"))
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(1)
                .max(1);
            let line_height = spec
                .and_then(|s| s.get("lineHeight"))
                .and_then(|v| v.as_f64())
                .unwrap_or(fs + 2.0);
            let title_y =
                pb.y1 - fs - title_offset - (line_count.saturating_sub(1) as f64) * line_height;
            if anchor_start {
                group.set("_anchor_start", json!(true));
                t.set("x", json_number(pb.x1));
                t.set("y", json_number(title_y));
                t.set("align", json!("left"));
                t.set("baseline", json!("top"));
            } else {
                let x = if frame_bounds {
                    ((pb.x1 + pb.x2) / 2.0).floor()
                } else {
                    width / 2.0
                };
                t.set("x", json_number(x));
                t.set("y", json_number(title_y));
                t.set("align", json!("center"));
                t.set("baseline", json!("top"));
            }
        }
    }
    group.items.push(t);
    group
}

fn layout_get(opt: &Value, key: &str, env: &Env) -> Option<Value> {
    let v = match opt {
        Value::Object(o) => o.get(key).cloned(),
        Value::Null => None,
        other => Some(other.clone()),
    };
    v.map(|v| eval_signal_value(&v, env).unwrap_or(v))
}

fn layout_num(opt: &Value, key: &str, env: &Env) -> Option<f64> {
    layout_get(opt, key, env).and_then(|v| match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    })
}

fn layout_flag(opt: &Value, key: &str, env: &Env) -> bool {
    match layout_get(opt, key, env) {
        Some(Value::Bool(b)) => b,
        _ => false,
    }
}

fn layout_align(opt: &Value, key: &str, env: &Env) -> String {
    match layout_get(opt, "align", env) {
        Some(a) => match layout_get(&a, key, env) {
            Some(Value::String(s)) => s,
            _ => String::new(),
        },
        None => String::new(),
    }
}

fn layout_str(opt: &Value, key: &str, env: &Env) -> Option<String> {
    layout_get(opt, key, env).and_then(|v| v.as_str().map(String::from))
}

fn offset_value(v: f64) -> f64 {
    if v < 0.0 {
        (-v).ceil()
    } else {
        0.0
    }
}

fn layout_bbox(item: &Item, flush: bool) -> crate::bounds::Bounds {
    use crate::bounds::Bounds;
    if flush {
        Bounds::new(
            0.0,
            0.0,
            item.f64("width").unwrap_or(0.0),
            item.f64("height").unwrap_or(0.0),
        )
    } else {
        let mut b = item_bounds(item);
        if b.is_empty() {
            b = Bounds::new(0.0, 0.0, 0.0, 0.0);
        }
        b.translate(-item.f64("x").unwrap_or(0.0), -item.f64("y").unwrap_or(0.0))
    }
}

#[derive(Clone, Copy, PartialEq)]
enum BoundField {
    X1,
    Y1,
    X2,
    Y2,
}

fn layout_bound(item: &Item, field: BoundField, flush: bool) -> f64 {
    if flush {
        let x = item.f64("x").unwrap_or(0.0);
        let y = item.f64("y").unwrap_or(0.0);
        return match field {
            BoundField::X1 => x,
            BoundField::Y1 => y,
            BoundField::X2 => x + item.f64("width").unwrap_or(0.0),
            BoundField::Y2 => y + item.f64("height").unwrap_or(0.0),
        };
    }
    let b = item_bounds(item);
    match field {
        BoundField::X1 => b.x1,
        BoundField::Y1 => b.y1,
        BoundField::X2 => b.x2,
        BoundField::Y2 => b.y2,
    }
}

/// Port of vega-view-transforms' `gridLayout(body groups)`.
fn grid_layout(
    items: &mut [Item],
    groups: &[usize],
    opt: &Value,
    env: &Env,
) -> crate::bounds::Bounds {
    use crate::bounds::Bounds;
    let n = groups.len();
    if n == 0 {
        return Bounds::empty();
    }
    let flush = layout_str(opt, "bounds", env).as_deref() == Some("flush");
    let columns = layout_num(opt, "columns", env).unwrap_or(0.0);
    let ncols = if columns > 0.0 {
        columns.max(1.0) as usize
    } else {
        n
    };
    let nrows = n.div_ceil(ncols);
    let align_col = layout_align(opt, "column", env);
    let align_row = layout_align(opt, "row", env);
    let pad_col = {
        let pad = layout_get(opt, "padding", env).unwrap_or(Value::from(0.0));
        layout_num(&pad, "column", env).unwrap_or(0.0)
    };
    let pad_row = {
        let pad = layout_get(opt, "padding", env).unwrap_or(Value::from(0.0));
        layout_num(&pad, "row", env).unwrap_or(0.0)
    };
    let center = layout_get(opt, "center", env).unwrap_or(Value::Null);
    let center_col = layout_flag(&center, "column", env);
    let center_row = layout_flag(&center, "row", env);

    let mut x_offset = vec![0.0f64; n];
    let mut y_offset = vec![0.0f64; n];
    let mut x_extent = vec![0.0f64; ncols];
    let mut y_extent = vec![0.0f64; nrows];
    let mut dx = vec![0.0f64; n];
    let mut dy = vec![0.0f64; n];
    let mut boxes = vec![Bounds::empty(); n];
    let mut x_max = 0.0f64;
    let mut y_max = 0.0f64;
    let mut gx = vec![0.0f64; n];
    let mut gy = vec![0.0f64; n];

    for i in 0..n {
        let it = &items[groups[i]];
        let b = layout_bbox(it, flush);
        boxes[i] = b;
        gx[i] = it.f64("x").unwrap_or(0.0);
        gy[i] = it.f64("y").unwrap_or(0.0);
        let c = i % ncols;
        let r = i / ncols;
        let px = b.x2.ceil();
        let py = b.y2.ceil();
        x_max = x_max.max(px);
        y_max = y_max.max(py);
        x_extent[c] = x_extent[c].max(px);
        y_extent[r] = y_extent[r].max(py);
        x_offset[i] = pad_col + offset_value(b.x1);
        y_offset[i] = pad_row + offset_value(b.y1);
    }
    for i in 0..n {
        if i % ncols == 0 {
            x_offset[i] = 0.0;
        }
        if i < ncols {
            y_offset[i] = 0.0;
        }
    }

    if align_col == "each" {
        for c in 1..ncols {
            let mut offset = 0.0f64;
            let mut i = c;
            while i < n {
                offset = offset.max(x_offset[i]);
                i += ncols;
            }
            let mut i = c;
            while i < n {
                x_offset[i] = offset + x_extent[c - 1];
                i += ncols;
            }
        }
    } else if align_col == "all" {
        let mut offset = 0.0f64;
        for i in 0..n {
            if i % ncols != 0 && offset < x_offset[i] {
                offset = x_offset[i];
            }
        }
        for i in 0..n {
            if i % ncols != 0 {
                x_offset[i] = offset + x_max;
            }
        }
    } else {
        for c in 1..ncols {
            let mut i = c;
            while i < n {
                x_offset[i] += x_extent[c - 1];
                i += ncols;
            }
        }
    }

    if align_row == "each" {
        for r in 1..nrows {
            let start = r * ncols;
            let end = (start + ncols).min(n);
            let mut offset = 0.0f64;
            for i in start..end {
                offset = offset.max(y_offset[i]);
            }
            for i in start..end {
                y_offset[i] = offset + y_extent[r - 1];
            }
        }
    } else if align_row == "all" {
        let mut offset = 0.0f64;
        for i in ncols..n {
            if offset < y_offset[i] {
                offset = y_offset[i];
            }
        }
        for i in ncols..n {
            y_offset[i] = offset + y_max;
        }
    } else {
        for r in 1..nrows {
            let start = r * ncols;
            let end = (start + ncols).min(n);
            for i in start..end {
                y_offset[i] += y_extent[r - 1];
            }
        }
    }

    let mut x = 0.0f64;
    for i in 0..n {
        x = x_offset[i] + if i % ncols != 0 { x } else { 0.0 };
        dx[i] += x - gx[i];
    }
    for c in 0..ncols {
        let mut y = 0.0f64;
        let mut i = c;
        while i < n {
            y += y_offset[i];
            dy[i] += y - gy[i];
            i += ncols;
        }
    }

    if (align_col == "each" || align_col == "all") && center_col && nrows > 1 {
        for i in 0..n {
            let b = if align_col == "all" {
                x_max
            } else {
                x_extent[i % ncols]
            };
            let extra = b - boxes[i].x2 - gx[i] - dx[i];
            if extra > 0.0 {
                dx[i] += extra / 2.0;
            }
        }
    }
    if (align_row == "each" || align_row == "all") && center_row && ncols != 1 {
        for i in 0..n {
            let b = if align_row == "all" {
                y_max
            } else {
                y_extent[i / ncols]
            };
            let extra = b - boxes[i].y2 - gy[i] - dy[i];
            if extra > 0.0 {
                dy[i] += extra / 2.0;
            }
        }
    }

    let mut bounds = Bounds::empty();
    for i in 0..n {
        bounds.union(boxes[i].translate(dx[i], dy[i]));
    }
    let anchor = layout_get(opt, "anchor", env).unwrap_or(Value::Null);
    let mut ax = layout_num(&anchor, "x", env).unwrap_or(0.0);
    let mut ay = layout_num(&anchor, "y", env).unwrap_or(0.0);
    match layout_get(&anchor, "column", env).and_then(|v| v.as_str().map(String::from)) {
        Some(s) if s == "end" => ax -= bounds.width(),
        Some(s) if s == "middle" => ax -= bounds.width() / 2.0,
        _ => {}
    }
    match layout_get(&anchor, "row", env).and_then(|v| v.as_str().map(String::from)) {
        Some(s) if s == "end" => ay -= bounds.height(),
        Some(s) if s == "middle" => ay -= bounds.height() / 2.0,
        _ => {}
    }
    let ax = ax.round();
    let ay = ay.round();

    let mut out = Bounds::empty();
    for i in 0..n {
        dx[i] += ax;
        dy[i] += ay;
        let it = &mut items[groups[i]];
        let nx = gx[i] + dx[i];
        let ny = gy[i] + dy[i];
        it.set("x", json_number(nx));
        it.set("y", json_number(ny));
    }
    for i in 0..n {
        out.union(boxes[i].translate(dx[i], dy[i]));
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn layout_headers(
    items: &mut [Item],
    headers: &[usize],
    groups: &[usize],
    limit: usize,
    offset: f64,
    agg_max: bool,
    is_x: bool,
    field: BoundField,
    start: usize,
    stride: usize,
    back: usize,
    band: Option<f64>,
    flush: bool,
) -> f64 {
    use crate::bounds::Bounds;
    let n = groups.len();
    let mut init = 0.0f64;
    if n == 0 {
        return init;
    }
    let agg = |a: f64, b: f64| {
        if agg_max {
            a.max(b).ceil()
        } else {
            a.min(b).floor()
        }
    };
    let mut i = start;
    while i < n {
        init = agg(init, layout_bound(&items[groups[i]], field, flush));
        i += stride;
    }
    if headers.is_empty() {
        return init;
    }
    let headers: Vec<usize> = if headers.len() > limit {
        headers[..limit].to_vec()
    } else {
        headers.to_vec()
    };
    init += offset;
    let mut edge = 0.0f64;
    for (j, &h) in headers.iter().enumerate() {
        let mut k = start + j * stride;
        // Search for the nearest existing group (empty grid cells).
        while k >= n {
            if k < back {
                break;
            }
            k -= back;
        }
        let (nx, ny) = if k < n {
            let g = &items[groups[k]];
            if is_x {
                let gx = if let Some(band) = band {
                    let b = item_bounds(g);
                    (b.x1 + band * b.width()).round()
                } else {
                    g.f64("x").unwrap_or(0.0)
                };
                (gx, init)
            } else {
                let gy = if let Some(band) = band {
                    let b = item_bounds(g);
                    (b.y1 + band * b.height()).round()
                } else {
                    g.f64("y").unwrap_or(0.0)
                };
                (init, gy)
            }
        } else {
            (init, init)
        };
        let b: Bounds = {
            let it = &items[h];
            let ox = it.f64("x").unwrap_or(0.0);
            let oy = it.f64("y").unwrap_or(0.0);
            item_bounds(it).translate(nx - ox, ny - oy)
        };
        let it = &mut items[h];
        it.set("x", json_number(nx));
        it.set("y", json_number(ny));
        let v = match field {
            BoundField::X1 => b.x1,
            BoundField::Y1 => b.y1,
            BoundField::X2 => b.x2,
            BoundField::Y2 => b.y2,
        };
        edge = agg(edge, v);
    }
    edge
}

fn layout_title(
    items: &mut [Item],
    title: usize,
    offset: f64,
    is_x: bool,
    bounds: crate::bounds::Bounds,
    band: f64,
) {
    let item = &mut items[title];
    let x = if is_x {
        (bounds.x1 + band * bounds.width()).round()
    } else {
        offset
    };
    let y = if is_x {
        offset
    } else {
        (bounds.y1 + band * bounds.height()).round()
    };
    item.set("x", json_number(x));
    item.set("y", json_number(y));
}

fn layout_groups(items: &mut [Item], layout: &Value, env: &Env) {
    use crate::bounds::Bounds;
    if items.is_empty() {
        return;
    }
    let flush = layout_str(layout, "bounds", env).as_deref() == Some("flush");
    let mut marks: Vec<usize> = Vec::new();
    let mut row_headers: Vec<usize> = Vec::new();
    let mut col_headers: Vec<usize> = Vec::new();
    let mut row_footers: Vec<usize> = Vec::new();
    let mut col_footers: Vec<usize> = Vec::new();
    let mut row_title: Option<usize> = None;
    let mut col_title: Option<usize> = None;
    for (i, it) in items.iter().enumerate() {
        let role = it.get("_role").and_then(|v| v.as_str()).unwrap_or("");
        let guide = it
            .class
            .iter()
            .any(|c| matches!(c.as_str(), "role-axis" | "role-legend" | "role-title"));
        match role {
            "axis" | "legend" | "title" => {}
            _ if guide => {}
            "row-header" => row_headers.push(i),
            "row-footer" => row_footers.push(i),
            "column-header" => col_headers.push(i),
            "column-footer" => col_footers.push(i),
            "row-title" => row_title = Some(i),
            "column-title" => col_title = Some(i),
            _ => {
                if it.kind == "group" {
                    marks.push(i);
                }
            }
        }
    }
    let ncols_opt = layout_num(layout, "columns", env).unwrap_or(0.0);
    let ncols = if ncols_opt > 0.0 {
        ncols_opt.max(1.0) as usize
    } else {
        marks.len().max(1)
    };
    let nrows = if marks.is_empty() {
        1
    } else {
        marks.len().div_ceil(ncols)
    };
    let offset = layout_get(layout, "offset", env).unwrap_or(Value::from(0.0));
    let off = |key: &str| layout_num(&offset, key, env).unwrap_or(0.0);
    let header_band = layout_get(layout, "headerBand", env).unwrap_or(Value::Null);
    let footer_band = layout_get(layout, "footerBand", env).unwrap_or(Value::Null);
    let title_band = layout_get(layout, "titleBand", env).unwrap_or(Value::Null);
    let title_anchor = layout_get(layout, "titleAnchor", env).unwrap_or(Value::Null);

    let mut bounds = grid_layout(items, &marks, layout, env);
    if bounds.is_empty() {
        bounds = Bounds::new(0.0, 0.0, 0.0, 0.0);
    }
    let row_band = layout_num(&header_band, "row", env);
    let col_band = layout_num(&header_band, "column", env);
    let rowf_band = layout_num(&footer_band, "row", env);
    let colf_band = layout_num(&footer_band, "column", env);

    let mut x = 0.0;
    if !row_headers.is_empty() || !marks.is_empty() {
        x = layout_headers(
            items,
            &row_headers,
            &marks,
            nrows,
            -off("rowHeader"),
            false,
            false,
            BoundField::X1,
            0,
            ncols,
            1,
            row_band,
            flush,
        );
    }
    let mut y = 0.0;
    if !col_headers.is_empty() || !marks.is_empty() {
        y = layout_headers(
            items,
            &col_headers,
            &marks,
            ncols,
            -off("columnHeader"),
            false,
            true,
            BoundField::Y1,
            0,
            1,
            ncols,
            col_band,
            flush,
        );
    }
    let mut x2 = 0.0;
    if !row_footers.is_empty() {
        x2 = layout_headers(
            items,
            &row_footers,
            &marks,
            nrows,
            off("rowFooter"),
            true,
            false,
            BoundField::X2,
            ncols.saturating_sub(1),
            ncols,
            1,
            rowf_band,
            flush,
        );
    }
    let mut y2 = 0.0;
    if !col_footers.is_empty() {
        y2 = layout_headers(
            items,
            &col_footers,
            &marks,
            ncols,
            off("columnFooter"),
            true,
            true,
            BoundField::Y2,
            (nrows * ncols).saturating_sub(ncols),
            1,
            ncols,
            colf_band,
            flush,
        );
    }
    if let Some(t) = row_title {
        let anchor = layout_get(&title_anchor, "row", env)
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "start".to_string());
        let offset = off("rowTitle");
        let offset = if anchor == "end" { x2 + offset } else { x - offset };
        let band = layout_num(&title_band, "row", env).unwrap_or(0.5);
        layout_title(items, t, offset, false, bounds, band);
    }
    if let Some(t) = col_title {
        let anchor = layout_get(&title_anchor, "column", env)
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "start".to_string());
        let offset = off("columnTitle");
        let offset = if anchor == "end" { y2 + offset } else { y - offset };
        let band = layout_num(&title_band, "column", env).unwrap_or(0.5);
        layout_title(items, t, offset, true, bounds, band);
    }
}

/// Render a Vega spec to an SVG string.
pub fn vega_to_svg(spec: &Value) -> Result<String, Error> {
    vega_to_svg_with_options(spec, &RenderOptions::default())
}

pub fn vega_to_svg_with_options(spec: &Value, opts: &RenderOptions) -> Result<String, Error> {
    let runtime = Runtime::new(spec, opts)?;
    let root = runtime.build(opts)?;
    let background = runtime.background(opts);
    Ok(crate::svg::render(&root, background.as_deref()))
}

/// Render a Vega-Lite specification directly to SVG. The specification is
/// compiled in memory (no intermediate JSON serialization) and immediately
/// rendered.
pub fn vega_lite_to_svg(spec: &Value) -> Result<String, Error> {
    vega_lite_to_svg_with_options(spec, &RenderOptions::default())
}

pub fn vega_lite_to_svg_with_options(spec: &Value, opts: &RenderOptions) -> Result<String, Error> {
    let compiled = vega_lite::compile::compile::compile(spec).map_err(Error::spec)?;
    vega_to_svg_with_options(&compiled.spec, opts)
}

/// Render a previously compiled Vega-Lite result.
pub fn compiled_to_svg(
    compiled: &vega_lite::compile::compile::CompileResult,
    opts: &RenderOptions,
) -> Result<String, Error> {
    vega_to_svg_with_options(&compiled.spec, opts)
}

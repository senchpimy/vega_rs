//! Scale construction and evaluation (port of the parts of
//! [vega-scale] used by static rendering).
//!
//! [vega-scale]: https://github.com/vega/vega/tree/main/packages/vega-scale

use std::collections::HashMap;

use serde_json::Value;

use crate::color;
use crate::data::{civil_to_ms, ms_to_civil, DataStore};
use crate::error::Error;
use crate::expression::{self, Environment};
use crate::format::{
    format_date, format_date_multi_span, format_span, format_value, parse_date,
};
use crate::value::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleKind {
    Linear,
    Log,
    Pow,
    Sqrt,
    Symlog,
    Time,
    Utc,
    Band,
    Point,
    Ordinal,
    Quantile,
    Quantize,
    Threshold,
    Sequential,
    Identity,
}

impl ScaleKind {
    pub fn parse(s: &str) -> ScaleKind {
        match s {
            "log" => ScaleKind::Log,
            "pow" => ScaleKind::Pow,
            "sqrt" => ScaleKind::Sqrt,
            "symlog" => ScaleKind::Symlog,
            "time" => ScaleKind::Time,
            "utc" => ScaleKind::Utc,
            "band" => ScaleKind::Band,
            "point" => ScaleKind::Point,
            "ordinal" => ScaleKind::Ordinal,
            "quantile" => ScaleKind::Quantile,
            "quantize" => ScaleKind::Quantize,
            "threshold" => ScaleKind::Threshold,
            "sequential" => ScaleKind::Sequential,
            "identity" => ScaleKind::Identity,
            _ => ScaleKind::Linear,
        }
    }

    pub fn is_discrete(self) -> bool {
        matches!(
            self,
            ScaleKind::Band | ScaleKind::Point | ScaleKind::Ordinal
        )
    }

    pub fn is_continuous(self) -> bool {
        matches!(
            self,
            ScaleKind::Linear
                | ScaleKind::Log
                | ScaleKind::Pow
                | ScaleKind::Sqrt
                | ScaleKind::Symlog
                | ScaleKind::Time
                | ScaleKind::Utc
                | ScaleKind::Quantile
                | ScaleKind::Quantize
                | ScaleKind::Threshold
                | ScaleKind::Sequential
                | ScaleKind::Identity
        )
    }
}

#[derive(Debug, Clone)]
pub struct Scale {
    pub name: String,
    pub kind: ScaleKind,
    pub domain: Vec<Value>,
    pub d0: f64,
    pub d1: f64,
    pub r0: f64,
    pub r1: f64,
    pub step: f64,
    pub bandwidth: f64,
    pub padding_inner: f64,
    pub padding_outer: f64,
    pub clamp: bool,
    pub exponent: f64,
    /// Colors for color scales (ordinal palettes and continuous interpolators).
    pub colors: Vec<String>,
    /// Raw range values for ordinal / shape / custom ranges.
    pub range: Vec<Value>,
    /// Bin boundaries exposed by binned scales (`scale.bins`).
    pub bins: Vec<Value>,
    /// Tick count from the most recent `ticks` call (used for label precision).
    pub last_count: std::cell::Cell<f64>,
    /// Sorted breakpoints for `quantile` and `threshold` color lookups.
    pub stops: Vec<f64>,
    /// Numeric base for logarithmic scales.
    pub base: f64,
    pub reverse: bool,
    /// Color interpolation mode (`rgb`, `hcl`, `hcl-long`, ...).
    pub interpolate: Option<String>,
    /// Diverging scale midpoint (`domainMid`).
    pub domain_mid: Option<f64>,
}

impl Scale {
    pub fn kind_from_spec(spec: &Value) -> ScaleKind {
        ScaleKind::parse(
            spec.get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("linear"),
        )
    }

    pub fn is_discrete(&self) -> bool {
        self.kind.is_discrete()
    }

    pub fn is_color_scale(&self) -> bool {
        !self.colors.is_empty()
    }

    pub fn scale(&self, value: &Value) -> Value {
        self.scale_opt(value).unwrap_or(Value::Null)
    }

    pub fn scale_opt(&self, value: &Value) -> Option<Value> {
        match self.kind {
            ScaleKind::Band => {
                let i = self.domain_index(value)?;
                Some(json_number(self.band_start(i)))
            }
            ScaleKind::Point => {
                let i = self.domain_index(value)?;
                Some(json_number(self.point_pos(i)))
            }
            ScaleKind::Ordinal => {
                let i = self.domain_index(value)?;
                if !self.colors.is_empty() {
                    let c = &self.colors[i % self.colors.len()];
                    Some(Value::String(c.clone()))
                } else if !self.range.is_empty() {
                    Some(self.range[i % self.range.len()].clone())
                } else {
                    Some(value.clone())
                }
            }
            ScaleKind::Quantile | ScaleKind::Quantize | ScaleKind::Threshold => {
                if !self.colors.is_empty() {
                    let n = self.colors.len().max(1);
                    let x = to_number(value);
                    let idx = if x.is_finite() {
                        match self.kind {
                            // d3-scale's discretizing scales use bisecting over
                            // explicit thresholds, or equal-width bins for
                            // `quantize`.
                            ScaleKind::Quantize => {
                                let t = self.numeric_t(value).clamp(0.0, 1.0);
                                (t * n as f64).floor() as usize
                            }
                            _ => self.stops.partition_point(|s| *s <= x),
                        }
                    } else {
                        0
                    };
                    Some(Value::String(self.colors[idx.min(n - 1)].clone()))
                } else if !self.range.is_empty() {
                    let n = self.range.len().max(1);
                    let x = to_number(value);
                    let idx = if x.is_finite() {
                        match self.kind {
                            ScaleKind::Quantize => {
                                let t = self.numeric_t(value).clamp(0.0, 1.0);
                                (t * n as f64).floor() as usize
                            }
                            _ => self.stops.partition_point(|s| *s <= x),
                        }
                    } else {
                        0
                    };
                    Some(self.range[idx.min(n - 1)].clone())
                } else {
                    Some(self.scale_numeric(value))
                }
            }
            ScaleKind::Sequential => {
                if !self.colors.is_empty() {
                    let t = self.numeric_t(value);
                    Some(Value::String(interpolate_palette(&self.colors, t, self.interpolate.as_deref())))
                } else {
                    Some(self.scale_numeric(value))
                }
            }
            _ => Some(self.scale_numeric(value)),
        }
    }

    pub fn scale_band(&self, value: &Value, band: f64) -> Value {
        match self.kind {
            ScaleKind::Band => match self.domain_index(value) {
                Some(i) => json_number(self.band_start(i) + band * self.bandwidth),
                None => Value::Null,
            },
            ScaleKind::Point => self.scale(value),
            _ => self.scale(value),
        }
    }

    fn domain_index(&self, value: &Value) -> Option<usize> {
        self.domain
            .iter()
            .position(|v| value_eq(v, value))
            .or_else(|| {
                // Numeric/date values may be compared after coercion.
                let n = to_number(value);
                if n.is_finite() {
                    self.domain
                        .iter()
                        .position(|v| (to_number(v) - n).abs() < 1e-9)
                } else {
                    None
                }
            })
    }

    fn band_start(&self, index: usize) -> f64 {
        let n = self.domain.len() as f64;
        let start = self.r0
            + (self.r1 - self.r0 - self.step * (n - self.padding_inner)) * 0.5;
        start + index as f64 * self.step
    }

    fn point_pos(&self, index: usize) -> f64 {
        if self.domain.len() <= 1 {
            (self.r0 + self.r1) / 2.0
        } else {
            self.r0 + self.step * self.padding_outer + index as f64 * self.step
        }
    }

    pub fn invert(&self, value: f64) -> Value {
        match self.kind {
            ScaleKind::Band => {
                if self.step == 0.0 {
                    return Value::Null;
                }
                let i = ((value - self.r0 - self.step * self.padding_outer) / self.step).floor();
                self.domain
                    .get(i.max(0.0) as usize)
                    .cloned()
                    .unwrap_or(Value::Null)
            }
            ScaleKind::Point => {
                if self.step == 0.0 || self.domain.is_empty() {
                    return Value::Null;
                }
                let i = ((value - self.r0 - self.step * self.padding_outer) / self.step).round();
                self.domain
                    .get(i.max(0.0) as usize)
                    .cloned()
                    .unwrap_or(Value::Null)
            }
            _ => {
                if self.r1 == self.r0 {
                    return Value::Null;
                }
                let t = (value - self.r0) / (self.r1 - self.r0);
                let t = if self.clamp { t.clamp(0.0, 1.0) } else { t };
                let x = self.d0 + t * (self.d1 - self.d0);
                if matches!(self.kind, ScaleKind::Time | ScaleKind::Utc) {
                    json_number(x)
                } else {
                    json_number(x)
                }
            }
        }
    }

    fn numeric_t(&self, value: &Value) -> f64 {
        let x = to_number(value);
        if self.kind == ScaleKind::Log {
            // d3-scale reflects logarithmic transforms for negative domains.
            let base = if self.base > 0.0 { self.base } else { 10.0 };
            let reflect = self.d0 < 0.0;
            let log = |v: f64| {
                let l = if reflect {
                    -(-v).max(f64::MIN_POSITIVE).log(base)
                } else {
                    v.max(f64::MIN_POSITIVE).log(base)
                };
                l
            };
            let d0 = log(self.d0);
            let d1 = log(self.d1);
            let x = log(x);
            let t = if d1 == d0 { 0.5 } else { (x - d0) / (d1 - d0) };
            if self.clamp {
                t.clamp(0.0, 1.0)
            } else {
                t
            }
        } else if self.kind == ScaleKind::Pow || self.kind == ScaleKind::Sqrt {
            let exponent = if self.kind == ScaleKind::Sqrt {
                0.5
            } else {
                self.exponent
            };
            let f = |v: f64| {
                if v < 0.0 {
                    -((-v).powf(exponent))
                } else {
                    v.powf(exponent)
                }
            };
            let d0 = f(self.d0);
            let d1 = f(self.d1);
            let x = f(x);
            let t = if d1 == d0 { 0.5 } else { (x - d0) / (d1 - d0) };
            if self.clamp {
                t.clamp(0.0, 1.0)
            } else {
                t
            }
        } else {
            let t = if self.d1 == self.d0 {
                0.5
            } else {
                (x - self.d0) / (self.d1 - self.d0)
            };
            if self.clamp {
                t.clamp(0.0, 1.0)
            } else {
                t
            }
        }
    }

    fn color_t(&self, value: &Value) -> f64 {
        if let Some(mid) = self.domain_mid {
            let x = to_number(value);
            let d0 = self.d0.min(self.d1);
            let d1 = self.d0.max(self.d1);
            let mut t = if x <= mid {
                if mid > d0 {
                    0.5 * (x - d0) / (mid - d0)
                } else {
                    0.0
                }
            } else if d1 > mid {
                0.5 + 0.5 * (x - mid) / (d1 - mid)
            } else {
                1.0
            };
            if self.clamp {
                t = t.clamp(0.0, 1.0);
            }
            t
        } else {
            self.numeric_t(value)
        }
    }

    fn scale_numeric(&self, value: &Value) -> Value {
        if self.kind == ScaleKind::Identity {
            return json_number(to_number(value));
        }
        let t = if self.colors.is_empty() {
            self.numeric_t(value)
        } else {
            self.color_t(value)
        };
        if !self.colors.is_empty() {
            Value::String(interpolate_palette(&self.colors, t, self.interpolate.as_deref()))
        } else {
            json_number(self.r0 + t * (self.r1 - self.r0))
        }
    }

    /// Tick values for an axis, following d3's tick generation.
    pub fn ticks(&self, count: f64) -> Vec<Value> {
        self.last_count.set(count);
        if !self.bins.is_empty() {
            return self.bin_ticks(count);
        }
        match self.kind {
            ScaleKind::Band | ScaleKind::Point | ScaleKind::Ordinal => self.domain.clone(),
            ScaleKind::Quantile | ScaleKind::Quantize | ScaleKind::Threshold => {
                if self.colors.len() > 1 && self.kind == ScaleKind::Quantize {
                    let n = self.colors.len().max(2);
                    (0..n)
                        .map(|i| {
                            let t = i as f64 / (n - 1) as f64;
                            json_number(self.d0 + t * (self.d1 - self.d0))
                        })
                        .collect()
                } else {
                    self.domain.clone()
                }
            }
            ScaleKind::Log => log_ticks(self.d0, self.d1, count),
            ScaleKind::Time | ScaleKind::Utc => {
                let mut values = if self.kind == ScaleKind::Time {
                    time_ticks_local(self.d0, self.d1, count)
                } else {
                    time_ticks(self.d0, self.d1, count)
                };
                // d3 time intervals include a tick that lands exactly on the
                // domain end. Nudge it by a fraction of a millisecond so
                // endpoint-aware consumers do not mistake it for an excluded
                // bound; the rendered position and label are unchanged.
                if let Some(last) = values.last_mut() {
                    if (*last - self.d1).abs() < 1e-6 && self.d1 > self.d0 {
                        *last += 0.001;
                    }
                }
                values.into_iter().map(json_number).collect()
            }
            _ => linear_ticks(self.d0, self.d1, count)
                .into_iter()
                .map(json_number)
                .collect(),
        }
    }

    /// Filter bin boundaries to those inside the scale range and, when
    /// requested, thin them the way vega-scale's `validTicks` does.
    fn bin_ticks(&self, count: f64) -> Vec<Value> {
        let lo = self.r0.min(self.r1).floor();
        let hi = self.r0.max(self.r1).ceil();
        let mut pairs: Vec<(f64, Value)> = self
            .bins
            .iter()
            .map(|v| {
                let p = self.scale_numeric(v).as_f64().unwrap_or(f64::NAN);
                (p, v.clone())
            })
            .filter(|(p, _)| p.is_finite() && lo <= *p && *p <= hi)
            .collect();
        if self.r0 <= self.r1 {
            pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        } else {
            pairs.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        }
        let mut ticks: Vec<Value> = pairs.into_iter().map(|(_, v)| v).collect();
        // vega-scale's tickCount() expands the count to at least the number of
        // bin boundaries before validTicks() thins the result.
        let count = count.max(self.bins.len() as f64);
        if count > 0.0 && ticks.len() > 1 {
            let endpoints = vec![ticks[0].clone(), ticks[ticks.len() - 1].clone()];
            while ticks.len() as f64 > count && ticks.len() >= 3 {
                ticks = ticks
                    .into_iter()
                    .enumerate()
                    .filter(|(i, _)| i % 2 == 0)
                    .map(|(_, v)| v)
                    .collect();
            }
            if ticks.len() < 3 {
                ticks = endpoints;
            }
        }
        ticks
    }

    pub fn label(
        &self,
        value: &Value,
        format_spec: Option<&str>,
        format_type: Option<&str>,
    ) -> String {
        // Vega's default formatter applies JavaScript string coercion, which
        // renders domain values of `null` as "null" (e.g. null bins).
        if value.is_null() {
            return "null".to_string();
        }
        let spec = format_spec.unwrap_or("");
        match self.kind {
            ScaleKind::Time | ScaleKind::Utc => {
                if spec.is_empty()
                    && (format_type.is_none()
                        || matches!(format_type, Some("time") | Some("utc")))
                {
                    // Vega's time locale falls back to d3-scale's multi-format
                    // when no specifier is supplied.
                    return if self.kind == ScaleKind::Utc {
                        crate::format::format_date_multi_utc(to_number(value))
                    } else {
                        format_date_multi_span(to_number(value), (self.d1 - self.d0).abs())
                    };
                }
                let ty = format_type
                    .or(if self.kind == ScaleKind::Utc { Some("utc") } else { Some("time") });
                format_value(value, spec, ty)
            }
            ScaleKind::Band | ScaleKind::Point | ScaleKind::Ordinal => {
                if format_type == Some("time") || format_type == Some("utc") {
                    if self.kind.is_discrete() && spec.is_empty() {
                        return self.discrete_time_label(value);
                    }
                    let n = to_number(value);
                    if n.is_finite() {
                        return format_value(&json_number(n), spec, format_type.or(Some("time")));
                    }
                }
                if format_type == Some("time") || spec.contains('%') {
                    let n = to_number(value);
                    if n.is_finite() {
                        return format_value(&json_number(n), spec, format_type.or(Some("time")));
                    }
                }
                if spec.is_empty() && format_type.is_none() {
                    // Vega's default formatter is `String(value)`, which uses
                    // a plain ASCII minus sign (unlike d3-format's `−`).
                    crate::value::to_string_js(value)
                } else if value.is_string() {
                    let s = value.as_str().unwrap_or("");
                    if spec.is_empty() {
                        s.to_string()
                    } else {
                        format_value(value, spec, format_type)
                    }
                } else {
                    format_value(value, spec, format_type)
                }
            }
            _ => {
                if !self.bins.is_empty() && is_inferred_numeric_spec(spec) {
                    return self.bin_label(value);
                }
                if self.kind == ScaleKind::Log {
                    if spec.is_empty() && matches!(format_type, None | Some("number")) {
                        return crate::format::format_number(to_number(value), ",~g");
                    }
                    return format_value(value, spec, format_type);
                }
                if self.kind.is_continuous()
                    && !self.kind.is_discrete()
                    && matches!(format_type, None | Some("number"))
                {
                    return format_span(
                        to_number(value),
                        self.d0,
                        self.d1,
                        self.last_count.get(),
                        if spec.is_empty() { ",f" } else { spec },
                    );
                }
                format_value(value, spec, format_type)
            }
        }
    }

    /// Labels for discrete time-valued scales (band/point domains built from
    /// time-unit fields). Mirrors vega-time's unit default specifiers.
    fn discrete_time_label(&self, value: &Value) -> String {
        const DAY: f64 = 86_400_000.0;
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for v in &self.domain {
            let n = time_value(v);
            if n.is_finite() {
                min = min.min(n);
                max = max.max(n);
            }
        }
        let span = if min.is_finite() && max.is_finite() {
            max - min
        } else {
            f64::INFINITY
        };
        let n = time_value(value);
        if span < 2.0 * DAY {
            format_date_multi_span(n, span)
        } else if span <= 8.0 * DAY {
            format_date(n, "%a")
        } else if span <= 40.0 * DAY {
            format_date(n, "%b %d")
        } else if span <= 400.0 * DAY {
            format_date(n, "%b")
        } else {
            format_date(n, "%Y")
        }
    }

    /// Labels for binned continuous scales.
    ///
    /// Vega's axis formatter derives its precision from `tickStep(domain,
    /// count)` rather than from the spacing of the bin boundaries. The count
    /// comes from the most recent `ticks` call (bin-aware).
    fn bin_label(&self, value: &Value) -> String {
        let count = self.last_count.get().max(1.0).max(self.bins.len() as f64);
        let step = tick_step(self.d0, self.d1, count).abs();
        let decimals = if step > 0.0 && step.is_finite() {
            (-step.log10().floor()).max(0.0) as usize
        } else {
            0
        };
        format_value(value, &format!(",.{}f", decimals.min(20)), None)
    }

    pub fn range_numeric(&self) -> Vec<Value> {
        match self.kind {
            ScaleKind::Band => vec![
                json_number(self.r0),
                json_number(self.band_start(self.domain.len().saturating_sub(1)) + self.bandwidth),
            ],
            ScaleKind::Point => {
                if self.domain.is_empty() {
                    vec![json_number(self.r0), json_number(self.r1)]
                } else {
                    vec![
                        json_number(self.point_pos(0)),
                        json_number(self.point_pos(self.domain.len() - 1)),
                    ]
                }
            }
            _ => vec![json_number(self.r0), json_number(self.r1)],
        }
    }
}

/// Recognize the format specifiers the renderer infers for continuous axes.
fn is_inferred_numeric_spec(spec: &str) -> bool {
    if spec == ",d" {
        return true;
    }
    if let Some(rest) = spec.strip_prefix(",.") {
        if let Some(digits) = rest.strip_suffix('f') {
            return !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit());
        }
    }
    false
}

fn value_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let x = x.as_f64().unwrap_or(f64::NAN);
            let y = y.as_f64().unwrap_or(f64::NAN);
            x == y
        }
        _ => a == b,
    }
}

fn interpolate_palette(palette: &[String], t: f64, interpolate: Option<&str>) -> String {
    sample_anchors(palette, t, interpolate)
}

pub struct RangeResolution {
    pub values: Vec<Value>,
    pub colors: Vec<String>,
    pub step: Option<f64>,
    pub reverse: bool,
}

/// Vega's default scale-range aliases (see `config.range` in vega-parser).
///
/// Returns the scheme name and whether the scheme extent is reversed.
fn range_alias(name: &str) -> (&str, bool) {
    match name {
        "category" => ("tableau10", false),
        "ordinal" | "ramp" => ("blues", false),
        "heatmap" => ("yellowgreenblue", false),
        "diverging" => ("blueorange", true),
        other => (other, false),
    }
}

/// Hex palettes used by vega-scale's continuous (interpolating) schemes.
///
/// These differ from the ColorBrewer step lists in [`crate::color`]: Vega
/// builds a piecewise RGB interpolator over these anchor colors.
const CONTINUOUS_PALETTES: &[(&str, &str)] = &[
    (
        "lighttealblue",
        "e3e9e0c0dccf9aceca7abfc859afc0389fb9328dad2f7ca0276b95255988",
    ),
    (
        "blues",
        "cfe1f2bed8eca8cee58fc1de74b2d75ba3cf4592c63181bd206fb2125ca40a4a90",
    ),
    (
        "greens",
        "d3eecdc0e6baabdda594d3917bc77d60ba6c46ab5e329a512089430e7735036429",
    ),
    (
        "greys",
        "e2e2e2d4d4d4c4c4c4b1b1b19d9d9d8888887575756262624d4d4d3535351e1e1e",
    ),
    (
        "oranges",
        "fdd8b3fdc998fdb87bfda55efc9244f87f2cf06b18e4580bd14904b93d029f3303",
    ),
    (
        "purples",
        "e2e1efd4d4e8c4c5e0b4b3d6a3a0cc928ec3827cb97566ae684ea25c3696501f8c",
    ),
    (
        "reds",
        "fdc9b4fcb49afc9e80fc8767fa7051f6573fec3f2fdc2a25c81b1db21218970b13",
    ),
    (
        "bluegreen",
        "d5efedc1e8e0a7ddd18bd2be70c6a958ba9144ad77319c5d2089460e7736036429",
    ),
    (
        "blueorange",
        "134b852f78b35da2cb9dcae1d2e5eff2f0ebfce0bafbbf74e8932fc5690d994a07",
    ),
    (
        "redblue",
        "8c0d25bf363adf745ef4ae91fbdbc9f2efeed2e5ef9dcae15da2cb2f78b3134b85",
    ),
    (
        "redgrey",
        "8c0d25bf363adf745ef4ae91fcdccbfaf4f1e2e2e2c0c0c0969696646464343434",
    ),
    (
        "purplegreen",
        "5b1667834792a67fb6c9aed3e6d6e8eff0efd9efd5aedda971bb75368e490e5e29",
    ),
    (
        "spectral",
        "9e0142d13c4bf0704afcac63fedd8dfbf8b0e0f3a1a9dda269bda94288b55e4fa2",
    ),
    (
        "yellowgreenblue",
        "eff9bddbf1b4bde5b594d5b969c5be45b4c22c9ec02182b82163aa23479c1c3185",
    ),
    (
        "yellowgreen",
        "e4f4acd1eca0b9e2949ed68880c97c62bb6e47aa5e3297502083440e723b036034",
    ),
    (
        "yelloworangebrown",
        "feeaa1fedd84fecc63feb746fca031f68921eb7215db5e0bc54c05ab3d038f3204",
    ),
    (
        "yelloworangered",
        "fee087fed16ffebd59fea849fd903efc7335f9522bee3423de1b20ca0b22af0225",
    ),
    (
        "viridis",
        "440154470e61481a6c482575472f7d443a834144873d4e8a39568c35608d31688e2d708e2a788e27818e23888e21918d1f988b1fa08822a8842ab07f35b77943bf7154c56866cc5d7ad1518fd744a5db36bcdf27d2e21be9e51afde725",
    ),
    (
        "magma",
        "0000040404130b0924150e3720114b2c11603b0f704a107957157e651a80721f817f24828c29819a2e80a8327db6377ac43c75d1426fde4968e95462f1605df76f5cfa7f5efc8f65fe9f6dfeaf78febf84fece91fddea0fcedaffcfdbf",
    ),
    (
        "inferno",
        "0000040403130c0826170c3b240c4f330a5f420a68500d6c5d126e6b176e781c6d86216b932667a12b62ae305cbb3755c73e4cd24644dd513ae65c30ed6925f3771af8850ffb9506fca50afcb519fac62df6d645f2e661f3f484fcffa4",
    ),
    (
        "plasma",
        "0d088723069033059742039d5002a25d01a66a00a87801a88405a7900da49c179ea72198b12a90ba3488c33d80cb4779d35171da5a69e16462e76e5bed7953f2834cf68f44fa9a3dfca636fdb32ffec029fcce25f9dc24f5ea27f0f921",
    ),
    (
        "cividis",
        "00205100235800265d002961012b65042e670831690d346b11366c16396d1c3c6e213f6e26426e2c456e31476e374a6e3c4d6e42506e47536d4c566d51586e555b6e5a5e6e5e616e62646f66676f6a6a706e6d717270717573727976737c79747f7c75827f758682768985778c8877908b78938e789691789a94789e9778a19b78a59e77a9a177aea575b2a874b6ab73bbaf71c0b26fc5b66dc9b96acebd68d3c065d8c462ddc85fe2cb5ce7cf58ebd355f0d652f3da4ff7de4cfae249fce647",
    ),
    (
        "turbo",
        "23171b32204a3e2a71453493493eae4b49c54a53d7485ee44569ee4074f53c7ff8378af93295f72e9ff42ba9ef28b3e926bce125c5d925cdcf27d5c629dcbc2de3b232e9a738ee9d3ff39347f68950f9805afc7765fd6e70fe667cfd5e88fc5795fb51a1f84badf545b9f140c5ec3cd0e637dae034e4d931ecd12ef4c92bfac029ffb626ffad24ffa223ff9821ff8d1fff821dff771cfd6c1af76118f05616e84b14df4111d5380fcb2f0dc0260ab61f07ac1805a313029b0f00950c00910b00",
    ),
    (
        "rainbow",
        "6e40aa883eb1a43db3bf3cafd83fa4ee4395fe4b83ff576eff6659ff7847ff8c38f3a130e2b72fcfcc36bee044aff05b8ff4576ff65b52f6673af27828ea8d1ddfa319d0b81cbecb23abd82f96e03d82e14c6edb5a5dd0664dbf6e40aa",
    ),
    (
        "sinebow",
        "ff4040fc582af47218e78d0bd5a703bfbf00a7d5038de70b72f41858fc2a40ff402afc5818f4720be78d03d5a700bfbf03a7d50b8de71872f42a58fc4040ff582afc7218f48d0be7a703d5bf00bfd503a7e70b8df41872fc2a58ff4040",
    ),
];

/// Hex palettes used by vega-scale's discrete (categorical) schemes.
const DISCRETE_PALETTES: &[(&str, &str)] = &[
    (
        "category20",
        "1f77b4aec7e8ff7f0effbb782ca02c98df8ad62728ff98969467bdc5b0d58c564bc49c94e377c2f7b6d27f7f7fc7c7c7bcbd22dbdb8d17becf9edae5",
    ),
    (
        "category20b",
        "393b795254a36b6ecf9c9ede6379398ca252b5cf6bcedb9c8c6d31bd9e39e7ba52e7cb94843c39ad494ad6616be7969c7b4173a55194ce6dbdde9ed6",
    ),
    (
        "category20c",
        "3182bd6baed69ecae1c6dbefe6550dfd8d3cfdae6bfdd0a231a35474c476a1d99bc7e9c0756bb19e9ac8bcbddcdadaeb636363969696bdbdbdd9d9d9",
    ),
];

fn normalized_scheme_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn hex_palette(digits: &str) -> Vec<String> {
    digits
        .as_bytes()
        .chunks(6)
        .filter(|c| c.len() == 6)
        .map(|c| format!("#{}", std::str::from_utf8(c).unwrap_or("")))
        .collect()
}

fn continuous_anchors(name: &str) -> Option<Vec<String>> {
    let key = normalized_scheme_name(name);
    CONTINUOUS_PALETTES
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, p)| hex_palette(p))
}

fn discrete_colors(name: &str) -> Option<Vec<String>> {
    let key = normalized_scheme_name(name);
    if let Some((_, p)) = DISCRETE_PALETTES.iter().find(|(k, _)| *k == key) {
        return Some(hex_palette(p));
    }
    if key == "symbol" {
        return Some(color::symbol_scheme());
    }
    // Categorical schemes from the shared color module. `category20` and
    // friends are already handled above (Vega's palettes differ from the
    // ColorBrewer lists used elsewhere).
    let categorical = matches!(
        key.as_str(),
        "tableau10"
            | "tableau"
            | "category10"
            | "category"
            | "cat10"
            | "tableau20"
            | "cat20"
            | "accent"
            | "dark2"
            | "dark"
            | "paired"
            | "set1"
            | "set2"
            | "set3"
            | "pastel1"
            | "pastel2"
            | "observable10"
    );
    if categorical {
        color::scheme(&key)
    } else {
        None
    }
}

const QUANTIZE_COUNT: usize = 5;

/// Sample a continuous scheme the way vega-scale's `quantizeInterpolator`
/// does: `count` colors at evenly spaced interior positions.
fn quantized_scheme(anchors: &[String], count: usize) -> Vec<String> {
    (1..=count)
        .map(|i| sample_anchors(anchors, i as f64 / (count + 1) as f64, None))
        .collect()
}

/// Piecewise RGB interpolation across scheme anchors, serialized the way
/// d3-interpolate does (`rgb(r, g, b)` with rounded channels).
fn sample_anchors(anchors: &[String], t: f64, interpolate: Option<&str>) -> String {
    let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
    if anchors.is_empty() {
        return "none".to_string();
    }
    if anchors.len() == 1 {
        return rgb_string(&anchors[0]);
    }
    let pos = t * (anchors.len() - 1) as f64;
    let idx = (pos.floor() as usize).min(anchors.len() - 1);
    let frac = pos - idx as f64;
    let a = color::parse_color(&anchors[idx]);
    let b = color::parse_color(&anchors[(idx + 1).min(anchors.len() - 1)]);
    match (a, b) {
        (Some(a), Some(b)) => {
            let c = if matches!(interpolate, Some("hcl") | Some("hcl-long")) {
                color::interpolate_hcl(a, b, frac)
            } else {
                color::interpolate_rgb(a, b, frac)
            };
            format!("rgb({}, {}, {})", c.r, c.g, c.b)
        }
        _ => anchors[idx].clone(),
    }
}

fn rgb_string(value: &str) -> String {
    match color::parse_color(value) {
        Some(c) => format!("rgb({}, {}, {})", c.r, c.g, c.b),
        None => value.to_string(),
    }
}

/// Number of colors a scheme range resolves to for a given scale type.
fn scheme_count(kind: ScaleKind, domain_len: usize) -> usize {
    match kind {
        ScaleKind::Ordinal if domain_len > 0 => domain_len,
        ScaleKind::Ordinal | ScaleKind::Quantile | ScaleKind::Quantize => QUANTIZE_COUNT,
        ScaleKind::Threshold => domain_len.max(1) + 1,
        _ => domain_len.max(2),
    }
}

fn palette_for_scheme(
    name: &str,
    kind: ScaleKind,
    domain_len: usize,
    count_override: Option<usize>,
) -> Vec<String> {
    let (resolved, _) = range_alias(name);
    if let Some(anchors) = continuous_anchors(resolved) {
        if matches!(
            kind,
            ScaleKind::Linear
                | ScaleKind::Log
                | ScaleKind::Pow
                | ScaleKind::Sqrt
                | ScaleKind::Symlog
                | ScaleKind::Sequential
                | ScaleKind::Identity
        ) {
            // Vega turns continuous color scales into sequential scales: the
            // full interpolator is used as the scale range.
            return anchors;
        }
        let count = count_override.unwrap_or_else(|| scheme_count(kind, domain_len));
        return quantized_scheme(&anchors, count);
    }
    if let Some(colors) = discrete_colors(resolved) {
        return match kind {
            ScaleKind::Ordinal => colors,
            _ => colors
                .into_iter()
                .take(count_override.unwrap_or_else(|| scheme_count(kind, domain_len)))
                .collect(),
        };
    }
    // Unknown scheme: fall back to the shared color module (ColorBrewer
    // sampling) so the renderer still produces colors instead of a numeric
    // range.
    let fallback = color::continuous_scheme(resolved, scheme_count(kind, domain_len).max(2));
    if fallback.is_empty() {
        continuous_anchors("blues").unwrap_or_default()
    } else {
        fallback
    }
}

fn eval_range_number(v: &Value, env: &dyn Environment) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse::<f64>().ok(),
        Value::Object(o) => {
            if let Some(sig) = o.get("signal").and_then(|s| s.as_str()) {
                expression::evaluate(sig, None, env)
                    .ok()
                    .map(|v| to_number(&v))
            } else if let Some(sig) = o.get("value") {
                eval_range_number(sig, env)
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn resolve_range(
    spec: Option<&Value>,
    kind: ScaleKind,
    domain_len: usize,
    env: &dyn Environment,
) -> RangeResolution {
    let mut out = RangeResolution {
        values: Vec::new(),
        colors: Vec::new(),
        step: None,
        reverse: false,
    };
    let Some(spec) = spec else {
        return out;
    };

    // Resolve an object-form range scheme (`{"scheme": ..., "count": ...,
    // "extent": [...]}`) into colors.
    let scheme_colors = |name: &str, o: &serde_json::Map<String, Value>| -> Vec<String> {
        let count = o
            .get("count")
            .and_then(|c| c.as_f64())
            .filter(|c| *c >= 1.0)
            .map(|c| c as usize);
        let mut palette = palette_for_scheme(name, kind, domain_len, count);
        if let Some(extent) = o.get("extent").and_then(|e| e.as_array()) {
            if extent.first().and_then(|v| v.as_f64()) == Some(1.0) {
                palette.reverse();
            }
        }
        palette
    };

    match spec {
        Value::String(name) => {
            if name == "symbol" {
                out.values = color::symbol_scheme()
                    .into_iter()
                    .map(Value::String)
                    .collect();
            } else {
                let (_, reversed) = range_alias(name);
                let mut palette = palette_for_scheme(name, kind, domain_len, None);
                if reversed {
                    palette.reverse();
                }
                if kind.is_discrete() {
                    out.values = palette.iter().cloned().map(Value::String).collect();
                }
                out.colors = palette;
            }
        }
        Value::Array(a) => {
            for v in a {
                match v {
                    Value::Object(o) => {
                        if let Some(scheme) = o.get("scheme").and_then(|s| s.as_str()) {
                            out.colors = scheme_colors(scheme, o);
                        } else if let Some(sig) = o.get("signal").and_then(|s| s.as_str()) {
                            if let Ok(v) = expression::evaluate(sig, None, env) {
                                match v {
                                    Value::Array(items) => out.values.extend(items),
                                    other => out.values.push(other),
                                }
                            }
                        } else if let Some(value) = o.get("value") {
                            out.values.push(value.clone());
                        } else {
                            out.values.push(v.clone());
                        }
                    }
                    other => out.values.push(other.clone()),
                }
            }
            if out.colors.is_empty() {
                let strings: Vec<String> = out
                    .values
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                if strings.len() == out.values.len() && !strings.is_empty() {
                    out.colors = strings;
                }
            }
        }
        Value::Object(o) => {
            if let Some(scheme) = o.get("scheme").and_then(|s| s.as_str()) {
                out.colors = scheme_colors(scheme, o);
            } else if let Some(data) = o.get("data").and_then(|d| d.as_str()) {
                // Field-driven range: `{"data": ..., "field": ...}` supplies
                // the output values directly (e.g. a column of color names).
                if let Some(rows) = env.dataset(data) {
                    let field = o.get("field").cloned().unwrap_or(Value::Null);
                    let sort_field = o
                        .get("sort")
                        .and_then(|s| s.get("field"))
                        .and_then(|f| f.as_str())
                        .map(|s| Value::String(s.to_string()));
                    let mut pairs: Vec<(Option<Value>, Value)> = rows
                        .iter()
                        .map(|row| {
                            (
                                sort_field
                                    .as_ref()
                                    .map(|f| field_value(row, f)),
                                field_value(row, &field),
                            )
                        })
                        .collect();
                    if sort_field.is_some() {
                        pairs.sort_by(|a, b| match (&a.0, &b.0) {
                            (Some(x), Some(y)) => crate::value::compare(x, y),
                            _ => std::cmp::Ordering::Equal,
                        });
                    }
                    out.values = pairs.into_iter().map(|(_, v)| v).collect();
                }
            } else if let Some(step) = o.get("step") {
                if let Some(n) = eval_range_number(step, env) {
                    out.step = Some(n);
                }
            } else if let Some(sig) = o.get("signal").and_then(|s| s.as_str()) {
                if let Ok(v) = expression::evaluate(sig, None, env) {
                    match v {
                        Value::Array(items) => out.values.extend(items),
                        Value::Number(n) => out.step = n.as_f64(),
                        other => out.values.push(other),
                    }
                }
            } else if let Some(v) = o.get("value") {
                out.values.push(v.clone());
            }
        }
        Value::Number(n) => out.values.push(Value::Number(n.clone())),
        other => out.values.push(other.clone()),
    }
    out
}

pub fn build_scale(
    name: &str,
    spec: &Value,
    store: &DataStore,
    signals: &HashMap<String, Value>,
    env: &dyn Environment,
) -> Result<Scale, Error> {
    // Vega's `reverse` flag reverses the range (d3's `.range(range.reverse())`).
    let mut reversed_spec;
    let spec = if spec.get("reverse").map(truthy).unwrap_or(false) {
        reversed_spec = spec.clone();
        if let Value::Object(o) = &mut reversed_spec {
            if let Some(Value::Array(a)) = o.get_mut("range") {
                a.reverse();
            }
        }
        &reversed_spec
    } else {
        spec
    };
    let type_name = spec
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("linear");
    if type_name == "bin-ordinal" {
        let bins = resolve_bins(spec.get("bins"), env, f64::NEG_INFINITY, f64::INFINITY);
        if bins.len() >= 2 {
            let n = bins.len() - 1;
            let (colors, values) = if let Some(range) = spec.get("range").and_then(|v| v.as_str()) {
                let (name, reversed) = range_alias(range);
                let mut palette = palette_for_scheme(name, ScaleKind::Ordinal, n, Some(n));
                if reversed {
                    palette.reverse();
                }
                (palette.clone(), palette.into_iter().map(Value::String).collect())
            } else {
                (Vec::new(), Vec::new())
            };
            let stops: Vec<f64> = bins
                .iter()
                .skip(1)
                .take(n.saturating_sub(1))
                .map(|v| to_number(v))
                .collect();
            let d0 = bins.first().map(|v| to_number(v)).unwrap_or(0.0);
            let d1 = bins.last().map(|v| to_number(v)).unwrap_or(0.0);
            let range = resolve_range(spec.get("range"), ScaleKind::Ordinal, n, env);
            return Ok(Scale {
                name: name.to_string(),
                kind: ScaleKind::Threshold,
                domain: bins.clone(),
                d0,
                d1,
                r0: 0.0,
                r1: (n.saturating_sub(1)) as f64,
                step: 0.0,
                bandwidth: 0.0,
                padding_inner: 0.0,
                padding_outer: 0.0,
                clamp: false,
                exponent: 1.0,
                colors: if colors.is_empty() { range.colors } else { colors },
                range: if values.is_empty() { range.values } else { values },
                bins,
                last_count: std::cell::Cell::new(10.0),
                stops,
                base: 10.0,
                reverse: false,
                interpolate: if spec.get("range").map(|r| r.is_array()).unwrap_or(false) {
                    spec.get("interpolate").and_then(|v| v.as_str()).map(String::from)
                } else {
                    None
                },
                domain_mid: spec
                    .get("domainMid")
                    .and_then(|v| eval_range_number(v, env)),
            });
        }
    }
    let kind = ScaleKind::parse(type_name);
    let mut domain = resolve_domain(spec.get("domain"), store, signals)?;
    if domain.is_empty() {
        domain = vec![json_number(0.0), json_number(1.0)];
    }
    if kind.is_discrete() {
        // Preserve first appearance, then apply domain sorting.
        let mut seen: Vec<Value> = Vec::new();
        for v in domain {
            if !seen.iter().any(|x| value_eq(x, &v)) {
                seen.push(v);
            }
        }
        apply_domain_sort(&mut seen, spec.get("domain"), store)?;
        domain = seen;
    } else {
        let mut nums: Vec<f64> = domain
            .iter()
            .filter_map(|v| {
                // d3's numeric extent ignores null/undefined/NaN values.
                if v.is_null() {
                    return None;
                }
                let n = if matches!(kind, ScaleKind::Time | ScaleKind::Utc) {
                    time_value(v)
                } else {
                    to_number(v)
                };
                n.is_finite().then_some(n)
            })
            .collect();
        if nums.is_empty() {
            nums = vec![0.0, 1.0];
        }
        let domain_count = nums.len();
        let mut min = nums.iter().cloned().fold(f64::INFINITY, f64::min);
        let mut max = nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        if spec.get("zero").and_then(|v| v.as_bool()).unwrap_or(false) {
            if min > 0.0 {
                min = 0.0;
            }
            if max < 0.0 {
                max = 0.0;
            }
        }
        if min == max {
            min -= 0.5;
            max += 0.5;
        }
        let raw_min = min;
        let raw_max = max;
        let nice_value = spec.get("nice");
        let nice = nice_value.map(truthy).unwrap_or(false);
        let nice_count = nice_value
            .and_then(|v| v.as_f64())
            .filter(|n| *n > 0.0)
            .unwrap_or(10.0);
        if nice && !matches!(kind, ScaleKind::Time | ScaleKind::Utc) {
            if kind == ScaleKind::Log && min > 0.0 && max > 0.0 {
                let base = spec.get("base").and_then(|v| v.as_f64()).unwrap_or(10.0);
                let base = if base > 0.0 { base } else { 10.0 };
                min = base.powf((min.log(base)).floor());
                max = base.powf((max.log(base)).ceil());
            } else {
                let (a, b) = nice_extent(min, max, nice_count);
                min = a;
                max = b;
            }
        }
        domain = vec![json_number(min), json_number(max)];
        let mut sorted = nums.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        // Continuous scales expose only their extent to the range resolver;
        // discretizing/ordinal scales see the full domain cardinality.
        let range_len = if matches!(kind, ScaleKind::Time | ScaleKind::Utc) {
            2
        } else {
            domain_count
        };
        let range = resolve_range(spec.get("range"), kind, range_len, env);
        let stops = match kind {
            ScaleKind::Threshold => sorted.clone(),
            ScaleKind::Quantile => {
                let n = range.colors.len().max(range.values.len()).max(1);
                (1..n)
                    .map(|i| quantile_sorted(&sorted, i as f64 / n as f64))
                    .collect()
            }
            _ => Vec::new(),
        };
        let (r0, r1) = if range.values.len() >= 2 {
            (
                to_number(&range.values[0]),
                to_number(range.values.last().unwrap()),
            )
        } else if range.values.len() == 1 {
            (to_number(&range.values[0]), 1.0)
        } else {
            (0.0, 1.0)
        };
        let padding_inner = spec.get("padding").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let padding_outer = spec
            .get("paddingOuter")
            .and_then(|v| v.as_f64())
            .unwrap_or(padding_inner);
        if let Some(pad) = spec.get("padding").and_then(|v| v.as_f64()) {
            if pad > 0.0 && !matches!(kind, ScaleKind::Time | ScaleKind::Utc) {
                let span = (r1 - r0).abs();
                if span > 2.0 * pad {
                    if kind == ScaleKind::Log && raw_min > 0.0 && raw_max > 0.0 {
                        let base = spec.get("base").and_then(|v| v.as_f64()).unwrap_or(10.0);
                        let base = if base > 0.0 { base } else { 10.0 };
                        let (lmin, lmax) = (raw_min.log(base), raw_max.log(base));
                        let pad_value = pad * (lmax - lmin) / (span - 2.0 * pad);
                        let mut lo = lmin - pad_value;
                        let mut hi = lmax + pad_value;
                        if spec.get("nice").map(truthy).unwrap_or(false) {
                            lo = lo.floor();
                            hi = hi.ceil();
                        }
                        min = base.powf(lo);
                        max = base.powf(hi);
                    } else {
                        let pad_value = pad * (raw_max - raw_min) / (span - 2.0 * pad);
                        min = raw_min - pad_value;
                        max = raw_max + pad_value;
                        if spec.get("nice").map(truthy).unwrap_or(false) {
                            let (a, b) = nice_extent(min, max, nice_count);
                            min = a;
                            max = b;
                        }
                    }
                    domain = vec![json_number(min), json_number(max)];
                }
            }
        }
        let bins = resolve_bins(spec.get("bins"), env, min, max);
        return Ok(Scale {
            name: name.to_string(),
            kind,
            domain,
            d0: min,
            d1: max,
            r0,
            r1,
            step: 0.0,
            bandwidth: 0.0,
            padding_inner,
            padding_outer,
            clamp: spec.get("clamp").and_then(|v| v.as_bool()).unwrap_or(false),
            exponent: spec.get("exponent").and_then(|v| v.as_f64()).unwrap_or(1.0),
            colors: range.colors,
            range: range.values,
            bins,
            last_count: std::cell::Cell::new(10.0),
            stops,
            base: spec.get("base").and_then(|v| v.as_f64()).unwrap_or(10.0),
            reverse: range.reverse,
            interpolate: if spec.get("range").map(|r| r.is_array()).unwrap_or(false) {
                spec.get("interpolate").and_then(|v| v.as_str()).map(String::from)
            } else {
                None
            },
            domain_mid: spec
                .get("domainMid")
                .and_then(|v| eval_range_number(v, env)),
        });
    }

    let range = resolve_range(spec.get("range"), kind, domain.len(), env);
    let n = domain.len();
    let padding_inner = if kind == ScaleKind::Band {
        spec.get("paddingInner")
            .and_then(|v| v.as_f64())
            .or_else(|| spec.get("padding").and_then(|v| v.as_f64()))
            .unwrap_or(0.0)
    } else {
        0.0
    };
    let padding_outer = if kind == ScaleKind::Band {
        spec.get("paddingOuter")
            .and_then(|v| v.as_f64())
            .or_else(|| spec.get("padding").and_then(|v| v.as_f64()))
            .unwrap_or(0.0)
    } else {
        spec.get("padding").and_then(|v| v.as_f64()).unwrap_or(0.0)
    };
    let (r0, r1, step, bandwidth) = match kind {
        ScaleKind::Band => {
            let space = n as f64 - padding_inner + 2.0 * padding_outer;
            let (r0, r1, step) = if let Some(step) = range.step {
                (0.0, step * space, step)
            } else if range.values.len() >= 2 {
                let r0 = to_number(&range.values[0]);
                let r1 = to_number(range.values.last().unwrap());
                (
                    r0,
                    r1,
                    if space > 0.0 { (r1 - r0) / space } else { 0.0 },
                )
            } else {
                (0.0, 21.0, 21.0)
            };
            let bw = step * (1.0 - padding_inner);
            (r0, r1, step, bw)
        }
        ScaleKind::Point => {
            let pad = padding_outer;
            let span = (n.max(2) as f64 - 1.0) + 2.0 * pad;
            let (r0, step) = if let Some(step) = range.step {
                (0.0, step)
            } else if range.values.len() >= 2 {
                let a = to_number(&range.values[0]);
                let b = to_number(range.values.last().unwrap());
                (a, if span > 0.0 { (b - a) / span } else { 0.0 })
            } else {
                (0.0, 1.0)
            };
            let r1 = r0 + step * span;
            (r0, r1, step, 0.0)
        }
        _ => (0.0, 0.0, 0.0, 0.0),
    };
    Ok(Scale {
        name: name.to_string(),
        kind,
        domain,
        d0: 0.0,
        d1: (n.saturating_sub(1)) as f64,
        r0,
        r1,
        step,
        bandwidth,
        padding_inner,
        padding_outer,
        clamp: spec.get("clamp").and_then(|v| v.as_bool()).unwrap_or(false),
        exponent: spec.get("exponent").and_then(|v| v.as_f64()).unwrap_or(1.0),
        colors: range.colors,
        range: range.values,
        bins: Vec::new(),
        last_count: std::cell::Cell::new(10.0),
        stops: Vec::new(),
        base: spec.get("base").and_then(|v| v.as_f64()).unwrap_or(10.0),
        reverse: range.reverse,
        interpolate: if spec.get("range").map(|r| r.is_array()).unwrap_or(false) {
            spec.get("interpolate").and_then(|v| v.as_str()).map(String::from)
        } else {
            None
        },
        domain_mid: spec
            .get("domainMid")
            .and_then(|v| eval_range_number(v, env)),
    })
}

/// d3-array's `quantileSorted` (type-7 linear interpolation).
fn quantile_sorted(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return f64::NAN;
    }
    if p <= 0.0 || n < 2 {
        return sorted[0];
    }
    if p >= 1.0 {
        return sorted[n - 1];
    }
    let i = (n - 1) as f64 * p;
    let i0 = i.floor() as usize;
    let v0 = sorted[i0];
    let v1 = sorted[(i0 + 1).min(n - 1)];
    v0 + (v1 - v0) * (i - i0 as f64)
}

fn resolve_bins(spec: Option<&Value>, env: &dyn Environment, lo: f64, hi: f64) -> Vec<Value> {
    match spec {
        Some(Value::Array(items)) => items.clone(),
        Some(Value::Object(o)) => {
            let evaluated = if let Some(sig) = o.get("signal").and_then(|s| s.as_str()) {
                expression::evaluate(sig, None, env).ok()
            } else {
                Some(Value::Object(o.clone()))
            };
            match evaluated {
                Some(Value::Array(items)) => items,
                Some(Value::Object(b)) => {
                    let Some(step) = b.get("step").and_then(|v| v.as_f64()) else {
                        return Vec::new();
                    };
                    if !(step > 0.0) {
                        return Vec::new();
                    }
                    let mut start = b.get("start").and_then(|v| v.as_f64()).unwrap_or(lo);
                    let mut stop = b.get("stop").and_then(|v| v.as_f64()).unwrap_or(hi);
                    if start < lo {
                        start = step * (lo / step).ceil();
                    }
                    if stop > hi {
                        stop = step * (hi / step).floor();
                    }
                    let mut out = Vec::new();
                    let mut v = start;
                    while v <= stop + step / 2.0 && out.len() < 100_000 {
                        out.push(json_number(v));
                        v += step;
                    }
                    out
                }
                _ => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}

fn apply_domain_sort(
    values: &mut [Value],
    domain_spec: Option<&Value>,
    store: &DataStore,
) -> Result<(), Error> {
    let Some(sort) = domain_spec.and_then(|d| d.get("sort")) else {
        return Ok(());
    };
    match sort {
        Value::Bool(true) => {
            values.sort_by(compare);
        }
        Value::Bool(false) | Value::Null => {}
        Value::Object(o) => {
            // `sort.field` names the fields aggregated to derive the sort key;
            // the domain's own `field`/`fields` identify the tuple field(s)
            // whose values are being ordered.
            let sort_fields: Vec<String> = match o.get("field") {
                Some(Value::String(s)) => vec![s.clone()],
                Some(Value::Array(a)) => a
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect(),
                _ => vec![],
            };
            let domain_fields: Vec<String> = match domain_spec.and_then(|d| d.get("field")) {
                Some(Value::String(s)) => vec![s.clone()],
                Some(Value::Array(a)) => a
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect(),
                _ => match domain_spec.and_then(|d| d.get("fields")) {
                    Some(Value::Array(a)) => a
                        .iter()
                        .filter_map(|v| match v {
                            Value::String(s) => Some(s.clone()),
                            Value::Object(fo) => fo
                                .get("field")
                                .and_then(|f| f.as_str())
                                .map(String::from),
                            _ => None,
                        })
                        .collect(),
                    _ => vec![],
                },
            };
            let orders: Vec<bool> = match o.get("order") {
                Some(Value::String(s)) => vec![s == "descending"],
                Some(Value::Array(a)) => {
                    a.iter().map(|v| v.as_str() == Some("descending")).collect()
                }
                _ => vec![],
            };
            let op = o.get("op").and_then(|v| v.as_str()).unwrap_or("min");
            let data = domain_spec
                .and_then(|d| d.get("data"))
                .and_then(|v| v.as_str())
                .and_then(|n| store.get(n))
                .cloned()
                .unwrap_or_default();
            let mut keyed: Vec<(Value, Vec<Value>)> = values
                .iter()
                .map(|v| {
                    let rows: Vec<Value> = data
                        .iter()
                        .filter(|d| {
                            domain_fields.is_empty()
                                || domain_fields
                                    .iter()
                                    .any(|f| value_eq(&get_field_owned(d, f), v))
                        })
                        .cloned()
                        .collect();
                    let key = if op == "count" {
                        vec![json_number(rows.len() as f64)]
                    } else if sort_fields.is_empty() {
                        vec![v.clone()]
                    } else {
                        sort_fields
                            .iter()
                            .map(|f| crate::data::aggregate_op(op, f, &rows))
                            .collect()
                    };
                    (v.clone(), key)
                })
                .collect();
            keyed.sort_by(|a, b| {
                if sort_fields.is_empty() {
                    let mut ord = if op == "count" {
                        compare(&a.1[0], &b.1[0])
                    } else {
                        compare(&a.0, &b.0)
                    };
                    if orders.first().copied().unwrap_or(false) {
                        ord = ord.reverse();
                    }
                    return ord;
                }
                for (i, _) in sort_fields.iter().enumerate() {
                    let mut ord = compare(&a.1[i], &b.1[i]);
                    if orders.get(i).copied().unwrap_or(false) {
                        ord = ord.reverse();
                    }
                    if ord != std::cmp::Ordering::Equal {
                        return ord;
                    }
                }
                std::cmp::Ordering::Equal
            });
            for (i, (v, _)) in keyed.into_iter().enumerate() {
                values[i] = v;
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn resolve_domain(
    spec: Option<&Value>,
    store: &DataStore,
    signals: &HashMap<String, Value>,
) -> Result<Vec<Value>, Error> {
    let Some(spec) = spec else {
        return Ok(vec![]);
    };
    match spec {
        Value::Array(items) => {
            let mut out = Vec::new();
            for item in items {
                match item {
                    Value::Object(o) => {
                        if let Some(sig) = o.get("signal").and_then(|s| s.as_str()) {
                            if let Some(v) = signals.get(sig) {
                                match v {
                                    Value::Array(a) => out.extend(a.clone()),
                                    other => out.push(other.clone()),
                                }
                            } else if let Ok(v) =
                                expression::evaluate(sig, None, &SignalEnv(signals))
                            {
                                match v {
                                    Value::Array(a) => out.extend(a),
                                    other => out.push(other),
                                }
                            }
                        } else if o.contains_key("data") || o.contains_key("fields") {
                            out.extend(domain_from_object(o, store, signals)?);
                        } else if let Some(v) = o.get("value") {
                            out.push(v.clone());
                        } else {
                            out.push(item.clone());
                        }
                    }
                    other => out.push(other.clone()),
                }
            }
            Ok(out)
        }
        Value::Object(o) => domain_from_object(o, store, signals),
        Value::String(s) => {
            if let Some(v) = signals.get(s) {
                Ok(match v {
                    Value::Array(a) => a.clone(),
                    other => vec![other.clone()],
                })
            } else {
                Ok(vec![spec.clone()])
            }
        }
        Value::Number(_) | Value::Bool(_) | Value::Null => Ok(vec![spec.clone()]),
    }
}

struct SignalEnv<'a>(&'a HashMap<String, Value>);
impl<'a> Environment for SignalEnv<'a> {
    fn signal(&self, name: &str) -> Option<Value> {
        self.0.get(name).cloned()
    }
}

fn domain_from_object(
    o: &serde_json::Map<String, Value>,
    store: &DataStore,
    signals: &HashMap<String, Value>,
) -> Result<Vec<Value>, Error> {
    let mut out = Vec::new();
    if let Some(fields) = o.get("fields").and_then(|f| f.as_array()) {
        for f in fields {
            match f {
                Value::Object(fo) => {
                    if fo.contains_key("data") {
                        out.extend(domain_from_object(fo, store, signals)?);
                    } else if let Some(field) = fo.get("field").and_then(|v| v.as_str()) {
                        out.push(Value::String(field.to_string()));
                    } else if let Some(sig) = fo.get("signal").and_then(|v| v.as_str()) {
                        if let Some(v) = signals.get(sig) {
                            match v {
                                Value::Array(a) => out.extend(a.clone()),
                                other => out.push(other.clone()),
                            }
                        } else if let Ok(v) = expression::evaluate(sig, None, &SignalEnv(signals)) {
                            match v {
                                Value::Array(a) => out.extend(a),
                                other => out.push(other),
                            }
                        }
                    }
                }
                Value::String(_) => {
                    if let Some(data) = o.get("data").and_then(|d| d.as_str()) {
                        if let Some(rows) = store.get(data) {
                            for row in rows {
                                out.push(field_value(row, f));
                            }
                        }
                    }
                }
                // Literal values inside a multi-field domain, e.g.
                // `"fields": [[5, 6], {"data": ..., "field": ...}]`.
                Value::Array(a) => out.extend(a.clone()),
                _ => {}
            }
        }
    }
    if let Some(data) = o.get("data").and_then(|d| d.as_str()) {
        let rows: &[Value] = store.get(data).map(|v| v.as_slice()).unwrap_or(&[]);
        if let Some(field) = o.get("field") {
            for row in rows {
                out.push(field_value(row, field));
            }
        } else if o.get("fields").is_none() {
            // Domain of entire tuples: use the tuple itself.
            out.extend_from_slice(rows);
        }
    }
    if let Some(sig) = o.get("signal").and_then(|s| s.as_str()) {
        if let Some(v) = signals.get(sig) {
            match v {
                Value::Array(a) => out.extend(a.clone()),
                other => out.push(other.clone()),
            }
        } else if let Ok(v) = expression::evaluate(sig, None, &SignalEnv(signals)) {
            match v {
                Value::Array(a) => out.extend(a),
                other => out.push(other),
            }
        }
    }
    Ok(out)
}

/// JavaScript-compatible rounding (ties round toward positive infinity).
fn js_round(value: f64) -> f64 {
    if !value.is_finite() {
        return value;
    }
    (value + 0.5).floor()
}

const TICK_E10: f64 = 7.0710678118654755; // sqrt(50)
const TICK_E5: f64 = 3.1622776601683795; // sqrt(10)
const TICK_E2: f64 = 1.4142135623730951; // sqrt(2)

/// Port of d3-array's `tickSpec`: returns `(i1, i2, inc)`.
fn tick_spec(start: f64, stop: f64, count: f64) -> (f64, f64, f64) {
    let step = (stop - start) / count.max(0.0);
    if !step.is_finite() || step == 0.0 {
        return (0.0, -1.0, 0.0);
    }
    let power = step.log10().floor();
    let error = step / 10f64.powf(power);
    let factor = if error >= TICK_E10 {
        10.0
    } else if error >= TICK_E5 {
        5.0
    } else if error >= TICK_E2 {
        2.0
    } else {
        1.0
    };
    let (i1, i2, inc);
    if power < 0.0 {
        inc = 10f64.powf(-power) / factor;
        let mut a = js_round(start * inc);
        let mut b = js_round(stop * inc);
        if a / inc < start {
            a += 1.0;
        }
        if b / inc > stop {
            b -= 1.0;
        }
        i1 = a;
        i2 = b;
        return (i1, i2, -inc);
    }
    inc = 10f64.powf(power) * factor;
    let mut a = js_round(start / inc);
    let mut b = js_round(stop / inc);
    if a * inc < start {
        a += 1.0;
    }
    if b * inc > stop {
        b -= 1.0;
    }
    i1 = a;
    i2 = b;
    if i2 < i1 && count >= 0.5 && count < 2.0 {
        return tick_spec(start, stop, count * 2.0);
    }
    (i1, i2, inc)
}

fn tick_increment(start: f64, stop: f64, count: f64) -> f64 {
    tick_spec(start, stop, count).2
}

/// Port of d3-array's `tickStep` (positive magnitude, sign follows direction).
fn tick_step(start: f64, stop: f64, count: f64) -> f64 {
    let reverse = stop < start;
    let inc = if reverse {
        tick_increment(stop, start, count)
    } else {
        tick_increment(start, stop, count)
    };
    let magnitude = if inc < 0.0 { 1.0 / -inc } else { inc };
    if reverse {
        -magnitude
    } else {
        magnitude
    }
}

/// Port of d3-array's `ticks`.
fn d3_ticks(start: f64, stop: f64, count: f64) -> Vec<f64> {
    if !(count > 0.0) || !start.is_finite() || !stop.is_finite() {
        return vec![];
    }
    if start == stop {
        return vec![start];
    }
    let reverse = stop < start;
    let (i1, i2, inc) = if reverse {
        tick_spec(stop, start, count)
    } else {
        tick_spec(start, stop, count)
    };
    if !(i2 >= i1) {
        return vec![];
    }
    let n = (i2 - i1 + 1.0) as usize;
    if n > 100_000 {
        return vec![];
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let k = i as f64;
        let value = if inc < 0.0 {
            if reverse {
                (i2 - k) / -inc
            } else {
                (i1 + k) / -inc
            }
        } else if reverse {
            (i2 - k) * inc
        } else {
            (i1 + k) * inc
        };
        out.push(value);
    }
    out
}

/// d3-style nice extent for linear scales.
pub fn nice_extent(min: f64, max: f64, count: f64) -> (f64, f64) {
    if !min.is_finite() || !max.is_finite() || min == max {
        return (min, max);
    }
    let (mut a, mut b) = (min, max);
    let mut previous = f64::NAN;
    loop {
        let step = tick_increment(a, b, count);
        if step == previous || step == 0.0 || !step.is_finite() {
            return (a, b);
        } else if step > 0.0 {
            a = (a / step).floor() * step;
            b = (b / step).ceil() * step;
        } else if step < 0.0 {
            a = (a * step).ceil() / step;
            b = (b * step).floor() / step;
        }
        previous = step;
    }
}

fn linear_ticks(start: f64, stop: f64, count: f64) -> Vec<f64> {
    d3_ticks(start, stop, count)
}

/// d3 log-scale ticks (base 10).
fn log_ticks(start: f64, stop: f64, count: f64) -> Vec<Value> {
    if start <= 0.0 || stop <= 0.0 {
        return vec![];
    }
    let reverse = stop < start;
    let (u, v) = if reverse { (stop, start) } else { (start, stop) };
    let mut i = u.log10();
    let mut j = v.log10();
    let n = if count > 0.0 { count } else { 10.0 };
    let mut z: Vec<f64> = Vec::new();
    if j - i < n {
        i = i.floor();
        j = j.ceil();
        let mut p = i;
        while p <= j {
            for k in 1..10 {
                let t = if p < 0.0 {
                    k as f64 / 10f64.powf(-p)
                } else {
                    k as f64 * 10f64.powf(p)
                };
                if t < u {
                    continue;
                }
                if t > v {
                    break;
                }
                z.push(t);
            }
            p += 1.0;
        }
        if z.len() * 2 < n as usize {
            z = d3_ticks(u, v, n);
        }
    } else {
        let exponent_count = (j - i).min(n);
        z = d3_ticks(i, j, exponent_count)
            .into_iter()
            .map(|e| 10f64.powf(e))
            .collect();
    }
    let mut out: Vec<Value> = z.into_iter().map(json_number).collect();
    if reverse {
        out.reverse();
    }
    out
}

const SECOND_MS: f64 = 1_000.0;
const MINUTE_MS: f64 = 60_000.0;
const HOUR_MS: f64 = 3_600_000.0;
const DAY_MS: f64 = 86_400_000.0;
const WEEK_MS: f64 = 7.0 * DAY_MS;
const MONTH_MS: f64 = 30.0 * DAY_MS;
const YEAR_MS: f64 = 365.0 * DAY_MS;

#[derive(Clone, Copy, PartialEq, Eq)]
enum TimeInterval {
    Millisecond,
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
}

/// d3-time's tick interval table (UTC variant).
const TIME_INTERVALS: [(TimeInterval, f64, f64); 18] = [
    (TimeInterval::Second, 1.0, SECOND_MS),
    (TimeInterval::Second, 5.0, 5.0 * SECOND_MS),
    (TimeInterval::Second, 15.0, 15.0 * SECOND_MS),
    (TimeInterval::Second, 30.0, 30.0 * SECOND_MS),
    (TimeInterval::Minute, 1.0, MINUTE_MS),
    (TimeInterval::Minute, 5.0, 5.0 * MINUTE_MS),
    (TimeInterval::Minute, 15.0, 15.0 * MINUTE_MS),
    (TimeInterval::Minute, 30.0, 30.0 * MINUTE_MS),
    (TimeInterval::Hour, 1.0, HOUR_MS),
    (TimeInterval::Hour, 3.0, 3.0 * HOUR_MS),
    (TimeInterval::Hour, 6.0, 6.0 * HOUR_MS),
    (TimeInterval::Hour, 12.0, 12.0 * HOUR_MS),
    (TimeInterval::Day, 1.0, DAY_MS),
    (TimeInterval::Day, 2.0, 2.0 * DAY_MS),
    (TimeInterval::Week, 1.0, WEEK_MS),
    (TimeInterval::Month, 1.0, MONTH_MS),
    (TimeInterval::Month, 3.0, 3.0 * MONTH_MS),
    (TimeInterval::Year, 1.0, YEAR_MS),
];

fn floor_to(value: f64, unit: f64) -> f64 {
    (value / unit).floor() * unit
}

fn civil_month_start(ms: f64) -> f64 {
    let (y, m, ..) = ms_to_civil(ms);
    civil_to_ms(y, m, 1, 0, 0, 0, 0)
}

fn next_month(ms: f64) -> f64 {
    let (y, m, ..) = ms_to_civil(ms);
    let (ny, nm) = if m >= 12 { (y + 1, 1) } else { (y, m + 1) };
    civil_to_ms(ny, nm, 1, 0, 0, 0, 0)
}

fn year_start(ms: f64) -> f64 {
    let (y, ..) = ms_to_civil(ms);
    civil_to_ms(y, 1, 1, 0, 0, 0, 0)
}

fn next_year(ms: f64) -> f64 {
    let (y, ..) = ms_to_civil(ms);
    civil_to_ms(y + 1, 1, 1, 0, 0, 0, 0)
}

/// Inclusive-or-after boundary for `ms` on a repeated calendar interval.
fn ceil_interval(ms: f64, unit: f64) -> f64 {
    let floor = floor_to(ms, unit);
    if floor < ms {
        floor + unit
    } else {
        floor
    }
}

fn interval_ticks(kind: TimeInterval, step: f64, start: f64, stop: f64) -> Vec<f64> {
    let k = step.max(1.0).floor() as i64;
    let mut out = Vec::new();
    let mut v;
    match kind {
        TimeInterval::Millisecond => {
            let s = k as f64;
            v = ceil_interval(start, s);
            while v <= stop {
                out.push(v);
                v += s;
            }
        }
        TimeInterval::Second => {
            v = ceil_interval(start, SECOND_MS);
            let mut secs = (v / SECOND_MS).floor() as i64;
            while secs.rem_euclid(60) % k != 0 {
                secs += 1;
                v += SECOND_MS;
            }
            while v <= stop {
                out.push(v);
                secs += k;
                v = secs as f64 * SECOND_MS;
            }
        }
        TimeInterval::Minute => {
            v = ceil_interval(start, MINUTE_MS);
            let mut mins = (v / MINUTE_MS).floor() as i64;
            while mins.rem_euclid(60) % k != 0 {
                mins += 1;
                v += MINUTE_MS;
            }
            while v <= stop {
                out.push(v);
                mins += k;
                v = mins as f64 * MINUTE_MS;
            }
        }
        TimeInterval::Hour => {
            v = ceil_interval(start, HOUR_MS);
            let mut hours = (v / HOUR_MS).floor() as i64;
            while hours.rem_euclid(24) % k != 0 {
                hours += 1;
                v += HOUR_MS;
            }
            while v <= stop {
                out.push(v);
                hours += k;
                v = hours as f64 * HOUR_MS;
            }
        }
        TimeInterval::Day => {
            v = ceil_interval(start, DAY_MS);
            let mut days = (v / DAY_MS).floor() as i64;
            while days.rem_euclid(k) != 0 {
                days += 1;
                v += DAY_MS;
            }
            while v <= stop {
                out.push(v);
                days += k;
                v = days as f64 * DAY_MS;
            }
        }
        TimeInterval::Week => {
            let day = floor_to(start, DAY_MS);
            let weekday = (((day / DAY_MS) as i64) + 4).rem_euclid(7);
            v = day - weekday as f64 * DAY_MS;
            if v < start {
                v += WEEK_MS;
            }
            let mut weeks = (v / WEEK_MS).floor() as i64;
            while weeks.rem_euclid(k) != 0 {
                weeks += 1;
                v += WEEK_MS;
            }
            while v <= stop {
                out.push(v);
                weeks += k;
                v = weeks as f64 * WEEK_MS;
            }
        }
        TimeInterval::Month => {
            v = civil_month_start(start);
            if v < start {
                v = next_month(v);
            }
            let mut index = {
                let (y, m, ..) = ms_to_civil(v);
                y * 12 + (m - 1)
            };
            while (index.rem_euclid(12)) % k != 0 {
                index += 1;
                v = next_month(v);
            }
            while v <= stop {
                out.push(v);
                for _ in 0..k {
                    v = next_month(v);
                }
            }
        }
        TimeInterval::Year => {
            v = year_start(start);
            if v < start {
                v = next_year(v);
            }
            let mut year = ms_to_civil(v).0;
            while year.rem_euclid(k) != 0 {
                year += 1;
                v = next_year(v);
            }
            while v <= stop {
                out.push(v);
                for _ in 0..k {
                    v = next_year(v);
                }
            }
        }
    }
    out
}

/// Local-time calendar boundaries, matching d3-time's local intervals
/// (`timeMonth`, `timeYear`, ...). Wall-clock arithmetic is resolved back to
/// epoch instants so DST transitions are respected.
fn local_month_start(ms: f64) -> f64 {
    let (y, m, ..) = ms_to_civil(ms + crate::data::local_offset_minutes(ms) * 60_000.0);
    crate::data::local_civil_to_ms(y, m, 1, 0, 0, 0, 0)
}

fn local_next_month(ms: f64) -> f64 {
    let (y, m, ..) = ms_to_civil(ms + crate::data::local_offset_minutes(ms) * 60_000.0);
    let (ny, nm) = if m >= 12 { (y + 1, 1) } else { (y, m + 1) };
    crate::data::local_civil_to_ms(ny, nm, 1, 0, 0, 0, 0)
}

fn local_year_start(ms: f64) -> f64 {
    let (y, ..) = ms_to_civil(ms + crate::data::local_offset_minutes(ms) * 60_000.0);
    crate::data::local_civil_to_ms(y, 1, 1, 0, 0, 0, 0)
}

fn local_next_year(ms: f64) -> f64 {
    let (y, ..) = ms_to_civil(ms + crate::data::local_offset_minutes(ms) * 60_000.0);
    crate::data::local_civil_to_ms(y + 1, 1, 1, 0, 0, 0, 0)
}

/// Local wall-clock milliseconds for an epoch instant.
fn local_wall(ms: f64) -> f64 {
    ms + crate::data::local_offset_minutes(ms) * 60_000.0
}

/// Convert local wall-clock milliseconds back to an epoch instant.
fn wall_to_epoch(wall: f64) -> f64 {
    let (y, m, d, h, mi, s, ms) = ms_to_civil(wall);
    crate::data::local_civil_to_ms(y, m, d, h, mi, s, ms)
}

/// Local (d3-time) tick generation for sub-day and day/week intervals.
fn local_calendar_ticks(kind: TimeInterval, k: i64, start: f64, stop: f64) -> Vec<f64> {
    let wall_start = local_wall(start);
    let wall_stop = local_wall(stop);
    let mut out = Vec::new();
    let push = |out: &mut Vec<f64>, wall: f64| out.push(wall_to_epoch(wall));
    match kind {
        TimeInterval::Millisecond => {
            let s = k as f64;
            let mut v = ceil_interval(wall_start, s);
            while v <= wall_stop {
                push(&mut out, v);
                v += s;
            }
        }
        TimeInterval::Second => {
            let mut v = floor_to(wall_start, SECOND_MS);
            let mut secs = (v / SECOND_MS).floor() as i64;
            while secs.rem_euclid(60) % k != 0 {
                secs += 1;
                v += SECOND_MS;
            }
            while v <= wall_stop {
                push(&mut out, v);
                secs += k;
                v = secs as f64 * SECOND_MS;
            }
        }
        TimeInterval::Minute => {
            let mut v = floor_to(wall_start, MINUTE_MS);
            let mut mins = (v / MINUTE_MS).floor() as i64;
            while mins.rem_euclid(60) % k != 0 {
                mins += 1;
                v += MINUTE_MS;
            }
            while v <= wall_stop {
                push(&mut out, v);
                mins += k;
                v = mins as f64 * MINUTE_MS;
            }
        }
        TimeInterval::Hour => {
            let mut v = floor_to(wall_start, HOUR_MS);
            let mut hours = (v / HOUR_MS).floor() as i64;
            while hours.rem_euclid(24) % k != 0 {
                hours += 1;
                v += HOUR_MS;
            }
            while v <= wall_stop {
                push(&mut out, v);
                hours += k;
                v = hours as f64 * HOUR_MS;
            }
        }
        TimeInterval::Day => {
            let mut v = floor_to(wall_start, DAY_MS);
            let (mut y, mut m, mut d, ..) = ms_to_civil(v);
            while (d - 1).rem_euclid(k) != 0 {
                let (ny, nm, nd, ..) = next_civil_day(y, m, d);
                (y, m, d) = (ny, nm, nd);
                v = civil_to_ms(y, m, d, 0, 0, 0, 0);
            }
            while v <= wall_stop {
                push(&mut out, v);
                for _ in 0..k {
                    let (ny, nm, nd, ..) = next_civil_day(y, m, d);
                    (y, m, d) = (ny, nm, nd);
                }
                v = civil_to_ms(y, m, d, 0, 0, 0, 0);
            }
        }
        TimeInterval::Week => {
            let day = floor_to(wall_start, DAY_MS);
            let weekday = (((day / DAY_MS) as i64) + 4).rem_euclid(7);
            let mut v = day - weekday as f64 * DAY_MS;
            if v < wall_start {
                v += WEEK_MS;
            }
            let mut weeks = (v / WEEK_MS).floor() as i64;
            while weeks.rem_euclid(k) != 0 {
                weeks += 1;
                v += WEEK_MS;
            }
            while v <= wall_stop {
                push(&mut out, v);
                weeks += k;
                v = weeks as f64 * WEEK_MS;
            }
        }
        _ => return interval_ticks(kind, step_to_f64(k), start, stop),
    }
    out
}

fn step_to_f64(k: i64) -> f64 {
    k as f64
}

/// Returns the next civil day `(y, m, d)`.
fn next_civil_day(y: i64, m: i64, d: i64) -> (i64, i64, i64) {
    let next = civil_to_ms(y, m, d + 1, 0, 0, 0, 0);
    let (ny, nm, nd, ..) = ms_to_civil(next);
    (ny, nm, nd)
}

fn interval_ticks_local(kind: TimeInterval, step: f64, start: f64, stop: f64) -> Vec<f64> {
    let k = step.max(1.0).floor() as i64;
    let mut out = Vec::new();
    match kind {
        TimeInterval::Month => {
            let mut v = local_month_start(start);
            if v < start {
                v = local_next_month(v);
            }
            let mut index = {
                let (y, m, ..) =
                    ms_to_civil(v + crate::data::local_offset_minutes(v) * 60_000.0);
                y * 12 + (m - 1)
            };
            while index.rem_euclid(12) % k != 0 {
                index += 1;
                v = local_next_month(v);
            }
            while v <= stop {
                out.push(v);
                for _ in 0..k {
                    v = local_next_month(v);
                }
            }
        }
        TimeInterval::Year => {
            let mut v = local_year_start(start);
            if v < start {
                v = local_next_year(v);
            }
            let mut year =
                ms_to_civil(v + crate::data::local_offset_minutes(v) * 60_000.0).0;
            while year.rem_euclid(k) != 0 {
                year += 1;
                v = local_next_year(v);
            }
            while v <= stop {
                out.push(v);
                for _ in 0..k {
                    v = local_next_year(v);
                }
            }
        }
        _ => return local_calendar_ticks(kind, k, start, stop),
    }
    out
}

/// d3-time local tick generation used by `time` scales.
pub fn time_ticks_local(start: f64, stop: f64, count: f64) -> Vec<f64> {
    time_ticks_mode(start, stop, count, true)
}

/// d3-time / vega-time UTC tick generation.
pub fn time_ticks(start: f64, stop: f64, count: f64) -> Vec<f64> {
    time_ticks_mode(start, stop, count, false)
}

fn time_ticks_mode(start: f64, stop: f64, count: f64, local: bool) -> Vec<f64> {
    let reverse = stop < start;
    let (a, b) = if reverse { (stop, start) } else { (start, stop) };
    if !a.is_finite() || !b.is_finite() || a == b {
        return vec![a];
    }
    let count = if count > 0.0 { count } else { 10.0 };
    let target = (b - a) / count;
    let i = TIME_INTERVALS.partition_point(|(_, _, duration)| *duration <= target);
    let ticks = |kind, step| {
        if local {
            interval_ticks_local(kind, step, a, b)
        } else {
            interval_ticks(kind, step, a, b)
        }
    };
    let mut out;
    if i == TIME_INTERVALS.len() {
        let step = tick_step(a / YEAR_MS, b / YEAR_MS, count).max(1.0).floor();
        out = ticks(TimeInterval::Year, step);
    } else if i == 0 {
        if b - a >= SECOND_MS {
            // Sub-second intervals are rarely meaningful for axes spanning
            // whole seconds; prefer second boundaries.
            out = ticks(TimeInterval::Second, 1.0);
        } else {
            let step = tick_step(a, b, count).max(1.0).floor();
            out = ticks(TimeInterval::Millisecond, step);
        }
    } else {
        let (prev_kind, prev_step, prev_dur) = TIME_INTERVALS[i - 1];
        let (kind, step, dur) = TIME_INTERVALS[i];
        let (kind, step) = if target / prev_dur < dur / target {
            (prev_kind, prev_step)
        } else {
            (kind, step)
        };
        out = ticks(kind, step);
    }
    if reverse {
        out.reverse();
    }
    out
}

/// Convert a scale's range to a `[min,max]` pair, used by axes and legends.
pub fn range_pair(scale: &Scale) -> (f64, f64) {
    match scale.kind {
        ScaleKind::Band => (scale.r0, scale.r1),
        ScaleKind::Point => (scale.r0, scale.r1),
        _ => (scale.r0, scale.r1),
    }
}

/// Parse a time value (number, ISO string) the way Vega's time scales do.
pub fn time_value(v: &Value) -> f64 {
    match v {
        Value::String(s) => parse_date(s).unwrap_or_else(|| to_number(v)),
        _ => to_number(v),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expression::EmptyEnv;
    use serde_json::json;

    #[test]
    fn linear_ticks_match_d3() {
        assert_eq!(linear_ticks(0.0, 100.0, 10.0).len(), 11);
        assert_eq!(
            linear_ticks(10.0, -10.0, 5.0),
            vec![10.0, 5.0, 0.0, -5.0, -10.0]
        );
        assert_eq!(
            linear_ticks(0.13, 0.96, 10.0),
            vec![0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9]
        );
        assert_eq!(linear_ticks(0.0, 1.0, 3.0), vec![0.0, 0.5, 1.0]);
    }

    #[test]
    fn nice_extent_matches_d3() {
        assert_eq!(nice_extent(0.13, 0.96, 10.0), (0.1, 1.0));
        assert_eq!(nice_extent(0.0001, 0.001, 5.0), (0.0, 0.001));
        assert_eq!(nice_extent(0.0, 100.0, 10.0), (0.0, 100.0));
    }

    #[test]
    fn time_ticks_hours_and_years() {
        let a = civil_to_ms(2012, 1, 1, 0, 0, 0, 0);
        let b = civil_to_ms(2012, 1, 2, 0, 0, 0, 0);
        let ticks = time_ticks(a, b, 10.0);
        assert_eq!(ticks.len(), 9);
        assert_eq!(ticks[0], a);
        assert_eq!(ticks[ticks.len() - 1], b);
        assert_eq!(ticks[1] - ticks[0], 3.0 * 3_600_000.0);

        let start = civil_to_ms(2000, 1, 1, 0, 0, 0, 0);
        let stop = civil_to_ms(2010, 12, 1, 0, 0, 0, 0);
        let years = time_ticks(start, stop, 10.0);
        assert_eq!(years.len(), 11);
        assert_eq!(years[0], start);
        assert_eq!(years[10], civil_to_ms(2010, 1, 1, 0, 0, 0, 0));
    }

    #[test]
    fn linear_ticks_basic() {
        assert_eq!(
            linear_ticks(0.0, 100.0, 10.0),
            vec![0.0, 10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0]
        );
    }

    #[test]
    fn sqrt_scale_uses_radial_interpolation() {
        let spec = json!({
            "type": "sqrt",
            "domain": [0, 52],
            "range": [20, 150],
            "zero": true
        });
        let store = DataStore::default();
        let scale = build_scale("r", &spec, &store, &HashMap::new(), &EmptyEnv).unwrap();
        // d3 scaleSqrt: 20 + 130 * sqrt(12 / 52)
        let v = to_number(&scale.scale(&json!(12)));
        assert!((v - 82.44998).abs() < 1e-4, "{v}");
        assert_eq!(scale.scale(&json!(0)), json!(20));
        assert_eq!(scale.scale(&json!(52)), json!(150));
    }

    #[test]
    fn band_positions() {
        let spec = json!({
            "type": "band",
            "domain": ["a", "b", "c"],
            "range": [0, 30],
            "paddingInner": 0.0,
            "paddingOuter": 0.0
        });
        let store = DataStore::default();
        let scale = build_scale("x", &spec, &store, &HashMap::new(), &EmptyEnv).unwrap();
        assert_eq!(scale.bandwidth, 10.0);
        assert_eq!(scale.scale(&json!("b")), json!(10));
    }

    #[test]
    fn ordinal_colors() {
        let spec = json!({"type": "ordinal", "domain": ["a", "b"], "range": "category"});
        let store = DataStore::default();
        let scale = build_scale("c", &spec, &store, &HashMap::new(), &EmptyEnv).unwrap();
        assert_eq!(scale.colors.len(), 10);
    }

    #[test]
    fn null_domain_value_labels_as_null() {
        let spec = json!({
            "type": "point",
            "domain": [null, "a", "b"],
            "range": [0, 60]
        });
        let store = DataStore::default();
        let scale = build_scale("x", &spec, &store, &HashMap::new(), &EmptyEnv).unwrap();
        assert_eq!(scale.label(&Value::Null, None, None), "null");
        assert_eq!(
            scale.ticks(10.0),
            vec![Value::Null, json!("a"), json!("b")]
        );
    }

    #[test]
    fn time_ticks_months() {
        let a = civil_to_ms(2020, 1, 1, 0, 0, 0, 0);
        let b = civil_to_ms(2020, 12, 31, 0, 0, 0, 0);
        let ticks = time_ticks(a, b, 12.0);
        assert!(ticks.len() >= 10);
    }
}

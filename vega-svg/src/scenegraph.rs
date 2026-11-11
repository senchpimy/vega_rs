//! Minimal scenegraph model and path generation helpers (port of the parts of
//! [vega-scenegraph] needed for SVG output).
//!
//! [vega-scenegraph]: https://github.com/vega/vega/tree/main/packages/vega-scenegraph

use indexmap::IndexMap;
use serde_json::Value;

use crate::value::to_number;

#[derive(Clone, Debug, Default)]
pub struct Item {
    /// Mark type: group, rect, rule, symbol, text, line, area, trail, arc, path, image.
    pub kind: String,
    /// Optional guide role (`axis`, `legend`, `title`, ...).
    pub role: Option<String>,
    pub class: Vec<String>,
    pub props: IndexMap<String, Value>,
    /// Child marks (for group marks) or vertices (for line/area marks).
    pub items: Vec<Item>,
}

impl Item {
    pub fn new(kind: &str) -> Self {
        Self {
            kind: kind.to_string(),
            ..Default::default()
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.props.get(key)
    }

    pub fn f64(&self, key: &str) -> Option<f64> {
        self.props.get(key).and_then(|v| match v {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => s.parse().ok(),
            _ => None,
        })
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        self.props.get(key).and_then(|v| v.as_str())
    }

    pub fn bool(&self, key: &str) -> Option<bool> {
        self.props.get(key).and_then(|v| v.as_bool())
    }

    pub fn set(&mut self, key: &str, value: Value) {
        self.props.insert(key.to_string(), value);
    }
}

pub fn text_width(text: &str, font_size: f64) -> f64 {
    // Mirrors vega-scenegraph's fallback estimator when no canvas is available:
    //   ~~(0.8 * text.length * fontSize)
    (0.8 * text.chars().count() as f64 * font_size).floor()
}

pub fn text_height(font_size: f64) -> f64 {
    font_size * 1.15
}

pub fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return "0".to_string();
    }
    let s = format!("{:.6}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" {
        "0".to_string()
    } else {
        s
    }
}

fn f(item: &Item, key: &str, default: f64) -> f64 {
    item.f64(key).unwrap_or(default)
}

/// Rectangle path like Vega's `rectangle()` shape helper.
///
/// Mirrors `vega-scenegraph/src/path/rectangle.js`: independent corners,
/// clamped to half the shorter side, and cubic Bézier corners. The fallback
/// for non-rounded rectangles uses the same `M…h…v…h…Z` form d3 emits.
pub fn rect_path(item: &Item, offset: f64) -> String {
    /// Bezier circle constant from `rectangle.js` (`C = 1 - c`).
    const C: f64 = 0.448084975506;

    let x = f(item, "x", 0.0) + offset;
    let y = f(item, "y", 0.0) + offset;
    let w = f(item, "width", 0.0);
    let h = f(item, "height", 0.0);

    // `value(item.cornerRadiusTopLeft, item.cornerRadius) || 0`, as upstream.
    let radius = |key: &str| {
        item.f64(key)
            .unwrap_or_else(|| f(item, "cornerRadius", 0.0))
    };
    let clamp = |v: f64| {
        let max = (w.min(h) / 2.0).max(0.0);
        v.max(0.0).min(max)
    };
    let tl = clamp(radius("cornerRadiusTopLeft"));
    let tr = clamp(radius("cornerRadiusTopRight"));
    let br = clamp(radius("cornerRadiusBottomRight"));
    let bl = clamp(radius("cornerRadiusBottomLeft"));

    if tl <= 0.0 && tr <= 0.0 && br <= 0.0 && bl <= 0.0 {
        return format!(
            "M{},{}h{}v{}h{}Z",
            fmt_num(x),
            fmt_num(y),
            fmt_num(w),
            fmt_num(h),
            fmt_num(-w)
        );
    }

    let x2 = x + w;
    let y2 = y + h;
    format!(
        "M{},{}L{},{}C{},{},{},{},{},{}L{},{}C{},{},{},{},{},{}L{},{}C{},{},{},{},{},{}L{},{}C{},{},{},{},{},{}Z",
        fmt_num(x + tl),
        fmt_num(y),
        fmt_num(x2 - tr),
        fmt_num(y),
        fmt_num(x2 - C * tr),
        fmt_num(y),
        fmt_num(x2),
        fmt_num(y + C * tr),
        fmt_num(x2),
        fmt_num(y + tr),
        fmt_num(x2),
        fmt_num(y2 - br),
        fmt_num(x2),
        fmt_num(y2 - C * br),
        fmt_num(x2 - C * br),
        fmt_num(y2),
        fmt_num(x2 - br),
        fmt_num(y2),
        fmt_num(x + bl),
        fmt_num(y2),
        fmt_num(x + C * bl),
        fmt_num(y2),
        fmt_num(x),
        fmt_num(y2 - C * bl),
        fmt_num(x),
        fmt_num(y2 - bl),
        fmt_num(x),
        fmt_num(y + tl),
        fmt_num(x),
        fmt_num(y + C * tl),
        fmt_num(x + C * tl),
        fmt_num(y),
        fmt_num(x + tl),
        fmt_num(y),
    )
}

/// True when `shape` is a custom SVG path string rather than a built-in symbol.
pub fn is_custom_symbol(shape: &str) -> bool {
    !matches!(
        shape,
        "circle"
            | "square"
            | "cross"
            | "diamond"
            | "arrow"
            | "wedge"
            | "triangle"
            | "triangle-up"
            | "triangle-down"
            | "triangle-right"
            | "triangle-left"
            | "stroke"
    )
}

/// Scale applied by `vega-scenegraph` to custom symbol path strings:
/// `Math.sqrt(size) / 2`.
pub fn custom_symbol_scale(size: f64) -> f64 {
    size.max(0.0).sqrt() / 2.0
}

/// Scale an SVG path's coordinates by `k`.
///
/// Vega bakes this scale into the custom-symbol coordinates. Using an SVG
/// `scale()` transform instead would also scale the stroke width (turning a
/// 2px outline into e.g. a 15px blob for large symbols), so the scale has to
/// be applied to the path data itself.
pub fn scale_path(d: &str, k: f64) -> String {
    #[derive(Clone, Copy, PartialEq)]
    enum Cmd {
        M,
        L,
        H,
        V,
        C,
        S,
        Q,
        T,
        A,
        Z,
    }

    fn command(c: char) -> Option<(Cmd, bool)> {
        let lower = c.to_ascii_lowercase();
        let cmd = match lower {
            'm' => Cmd::M,
            'l' => Cmd::L,
            'h' => Cmd::H,
            'v' => Cmd::V,
            'c' => Cmd::C,
            's' => Cmd::S,
            'q' => Cmd::Q,
            't' => Cmd::T,
            'a' => Cmd::A,
            'z' => Cmd::Z,
            _ => return None,
        };
        Some((cmd, c.is_ascii_uppercase()))
    }

    fn arity(cmd: Cmd) -> usize {
        match cmd {
            Cmd::M | Cmd::L | Cmd::T => 2,
            Cmd::H | Cmd::V => 1,
            Cmd::C => 6,
            Cmd::S | Cmd::Q => 4,
            Cmd::A => 7,
            Cmd::Z => 0,
        }
    }

    let mut out = String::with_capacity(d.len() + 16);
    let mut nums: Vec<f64> = Vec::new();
    let mut current: Option<(Cmd, bool)> = None;

    let flush = |cmd: Option<(Cmd, bool)>, nums: &mut Vec<f64>, out: &mut String| {
        let Some((cmd, absolute)) = cmd else {
            nums.clear();
            return;
        };
        let n = arity(cmd);
        if n == 0 {
            nums.clear();
            return;
        }
        let letter = match cmd {
            Cmd::M => 'M',
            Cmd::L => 'L',
            Cmd::H => 'H',
            Cmd::V => 'V',
            Cmd::C => 'C',
            Cmd::S => 'S',
            Cmd::Q => 'Q',
            Cmd::T => 'T',
            Cmd::A => 'A',
            Cmd::Z => 'Z',
        };
        let letter = if absolute {
            letter
        } else {
            letter.to_ascii_lowercase()
        };
        for (chunk_idx, chunk) in nums.chunks(n).enumerate() {
            if chunk.len() < n {
                break;
            }
            // Additional pairs after `M` are implicit `L` commands.
            if cmd == Cmd::M && chunk_idx > 0 {
                out.push(if absolute { 'L' } else { 'l' });
            } else {
                out.push(letter);
            }
            for (i, v) in chunk.iter().enumerate() {
                let scaled = if cmd == Cmd::A {
                    match i {
                        // rx, ry and the endpoint x/y scale; the x-axis
                        // rotation and the two arc flags stay untouched.
                        0 | 1 | 5 | 6 => v * k,
                        2 => *v,
                        _ => *v,
                    }
                } else {
                    v * k
                };
                out.push(' ');
                out.push_str(&fmt_num(scaled));
            }
        }
        nums.clear();
    };

    let mut chars = d.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_alphabetic() {
            flush(current, &mut nums, &mut out);
            if let Some(cmd) = command(c) {
                if matches!(cmd.0, Cmd::Z) {
                    out.push(if cmd.1 { 'Z' } else { 'z' });
                    current = None;
                } else {
                    current = Some(cmd);
                }
            } else {
                current = None;
            }
            chars.next();
            continue;
        }
        if c == '-' || c == '+' || c == '.' || c.is_ascii_digit() {
            let mut token = String::new();
            token.push(chars.next().unwrap());
            while let Some(&next) = chars.peek() {
                if next.is_ascii_digit()
                    || next == '.'
                    || next == 'e'
                    || next == 'E'
                    || ((next == '-' || next == '+')
                        && token
                            .chars()
                            .last()
                            .map(|l| l == 'e' || l == 'E')
                            .unwrap_or(false))
                {
                    token.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            if let Ok(v) = token.parse::<f64>() {
                nums.push(v);
            }
            continue;
        }
        // separators (whitespace, commas) and anything else
        chars.next();
    }
    flush(current, &mut nums, &mut out);
    out
}

/// Symbol geometry, ported from `vega-scenegraph/src/path/symbols.js`.
///
/// `size` is an area (square pixels) as in Vega: built-in shapes size
/// themselves from `r = sqrt(size) / 2`. Coordinates are emitted relative to
/// `(cx, cy)` so the SVG renderer can either translate the item or keep the
/// caller-supplied center.
pub fn symbol_path(cx: f64, cy: f64, size: f64, shape: &str) -> String {
    let r = size.max(0.0).sqrt() / 2.0;
    let h = 0.8660254037844386 * r; // HalfSqrt3
    let tan30 = 0.5773502691896257;
    let p = |x: f64, y: f64| format!("{},{}", fmt_num(cx + x), fmt_num(cy + y));
    match shape {
        "square" => {
            let w = size.max(0.0).sqrt();
            let x = -w / 2.0;
            format!(
                "M{},{}h{}v{}h{}Z",
                fmt_num(cx + x),
                fmt_num(cy + x),
                fmt_num(w),
                fmt_num(w),
                fmt_num(-w)
            )
        }
        "cross" => {
            let s = r / 2.5;
            format!(
                "M{}L{}L{}L{}L{}L{}L{}L{}L{}L{}L{}L{}Z",
                p(-r, -s),
                p(-r, s),
                p(-s, s),
                p(-s, r),
                p(s, r),
                p(s, s),
                p(r, s),
                p(r, -s),
                p(s, -s),
                p(s, -r),
                p(-s, -r),
                p(-s, -s)
            )
        }
        "diamond" => format!("M{}L{}L{}L{}Z", p(-r, 0.0), p(0.0, -r), p(r, 0.0), p(0.0, r)),
        "triangle" => {
            let o = h - r * tan30;
            format!(
                "M{}L{}L{}Z",
                p(0.0, -h - o),
                p(-r, h - o),
                p(r, h - o)
            )
        }
        "triangle-up" => format!("M{}L{}L{}Z", p(0.0, -h), p(-r, h), p(r, h)),
        "triangle-down" => format!("M{}L{}L{}Z", p(0.0, h), p(-r, -h), p(r, -h)),
        "triangle-right" => format!("M{}L{}L{}Z", p(h, 0.0), p(-h, -r), p(-h, r)),
        "triangle-left" => format!("M{}L{}L{}Z", p(-h, 0.0), p(h, -r), p(h, r)),
        "wedge" => {
            let o = h - r * tan30;
            let b = r / 4.0;
            format!(
                "M{}L{}L{}Z",
                p(0.0, -h - o),
                p(-b, h - o),
                p(b, h - o)
            )
        }
        "arrow" => {
            let s = r / 7.0;
            let t = r / 2.5;
            let v = r / 8.0;
            format!(
                "M{}L{}L{}L{}L{}L{}L{}Z",
                p(-s, r),
                p(s, r),
                p(s, -v),
                p(t, -v),
                p(0.0, -r),
                p(-t, -v),
                p(-s, -v)
            )
        }
        "stroke" => format!("M{}L{}", p(-r, 0.0), p(r, 0.0)),
        shape if is_custom_symbol(shape) => shape.to_string(),
        // `circle` and any unknown built-in fallback. d3's symbolCircle does
        // not close the path, so neither do we.
        _ => format!(
            "M{},{}A{},{} 0 1 1 {},{}A{},{} 0 1 1 {},{}",
            fmt_num(cx + r),
            fmt_num(cy),
            fmt_num(r),
            fmt_num(r),
            fmt_num(cx - r),
            fmt_num(cy),
            fmt_num(r),
            fmt_num(r),
            fmt_num(cx + r),
            fmt_num(cy)
        ),
    }
}

/// Arc path, angles measured clockwise from 12 o'clock.
///
/// Geometry is emitted at the origin; the SVG renderer positions each arc with
/// `transform="translate(x,y) rotate(angle)"`, matching upstream
/// `marks/arc.js` / `markItemPath.js`.
pub fn arc_path(item: &Item) -> String {
    let cx = 0.0;
    let cy = 0.0;
    let a0 = f(item, "startAngle", 0.0);
    let a1 = f(item, "endAngle", 0.0);
    let mut ir = f(item, "innerRadius", 0.0).max(0.0);
    let mut or = f(item, "outerRadius", 0.0).max(0.0);
    // d3-shape ensures the outer radius is the larger of the two.
    if or < ir {
        std::mem::swap(&mut or, &mut ir);
    }
    if (a1 - a0).abs() < 1e-9 || or <= 0.0 {
        return String::new();
    }
    let pt = |angle: f64, r: f64| -> (f64, f64) { (cx + r * angle.sin(), cy - r * angle.cos()) };
    let xy = |(x, y): (f64, f64)| format!("{},{}", fmt_num(x), fmt_num(y));
    let full = (a1 - a0).abs() >= std::f64::consts::TAU - 1e-6;
    let large = if (a1 - a0).abs() > std::f64::consts::PI {
        1
    } else {
        0
    };
    // d3-shape emits `context.arc(..., !cw)`, and d3-path flips that back:
    // the SVG sweep flag equals `cw`, i.e. 1 when the end angle is greater
    // than the start angle and 0 otherwise. The inner ring uses the
    // opposite winding.
    let sweep = if a1 > a0 { 1 } else { 0 };
    let inner_sweep = 1 - sweep;
    let (x0, y0) = pt(a0, or);
    let (x1, y1) = pt(a1, or);
    if full {
        let top = format!("{},{}", fmt_num(cx), fmt_num(cy - or));
        let bottom = format!("{},{}", fmt_num(cx), fmt_num(cy + or));
        let mut d = format!(
            "M{}A{},{},0,1,{},{}A{},{},0,1,{},{}Z",
            top,
            fmt_num(or),
            fmt_num(or),
            sweep,
            bottom,
            fmt_num(or),
            fmt_num(or),
            sweep,
            top
        );
        if ir > 0.0 {
            let itop = format!("{},{}", fmt_num(cx), fmt_num(cy - ir));
            let ibottom = format!("{},{}", fmt_num(cx), fmt_num(cy + ir));
            d.push_str(&format!(
                "M{}A{},{},0,1,{},{}A{},{},0,1,{},{}Z",
                itop,
                fmt_num(ir),
                fmt_num(ir),
                inner_sweep,
                ibottom,
                fmt_num(ir),
                fmt_num(ir),
                inner_sweep,
                itop
            ));
        }
        return d;
    }
    let center = format!("{},{}", fmt_num(cx), fmt_num(cy));
    if ir <= 0.0 {
        // d3 arc: start on the outer edge, sweep to the end angle, then close
        // through the center.
        return format!(
            "M{}A{},{},0,{},{},{}L{}Z",
            xy((x0, y0)),
            fmt_num(or),
            fmt_num(or),
            large,
            sweep,
            xy((x1, y1)),
            center
        );
    }
    let (x2, y2) = pt(a1, ir);
    let (x3, y3) = pt(a0, ir);
    format!(
        "M{}A{},{},0,{},{},{}L{}A{},{},0,{},{},{}Z",
        xy((x0, y0)),
        fmt_num(or),
        fmt_num(or),
        large,
        sweep,
        xy((x1, y1)),
        xy((x2, y2)),
        fmt_num(ir),
        fmt_num(ir),
        large,
        inner_sweep,
        xy((x3, y3))
    )
}

pub fn line_path(items: &[Item], interpolate: &str) -> String {
    let points: Vec<(f64, f64, bool)> = items
        .iter()
        .map(|it| {
            (
                it.f64("x").unwrap_or(0.0),
                it.f64("y").unwrap_or(0.0),
                it.get("defined")
                    .map(|v| v.as_bool().unwrap_or(true))
                    .unwrap_or(true),
            )
        })
        .collect();
    crate::interpolate::line_path(&points, interpolate)
}

/// Port of `vega-scenegraph/src/path/trail.js`: each segment becomes a
/// closed capsule outline whose width is the item's `size`.
pub fn trail_path(items: &[Item]) -> String {
    use std::f64::consts::{PI, TAU};
    let mut out = String::new();
    let mut ready = false;
    let (mut x1, mut y1, mut r1) = (0.0f64, 0.0f64, 0.0f64);
    let mut defined0 = false;

    let arc = |out: &mut String, cx: f64, cy: f64, r: f64, a0: f64, a1: f64| {
        let x = cx + r * a1.cos();
        let y = cy + r * a1.sin();
        let large = if (a1 - a0).abs() >= PI { 1 } else { 0 };
        let sweep = if a1 > a0 { 1 } else { 0 };
        out.push_str(&format!(
            "A {} {} 0 {} {} {} {}",
            fmt_num(r),
            fmt_num(r),
            large,
            sweep,
            fmt_num(x),
            fmt_num(y)
        ));
    };

    let n = items.len();
    for i in 0..=n {
        let defined = i < n
            && items[i]
                .get("defined")
                .map(|v| v.as_bool().unwrap_or(true))
                .unwrap_or(true);
        if defined != defined0 {
            defined0 = defined;
            if defined0 {
                ready = false;
            }
        }
        if !defined {
            continue;
        }
        let it = &items[i];
        let x2 = it.f64("x").unwrap_or(0.0);
        let y2 = it.f64("y").unwrap_or(0.0);
        let size = it.f64("size").unwrap_or(2.0);
        let r2 = size / 2.0;
        if ready {
            let ux0 = y1 - y2;
            let uy0 = x2 - x1;
            if ux0 != 0.0 || uy0 != 0.0 {
                let ud = ux0.hypot(uy0);
                let ux = ux0 / ud;
                let uy = uy0 / ud;
                let rx = ux * r1;
                let ry = uy * r1;
                let t = uy.atan2(ux);
                out.push_str(&format!(
                    "M {} {} L {} {} ",
                    fmt_num(x1 - rx),
                    fmt_num(y1 - ry),
                    fmt_num(x2 - ux * r2),
                    fmt_num(y2 - uy * r2)
                ));
                arc(&mut out, x2, y2, r2, t - PI, t);
                out.push_str(&format!(
                    " L {} {} ",
                    fmt_num(x1 + rx),
                    fmt_num(y1 + ry)
                ));
                arc(&mut out, x1, y1, r1, t, t + PI);
            } else {
                out.push_str(&format!("M {} {} ", fmt_num(x2), fmt_num(y2 - r2)));
                arc(&mut out, x2, y2, r2, 0.0, TAU);
            }
            out.push_str("Z ");
        } else {
            ready = true;
        }
        x1 = x2;
        y1 = y2;
        r1 = r2;
    }
    out.trim_end().to_string()
}

pub fn area_path(items: &[Item], interpolate: &str, orient: &str) -> String {
    let vertical = orient != "horizontal";
    let upper: Vec<(f64, f64, bool)> = items
        .iter()
        .map(|it| {
            (
                it.f64("x").unwrap_or(0.0),
                it.f64("y").unwrap_or(0.0),
                it.get("defined")
                    .map(|v| v.as_bool().unwrap_or(true))
                    .unwrap_or(true),
            )
        })
        .collect();
    let lower: Vec<(f64, f64, bool)> = items
        .iter()
        .map(|it| {
            let defined = it
                .get("defined")
                .map(|v| v.as_bool().unwrap_or(true))
                .unwrap_or(true);
            if vertical {
                (
                    it.f64("x").unwrap_or(0.0),
                    it.f64("y2").unwrap_or(0.0),
                    defined,
                )
            } else {
                (
                    it.f64("x2").unwrap_or(0.0),
                    it.f64("y").unwrap_or(0.0),
                    defined,
                )
            }
        })
        .collect();
    crate::interpolate::area_path(&upper, &lower, interpolate, !vertical)
}

pub fn value_to_scalar(v: &Value) -> f64 {
    to_number(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn arc(start: f64, end: f64, inner: f64) -> Item {
        let mut item = Item::new("arc");
        item.set("startAngle", json!(start));
        item.set("endAngle", json!(end));
        item.set("innerRadius", json!(inner));
        item.set("outerRadius", json!(100));
        item
    }

    #[test]
    fn pie_arcs_sweep_clockwise() {
        // d3/vega arcs sweep from the start angle to the end angle; the SVG
        // sweep flag is 1 when the end angle is greater than the start angle.
        let d = arc_path(&arc(0.0, std::f64::consts::FRAC_PI_2, 0.0));
        assert!(d.contains("A100,100,0,0,1,"), "{d}");
        assert!(d.ends_with("L0,0Z"), "{d}");
    }

    #[test]
    fn reversed_arcs_sweep_counter_clockwise() {
        let d = arc_path(&arc(std::f64::consts::FRAC_PI_2, 0.0, 0.0));
        assert!(d.contains("A100,100,0,0,0,"), "{d}");
    }

    #[test]
    fn donut_hole_has_opposite_winding() {
        let d = arc_path(&arc(0.0, std::f64::consts::FRAC_PI_2, 50.0));
        assert!(d.contains("A100,100,0,0,1,"), "{d}");
        assert!(d.contains("A50,50,0,0,0,"), "{d}");
    }

    #[test]
    fn full_circle_uses_upstream_point_order() {
        let d = arc_path(&arc(0.0, std::f64::consts::TAU, 0.0));
        assert!(d.starts_with("M0,-100A100,100,0,1,1,0,100"), "{d}");
    }
}

//! SVG path generation for line and area marks.
//!
//! The interpolation methods mirror the ones supported by Vega, which in turn
//! delegates to [d3-shape](https://github.com/d3/d3-shape). Curves are ported
//! from the corresponding d3 curve implementations so that the generated paths
//! are identical (up to number formatting).

/// A single defined vertex.
type Pt = (f64, f64);

/// Format a coordinate the way the SVG renderer expects: six decimal places,
/// trailing zeros (and a trailing `.`) removed, and `-0` normalized to `0`.
fn num(v: f64) -> String {
    if v == 0.0 {
        return "0".to_string();
    }
    let s = format!("{:.6}", v);
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.')
    } else {
        s.as_str()
    };
    if s == "-0" || s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

fn xy(p: Pt) -> String {
    format!("{},{}", num(p.0), num(p.1))
}

fn push_move(d: &mut String, p: Pt) {
    d.push('M');
    d.push_str(&xy(p));
}

fn push_line(d: &mut String, p: Pt) {
    d.push('L');
    d.push_str(&xy(p));
}

fn push_curve(d: &mut String, c1: Pt, c2: Pt, p: Pt) {
    d.push('C');
    d.push_str(&xy(c1));
    d.push(' ');
    d.push_str(&xy(c2));
    d.push(' ');
    d.push_str(&xy(p));
}

fn push_first(d: &mut String, p: Pt, as_line: bool) {
    if as_line {
        push_line(d, p);
    } else {
        push_move(d, p);
    }
}

/// Resolve a Vega interpolation name to the curve implementation and whether
/// the resulting path should be closed. Unknown names fall back to `linear`.
fn resolve(method: &str) -> (&'static str, bool) {
    match method {
        "linear" => ("linear", false),
        "linear-closed" => ("linear", true),
        "step" | "step-mid" | "step-midpoint" => ("step-mid", false),
        "step-before" => ("step-before", false),
        "step-after" => ("step-after", false),
        "basis" => ("basis", false),
        "basis-closed" => ("basis", true),
        "cardinal" => ("cardinal", false),
        "cardinal-closed" => ("cardinal", true),
        "catmull-rom" => ("catmull-rom", false),
        "catmull-rom-closed" => ("catmull-rom", true),
        "monotone" | "monotone-x" => ("monotone", false),
        "monotone-closed" | "monotone-x-closed" => ("monotone", true),
        _ => ("linear", false),
    }
}

/// Collect the runs of consecutive defined vertices.
fn defined_runs(points: &[(f64, f64, bool)]) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut start: Option<usize> = None;
    for (i, &(_, _, defined)) in points.iter().enumerate() {
        if defined {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take() {
            runs.push((s, i));
        }
    }
    if let Some(s) = start {
        runs.push((s, points.len()));
    }
    runs
}

fn run_points(points: &[(f64, f64, bool)], start: usize, end: usize) -> Vec<Pt> {
    points[start..end].iter().map(|&(x, y, _)| (x, y)).collect()
}

#[derive(Clone, Copy)]
enum StepMode {
    Mid,
    Before,
    After,
}

fn emit_linear(d: &mut String, pts: &[Pt], as_line: bool) {
    if pts.is_empty() {
        return;
    }
    push_first(d, pts[0], as_line);
    for &p in &pts[1..] {
        push_line(d, p);
    }
}

fn emit_step(d: &mut String, pts: &[Pt], mode: StepMode, as_line: bool) {
    if pts.is_empty() {
        return;
    }
    push_first(d, pts[0], as_line);
    for i in 1..pts.len() {
        let (px, py) = pts[i - 1];
        let (x, y) = pts[i];
        match mode {
            StepMode::Mid => {
                let mx = (px + x) / 2.0;
                push_line(d, (mx, py));
                push_line(d, (mx, y));
            }
            StepMode::Before => push_line(d, (px, y)),
            StepMode::After => push_line(d, (x, py)),
        }
        push_line(d, (x, y));
    }
}

fn emit_basis(d: &mut String, pts: &[Pt], as_line: bool) {
    if pts.is_empty() {
        return;
    }
    push_first(d, pts[0], as_line);
    if pts.len() < 2 {
        return;
    }
    if pts.len() == 2 {
        // d3 draws nothing for two points with curveBasis; a straight line is
        // friendlier and keeps the path visible.
        push_line(d, pts[1]);
        return;
    }
    for i in 2..pts.len() {
        let (x0, y0) = pts[i - 2];
        let (x1, y1) = pts[i - 1];
        let (x, y) = pts[i];
        if i == 2 {
            push_line(d, ((5.0 * x0 + x1) / 6.0, (5.0 * y0 + y1) / 6.0));
        }
        push_curve(
            d,
            ((2.0 * x0 + x1) / 3.0, (2.0 * y0 + y1) / 3.0),
            ((x0 + 2.0 * x1) / 3.0, (y0 + 2.0 * y1) / 3.0),
            ((x0 + 4.0 * x1 + x) / 6.0, (y0 + 4.0 * y1 + y) / 6.0),
        );
    }
}

/// d3 `curveCardinal` with the given `k = (1 - tension) / 6`.
fn emit_cardinal(d: &mut String, pts: &[Pt], as_line: bool, k: f64) {
    if pts.is_empty() {
        return;
    }
    push_first(d, pts[0], as_line);
    let n = pts.len();
    if n < 2 {
        return;
    }
    if n == 2 {
        push_line(d, pts[1]);
        return;
    }
    for i in 2..n {
        let (x0, y0) = if i >= 3 { pts[i - 3] } else { pts[i - 1] };
        let (x1, y1) = pts[i - 2];
        let (x2, y2) = pts[i - 1];
        let (x, y) = pts[i];
        push_curve(
            d,
            (x1 + k * (x2 - x0), y1 + k * (y2 - y0)),
            (x2 + k * (x1 - x), y2 + k * (y1 - y)),
            (x2, y2),
        );
    }
    let (x2, y2) = pts[n - 1];
    let (x1, y1) = pts[n - 2];
    let (x0, y0) = pts[n - 3];
    push_curve(
        d,
        (x1 + k * (x2 - x0), y1 + k * (y2 - y0)),
        (x2, y2),
        (x2, y2),
    );
}

fn dist(a: Pt, b: Pt) -> f64 {
    (b.0 - a.0).hypot(b.1 - a.1)
}

/// d3 `curveCatmullRom` with `alpha = 0.5` (centripetal by chord length).
fn emit_catmull_rom(d: &mut String, pts: &[Pt], as_line: bool) {
    if pts.is_empty() {
        return;
    }
    push_first(d, pts[0], as_line);
    let n = pts.len();
    if n < 2 {
        return;
    }
    if n == 2 {
        push_line(d, pts[1]);
        return;
    }
    const ALPHA: f64 = 0.5;
    const EPS: f64 = 1e-12;
    for i in 0..n - 1 {
        let a = pts[i];
        let b = pts[i + 1];
        let z = if i == 0 { pts[0] } else { pts[i - 1] };
        let c = if i + 2 < n { pts[i + 2] } else { b };

        let l01 = dist(z, a).powf(ALPHA);
        let l12 = dist(a, b).powf(ALPHA);
        let l23 = dist(b, c).powf(ALPHA);
        let l01_2 = dist(z, a).powf(2.0 * ALPHA);
        let l12_2 = dist(a, b).powf(2.0 * ALPHA);
        let l23_2 = dist(b, c).powf(2.0 * ALPHA);

        let cp1 = if l01 > EPS {
            let aa = 2.0 * l01_2 + 3.0 * l01 * l12 + l12_2;
            let nn = 3.0 * l01 * (l01 + l12);
            (
                (a.0 * aa - z.0 * l12_2 + b.0 * l01_2) / nn,
                (a.1 * aa - z.1 * l12_2 + b.1 * l01_2) / nn,
            )
        } else {
            a
        };
        let cp2 = if l23 > EPS {
            let bb = 2.0 * l23_2 + 3.0 * l23 * l12 + l12_2;
            let mm = 3.0 * l23 * (l23 + l12);
            (
                (b.0 * bb + a.0 * l23_2 - c.0 * l12_2) / mm,
                (b.1 * bb + a.1 * l23_2 - c.1 * l12_2) / mm,
            )
        } else {
            b
        };
        push_curve(d, cp1, cp2, b);
    }
}

fn sign(x: f64) -> f64 {
    if x < 0.0 {
        -1.0
    } else {
        1.0
    }
}

/// Steffen's method for monotone tangents (as used by d3 `curveMonotoneX`).
fn slope3(x0: f64, y0: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let h0 = x1 - x0;
    let h1 = x2 - x1;
    let d0 = if h0 != 0.0 {
        h0
    } else if h1 < 0.0 {
        -0.0
    } else {
        0.0
    };
    let d1 = if h1 != 0.0 {
        h1
    } else if h0 < 0.0 {
        -0.0
    } else {
        0.0
    };
    let s0 = (y1 - y0) / d0;
    let s1 = (y2 - y1) / d1;
    let p = (s0 * h1 + s1 * h0) / (h0 + h1);
    let a = s0.abs();
    let b = s1.abs();
    let c = 0.5 * p.abs();
    if a.is_nan() || b.is_nan() || c.is_nan() {
        return 0.0;
    }
    let v = (sign(s0) + sign(s1)) * a.min(b).min(c);
    if v == 0.0 || !v.is_finite() {
        0.0
    } else {
        v
    }
}

fn slope2(x0: f64, y0: f64, x1: f64, y1: f64, t: f64) -> f64 {
    let h = x1 - x0;
    if h != 0.0 && !h.is_nan() {
        (3.0 * (y1 - y0) / h - t) / 2.0
    } else {
        t
    }
}

fn hermite(d: &mut String, x0: f64, y0: f64, x1: f64, y1: f64, t0: f64, t1: f64) {
    let dx = (x1 - x0) / 3.0;
    push_curve(
        d,
        (x0 + dx, y0 + dx * t0),
        (x1 - dx, y1 - dx * t1),
        (x1, y1),
    );
}

fn emit_monotone(d: &mut String, pts: &[Pt], as_line: bool) {
    emit_monotone_xy(d, pts, as_line, false);
}

/// Swap every coordinate pair in a generated path (used for `curveMonotoneY`,
/// which is `curveMonotoneX` applied to transposed input and output).
fn swap_path_coords(path: &str) -> String {
    let mut out = String::new();
    let mut numbers: Vec<String> = Vec::new();
    let mut current = String::new();
    let flush_number = |numbers: &mut Vec<String>, current: &mut String| {
        if !current.is_empty() {
            numbers.push(std::mem::take(current));
        }
    };
    let flush_pairs = |out: &mut String, numbers: &mut Vec<String>| {
        let mut i = 0;
        while i + 1 < numbers.len() {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&numbers[i + 1]);
            out.push(',');
            out.push_str(&numbers[i]);
            i += 2;
        }
        if i < numbers.len() {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&numbers[i]);
        }
        numbers.clear();
    };
    for c in path.chars() {
        if c.is_ascii_alphabetic() {
            flush_number(&mut numbers, &mut current);
            flush_pairs(&mut out, &mut numbers);
            out.push(c);
        } else if c == ',' || c.is_whitespace() {
            flush_number(&mut numbers, &mut current);
        } else {
            current.push(c);
        }
    }
    flush_number(&mut numbers, &mut current);
    flush_pairs(&mut out, &mut numbers);
    out
}

fn emit_monotone_xy(d: &mut String, pts: &[Pt], as_line: bool, swap: bool) {
    if swap {
        let swapped: Vec<Pt> = pts.iter().map(|&(x, y)| (y, x)).collect();
        let mut tmp = String::new();
        emit_monotone(&mut tmp, &swapped, as_line);
        d.push_str(&swap_path_coords(&tmp));
        return;
    }
    if pts.is_empty() {
        return;
    }
    let mut x0 = f64::NAN;
    let mut y0 = f64::NAN;
    let mut x1 = f64::NAN;
    let mut y1 = f64::NAN;
    let mut t0 = f64::NAN;
    let mut count = 0usize;

    for &(x, y) in pts {
        if count > 0 && x == x1 && y == y1 {
            continue; // Ignore coincident points, like d3.
        }
        let mut t1 = f64::NAN;
        match count {
            0 => {
                push_first(d, (x, y), as_line);
                count = 1;
            }
            1 => {
                count = 2;
            }
            2 => {
                count = 3;
                t1 = slope3(x0, y0, x1, y1, x, y);
                let s = slope2(x0, y0, x1, y1, t1);
                hermite(d, x0, y0, x1, y1, s, t1);
            }
            _ => {
                t1 = slope3(x0, y0, x1, y1, x, y);
                hermite(d, x0, y0, x1, y1, t0, t1);
            }
        }
        x0 = x1;
        x1 = x;
        y0 = y1;
        y1 = y;
        t0 = t1;
    }

    match count {
        2 => push_line(d, (x1, y1)),
        n if n >= 3 => {
            let t1 = slope2(x0, y0, x1, y1, t0);
            hermite(d, x0, y0, x1, y1, t0, t1);
        }
        _ => {}
    }
}

/// Emit one open edge of a curve; `as_line` starts with `L` instead of `M`.
fn emit_curve(d: &mut String, pts: &[Pt], method: &str, as_line: bool) {
    match method {
        "step-mid" => emit_step(d, pts, StepMode::Mid, as_line),
        "step-before" => emit_step(d, pts, StepMode::Before, as_line),
        "step-after" => emit_step(d, pts, StepMode::After, as_line),
        "basis" => emit_basis(d, pts, as_line),
        "cardinal" => emit_cardinal(d, pts, as_line, 1.0 / 6.0),
        "catmull-rom" => emit_catmull_rom(d, pts, as_line),
        "monotone" => emit_monotone(d, pts, as_line),
        _ => emit_linear(d, pts, as_line),
    }
}

/// Build an SVG path from points. `defined == false` breaks the line.
pub fn line_path(points: &[(f64, f64, bool)], interpolate: &str) -> String {
    let (method, closed) = resolve(interpolate);
    let mut d = String::new();
    for (start, end) in defined_runs(points) {
        let pts = run_points(points, start, end);
        emit_curve(&mut d, &pts, method, false);
        if closed && !pts.is_empty() {
            d.push('Z');
        }
    }
    d
}

/// Build a closed SVG area path from upper and lower edges.
/// Points with `defined == false` split the area into separate subpaths.
pub fn area_path(
    upper: &[(f64, f64, bool)],
    lower: &[(f64, f64, bool)],
    interpolate: &str,
    horizontal: bool,
) -> String {
    let (method, _) = resolve(interpolate);
    let n = upper.len().min(lower.len());
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut start: Option<usize> = None;
    for i in 0..n {
        let defined = upper[i].2 && lower[i].2;
        if defined {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take() {
            runs.push((s, i));
        }
    }
    if let Some(s) = start {
        runs.push((s, n));
    }

    let mut d = String::new();
    for (s, e) in runs {
        let top: Vec<Pt> = upper[s..e].iter().map(|&(x, y, _)| (x, y)).collect();
        let mut bottom: Vec<Pt> = lower[s..e].iter().map(|&(x, y, _)| (x, y)).collect();
        bottom.reverse();

        if method == "monotone" {
            emit_monotone_xy(&mut d, &top, false, horizontal);
            emit_monotone_xy(&mut d, &bottom, true, horizontal);
        } else {
            emit_curve(&mut d, &top, method, false);
            emit_curve(&mut d, &bottom, method, true);
        }
        d.push('Z');
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_nan(s: &str) -> bool {
        s.contains("NaN") || s.contains("nan") || s.contains("inf")
    }

    #[test]
    fn linear_line() {
        let pts = [(0.0, 0.0, true), (10.0, 20.0, true), (20.0, 5.5, true)];
        assert_eq!(line_path(&pts, "linear"), "M0,0L10,20L20,5.5");
    }

    #[test]
    fn linear_breaks_on_undefined() {
        let pts = [
            (0.0, 0.0, true),
            (1.0, 1.0, true),
            (2.0, 2.0, false),
            (3.0, 3.0, true),
        ];
        assert_eq!(line_path(&pts, "linear"), "M0,0L1,1M3,3");
    }

    #[test]
    fn handles_short_inputs() {
        assert_eq!(line_path(&[], "linear"), "");
        assert_eq!(line_path(&[(1.0, 2.0, true)], "basis"), "M1,2");
        assert_eq!(
            line_path(&[(0.0, 0.0, true), (4.0, 8.0, true)], "monotone"),
            "M0,0L4,8"
        );
    }

    #[test]
    fn step_after_line() {
        let pts = [(0.0, 0.0, true), (10.0, 20.0, true)];
        assert_eq!(line_path(&pts, "step-after"), "M0,0L10,0L10,20");
        assert_eq!(line_path(&pts, "step"), "M0,0L5,0L5,20L10,20");
    }

    #[test]
    fn monotone_line_is_well_formed() {
        let pts = [
            (0.0, 0.0, true),
            (1.0, 4.0, true),
            (2.0, 2.0, true),
            (3.0, 8.0, true),
        ];
        let d = line_path(&pts, "monotone");
        assert!(d.starts_with('M'), "path should start with M: {d}");
        assert!(!has_nan(&d), "path should not contain NaN: {d}");
        assert!(d.contains('C'), "monotone should emit cubic curves: {d}");
    }

    #[test]
    fn area_closes() {
        let upper = [(0.0, 0.0, true), (10.0, 0.0, true)];
        let lower = [(0.0, 10.0, true), (10.0, 10.0, true)];
        assert_eq!(area_path(&upper, &lower, "linear", false), "M0,0L10,0L10,10L0,10Z");

        let d = area_path(&upper, &lower, "monotone", false);
        assert!(d.starts_with('M'));
        assert!(d.ends_with('Z'));
        assert!(!has_nan(&d));
    }

    #[test]
    fn area_splits_on_undefined() {
        let upper = [
            (0.0, 0.0, true),
            (10.0, 0.0, true),
            (20.0, 0.0, false),
            (30.0, 0.0, true),
        ];
        let lower = [
            (0.0, 5.0, true),
            (10.0, 5.0, true),
            (20.0, 5.0, false),
            (30.0, 5.0, true),
        ];
        assert_eq!(
            area_path(&upper, &lower, "linear", false),
            "M0,0L10,0L10,5L0,5ZM30,0L30,5Z"
        );
    }

    #[test]
    fn numbers_trim_and_normalize() {
        assert_eq!(num(-0.0), "0");
        assert_eq!(num(0.0), "0");
        assert_eq!(num(1.5), "1.5");
        assert_eq!(num(1.0 / 3.0), "0.333333");
        assert_eq!(num(2.0000004), "2");
    }

    #[test]
    fn unknown_falls_back_to_linear() {
        let pts = [(0.0, 0.0, true), (1.0, 1.0, true)];
        assert_eq!(line_path(&pts, "bogus"), "M0,0L1,1");
    }
}

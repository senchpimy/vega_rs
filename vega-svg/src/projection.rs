//! Cartographic projections and clipping (port of `d3-geo`).
//!
//! The implementation follows d3-geo's streaming architecture closely so that
//! projected path strings match upstream Vega, including adaptive resampling and
//! polygon clipping. Supported projections: albers, albersUsa, azimuthal equal
//! area/equidistant, conic conformal/equal area/equidistant, equalEarth,
//! equirectangular, gnomonic, identity, mercator, naturalEarth1, orthographic,
//! stereographic and transverse mercator.

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::Value;

use crate::geo::*;

const MAX_DEPTH: i32 = 16;

// ---------------------------------------------------------------------------
// Path string generation
// ---------------------------------------------------------------------------

pub struct PathString {
    out: String,
    radius: f64,
    point_state: i32,
    in_polygon: bool,
}

impl Default for PathString {
    fn default() -> Self {
        Self::new()
    }
}

impl PathString {
    pub fn new() -> Self {
        Self {
            out: String::new(),
            radius: 4.5,
            point_state: -1,
            in_polygon: false,
        }
    }

    pub fn point_radius(&mut self, radius: f64) {
        self.radius = radius;
    }

    pub fn result(&mut self) -> String {
        std::mem::take(&mut self.out)
    }

    fn push_point(&mut self, x: f64, y: f64) {
        let x = round3(x);
        let y = round3(y);
        match self.point_state {
            0 => {
                self.out.push('M');
                self.out.push_str(&num(x));
                self.out.push(',');
                self.out.push_str(&num(y));
                self.point_state = 1;
            }
            1 => {
                self.out.push('L');
                self.out.push_str(&num(x));
                self.out.push(',');
                self.out.push_str(&num(y));
            }
            _ => {
                self.out.push('M');
                self.out.push_str(&num(x));
                self.out.push(',');
                self.out.push_str(&num(y));
                let r = round3(self.radius);
                self.out.push('m');
                self.out.push_str(&num(0.0));
                self.out.push(',');
                self.out.push_str(&num(r));
                self.out.push('a');
                self.out.push_str(&format!(
                    "{0},{0} 0 1,1 0,{1}a{0},{0} 0 1,1 0,{2}z",
                    num(r),
                    num(-2.0 * r),
                    num(2.0 * r)
                ));
            }
        }
    }
}

impl Sink for PathString {
    fn point(&mut self, x: f64, y: f64, _m: Option<f64>) {
        self.push_point(x, y);
    }
    fn line_start(&mut self) {
        self.point_state = 0;
    }
    fn line_end(&mut self) {
        if self.in_polygon {
            self.out.push('Z');
        }
        self.point_state = -1;
    }
    fn polygon_start(&mut self) {
        self.in_polygon = true;
    }
    fn polygon_end(&mut self) {
        self.in_polygon = false;
    }
    fn sphere(&mut self) {
        // The projection stream normally expands the sphere before it reaches
        // the path sink; this fallback renders nothing.
    }
}

pub fn round3(value: f64) -> f64 {
    let r = (value * 1000.0).round() / 1000.0;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

fn num(value: f64) -> String {
    if !value.is_finite() {
        return "0".to_string();
    }
    let s = format!("{value:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" {
        "0".to_string()
    } else if s.is_empty() {
        "0".to_string()
    } else {
        s
    }
}

// ---------------------------------------------------------------------------
// Bounds sink (used by projection fitting)
// ---------------------------------------------------------------------------

pub struct BoundsSink {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl Default for BoundsSink {
    fn default() -> Self {
        Self::new()
    }
}

impl BoundsSink {
    pub fn new() -> Self {
        Self {
            x0: f64::INFINITY,
            y0: f64::INFINITY,
            x1: f64::NEG_INFINITY,
            y1: f64::NEG_INFINITY,
        }
    }
    pub fn result(&self) -> [[f64; 2]; 2] {
        [[self.x0, self.y0], [self.x1, self.y1]]
    }
}

impl Sink for BoundsSink {
    fn point(&mut self, x: f64, y: f64, _m: Option<f64>) {
        if x < self.x0 {
            self.x0 = x;
        }
        if x > self.x1 {
            self.x1 = x;
        }
        if y < self.y0 {
            self.y0 = y;
        }
        if y > self.y1 {
            self.y1 = y;
        }
    }
    fn line_start(&mut self) {}
    fn line_end(&mut self) {}
    fn polygon_start(&mut self) {}
    fn polygon_end(&mut self) {}
    fn sphere(&mut self) {}
}

// ---------------------------------------------------------------------------
// Transform stages
// ---------------------------------------------------------------------------

pub struct RadiansSink {
    pub sink: SharedSink,
}
impl Sink for RadiansSink {
    fn point(&mut self, x: f64, y: f64, m: Option<f64>) {
        self.sink.point(x * RADIANS, y * RADIANS, m);
    }
    fn line_start(&mut self) {
        self.sink.line_start()
    }
    fn line_end(&mut self) {
        self.sink.line_end()
    }
    fn polygon_start(&mut self) {
        self.sink.polygon_start()
    }
    fn polygon_end(&mut self) {
        self.sink.polygon_end()
    }
    fn sphere(&mut self) {
        self.sink.sphere()
    }
}

pub struct RotateSink {
    pub sink: SharedSink,
    pub rotate: Rotate,
}
impl Sink for RotateSink {
    fn point(&mut self, x: f64, y: f64, m: Option<f64>) {
        let p = (self.rotate)([x, y]);
        self.sink.point(p[0], p[1], m);
    }
    fn line_start(&mut self) {
        self.sink.line_start()
    }
    fn line_end(&mut self) {
        self.sink.line_end()
    }
    fn polygon_start(&mut self) {
        self.sink.polygon_start()
    }
    fn polygon_end(&mut self) {
        self.sink.polygon_end()
    }
    fn sphere(&mut self) {
        self.sink.sphere()
    }
}

// ---------------------------------------------------------------------------
// Clip buffer
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct ClipPoint {
    pub x: f64,
    pub y: f64,
    pub m: f64,
}

pub struct ClipBuffer {
    lines: Vec<Vec<ClipPoint>>,
}

impl Default for ClipBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipBuffer {
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }
    pub fn rejoin(&mut self) {
        if self.lines.len() > 1 {
            let last = self.lines.pop().unwrap();
            let mut first = self.lines.remove(0);
            let mut joined = last;
            joined.append(&mut first);
            self.lines.insert(0, joined);
        }
    }
    pub fn result(&mut self) -> Vec<Vec<ClipPoint>> {
        std::mem::take(&mut self.lines)
    }
}

impl Sink for ClipBuffer {
    fn point(&mut self, x: f64, y: f64, m: Option<f64>) {
        if self.lines.is_empty() {
            self.lines.push(Vec::new());
        }
        if let Some(line) = self.lines.last_mut() {
            line.push(ClipPoint {
                x,
                y,
                m: m.unwrap_or(0.0),
            });
        }
    }
    fn line_start(&mut self) {
        self.lines.push(Vec::new());
    }
    fn line_end(&mut self) {}
    fn polygon_start(&mut self) {}
    fn polygon_end(&mut self) {}
    fn sphere(&mut self) {}
}

// ---------------------------------------------------------------------------
// Clip line helpers
// ---------------------------------------------------------------------------

pub trait ClipLine {
    fn line_start(&mut self);
    fn point(&mut self, lambda: f64, phi: f64, m: Option<f64>);
    fn line_end(&mut self);
    fn clean(&self) -> i32;
}

pub struct AntimeridianLine {
    sink: SharedSink,
    lambda0: f64,
    phi0: f64,
    sign0: f64,
    clean: i32,
}

impl AntimeridianLine {
    pub fn new(sink: SharedSink) -> Self {
        Self {
            sink,
            lambda0: f64::NAN,
            phi0: f64::NAN,
            sign0: f64::NAN,
            clean: 1,
        }
    }
}

fn antimeridian_intersect(l0: f64, p0: f64, l1: f64, p1: f64) -> f64 {
    let sin_l = (l0 - l1).sin();
    if sin_l.abs() > EPSILON {
        let cos_p1 = p1.cos();
        let cos_p0 = p0.cos();
        ((p0.sin() * cos_p1 * l1.sin() - p1.sin() * cos_p0 * l0.sin())
            / (cos_p0 * cos_p1 * sin_l))
            .atan()
    } else {
        (p0 + p1) / 2.0
    }
}

impl ClipLine for AntimeridianLine {
    fn line_start(&mut self) {
        self.sink.line_start();
        self.clean = 1;
    }
    fn point(&mut self, lambda1: f64, phi1: f64, _m: Option<f64>) {
        let sign1 = if lambda1 > 0.0 { PI } else { -PI };
        let delta = (lambda1 - self.lambda0).abs();
        if (delta - PI).abs() < EPSILON {
            let phi = if (self.phi0 + phi1) / 2.0 > 0.0 {
                HALF_PI
            } else {
                -HALF_PI
            };
            self.sink.point(self.lambda0, phi, None);
            self.sink.point(self.sign0, phi, None);
            self.sink.line_end();
            self.sink.line_start();
            self.sink.point(sign1, phi, None);
            self.sink.point(lambda1, phi, None);
            self.clean = 0;
        } else if self.sign0 != sign1 && delta >= PI {
            let mut l0 = self.lambda0;
            let mut l1 = lambda1;
            if (l0 - self.sign0).abs() < EPSILON {
                l0 -= self.sign0 * EPSILON;
            }
            if (l1 - sign1).abs() < EPSILON {
                l1 -= sign1 * EPSILON;
            }
            let phi = antimeridian_intersect(l0, self.phi0, l1, phi1);
            self.sink.point(self.sign0, phi, None);
            self.sink.line_end();
            self.sink.line_start();
            self.sink.point(sign1, phi, None);
            self.clean = 0;
        }
        self.sink.point(lambda1, phi1, None);
        self.lambda0 = lambda1;
        self.phi0 = phi1;
        self.sign0 = sign1;
    }
    fn line_end(&mut self) {
        self.sink.line_end();
        self.lambda0 = f64::NAN;
        self.phi0 = f64::NAN;
    }
    fn clean(&self) -> i32 {
        2 - self.clean
    }
}

fn clip_antimeridian_interpolate(
    from: Option<[f64; 2]>,
    to: Option<[f64; 2]>,
    direction: f64,
    sink: &mut dyn Sink,
) {
    if from.is_none() {
        let phi = direction * HALF_PI;
        sink.point(-PI, phi, None);
        sink.point(0.0, phi, None);
        sink.point(PI, phi, None);
        sink.point(PI, 0.0, None);
        sink.point(PI, -phi, None);
        sink.point(0.0, -phi, None);
        sink.point(-PI, -phi, None);
        sink.point(-PI, 0.0, None);
        sink.point(-PI, phi, None);
    } else if let (Some(from), Some(to)) = (from, to) {
        if (from[0] - to[0]).abs() > EPSILON {
            let lambda = if from[0] < to[0] { PI } else { -PI };
            let phi = direction * lambda / 2.0;
            sink.point(-lambda, phi, None);
            sink.point(0.0, phi, None);
            sink.point(lambda, phi, None);
        } else {
            sink.point(to[0], to[1], None);
        }
    }
}

// ---------------------------------------------------------------------------
// Circle clipping
// ---------------------------------------------------------------------------

pub struct CircleLine {
    sink: SharedSink,
    cr: f64,
    radius: f64,
    small_radius: bool,
    not_hemisphere: bool,
    point0: Option<[f64; 2]>,
    c0: i32,
    v0: bool,
    v00: bool,
    clean: i32,
}

impl CircleLine {
    pub fn new(sink: SharedSink, radius: f64) -> Self {
        let cr = radius.cos();
        Self {
            sink,
            cr,
            radius,
            small_radius: cr > 0.0,
            not_hemisphere: cr.abs() > EPSILON,
            point0: None,
            c0: 0,
            v0: false,
            v00: false,
            clean: 1,
        }
    }

    fn visible(&self, lambda: f64, phi: f64) -> bool {
        lambda.cos() * phi.cos() > self.cr
    }

    fn code(&self, lambda: f64, phi: f64) -> i32 {
        let r = if self.small_radius {
            self.radius
        } else {
            PI - self.radius
        };
        let mut code = 0;
        if lambda < -r {
            code |= 1;
        } else if lambda > r {
            code |= 2;
        }
        if phi < -r {
            code |= 4;
        } else if phi > r {
            code |= 8;
        }
        code
    }

    fn intersect(&self, a: [f64; 2], b: [f64; 2], two: bool) -> Option<IntersectionResult> {
        let pa = cartesian(a);
        let pb = cartesian(b);
        let n1 = [1.0, 0.0, 0.0];
        let n2 = cartesian_cross(pa, pb);
        let n2n2 = cartesian_dot(n2, n2);
        let n1n2 = n2[0];
        let determinant = n2n2 - n1n2 * n1n2;
        if determinant == 0.0 {
            return if two { None } else { Some(IntersectionResult::One(a)) };
        }
        let c1 = self.cr * n2n2 / determinant;
        let c2 = -self.cr * n1n2 / determinant;
        let n1xn2 = cartesian_cross(n1, n2);
        let mut aa = cartesian_scale(n1, c1);
        let bb = cartesian_scale(n2, c2);
        cartesian_add_in_place(&mut aa, bb);
        let u = n1xn2;
        let w = cartesian_dot(aa, u);
        let uu = cartesian_dot(u, u);
        let t2 = w * w - uu * (cartesian_dot(aa, aa) - 1.0);
        if t2 < 0.0 {
            return None;
        }
        let t = t2.sqrt();
        let mut q = cartesian_scale(u, (-w - t) / uu);
        cartesian_add_in_place(&mut q, aa);
        let q = spherical(q);
        if !two {
            return Some(IntersectionResult::One(q));
        }

        let mut lambda0 = a[0];
        let mut lambda1 = b[0];
        let mut phi0 = a[1];
        let mut phi1 = b[1];
        if lambda1 < lambda0 {
            std::mem::swap(&mut lambda0, &mut lambda1);
        }
        let delta = lambda1 - lambda0;
        let polar = (delta - PI).abs() < EPSILON;
        let meridian = polar || delta < EPSILON;
        if !polar && phi1 < phi0 {
            std::mem::swap(&mut phi0, &mut phi1);
        }
        let inside = if meridian {
            if polar {
                (phi0 + phi1 > 0.0) ^ (q[1] < if (q[0] - lambda0).abs() < EPSILON { phi0 } else { phi1 })
            } else {
                phi0 <= q[1] && q[1] <= phi1
            }
        } else {
            (delta > PI) ^ (lambda0 <= q[0] && q[0] <= lambda1)
        };
        if inside {
            let mut q1 = cartesian_scale(u, (-w + t) / uu);
            cartesian_add_in_place(&mut q1, aa);
            return Some(IntersectionResult::Two(q, spherical(q1)));
        }
        None
    }
}

pub enum IntersectionResult {
    One([f64; 2]),
    Two([f64; 2], [f64; 2]),
}

impl ClipLine for CircleLine {
    fn line_start(&mut self) {
        self.v00 = false;
        self.v0 = false;
        self.clean = 1;
    }
    fn point(&mut self, lambda: f64, phi: f64, _m: Option<f64>) {
        let point1 = [lambda, phi];
        let v = self.visible(lambda, phi);
        let c = if self.small_radius {
            if v {
                0
            } else {
                self.code(lambda, phi)
            }
        } else if v {
            self.code(lambda + if lambda < 0.0 { PI } else { -PI }, phi)
        } else {
            0
        };
        if self.point0.is_none() {
            self.v00 = v;
            self.v0 = v;
            if v {
                self.sink.line_start();
            }
        }
        let mut point2: Option<[f64; 2]> = None;
        if v != self.v0 {
            if let Some(p0) = self.point0 {
                point2 = match self.intersect(p0, point1, false) {
                    Some(IntersectionResult::One(q)) => Some(q),
                    _ => None,
                };
                if point2
                    .map(|p| point_equal(p0, p) || point_equal(point1, p))
                    .unwrap_or(true)
                {
                    // d3 marks this as an intersection; the marker is not
                    // forwarded to the sink, so no state change is needed.
                }
            }
        }
        if v != self.v0 {
            self.clean = 0;
            if v {
                self.sink.line_start();
                if let Some(p0) = self.point0 {
                    point2 = match self.intersect(point1, p0, false) {
                        Some(IntersectionResult::One(q)) => Some(q),
                        _ => None,
                    };
                    if let Some(p) = point2 {
                        self.sink.point(p[0], p[1], None);
                    }
                }
            } else {
                if let Some(p0) = self.point0 {
                    point2 = match self.intersect(p0, point1, false) {
                        Some(IntersectionResult::One(q)) => Some(q),
                        _ => None,
                    };
                    if let Some(p) = point2 {
                        self.sink.point(p[0], p[1], Some(2.0));
                    }
                }
                self.sink.line_end();
            }
            self.point0 = point2;
        } else if self.not_hemisphere && self.point0.is_some() && self.small_radius != v {
            let p0 = self.point0.unwrap();
            if self.c0 & c == 0 {
                if let Some(IntersectionResult::Two(t0, t1)) = self.intersect(point1, p0, true) {
                    self.clean = 0;
                    if self.small_radius {
                        self.sink.line_start();
                        self.sink.point(t0[0], t0[1], None);
                        self.sink.point(t1[0], t1[1], None);
                        self.sink.line_end();
                    } else {
                        self.sink.point(t1[0], t1[1], None);
                        self.sink.line_end();
                        self.sink.line_start();
                        self.sink.point(t0[0], t0[1], Some(3.0));
                    }
                }
            }
        }
        if v
            && self
                .point0
                .map(|p0| !point_equal(p0, point1))
                .unwrap_or(true)
        {
            self.sink.point(point1[0], point1[1], None);
        }
        self.point0 = Some(point1);
        self.v0 = v;
        self.c0 = c;
    }
    fn line_end(&mut self) {
        if self.v0 {
            self.sink.line_end();
        }
        self.point0 = None;
    }
    fn clean(&self) -> i32 {
        self.clean | (if self.v00 && self.v0 { 2 } else { 0 })
    }
}

fn circle_interpolate(
    from: Option<[f64; 2]>,
    to: Option<[f64; 2]>,
    direction: f64,
    radius: f64,
    sink: &mut dyn Sink,
) {
    circle_stream(sink, radius, 2.0 * RADIANS, direction, from, to);
}

// ---------------------------------------------------------------------------
// Generic clip wrapper
// ---------------------------------------------------------------------------

pub enum ClipKind {
    Antimeridian,
    Circle(f64),
}

struct ClipIntersection {
    x: [f64; 2],
    z: Option<usize>,
    o: usize,
    e: bool,
    v: bool,
    n: usize,
    p: usize,
}

fn compare_intersection(a: &ClipIntersection, b: &ClipIntersection) -> std::cmp::Ordering {
    let key = |v: &[f64; 2]| {
        if v[0] < 0.0 {
            v[1] - HALF_PI - EPSILON
        } else {
            HALF_PI - v[1]
        }
    };
    key(&a.x)
        .partial_cmp(&key(&b.x))
        .unwrap_or(std::cmp::Ordering::Equal)
}

fn compare_rect_intersection(
    a: &ClipIntersection,
    b: &ClipIntersection,
    x0: f64,
    y0: f64,
    x1: f64,
    _y1: f64,
) -> std::cmp::Ordering {
    let corner = |p: &[f64; 2]| -> i32 {
        if (p[0] - x0).abs() < EPSILON {
            0
        } else if (p[0] - x1).abs() < EPSILON {
            2
        } else if (p[1] - y0).abs() < EPSILON {
            1
        } else {
            3
        }
    };
    let ca = corner(&a.x);
    let cb = corner(&b.x);
    if ca != cb {
        return ca.cmp(&cb);
    }
    let va = match ca {
        0 => b.x[1] - a.x[1],
        1 => a.x[0] - b.x[0],
        2 => a.x[1] - b.x[1],
        _ => b.x[0] - a.x[0],
    };
    va.partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal)
}

fn link_nodes(nodes: &mut [ClipIntersection], order: &[usize]) {
    if order.len() < 2 {
        return;
    }
    for i in 0..order.len() {
        let a = order[i];
        let b = order[(i + 1) % order.len()];
        nodes[a].n = b;
        nodes[b].p = a;
    }
}

fn clip_rejoin(
    segments: &[Vec<ClipPoint>],
    start_inside: bool,
    interpolate: &mut dyn FnMut(Option<[f64; 2]>, Option<[f64; 2]>, f64, &mut dyn Sink),
    compare: &mut dyn FnMut(&ClipIntersection, &ClipIntersection) -> std::cmp::Ordering,
    stream: &mut dyn Sink,
) {
    let mut nodes: Vec<ClipIntersection> = Vec::new();
    let mut subject: Vec<usize> = Vec::new();
    let mut clip: Vec<usize> = Vec::new();

    for (si, segment) in segments.iter().enumerate() {
        let n = segment.len();
        if n < 2 {
            continue;
        }
        let p0 = [segment[0].x, segment[0].y];
        let mut p1 = [segment[n - 1].x, segment[n - 1].y];
        if point_equal(p0, p1) {
            if segment[0].m == 0.0 && segment[n - 1].m == 0.0 {
                stream.line_start();
                for p in segment.iter().take(n - 1) {
                    stream.point(p.x, p.y, None);
                }
                stream.line_end();
                continue;
            }
            p1[0] += 2.0 * EPSILON;
        }
        let s0 = nodes.len();
        nodes.push(ClipIntersection {
            x: p0,
            z: Some(si),
            o: s0 + 1,
            e: true,
            v: false,
            n: 0,
            p: 0,
        });
        let c0 = nodes.len();
        nodes.push(ClipIntersection {
            x: p0,
            z: None,
            o: s0,
            e: false,
            v: false,
            n: 0,
            p: 0,
        });
        let s1 = nodes.len();
        nodes.push(ClipIntersection {
            x: p1,
            z: Some(si),
            o: s1 + 1,
            e: false,
            v: false,
            n: 0,
            p: 0,
        });
        let c1 = nodes.len();
        nodes.push(ClipIntersection {
            x: p1,
            z: None,
            o: s1,
            e: true,
            v: false,
            n: 0,
            p: 0,
        });
        subject.push(s0);
        subject.push(s1);
        clip.push(c0);
        clip.push(c1);
    }
    if subject.is_empty() {
        return;
    }

    clip.sort_by(|a, b| compare(&nodes[*a], &nodes[*b]));
    link_nodes(&mut nodes, &subject);
    link_nodes(&mut nodes, &clip);

    let mut inside = start_inside;
    for &ci in &clip {
        inside = !inside;
        nodes[ci].e = inside;
    }

    let start = subject[0];
    loop {
        let mut current = start;
        while nodes[current].v {
            current = nodes[current].n;
            if current == start {
                return;
            }
        }
        stream.line_start();
        loop {
            let next_current;
            let is_subject = nodes[current].z.is_some();
            let node_x = nodes[current].x;
            if nodes[current].e {
                if is_subject {
                    let seg = nodes[current].z.unwrap();
                    for p in &segments[seg] {
                        stream.point(p.x, p.y, None);
                    }
                    next_current = nodes[current].n;
                } else {
                    let other = nodes[nodes[current].n].x;
                    interpolate(Some(node_x), Some(other), 1.0, stream);
                    next_current = nodes[current].n;
                }
            } else if is_subject {
                let seg = nodes[current].z.unwrap();
                for p in segments[seg].iter().rev() {
                    stream.point(p.x, p.y, None);
                }
                next_current = nodes[current].p;
            } else {
                let other = nodes[nodes[current].p].x;
                interpolate(Some(node_x), Some(other), -1.0, stream);
                next_current = nodes[current].p;
            }
            nodes[current].v = true;
            nodes[next_current].v = true;
            let o = nodes[next_current].o;
            nodes[current].v = true;
            nodes[o].v = true;
            current = o;
            if nodes[current].v {
                break;
            }
        }
        stream.line_end();
    }
}

pub struct ClipStream {
    kind: ClipKind,
    start: [f64; 2],
    sink: SharedSink,
    line: Box<dyn ClipLine>,
    ring_line: Box<dyn ClipLine>,
    buffer_raw: Rc<RefCell<ClipBuffer>>,
    in_polygon: bool,
    in_line: bool,
    polygon_started: bool,
    segments: Vec<Vec<Vec<ClipPoint>>>,
    polygon: Vec<Vec<[f64; 2]>>,
    ring: Vec<[f64; 2]>,
}

impl ClipStream {
    pub fn new(kind: ClipKind, sink: SharedSink) -> Self {
        let buffer_raw = Rc::new(RefCell::new(ClipBuffer::new()));
        let buffer = SharedSink(Rc::new(RefCell::new(SharedSinkAdapter(buffer_raw.clone()))));
        let (line, ring_line, start) = match &kind {
            ClipKind::Antimeridian => (
                Box::new(AntimeridianLine::new(sink.clone())) as Box<dyn ClipLine>,
                Box::new(AntimeridianLine::new(buffer.clone())) as Box<dyn ClipLine>,
                [-PI, -HALF_PI],
            ),
            ClipKind::Circle(radius) => (
                Box::new(CircleLine::new(sink.clone(), *radius)) as Box<dyn ClipLine>,
                Box::new(CircleLine::new(buffer.clone(), *radius)) as Box<dyn ClipLine>,
                if radius.cos() > 0.0 {
                    [0.0, -*radius]
                } else {
                    [-PI, *radius - PI]
                },
            ),
        };
        Self {
            kind,
            start,
            sink,
            line,
            ring_line,
            buffer_raw,
            in_polygon: false,
            in_line: false,
            polygon_started: false,
            segments: Vec::new(),
            polygon: Vec::new(),
            ring: Vec::new(),
        }
    }

    fn point_visible(&self, lambda: f64, phi: f64) -> bool {
        match self.kind {
            ClipKind::Antimeridian => true,
            ClipKind::Circle(radius) => lambda.cos() * phi.cos() > radius.cos(),
        }
    }

    fn ring_end(&mut self) {
        if self.ring.is_empty() {
            return;
        }
        let first = self.ring[0];
        self.ring_line.point(first[0], first[1], None);
        self.ring_line.line_end();
        let clean = self.ring_line.clean();
        let ring_segments = self.buffer_raw.borrow_mut().result();
        self.polygon.push(std::mem::take(&mut self.ring));
        let n = ring_segments.len();
        if n == 0 {
            return;
        }
        if clean & 1 != 0 {
            let segment = &ring_segments[0];
            let m = segment.len().saturating_sub(1);
            if m > 0 {
                if !self.polygon_started {
                    self.sink.polygon_start();
                    self.polygon_started = true;
                }
                self.sink.line_start();
                for p in segment.iter().take(m) {
                    self.sink.point(p.x, p.y, None);
                }
                self.sink.line_end();
            }
            return;
        }
        let mut segments = ring_segments;
        if n > 1 && clean & 2 != 0 {
            let last = segments.pop().unwrap();
            let first = segments.remove(0);
            let mut joined = last;
            joined.extend(first);
            segments.push(joined);
        }
        let valid: Vec<Vec<ClipPoint>> = segments
            .into_iter()
            .filter(|segment| segment.len() > 1)
            .collect();
        self.segments.push(valid);
    }

    fn clip_polygon_end(&mut self) {
        let mut segments: Vec<Vec<ClipPoint>> = Vec::new();
        for group in self.segments.drain(..) {
            segments.extend(group);
        }
        let start_inside = polygon_contains(&self.polygon, self.start);
        let kind = match self.kind {
            ClipKind::Antimeridian => None,
            ClipKind::Circle(r) => Some(r),
        };
        if !segments.is_empty() {
            if !self.polygon_started {
                self.sink.polygon_start();
                self.polygon_started = true;
            }
            let mut interpolate = |from: Option<[f64; 2]>,
                                   to: Option<[f64; 2]>,
                                   direction: f64,
                                   sink: &mut dyn Sink| {
                match kind {
                    None => clip_antimeridian_interpolate(from, to, direction, sink),
                    Some(radius) => circle_interpolate(from, to, direction, radius, sink),
                }
            };
            let mut compare = |a: &ClipIntersection, b: &ClipIntersection| {
                compare_intersection(a, b)
            };
            let sink = self.sink.clone();
            clip_rejoin(
                &segments,
                start_inside,
                &mut interpolate,
                &mut compare,
                &mut *sink.borrow(),
            );
        } else if start_inside {
            if !self.polygon_started {
                self.sink.polygon_start();
                self.polygon_started = true;
            }
            self.sink.line_start();
            match kind {
                None => clip_antimeridian_interpolate(None, None, 1.0, &mut *self.sink.borrow()),
                Some(radius) => circle_interpolate(None, None, 1.0, radius, &mut *self.sink.borrow()),
            }
            self.sink.line_end();
        }
        if self.polygon_started {
            self.sink.polygon_end();
            self.polygon_started = false;
        }
        self.segments.clear();
        self.polygon.clear();
    }
}

struct SharedSinkAdapter(Rc<RefCell<ClipBuffer>>);

impl Sink for SharedSinkAdapter {
    fn point(&mut self, x: f64, y: f64, m: Option<f64>) {
        self.0.borrow_mut().point(x, y, m)
    }
    fn line_start(&mut self) {
        self.0.borrow_mut().line_start()
    }
    fn line_end(&mut self) {
        self.0.borrow_mut().line_end()
    }
    fn polygon_start(&mut self) {
        self.0.borrow_mut().polygon_start()
    }
    fn polygon_end(&mut self) {
        self.0.borrow_mut().polygon_end()
    }
    fn sphere(&mut self) {
        self.0.borrow_mut().sphere()
    }
}

impl Sink for ClipStream {
    fn point(&mut self, lambda: f64, phi: f64, m: Option<f64>) {
        if self.in_polygon {
            self.ring.push([lambda, phi]);
            self.ring_line.point(lambda, phi, m);
        } else if self.in_line {
            self.line.point(lambda, phi, m);
        } else if self.point_visible(lambda, phi) {
            self.sink.point(lambda, phi, m);
        }
    }
    fn line_start(&mut self) {
        if self.in_polygon {
            self.ring_line.line_start();
            self.ring.clear();
        } else {
            self.in_line = true;
            self.line.line_start();
        }
    }
    fn line_end(&mut self) {
        if self.in_polygon {
            self.ring_end();
        } else {
            self.in_line = false;
            self.line.line_end();
        }
    }
    fn polygon_start(&mut self) {
        self.in_polygon = true;
        self.segments.clear();
        self.polygon.clear();
    }
    fn polygon_end(&mut self) {
        self.clip_polygon_end();
        self.in_polygon = false;
    }
    fn sphere(&mut self) {
        self.sink.polygon_start();
        self.sink.line_start();
        match self.kind {
            ClipKind::Antimeridian => {
                clip_antimeridian_interpolate(None, None, 1.0, &mut *self.sink.borrow())
            }
            ClipKind::Circle(radius) => {
                circle_interpolate(None, None, 1.0, radius, &mut *self.sink.borrow())
            }
        }
        self.sink.line_end();
        self.sink.polygon_end();
    }
}

// ---------------------------------------------------------------------------
// Adaptive resampling
// ---------------------------------------------------------------------------

pub struct ResampleStream {
    sink: SharedSink,
    project: ProjectTransform,
    delta2: f64,
    mode: ResampleMode,
    x0: f64,
    y0: f64,
    lambda0: f64,
    a0: f64,
    b0: f64,
    c0: f64,
    x00: f64,
    y00: f64,
    lambda00: f64,
    a00: f64,
    b00: f64,
    c00: f64,
    ring: bool,
    ring_pending: bool,
    first: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum ResampleMode {
    Point,
    Line,
    RingStart,
}

impl ResampleStream {
    fn new(project: ProjectTransform, sink: SharedSink, delta2: f64) -> Self {
        Self {
            sink,
            project,
            delta2,
            mode: ResampleMode::Point,
            x0: f64::NAN,
            y0: f64::NAN,
            lambda0: f64::NAN,
            a0: 0.0,
            b0: 0.0,
            c0: 0.0,
            x00: 0.0,
            y00: 0.0,
            lambda00: 0.0,
            a00: 0.0,
            b00: 0.0,
            c00: 0.0,
            ring: false,
            ring_pending: false,
            first: false,
        }
    }

    fn line_start_mode(&mut self) {
        self.x0 = f64::NAN;
        self.mode = ResampleMode::Line;
        self.sink.line_start();
    }

    fn line_point(&mut self, lambda: f64, phi: f64) {
        let c = cartesian([lambda, phi]);
        let p = self.project.project(lambda, phi);
        let x1 = p[0];
        let y1 = p[1];
        if self.first {
            self.first = false;
            self.x00 = x1;
            self.y00 = y1;
            self.lambda00 = lambda;
            self.a00 = c[0];
            self.b00 = c[1];
            self.c00 = c[2];
        }
        if self.x0.is_nan() {
            self.sink.point(x1, y1, None);
        } else {
            let sink = self.sink.clone();
            resample_line_to(
                self.x0,
                self.y0,
                self.lambda0,
                self.a0,
                self.b0,
                self.c0,
                x1,
                y1,
                lambda,
                c[0],
                c[1],
                c[2],
                MAX_DEPTH,
                self.delta2,
                &self.project,
                &mut *sink.borrow(),
            );
            if self.delta2 == 0.0 {
                // resampleNone projects each point once
            }
            self.sink.point(x1, y1, None);
        }
        self.x0 = x1;
        self.y0 = y1;
        self.lambda0 = lambda;
        self.a0 = c[0];
        self.b0 = c[1];
        self.c0 = c[2];
    }
}

#[allow(clippy::too_many_arguments)]
fn resample_line_to(
    x0: f64,
    y0: f64,
    lambda0: f64,
    a0: f64,
    b0: f64,
    c0: f64,
    x1: f64,
    y1: f64,
    lambda1: f64,
    a1: f64,
    b1: f64,
    c1: f64,
    mut depth: i32,
    delta2: f64,
    project: &ProjectTransform,
    stream: &mut dyn Sink,
) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let d2 = dx * dx + dy * dy;
    if d2 > 4.0 * delta2 && depth > 0 {
        depth -= 1;
        let mut a = a0 + a1;
        let mut b = b0 + b1;
        let mut c = c0 + c1;
        let m = (a * a + b * b + c * c).sqrt();
        c /= m;
        let phi2 = asin(c);
        let lambda2 = if (c.abs() - 1.0).abs() < EPSILON || (lambda0 - lambda1).abs() < EPSILON {
            (lambda0 + lambda1) / 2.0
        } else {
            b.atan2(a)
        };
        let p = project.project(lambda2, phi2);
        let x2 = p[0];
        let y2 = p[1];
        let dx2 = x2 - x0;
        let dy2 = y2 - y0;
        let dz = dy * dx2 - dx * dy2;
        let cos_min_distance = (30.0 * RADIANS).cos();
        if dz * dz / d2 > delta2
            || ((dx * dx2 + dy * dy2) / d2 - 0.5).abs() > 0.3
            || a0 * a1 + b0 * b1 + c0 * c1 < cos_min_distance
        {
            a /= m;
            b /= m;
            resample_line_to(
                x0, y0, lambda0, a0, b0, c0, x2, y2, lambda2, a, b, c, depth, delta2, project,
                stream,
            );
            stream.point(x2, y2, None);
            resample_line_to(
                x2, y2, lambda2, a, b, c, x1, y1, lambda1, a1, b1, c1, depth, delta2, project,
                stream,
            );
        }
    }
}

impl Sink for ResampleStream {
    fn point(&mut self, x: f64, y: f64, _m: Option<f64>) {
        match self.mode {
            ResampleMode::Line => self.line_point(x, y),
            _ => {
                let p = self.project.project(x, y);
                self.sink.point(p[0], p[1], None);
            }
        }
    }
    fn line_start(&mut self) {
        self.line_start_mode();
        self.ring = std::mem::take(&mut self.ring_pending);
        self.first = self.ring;
    }
    fn line_end(&mut self) {
        if self.ring {
            let sink = self.sink.clone();
            resample_line_to(
                self.x0,
                self.y0,
                self.lambda0,
                self.a0,
                self.b0,
                self.c0,
                self.x00,
                self.y00,
                self.lambda00,
                self.a00,
                self.b00,
                self.c00,
                MAX_DEPTH,
                self.delta2,
                &self.project,
                &mut *sink.borrow(),
            );
            self.ring = false;
        }
        self.mode = ResampleMode::Point;
        self.sink.line_end();
    }
    fn polygon_start(&mut self) {
        self.sink.polygon_start();
        self.ring_pending = true;
        self.mode = ResampleMode::RingStart;
    }
    fn polygon_end(&mut self) {
        self.sink.polygon_end();
        self.mode = ResampleMode::Point;
    }
    fn sphere(&mut self) {
        self.sink.sphere();
    }
}

// ---------------------------------------------------------------------------
// Rectangle clipping (postclip / identity projections)
// ---------------------------------------------------------------------------

fn clip_segment(
    a: &mut [f64; 2],
    b: &mut [f64; 2],
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) -> bool {
    let (ax, ay) = (a[0], a[1]);
    let (bx, by) = (b[0], b[1]);
    let mut t0 = 0.0;
    let mut t1 = 1.0;
    let dx = bx - ax;
    let dy = by - ay;
    let mut r;

    r = x0 - ax;
    if dx == 0.0 {
        if r > 0.0 {
            return false;
        }
    } else {
        r /= dx;
        if dx < 0.0 {
            if r < t0 {
                return false;
            }
            if r < t1 {
                t1 = r;
            }
        } else if r > t1 {
            return false;
        } else if r > t0 {
            t0 = r;
        }
    }

    r = x1 - ax;
    if dx == 0.0 {
        if r < 0.0 {
            return false;
        }
    } else {
        r /= dx;
        if dx < 0.0 {
            if r > t1 {
                return false;
            }
            if r > t0 {
                t0 = r;
            }
        } else if r < t0 {
            return false;
        } else if r < t1 {
            t1 = r;
        }
    }

    r = y0 - ay;
    if dy == 0.0 {
        if r > 0.0 {
            return false;
        }
    } else {
        r /= dy;
        if dy < 0.0 {
            if r < t0 {
                return false;
            }
            if r < t1 {
                t1 = r;
            }
        } else if r > t1 {
            return false;
        } else if r > t0 {
            t0 = r;
        }
    }

    r = y1 - ay;
    if dy == 0.0 {
        if r < 0.0 {
            return false;
        }
    } else {
        r /= dy;
        if dy < 0.0 {
            if r > t1 {
                return false;
            }
            if r > t0 {
                t0 = r;
            }
        } else if r < t0 {
            return false;
        } else if r < t1 {
            t1 = r;
        }
    }

    if t0 > 0.0 {
        a[0] = ax + t0 * dx;
        a[1] = ay + t0 * dy;
    }
    if t1 < 1.0 {
        b[0] = ax + t1 * dx;
        b[1] = ay + t1 * dy;
    }
    true
}

pub struct RectangleClip {
    sink: SharedSink,
    buffer_raw: Rc<RefCell<ClipBuffer>>,
    buffer: SharedSink,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    in_polygon: bool,
    in_line: bool,
    polygon: Vec<Vec<[f64; 2]>>,
    ring: Vec<[f64; 2]>,
    segments: Vec<Vec<Vec<ClipPoint>>>,
    first: bool,
    x__: f64,
    y__: f64,
    v__: bool,
    x_: f64,
    y_: f64,
    v_: bool,
    clean: bool,
}

impl RectangleClip {
    pub fn new(sink: SharedSink, extent: [f64; 4]) -> Self {
        let buffer_raw = Rc::new(RefCell::new(ClipBuffer::new()));
        let buffer = SharedSink(Rc::new(RefCell::new(SharedSinkAdapter(buffer_raw.clone()))));
        Self {
            sink,
            buffer_raw,
            buffer,
            x0: extent[0],
            y0: extent[1],
            x1: extent[2],
            y1: extent[3],
            in_polygon: false,
            in_line: false,
            polygon: Vec::new(),
            ring: Vec::new(),
            segments: Vec::new(),
            first: true,
            x__: 0.0,
            y__: 0.0,
            v__: false,
            x_: f64::NAN,
            y_: f64::NAN,
            v_: false,
            clean: true,
        }
    }

    fn visible(&self, x: f64, y: f64) -> bool {
        self.x0 <= x && x <= self.x1 && self.y0 <= y && y <= self.y1
    }

    fn active_line_start(&mut self) {
        if self.in_polygon {
            self.buffer.line_start();
        } else {
            self.sink.line_start();
        }
    }

    fn active_point(&mut self, x: f64, y: f64) {
        if self.in_polygon {
            self.buffer.point(x, y, None);
        } else {
            self.sink.point(x, y, None);
        }
    }

    fn active_line_end(&mut self) {
        if self.in_polygon {
            self.buffer.line_end();
        } else {
            self.sink.line_end();
        }
    }

    fn line_point(&mut self, x: f64, y: f64) {
        let v = self.visible(x, y);
        if self.in_polygon {
            self.ring.push([x, y]);
        }
        if self.first {
            self.x__ = x;
            self.y__ = y;
            self.v__ = v;
            self.first = false;
            if v {
                self.active_line_start();
                self.active_point(x, y);
            }
        } else if v && self.v_ {
            self.active_point(x, y);
        } else {
            let clip_max = 1e9;
            let mut a = [self.x_.clamp(-clip_max, clip_max), self.y_.clamp(-clip_max, clip_max)];
            let mut b = [x.clamp(-clip_max, clip_max), y.clamp(-clip_max, clip_max)];
            if clip_segment(&mut a, &mut b, self.x0, self.y0, self.x1, self.y1) {
                if !self.v_ {
                    self.active_line_start();
                    self.active_point(a[0], a[1]);
                }
                self.active_point(b[0], b[1]);
                if !v {
                    self.active_line_end();
                }
                self.clean = false;
            } else if v {
                self.active_line_start();
                self.active_point(x, y);
                self.clean = false;
            }
        }
        self.x_ = x;
        self.y_ = y;
        self.v_ = v;
    }

    fn line_end(&mut self) {
        if self.in_polygon {
            let (x, y) = (self.x__, self.y__);
            self.line_point(x, y);
            if self.v__ && self.v_ {
                self.buffer_raw.borrow_mut().rejoin();
            }
            let lines = self.buffer_raw.borrow_mut().result();
            self.segments.push(lines);
        } else if self.v_ {
            self.sink.line_end();
        }
        self.in_line = false;
        self.x_ = f64::NAN;
        self.y_ = f64::NAN;
        self.v_ = false;
    }

    fn corner(&self, p: [f64; 2], direction: f64) -> i32 {
        if (p[0] - self.x0).abs() < EPSILON {
            if direction > 0.0 { 0 } else { 3 }
        } else if (p[0] - self.x1).abs() < EPSILON {
            if direction > 0.0 { 2 } else { 1 }
        } else if (p[1] - self.y0).abs() < EPSILON {
            if direction > 0.0 { 1 } else { 0 }
        } else if direction > 0.0 {
            3
        } else {
            2
        }
    }

    fn interpolate(
        &self,
        from: Option<[f64; 2]>,
        to: Option<[f64; 2]>,
        direction: f64,
        sink: &mut dyn Sink,
    ) {
        let step = |a: i32| (a + direction as i32 + 4) % 4;
        let emit = |a: i32, sink: &mut dyn Sink| {
            let x = if a == 0 || a == 3 { self.x0 } else { self.x1 };
            let y = if a > 1 { self.y1 } else { self.y0 };
            sink.point(x, y, None);
        };
        if let (Some(from), Some(to)) = (from, to) {
            let mut a = self.corner(from, direction);
            let target = self.corner(to, direction);
            let reverse = (self.compare_point(from, to) < 0.0) != (direction > 0.0);
            if a != target || reverse {
                loop {
                    emit(a, sink);
                    a = step(a);
                    if a == target {
                        break;
                    }
                }
            } else {
                sink.point(to[0], to[1], None);
            }
        } else {
            let mut a = 0;
            loop {
                emit(a, sink);
                a = step(a);
                if a == 0 {
                    break;
                }
            }
        }
    }

    fn compare_point(&self, a: [f64; 2], b: [f64; 2]) -> f64 {
        let ca = self.corner(a, 1.0);
        let cb = self.corner(b, 1.0);
        if ca != cb {
            (ca - cb) as f64
        } else if ca == 0 {
            b[1] - a[1]
        } else if ca == 1 {
            a[0] - b[0]
        } else if ca == 2 {
            a[1] - b[1]
        } else {
            b[0] - a[0]
        }
    }

    fn polygon_inside(&self) -> i32 {
        let mut winding = 0;
        for ring in &self.polygon {
            if ring.is_empty() {
                continue;
            }
            let mut a0 = ring[0][0];
            let mut a1 = ring[0][1];
            for point in ring.iter().skip(1) {
                let b0 = point[0];
                let b1 = point[1];
                if a1 <= self.y1 {
                    if b1 > self.y1 && (b0 - a0) * (self.y1 - a1) > (b1 - a1) * (self.x0 - a0) {
                        winding += 1;
                    }
                } else if b1 <= self.y1
                    && (b0 - a0) * (self.y1 - a1) < (b1 - a1) * (self.x0 - a0)
                {
                    winding -= 1;
                }
                a0 = b0;
                a1 = b1;
            }
        }
        winding
    }

    fn polygon_end(&mut self) {
        let start_inside = self.polygon_inside();
        let clean_inside = self.clean && start_inside != 0;
        let mut segments: Vec<Vec<ClipPoint>> = Vec::new();
        for group in self.segments.drain(..) {
            segments.extend(group);
        }
        let visible = !segments.is_empty();
        if clean_inside || visible {
            self.sink.polygon_start();
            if clean_inside {
                self.sink.line_start();
                let sink = self.sink.clone();
                self.interpolate(None, None, 1.0, &mut *sink.borrow());
                self.sink.line_end();
            }
            if visible {
                let mut compare = |a: &ClipIntersection, b: &ClipIntersection| {
                    compare_rect_intersection(a, b, self.x0, self.y0, self.x1, self.y1)
                };
                let mut interpolate = |from: Option<[f64; 2]>,
                                       to: Option<[f64; 2]>,
                                       direction: f64,
                                       sink: &mut dyn Sink| {
                    RectangleClip::interpolate(self, from, to, direction, sink)
                };
                let sink = self.sink.clone();
                clip_rejoin(
                    &segments,
                    start_inside != 0,
                    &mut interpolate,
                    &mut compare,
                    &mut *sink.borrow(),
                );
            }
            self.sink.polygon_end();
        }
        self.in_polygon = false;
    }
}

impl Sink for RectangleClip {
    fn point(&mut self, x: f64, y: f64, _m: Option<f64>) {
        if self.in_line {
            self.line_point(x, y);
        } else if self.visible(x, y) {
            if self.in_polygon {
                self.buffer.point(x, y, None);
            } else {
                self.sink.point(x, y, None);
            }
        }
    }
    fn line_start(&mut self) {
        if self.in_polygon {
            self.ring = Vec::new();
        }
        self.in_line = true;
        self.first = true;
        self.v_ = false;
        self.x_ = f64::NAN;
        self.y_ = f64::NAN;
    }
    fn line_end(&mut self) {
        RectangleClip::line_end(self);
    }
    fn polygon_start(&mut self) {
        self.in_polygon = true;
        self.segments.clear();
        self.polygon.clear();
        self.clean = true;
        self.ring.clear();
    }
    fn polygon_end(&mut self) {
        self.polygon_end();
    }
    fn sphere(&mut self) {
        self.sink.sphere();
    }
}

// ---------------------------------------------------------------------------
// Projections
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjKind {
    Albers,
    AlbersUsa,
    AzimuthalEqualArea,
    AzimuthalEquidistant,
    ConicConformal,
    ConicEqualArea,
    ConicEquidistant,
    EqualEarth,
    Equirectangular,
    Gnomonic,
    Identity,
    Mercator,
    NaturalEarth1,
    Orthographic,
    Stereographic,
    TransverseMercator,
}

#[derive(Clone)]
pub struct Projection {
    pub kind: ProjKind,
    pub scale: f64,
    pub translate: (f64, f64),
    pub center: (f64, f64),
    pub rotate: (f64, f64, f64),
    pub angle: f64,
    pub reflect_x: bool,
    pub reflect_y: bool,
    pub clip_angle: Option<f64>,
    pub clip_extent: Option<[f64; 4]>,
    pub precision: f64,
    pub parallels: (f64, f64),
}

impl Projection {
    pub fn from_type(name: &str) -> Option<Self> {
        let key = name.to_ascii_lowercase();
        let mut p = Self {
            kind: ProjKind::Mercator,
            scale: 150.0,
            translate: (480.0, 250.0),
            center: (0.0, 0.0),
            rotate: (0.0, 0.0, 0.0),
            angle: 0.0,
            reflect_x: false,
            reflect_y: false,
            clip_angle: None,
            clip_extent: None,
            precision: 0.5f64.sqrt(),
            parallels: (0.0, 60.0),
        };
        match key.as_str() {
            "albers" => {
                p.kind = ProjKind::Albers;
                p.parallels = (29.5, 45.5);
                p.scale = 1070.0;
                p.rotate = (96.0, 0.0, 0.0);
                p.center = (-0.6, 38.7);
            }
            "albersusa" => {
                p.kind = ProjKind::AlbersUsa;
                p.scale = 1070.0;
            }
            "azimuthalequalarea" => {
                p.kind = ProjKind::AzimuthalEqualArea;
                p.scale = 124.75;
                p.clip_angle = Some(180.0 - 1e-3);
            }
            "azimuthalequidistant" => {
                p.kind = ProjKind::AzimuthalEquidistant;
                p.scale = 79.4188;
                p.clip_angle = Some(180.0 - 1e-3);
            }
            "conicconformal" => {
                p.kind = ProjKind::ConicConformal;
                p.scale = 109.5;
                p.parallels = (30.0, 30.0);
            }
            "conicequalarea" => {
                p.kind = ProjKind::ConicEqualArea;
                p.scale = 155.424;
                p.center = (0.0, 33.6442);
            }
            "conicequidistant" => {
                p.kind = ProjKind::ConicEquidistant;
                p.scale = 131.154;
                p.center = (0.0, 13.9389);
            }
            "equalearth" => {
                p.kind = ProjKind::EqualEarth;
                p.scale = 177.158;
            }
            "equirectangular" => {
                p.kind = ProjKind::Equirectangular;
                p.scale = 152.63;
            }
            "gnomonic" => {
                p.kind = ProjKind::Gnomonic;
                p.scale = 144.049;
                p.clip_angle = Some(60.0);
            }
            "identity" => {
                p.kind = ProjKind::Identity;
                p.scale = 1.0;
                p.translate = (0.0, 0.0);
            }
            "mercator" => {
                p.kind = ProjKind::Mercator;
                p.scale = 961.0 / TAU;
            }
            "naturalearth1" => {
                p.kind = ProjKind::NaturalEarth1;
                p.scale = 175.295;
            }
            "orthographic" => {
                p.kind = ProjKind::Orthographic;
                p.scale = 249.5;
                p.clip_angle = Some(90.0 + EPSILON);
            }
            "stereographic" => {
                p.kind = ProjKind::Stereographic;
                p.scale = 250.0;
                p.clip_angle = Some(142.0);
            }
            "transversemercator" => {
                p.kind = ProjKind::TransverseMercator;
                p.scale = 159.155;
                p.rotate = (0.0, 0.0, 90.0);
            }
            _ => return None,
        }
        Some(p)
    }

    pub fn set_parallels(&mut self, value: &Value) {
        if let Value::Array(a) = value {
            let y0 = a.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
            let y1 = a.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
            self.parallels = (y0, y1);
        }
    }

    pub fn set_center(&mut self, value: &Value) {
        if let Value::Array(a) = value {
            self.center = (
                a.first().and_then(|v| v.as_f64()).unwrap_or(0.0),
                a.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0),
            );
        }
    }

    pub fn set_rotate(&mut self, value: &Value) {
        if let Value::Array(a) = value {
            self.rotate = (
                a.first().and_then(|v| v.as_f64()).unwrap_or(0.0),
                a.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0),
                a.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0),
            );
        }
    }

    pub fn set_translate(&mut self, value: &Value) {
        if let Value::Array(a) = value {
            self.translate = (
                a.first().and_then(|v| v.as_f64()).unwrap_or(0.0),
                a.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0),
            );
        }
    }

    pub fn set_clip_extent(&mut self, value: &Value) {
        if let Value::Array(a) = value {
            if let (Some(p0), Some(p1)) = (a.first().and_then(|v| v.as_array()), a.get(1).and_then(|v| v.as_array())) {
                self.clip_extent = Some([
                    p0.first().and_then(|v| v.as_f64()).unwrap_or(0.0),
                    p0.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0),
                    p1.first().and_then(|v| v.as_f64()).unwrap_or(0.0),
                    p1.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0),
                ]);
            }
        }
    }

    fn raw_project(&self, lambda: f64, phi: f64) -> [f64; 2] {
        match self.kind {
            ProjKind::Albers => conic_equal_area_raw(
                self.parallels.0 * RADIANS,
                self.parallels.1 * RADIANS,
                lambda,
                phi,
            ),
            ProjKind::ConicEqualArea => conic_equal_area_raw(
                self.parallels.0 * RADIANS,
                self.parallels.1 * RADIANS,
                lambda,
                phi,
            ),
            ProjKind::ConicConformal => conic_conformal_raw(
                self.parallels.0 * RADIANS,
                self.parallels.1 * RADIANS,
                lambda,
                phi,
            ),
            ProjKind::ConicEquidistant => conic_equidistant_raw(
                self.parallels.0 * RADIANS,
                self.parallels.1 * RADIANS,
                lambda,
                phi,
            ),
            ProjKind::AzimuthalEqualArea => azimuthal_raw(
                |c| (2.0 / (1.0 + c)).sqrt(),
                lambda,
                phi,
            ),
            ProjKind::AzimuthalEquidistant => azimuthal_raw(
                |c| {
                    let c = acos(c);
                    if c != 0.0 { c / c.sin() } else { 1.0 }
                },
                lambda,
                phi,
            ),
            ProjKind::EqualEarth => equal_earth_raw(lambda, phi),
            ProjKind::Equirectangular => [lambda, phi],
            ProjKind::Gnomonic => {
                let cy = phi.cos();
                let k = lambda.cos() * cy;
                [cy * lambda.sin() / k, phi.sin() / k]
            }
            ProjKind::Mercator => [lambda, ((HALF_PI + phi) / 2.0).tan().ln()],
            ProjKind::NaturalEarth1 => natural_earth1_raw(lambda, phi),
            ProjKind::Orthographic => [phi.cos() * lambda.sin(), phi.sin()],
            ProjKind::Stereographic => {
                let cy = phi.cos();
                let k = 1.0 + lambda.cos() * cy;
                [cy * lambda.sin() / k, phi.sin() / k]
            }
            ProjKind::TransverseMercator => [
                ((HALF_PI + phi) / 2.0).tan().ln(),
                -lambda,
            ],
            ProjKind::Identity | ProjKind::AlbersUsa => [lambda, phi],
        }
    }

    fn alpha_params(&self) -> (f64, f64) {
        if self.angle == 0.0 {
            (1.0, 0.0)
        } else {
            let a = self.angle * RADIANS;
            (a.cos(), a.sin())
        }
    }

    fn scale_translate_rotate(
        &self,
        k: f64,
        dx: f64,
        dy: f64,
        x: f64,
        y: f64,
    ) -> [f64; 2] {
        let sx = if self.reflect_x { -1.0 } else { 1.0 };
        let sy = if self.reflect_y { -1.0 } else { 1.0 };
        let mut x = x * sx;
        let mut y = y * sy;
        let (ca, sa) = self.alpha_params();
        if self.angle != 0.0 {
            let t = y * ca - x * sa;
            x = x * ca + y * sa;
            y = t;
        }
        [dx + k * x, dy - k * y]
    }

    fn project_transform(&self) -> ProjectTransform {
        let center = self.raw_project(self.center.0 * RADIANS, self.center.1 * RADIANS);
        let offset = self.scale_translate_rotate(self.scale, 0.0, 0.0, center[0], center[1]);
        ProjectTransform {
            projection: self.clone(),
            dx: self.translate.0 - offset[0],
            dy: self.translate.1 - offset[1],
        }
    }

    fn rotation(&self) -> Rotate {
        rotate_radians(
            self.rotate.0 * RADIANS,
            self.rotate.1 * RADIANS,
            self.rotate.2 * RADIANS,
        )
    }

    /// Projects a longitude/latitude pair (degrees). Returns `None` for points
    /// outside an albersUsa sub-region or an explicit clip extent.
    pub fn project(&self, lon: f64, lat: f64) -> Option<[f64; 2]> {
        match self.kind {
            ProjKind::Identity => {
                let x = lon;
                let y = lat;
                Some(self.identity_project(x, y))
            }
            ProjKind::AlbersUsa => self.albers_usa_project(lon, lat),
            _ => {
                let rotate = self.rotation();
                let p = rotate([lon * RADIANS, lat * RADIANS]);
                let t = self.project_transform();
                let xy = t.project(p[0], p[1]);
                if let Some(extent) = self.clip_extent {
                    if xy[0] < extent[0]
                        || xy[0] > extent[2]
                        || xy[1] < extent[1]
                        || xy[1] > extent[3]
                    {
                        return None;
                    }
                }
                Some(xy)
            }
        }
    }

    fn identity_project(&self, x: f64, y: f64) -> [f64; 2] {
        let sx = if self.reflect_x { -1.0 } else { 1.0 };
        let sy = if self.reflect_y { -1.0 } else { 1.0 };
        let mut x = x * self.scale * sx;
        let mut y = y * self.scale * sy;
        if self.angle != 0.0 {
            let a = self.angle * RADIANS;
            let t = y * a.cos() - x * a.sin();
            x = x * a.cos() + y * a.sin();
            y = t;
        }
        [x + self.translate.0, y + self.translate.1]
    }

    fn albers_usa_parts(&self) -> [Projection; 3] {
        let k = self.scale;
        let (x, y) = self.translate;
        let mut lower = Projection::from_type("albers").unwrap();
        lower.scale = k;
        lower.translate = (x, y);
        lower.clip_extent = Some([
            x - 0.455 * k,
            y - 0.238 * k,
            x + 0.455 * k,
            y + 0.238 * k,
        ]);

        let mut alaska = Projection::from_type("conicequalarea").unwrap();
        alaska.rotate = (154.0, 0.0, 0.0);
        alaska.center = (-2.0, 58.5);
        alaska.parallels = (55.0, 65.0);
        alaska.scale = k * 0.35;
        alaska.translate = (x - 0.307 * k, y + 0.201 * k);
        alaska.clip_extent = Some([
            x - 0.425 * k + EPSILON,
            y + 0.120 * k + EPSILON,
            x - 0.214 * k - EPSILON,
            y + 0.234 * k - EPSILON,
        ]);

        let mut hawaii = Projection::from_type("conicequalarea").unwrap();
        hawaii.rotate = (157.0, 0.0, 0.0);
        hawaii.center = (-3.0, 19.9);
        hawaii.parallels = (8.0, 18.0);
        hawaii.scale = k;
        hawaii.translate = (x - 0.205 * k, y + 0.212 * k);
        hawaii.clip_extent = Some([
            x - 0.214 * k + EPSILON,
            y + 0.166 * k + EPSILON,
            x - 0.115 * k - EPSILON,
            y + 0.234 * k - EPSILON,
        ]);
        [lower, alaska, hawaii]
    }

    fn albers_usa_project(&self, lon: f64, lat: f64) -> Option<[f64; 2]> {
        for part in self.albers_usa_parts() {
            if let Some(p) = part.project(lon, lat) {
                return Some(p);
            }
        }
        None
    }

    /// Returns a stream that writes projected geometry into `sink`.
    pub fn stream(&self, sink: SharedSink) -> Box<dyn Sink> {
        match self.kind {
            ProjKind::Identity => {
                let post: SharedSink = if let Some(extent) = self.clip_extent {
                    SharedSink::new(Box::new(RectangleClip::new(sink, extent)))
                } else {
                    sink
                };
                Box::new(IdentitySink {
                    sink: post,
                    projection: self.clone(),
                })
            }
            ProjKind::AlbersUsa => {
                let parts = self.albers_usa_parts();
                let sinks: Vec<SharedSink> = parts
                    .iter()
                    .map(|p| -> SharedSink {
                        SharedSink::new(p.stream(sink.clone()))
                    })
                    .collect();
                Box::new(MultiplexSink { sinks })
            }
            _ => {
                let post: SharedSink = if let Some(extent) = self.clip_extent {
                    SharedSink::new(Box::new(RectangleClip::new(sink, extent)))
                } else {
                    sink
                };
                let resample: SharedSink = SharedSink::new(Box::new(ResampleStream::new(
                    self.project_transform(),
                    post,
                    self.precision * self.precision,
                )));
                let pre: SharedSink = match self.clip_angle {
                    Some(angle) => SharedSink::new(Box::new(ClipStream::new(
                        ClipKind::Circle(angle * RADIANS),
                        resample,
                    ))),
                    None => SharedSink::new(Box::new(ClipStream::new(
                        ClipKind::Antimeridian,
                        resample,
                    ))),
                };
                let rotated: SharedSink = SharedSink::new(Box::new(RotateSink {
                    rotate: self.rotation(),
                    sink: pre,
                }));
                Box::new(RadiansSink { sink: rotated })
            }
        }
    }

    /// d3-geo `fitSize`: scale/translate the projection to fit `object` in size.
    pub fn fit_size(&mut self, size: [f64; 2], object: &Value) {
        self.fit_extent([[0.0, 0.0], size], object);
    }

    pub fn fit_extent(&mut self, extent: [[f64; 2]; 2], object: &Value) {
        let clip = self.clip_extent;
        let old_scale = self.scale;
        let old_translate = self.translate;
        self.scale = 150.0;
        self.translate = (0.0, 0.0);
        self.clip_extent = None;

        let bounds = Rc::new(RefCell::new(BoundsSink::new()));
        let handle = BoundsHandle(bounds.clone());
        let sink = SharedSink::new(Box::new(handle));
        {
            let mut stream = self.stream(sink.clone());
            stream_geometry(object, &mut *stream);
        }

        let mut b = bounds.borrow().result();
        // A few county polygons trigger the antimeridian clipper's complement
        // branch and contribute a full sphere to the bounds. Upstream d3 does
        // not inflate the fit for those degenerate rings; fall back to the raw
        // projected coordinate extent when the stream extent is the sphere.
        if object.get("features").is_some() {
            let sphere_bounds = {
                let sb = Rc::new(RefCell::new(BoundsSink::new()));
                let sh = BoundsHandle(sb.clone());
                let ssink = SharedSink::new(Box::new(sh));
                let mut sstream = self.stream(ssink);
                sstream.sphere();
                let result = sb.borrow().result();
                result
            };
            let close = |a: [[f64; 2]; 2], c: [[f64; 2]; 2]| {
                (a[0][0] - c[0][0]).abs() < 1e-6
                    && (a[0][1] - c[0][1]).abs() < 1e-6
                    && (a[1][0] - c[1][0]).abs() < 1e-6
                    && (a[1][1] - c[1][1]).abs() < 1e-6
            };
            if close(b, sphere_bounds) {
                let transform = self.project_transform();
                let mut raw = BoundsSink::new();
                raw_coordinate_bounds(object, &transform, &mut raw);
                let rb = raw.result();
                if rb[0][0].is_finite()
                    && rb[0][1].is_finite()
                    && rb[1][0].is_finite()
                    && rb[1][1].is_finite()
                {
                    b = rb;
                }
            }
        }

        let w = extent[1][0] - extent[0][0];
        let h = extent[1][1] - extent[0][1];
        let bw = b[1][0] - b[0][0];
        let bh = b[1][1] - b[0][1];
        if bw.is_finite() && bh.is_finite() && bw != 0.0 && bh != 0.0 {
            let k = (w / bw).min(h / bh);
            let x = extent[0][0] + (w - k * (b[1][0] + b[0][0])) / 2.0;
            let y = extent[0][1] + (h - k * (b[1][1] + b[0][1])) / 2.0;
            self.scale = 150.0 * k;
            self.translate = (x, y);
        } else {
            self.scale = old_scale;
            self.translate = old_translate;
        }
        self.clip_extent = clip;
    }
}

fn raw_coordinate_bounds(value: &Value, transform: &ProjectTransform, sink: &mut BoundsSink) {
    if let Some(features) = value.get("features").and_then(|f| f.as_array()) {
        for f in features {
            raw_coordinate_bounds(f, transform, sink);
        }
        return;
    }
    if let Some(geometries) = value.get("geometries").and_then(|g| g.as_array()) {
        for g in geometries {
            raw_coordinate_bounds(g, transform, sink);
        }
        return;
    }
    if let Some(geometry) = value.get("geometry") {
        if !geometry.is_null() {
            raw_coordinate_bounds(geometry, transform, sink);
        }
        return;
    }
    if let Some(coords) = value.get("coordinates") {
        raw_coord_array(coords, transform, sink);
    }
}

fn raw_coord_array(value: &Value, transform: &ProjectTransform, sink: &mut BoundsSink) {
    let Some(arr) = value.as_array() else { return };
    if arr.len() >= 2 && arr[0].is_number() && arr[1].is_number() {
        let lon = arr[0].as_f64().unwrap_or(0.0).to_radians();
        let lat = arr[1].as_f64().unwrap_or(0.0).to_radians();
        let p = transform.project(lon, lat);
        sink.point(p[0], p[1], None);
        return;
    }
    for item in arr {
        raw_coord_array(item, transform, sink);
    }
}

struct BoundsHandle(Rc<RefCell<BoundsSink>>);
impl Sink for BoundsHandle {
    fn point(&mut self, x: f64, y: f64, m: Option<f64>) {
        self.0.borrow_mut().point(x, y, m)
    }
    fn line_start(&mut self) {}
    fn line_end(&mut self) {}
    fn polygon_start(&mut self) {}
    fn polygon_end(&mut self) {}
    fn sphere(&mut self) {}
}

#[derive(Clone)]
pub struct ProjectTransform {
    projection: Projection,
    dx: f64,
    dy: f64,
}

impl ProjectTransform {
    pub fn project(&self, lambda: f64, phi: f64) -> [f64; 2] {
        let p = self.projection.raw_project(lambda, phi);
        self.projection
            .scale_translate_rotate(self.projection.scale, self.dx, self.dy, p[0], p[1])
    }
}

struct IdentitySink {
    sink: SharedSink,
    projection: Projection,
}
impl Sink for IdentitySink {
    fn point(&mut self, x: f64, y: f64, m: Option<f64>) {
        let p = self.projection.identity_project(x, y);
        self.sink.point(p[0], p[1], m);
    }
    fn line_start(&mut self) {
        self.sink.line_start()
    }
    fn line_end(&mut self) {
        self.sink.line_end()
    }
    fn polygon_start(&mut self) {
        self.sink.polygon_start()
    }
    fn polygon_end(&mut self) {
        self.sink.polygon_end()
    }
    fn sphere(&mut self) {
        self.sink.sphere()
    }
}

struct MultiplexSink {
    sinks: Vec<SharedSink>,
}
impl Sink for MultiplexSink {
    fn point(&mut self, x: f64, y: f64, m: Option<f64>) {
        for s in &mut self.sinks {
            s.point(x, y, m);
        }
    }
    fn line_start(&mut self) {
        for s in &mut self.sinks {
            s.line_start();
        }
    }
    fn line_end(&mut self) {
        for s in &mut self.sinks {
            s.line_end();
        }
    }
    fn polygon_start(&mut self) {
        for s in &mut self.sinks {
            s.polygon_start();
        }
    }
    fn polygon_end(&mut self) {
        for s in &mut self.sinks {
            s.polygon_end();
        }
    }
    fn sphere(&mut self) {
        for s in &mut self.sinks {
            s.sphere();
        }
    }
}

// ---------------------------------------------------------------------------
// Raw projection formulas
// ---------------------------------------------------------------------------

fn conic_equal_area_raw(y0: f64, y1: f64, x: f64, y: f64) -> [f64; 2] {
    let sy0 = y0.sin();
    let n = (sy0 + y1.sin()) / 2.0;
    if n.abs() < EPSILON {
        let cos_phi0 = y0.cos();
        return [x * cos_phi0, y.sin() / cos_phi0];
    }
    let c = 1.0 + sy0 * (2.0 * n - sy0);
    let r0 = c.sqrt() / n;
    let r = (c - 2.0 * n * y.sin()).sqrt() / n;
    let nx = x * n;
    [r * nx.sin(), r0 - r * nx.cos()]
}

fn conic_conformal_raw(y0: f64, y1: f64, x: f64, y: f64) -> [f64; 2] {
    let tany = |y: f64| ((HALF_PI + y) / 2.0).tan();
    let cy0 = y0.cos();
    let n = if y0 == y1 {
        y0.sin()
    } else {
        (cy0 / y1.cos()).ln() / (tany(y1) / tany(y0)).ln()
    };
    if !n.is_finite() || n == 0.0 {
        return [x, ((HALF_PI + y) / 2.0).tan().ln()];
    }
    let f = cy0 * tany(y0).powf(n) / n;
    let mut y = y;
    if f > 0.0 {
        if y < -HALF_PI + EPSILON {
            y = -HALF_PI + EPSILON;
        }
    } else if y > HALF_PI - EPSILON {
        y = HALF_PI - EPSILON;
    }
    let r = f / tany(y).powf(n);
    [r * (n * x).sin(), f - r * (n * x).cos()]
}

fn conic_equidistant_raw(y0: f64, y1: f64, x: f64, y: f64) -> [f64; 2] {
    let cy0 = y0.cos();
    let n = if y0 == y1 {
        y0.sin()
    } else {
        (cy0 - y1.cos()) / (y1 - y0)
    };
    if n.abs() < EPSILON {
        return [x, y];
    }
    let g = cy0 / n + y0;
    let gy = g - y;
    let nx = n * x;
    [gy * nx.sin(), g - gy * nx.cos()]
}

fn azimuthal_raw<F: Fn(f64) -> f64>(scale: F, x: f64, y: f64) -> [f64; 2] {
    let cx = x.cos();
    let cy = y.cos();
    let k = scale(cx * cy);
    if !k.is_finite() {
        return [2.0, 0.0];
    }
    [k * cy * x.sin(), k * y.sin()]
}

fn equal_earth_raw(lambda: f64, phi: f64) -> [f64; 2] {
    const A1: f64 = 1.340264;
    const A2: f64 = -0.081106;
    const A3: f64 = 0.000893;
    const A4: f64 = 0.003796;
    let m = 3.0f64.sqrt() / 2.0;
    let l = asin(m * phi.sin());
    let l2 = l * l;
    let l6 = l2 * l2 * l2;
    [
        lambda * l.cos() / (m * (A1 + 3.0 * A2 * l2 + l6 * (7.0 * A3 + 9.0 * A4 * l2))),
        l * (A1 + A2 * l2 + l6 * (A3 + A4 * l2)),
    ]
}

fn natural_earth1_raw(lambda: f64, phi: f64) -> [f64; 2] {
    let phi2 = phi * phi;
    let phi4 = phi2 * phi2;
    [
        lambda
            * (0.8707 - 0.131979 * phi2
                + phi4 * (-0.013791 + phi4 * (0.003971 * phi2 - 0.001529 * phi4))),
        phi * (1.007226
            + phi2 * (0.015085 + phi4 * (-0.044475 + 0.028874 * phi2 - 0.005916 * phi4))),
    ]
}

struct PathHandle(Rc<RefCell<PathString>>);
impl Sink for PathHandle {
    fn point(&mut self, x: f64, y: f64, m: Option<f64>) {
        self.0.borrow_mut().point(x, y, m)
    }
    fn line_start(&mut self) {
        self.0.borrow_mut().line_start()
    }
    fn line_end(&mut self) {
        self.0.borrow_mut().line_end()
    }
    fn polygon_start(&mut self) {
        self.0.borrow_mut().polygon_start()
    }
    fn polygon_end(&mut self) {
        self.0.borrow_mut().polygon_end()
    }
    fn sphere(&mut self) {
        self.0.borrow_mut().sphere()
    }
}

/// Generates the SVG path string for a GeoJSON object under `projection`.
pub fn project_path(
    projection: &Projection,
    object: &Value,
    point_radius: Option<f64>,
) -> String {
    let path = Rc::new(RefCell::new(PathString::new()));
    if let Some(radius) = point_radius {
        path.borrow_mut().point_radius(radius);
    }
    let handle = PathHandle(path.clone());
    let sink = SharedSink::new(Box::new(handle));
    let mut stream = projection.stream(sink.clone());
    stream_geometry(object, &mut *stream);
    let out = path.borrow_mut().result();
    out
}

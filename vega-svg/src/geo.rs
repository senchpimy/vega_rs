//! Spherical geometry helpers and GeoJSON streaming (port of `d3-geo`).
//!
//! Only the pieces needed by the static renderer are implemented: geometry
//! streaming, great-circle helpers, polygon containment and rotation. Cartographic
//! projections and clipping live in [`crate::projection`].

use serde_json::Value;

pub const EPSILON: f64 = 1e-6;
pub const EPSILON2: f64 = 1e-12;
pub const PI: f64 = std::f64::consts::PI;
pub const HALF_PI: f64 = PI / 2.0;
pub const QUARTER_PI: f64 = PI / 4.0;
pub const TAU: f64 = PI * 2.0;
pub const DEGREES: f64 = 180.0 / PI;
pub const RADIANS: f64 = PI / 180.0;

#[inline]
pub fn asin(x: f64) -> f64 {
    if x > 1.0 {
        HALF_PI
    } else if x < -1.0 {
        -HALF_PI
    } else {
        x.asin()
    }
}

#[inline]
pub fn acos(x: f64) -> f64 {
    if x > 1.0 {
        0.0
    } else if x < -1.0 {
        PI
    } else {
        x.acos()
    }
}

#[inline]
pub fn sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

pub type Cart = [f64; 3];

pub fn spherical(c: Cart) -> [f64; 2] {
    [c[1].atan2(c[0]), asin(c[2])]
}

pub fn cartesian(p: [f64; 2]) -> Cart {
    let (lambda, phi) = (p[0], p[1]);
    let cos_phi = phi.cos();
    [cos_phi * lambda.cos(), cos_phi * lambda.sin(), phi.sin()]
}

pub fn cartesian_dot(a: Cart, b: Cart) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cartesian_cross(a: Cart, b: Cart) -> Cart {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn cartesian_add_in_place(a: &mut Cart, b: Cart) {
    a[0] += b[0];
    a[1] += b[1];
    a[2] += b[2];
}

pub fn cartesian_scale(v: Cart, k: f64) -> Cart {
    [v[0] * k, v[1] * k, v[2] * k]
}

pub fn cartesian_normalize_in_place(d: &mut Cart) {
    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    d[0] /= l;
    d[1] /= l;
    d[2] /= l;
}

pub fn point_equal(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < EPSILON && (a[1] - b[1]).abs() < EPSILON
}

/// A GeoJSON consumer, mirroring d3-geo's stream interface.
pub trait Sink {
    fn point(&mut self, x: f64, y: f64, m: Option<f64>);
    fn line_start(&mut self);
    fn line_end(&mut self);
    fn polygon_start(&mut self);
    fn polygon_end(&mut self);
    fn sphere(&mut self);
}

impl Sink for Box<dyn Sink> {
    fn point(&mut self, x: f64, y: f64, m: Option<f64>) {
        (**self).point(x, y, m)
    }
    fn line_start(&mut self) {
        (**self).line_start()
    }
    fn line_end(&mut self) {
        (**self).line_end()
    }
    fn polygon_start(&mut self) {
        (**self).polygon_start()
    }
    fn polygon_end(&mut self) {
        (**self).polygon_end()
    }
    fn sphere(&mut self) {
        (**self).sphere()
    }
}

/// A sink that forwards everything to another stream.
#[derive(Clone)]
pub struct SharedSink(pub std::rc::Rc<std::cell::RefCell<dyn Sink>>);

impl SharedSink {
    pub fn new(sink: Box<dyn Sink>) -> Self {
        Self(std::rc::Rc::new(std::cell::RefCell::new(sink)))
    }
    pub fn borrow(&self) -> std::cell::RefMut<'_, dyn Sink> {
        self.0.borrow_mut()
    }
}

impl Sink for SharedSink {
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

/// Streams a GeoJSON object (geometry, Feature, FeatureCollection or GeometryCollection).
pub fn stream_geometry(geometry: &Value, sink: &mut dyn Sink) {
    let Some(kind) = geometry.get("type").and_then(|v| v.as_str()) else {
        return;
    };
    match kind {
        "Feature" => {
            if let Some(g) = geometry.get("geometry") {
                stream_geometry(g, sink);
            }
        }
        "FeatureCollection" => {
            if let Some(features) = geometry.get("features").and_then(|v| v.as_array()) {
                for feature in features {
                    stream_geometry(feature, sink);
                }
            }
        }
        "GeometryCollection" => {
            if let Some(gs) = geometry.get("geometries").and_then(|v| v.as_array()) {
                for g in gs {
                    stream_geometry(g, sink);
                }
            }
        }
        "Sphere" => sink.sphere(),
        "Point" => {
            if let Some(c) = geometry.get("coordinates").and_then(|v| v.as_array()) {
                sink.point(number(c, 0), number(c, 1), None);
            }
        }
        "MultiPoint" => {
            if let Some(coords) = geometry.get("coordinates").and_then(|v| v.as_array()) {
                for c in coords {
                    if let Some(c) = c.as_array() {
                        sink.point(number(c, 0), number(c, 1), None);
                    }
                }
            }
        }
        "LineString" => {
            if let Some(coords) = geometry.get("coordinates").and_then(|v| v.as_array()) {
                stream_line(coords, sink, false);
            }
        }
        "MultiLineString" => {
            if let Some(lines) = geometry.get("coordinates").and_then(|v| v.as_array()) {
                for line in lines {
                    if let Some(line) = line.as_array() {
                        stream_line(line, sink, false);
                    }
                }
            }
        }
        "Polygon" => {
            if let Some(rings) = geometry.get("coordinates").and_then(|v| v.as_array()) {
                stream_polygon(rings, sink);
            }
        }
        "MultiPolygon" => {
            if let Some(polys) = geometry.get("coordinates").and_then(|v| v.as_array()) {
                for poly in polys {
                    if let Some(poly) = poly.as_array() {
                        stream_polygon(poly, sink);
                    }
                }
            }
        }
        _ => {}
    }
}

fn number(values: &[Value], i: usize) -> f64 {
    values.get(i).and_then(|v| v.as_f64()).unwrap_or(0.0)
}

fn stream_line(coords: &[Value], sink: &mut dyn Sink, closed: bool) {
    let n = coords.len().saturating_sub(usize::from(closed));
    sink.line_start();
    for c in coords.iter().take(n) {
        if let Some(c) = c.as_array() {
            sink.point(number(c, 0), number(c, 1), None);
        }
    }
    sink.line_end();
}

fn stream_polygon(rings: &[Value], sink: &mut dyn Sink) {
    sink.polygon_start();
    for ring in rings {
        if let Some(ring) = ring.as_array() {
            stream_line(ring, sink, true);
        }
    }
    sink.polygon_end();
}

/// Generates a small circle, used by both the clip circle and graticule outline.
pub fn circle_stream(
    sink: &mut dyn Sink,
    radius: f64,
    delta: f64,
    direction: f64,
    t0: Option<[f64; 2]>,
    t1: Option<[f64; 2]>,
) {
    if delta == 0.0 {
        return;
    }
    let cos_radius = radius.cos();
    let sin_radius = radius.sin();
    let step = direction * delta;
    let (mut t0, t1) = match (t0, t1) {
        (Some(a), Some(b)) => {
            let mut a = circle_radius(cos_radius, a);
            let b = circle_radius(cos_radius, b);
            if if direction > 0.0 { a < b } else { a > b } {
                a += direction * TAU;
            }
            (a, b)
        }
        _ => (radius + direction * TAU, radius - step / 2.0),
    };
    while if direction > 0.0 { t0 > t1 } else { t0 < t1 } {
        let p = spherical([
            cos_radius,
            -sin_radius * t0.cos(),
            -sin_radius * t0.sin(),
        ]);
        sink.point(p[0], p[1], None);
        t0 -= step;
    }
}

fn circle_radius(cos_radius: f64, point: [f64; 2]) -> f64 {
    let mut point = cartesian(point);
    point[0] -= cos_radius;
    cartesian_normalize_in_place(&mut point);
    let radius = acos(-point[1]);
    let signed = if -point[2] < 0.0 { -radius } else { radius };
    (signed + TAU - EPSILON) % TAU
}

fn longitude(point: [f64; 2]) -> f64 {
    if point[0].abs() <= PI {
        point[0]
    } else {
        sign(point[0]) * ((point[0].abs() + PI) % TAU - PI)
    }
}

/// d3-geo's `polygonContains`, used to decide whether the south pole is in a
/// clipped polygon.
pub fn polygon_contains(polygon: &[Vec<[f64; 2]>], point: [f64; 2]) -> bool {
    let lambda = longitude(point);
    let mut phi = point[1];
    let sin_phi = phi.sin();
    let normal = [lambda.sin(), -lambda.cos(), 0.0];
    let mut angle = 0.0;
    let mut winding = 0i32;
    let mut sum = 0.0f64;

    if sin_phi == 1.0 {
        phi = HALF_PI + EPSILON;
    } else if sin_phi == -1.0 {
        phi = -HALF_PI - EPSILON;
    }

    for ring in polygon {
        let m = ring.len();
        if m == 0 {
            continue;
        }
        let mut point0 = ring[m - 1];
        let mut lambda0 = longitude(point0);
        let phi0 = point0[1] / 2.0 + QUARTER_PI;
        let mut sin_phi0 = phi0.sin();
        let mut cos_phi0 = phi0.cos();
        for j in 0..m {
            let point1 = ring[j];
            let lambda1 = longitude(point1);
            let phi1 = point1[1] / 2.0 + QUARTER_PI;
            let sin_phi1 = phi1.sin();
            let cos_phi1 = phi1.cos();
            let delta = lambda1 - lambda0;
            let sgn = if delta >= 0.0 { 1.0 } else { -1.0 };
            let abs_delta = sgn * delta;
            let antimeridian = abs_delta > PI;
            let k = sin_phi0 * sin_phi1;

            sum += (k * sgn * abs_delta.sin()).atan2(cos_phi0 * cos_phi1 + k * abs_delta.cos());
            angle += if antimeridian { delta + sgn * TAU } else { delta };

            if antimeridian ^ (lambda0 >= lambda) ^ (lambda1 >= lambda) {
                let mut arc = cartesian_cross(cartesian(point0), cartesian(point1));
                cartesian_normalize_in_place(&mut arc);
                let mut intersection = cartesian_cross(normal, arc);
                cartesian_normalize_in_place(&mut intersection);
                // JS `^` coerces booleans to integers; this mirrors it.
                let dir = if antimeridian ^ (delta >= 0.0) { -1.0 } else { 1.0 };
                let phi_arc = dir * asin(intersection[2]);
                if phi > phi_arc || (phi == phi_arc && (arc[0] != 0.0 || arc[1] != 0.0)) {
                    winding += if antimeridian ^ (delta >= 0.0) { 1 } else { -1 };
                }
            }

            point0 = point1;
            lambda0 = lambda1;
            sin_phi0 = sin_phi1;
            cos_phi0 = cos_phi1;
        }
    }

    ((angle < -EPSILON) || (angle < EPSILON && sum < -EPSILON2)) ^ (winding & 1 == 1)
}

/// Rotation in radians (`d3-geo` rotation).
pub type Rotate = Box<dyn Fn([f64; 2]) -> [f64; 2]>;

pub fn rotate_radians(delta_lambda: f64, delta_phi: f64, delta_gamma: f64) -> Rotate {
    let delta_lambda = delta_lambda % TAU;
    if delta_lambda != 0.0 {
        if delta_phi != 0.0 || delta_gamma != 0.0 {
            let a = rotation_lambda(delta_lambda);
            let b = rotation_phi_gamma(delta_phi, delta_gamma);
            return Box::new(move |p| {
                let p = a(p);
                b(p)
            });
        }
        return Box::new(rotation_lambda(delta_lambda));
    }
    if delta_phi != 0.0 || delta_gamma != 0.0 {
        return Box::new(rotation_phi_gamma(delta_phi, delta_gamma));
    }
    Box::new(|p| {
        let mut lambda = p[0];
        if lambda.abs() > PI {
            lambda -= (lambda / TAU).round() * TAU;
        }
        [lambda, p[1]]
    })
}

fn rotation_lambda(delta_lambda: f64) -> impl Fn([f64; 2]) -> [f64; 2] {
    move |p| {
        let mut lambda = p[0] + delta_lambda;
        if lambda.abs() > PI {
            lambda -= (lambda / TAU).round() * TAU;
        }
        [lambda, p[1]]
    }
}

fn rotation_phi_gamma(
    delta_phi: f64,
    delta_gamma: f64,
) -> impl Fn([f64; 2]) -> [f64; 2] {
    let cos_delta_phi = delta_phi.cos();
    let sin_delta_phi = delta_phi.sin();
    let cos_delta_gamma = delta_gamma.cos();
    let sin_delta_gamma = delta_gamma.sin();
    move |p| {
        let (lambda, phi) = (p[0], p[1]);
        let cos_phi = phi.cos();
        let x = lambda.cos() * cos_phi;
        let y = lambda.sin() * cos_phi;
        let z = phi.sin();
        let k = z * cos_delta_phi + x * sin_delta_phi;
        [
            (y * cos_delta_gamma - k * sin_delta_gamma)
                .atan2(x * cos_delta_phi - z * sin_delta_phi),
            asin(k * cos_delta_gamma + y * sin_delta_gamma),
        ]
    }
}

//! Color parsing, conversion, interpolation, and color scheme support.
//!
//! The public surface mirrors the color helpers Vega needs when rendering to
//! SVG: CSS color parsing, hex serialization, RGB interpolation, WCAG
//! luminance/contrast, and the `d3-scale-chromatic` style schemes.

use std::fmt::Write as _;

/// A parsed 8-bit RGB color.
#[derive(Clone, Copy, Debug)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

fn parse_hex(s: &str) -> Option<Rgb> {
    let digits = s.strip_prefix('#')?;
    if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&digits[i..i + 1], 16).ok();
    let pair = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
    match digits.len() {
        3 => Some(Rgb::new(byte(0)? * 17, byte(1)? * 17, byte(2)? * 17)),
        6 | 8 => Some(Rgb::new(pair(0)?, pair(2)?, pair(4)?)),
        _ => None,
    }
}

/// Parses a single `rgb()`/`rgba()` channel, accepting both integers and
/// percentages (or floats for the modern syntax).
fn parse_rgb_channel(token: &str) -> Option<u8> {
    let (value, percent) = match token.strip_suffix('%') {
        Some(v) => (v.parse::<f64>().ok()?, true),
        None => (token.parse::<f64>().ok()?, false),
    };
    let scaled = if percent {
        value / 100.0 * 255.0
    } else {
        value
    };
    Some(scaled.round().clamp(0.0, 255.0) as u8)
}

/// Parses a saturation/lightness component as a fraction in `[0, 1]`.
fn parse_fraction(token: &str) -> Option<f64> {
    let value = match token.strip_suffix('%') {
        Some(v) => v.parse::<f64>().ok()? / 100.0,
        None => token.parse::<f64>().ok()? / 100.0,
    };
    Some(value.clamp(0.0, 1.0))
}

/// Parses an angle in degrees, accepting `deg`, `grad`, `rad`, and `turn`.
fn parse_angle(token: &str) -> Option<f64> {
    let token = token.trim();
    let (value, unit) = if let Some(v) = token.strip_suffix("deg") {
        (v, 1.0)
    } else if let Some(v) = token.strip_suffix("grad") {
        (v, 360.0 / 400.0)
    } else if let Some(v) = token.strip_suffix("rad") {
        (v, 180.0 / std::f64::consts::PI)
    } else if let Some(v) = token.strip_suffix("turn") {
        (v, 360.0)
    } else {
        (token, 1.0)
    };
    Some(value.trim().parse::<f64>().ok()? * unit)
}

fn hue_to_rgb(p: f64, q: f64, mut t: f64) -> f64 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        p + (q - p) * 6.0 * t
    } else if t < 0.5 {
        q
    } else if t < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - t) * 6.0
    } else {
        p
    }
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> Rgb {
    let h = h.rem_euclid(360.0) / 360.0;
    if s <= 0.0 {
        let v = (l * 255.0).round().clamp(0.0, 255.0) as u8;
        return Rgb::new(v, v, v);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let channel = |t: f64| (hue_to_rgb(p, q, t) * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgb::new(channel(h + 1.0 / 3.0), channel(h), channel(h - 1.0 / 3.0))
}

fn parse_functional(s: &str) -> Option<Rgb> {
    let open = s.find('(')?;
    let close = s.rfind(')')?;
    if close < open {
        return None;
    }
    let name = s[..open].trim().to_ascii_lowercase();
    let inner = &s[open + 1..close];
    // Drop the alpha component written with a slash (`rgb(1 2 3 / 50%)`).
    let inner = inner.split('/').next().unwrap_or(inner);
    let parts: Vec<&str> = inner
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() < 3 {
        return None;
    }
    match name.as_str() {
        "rgb" | "rgba" => Some(Rgb::new(
            parse_rgb_channel(parts[0])?,
            parse_rgb_channel(parts[1])?,
            parse_rgb_channel(parts[2])?,
        )),
        "hsl" | "hsla" => Some(hsl_to_rgb(
            parse_angle(parts[0])?,
            parse_fraction(parts[1])?,
            parse_fraction(parts[2])?,
        )),
        _ => None,
    }
}

fn parse_named(s: &str) -> Option<Rgb> {
    let (r, g, b) = match s {
        "black" => (0, 0, 0),
        "white" => (255, 255, 255),
        "gray" | "grey" => (128, 128, 128),
        "lightgray" | "lightgrey" => (211, 211, 211),
        "darkgray" | "darkgrey" => (169, 169, 169),
        "silver" => (192, 192, 192),
        "red" => (255, 0, 0),
        "green" => (0, 128, 0),
        "blue" => (0, 0, 255),
        "orange" => (255, 165, 0),
        "yellow" => (255, 255, 0),
        "purple" => (128, 0, 128),
        "pink" => (255, 192, 203),
        "brown" => (165, 42, 42),
        "cyan" => (0, 255, 255),
        "magenta" => (255, 0, 255),
        "lime" => (0, 255, 0),
        "navy" => (0, 0, 128),
        "teal" => (0, 128, 128),
        "olive" => (128, 128, 0),
        "maroon" => (128, 0, 0),
        "aqua" => (0, 255, 255),
        "fuchsia" => (255, 0, 255),
        "darkgreen" => (0, 100, 0),
        "darkblue" => (0, 0, 139),
        "darkred" => (139, 0, 0),
        "royalblue" => (65, 105, 225),
        "steelblue" => (70, 130, 180),
        "tomato" => (255, 99, 71),
        "gold" => (255, 215, 0),
        _ => return None,
    };
    Some(Rgb::new(r, g, b))
}

/// Parses a CSS color string.
///
/// Supports `#rgb`, `#rrggbb`, `#rrggbbaa` (alpha ignored), `rgb()`/`rgba()`,
/// `hsl()`/`hsla()`, and a set of common CSS color names. `none` and
/// `transparent` (and anything unrecognized) return `None`.
pub fn parse_color(s: &str) -> Option<Rgb> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let lower = s.to_ascii_lowercase();
    match lower.as_str() {
        "none" | "transparent" => return None,
        _ => {}
    }
    if s.starts_with('#') {
        parse_hex(s)
    } else if lower.contains('(') {
        parse_functional(&lower)
    } else {
        parse_named(&lower)
    }
}

/// Serializes an RGB color as `#rrggbb`.
pub fn rgb_to_hex(c: Rgb) -> String {
    let mut out = String::with_capacity(7);
    out.push('#');
    let _ = write!(out, "{:02x}{:02x}{:02x}", c.r, c.g, c.b);
    out
}

// ---------------------------------------------------------------------------
// Interpolation, luminance, contrast
// ---------------------------------------------------------------------------

fn lerp_channel(a: u8, b: u8, t: f64) -> u8 {
    let v = a as f64 + (b as f64 - a as f64) * t;
    v.round().clamp(0.0, 255.0) as u8
}

/// Linearly interpolates two RGB colors; `t` is clamped to `[0, 1]`.
pub fn interpolate_rgb(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
    Rgb::new(
        lerp_channel(a.r, b.r, t),
        lerp_channel(a.g, b.g, t),
        lerp_channel(a.b, b.b, t),
    )
}


#[derive(Clone, Copy)]
struct Hcl {
    h: f64,
    c: f64,
    l: f64,
}

fn d3_rgb2lrgb(channel: u8) -> f64 {
    let x = channel as f64 / 255.0;
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

fn rgb_to_hcl(rgb: Rgb) -> Hcl {
    let r = d3_rgb2lrgb(rgb.r);
    let g = d3_rgb2lrgb(rgb.g);
    let b = d3_rgb2lrgb(rgb.b);
    let y = xyz_to_lab((0.2225045 * r + 0.7168786 * g + 0.0606169 * b) / 1.0);
    let (x, z) = if rgb.r == rgb.g && rgb.g == rgb.b {
        (y, y)
    } else {
        (
            xyz_to_lab((0.4360747 * r + 0.3850649 * g + 0.1430804 * b) / 0.96422),
            xyz_to_lab((0.0139322 * r + 0.0971045 * g + 0.7141733 * b) / 0.82521),
        )
    };
    let l = 116.0 * y - 16.0;
    let a = 500.0 * (x - y);
    let bb = 200.0 * (y - z);
    if a == 0.0 && bb == 0.0 {
        Hcl {
            h: f64::NAN,
            c: if l > 0.0 && l < 100.0 { 0.0 } else { f64::NAN },
            l,
        }
    } else {
        let mut h = bb.atan2(a).to_degrees();
        if h < 0.0 {
            h += 360.0;
        }
        Hcl {
            h,
            c: (a * a + bb * bb).sqrt(),
            l,
        }
    }
}

fn xyz_to_lab(t: f64) -> f64 {
    const T0: f64 = 4.0 / 29.0;
    const T1: f64 = 6.0 / 29.0;
    const T2: f64 = 3.0 * T1 * T1;
    const T3: f64 = T1 * T1 * T1;
    if t > T3 {
        t.powf(1.0 / 3.0)
    } else {
        t / T2 + T0
    }
}

fn lab_to_xyz(t: f64) -> f64 {
    const T0: f64 = 4.0 / 29.0;
    const T1: f64 = 6.0 / 29.0;
    const T2: f64 = 3.0 * T1 * T1;
    if t > T1 {
        t * t * t
    } else {
        T2 * (t - T0)
    }
}

fn delinearize(x: f64) -> u8 {
    let v = if x <= 0.0031308 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    };
    (255.0 * v).round().clamp(0.0, 255.0) as u8
}

fn hcl_to_rgb(hcl: Hcl) -> Rgb {
    let h = if hcl.h.is_nan() { 0.0 } else { hcl.h.to_radians() };
    let a = h.cos() * hcl.c;
    let b = h.sin() * hcl.c;
    let y = (hcl.l + 16.0) / 116.0;
    let x = if a.is_nan() { y } else { y + a / 500.0 };
    let z = if b.is_nan() { y } else { y - b / 200.0 };
    let x = 0.96422 * lab_to_xyz(x);
    let y = lab_to_xyz(y);
    let z = 0.82521 * lab_to_xyz(z);
    Rgb::new(
        delinearize(3.1338561 * x - 1.6168667 * y - 0.4906146 * z),
        delinearize(-0.9787684 * x + 1.9161415 * y + 0.0334540 * z),
        delinearize(0.0719453 * x - 0.2289914 * y + 1.4052427 * z),
    )
}

/// Interpolates in CIELAB / HCL space, matching `d3-interpolate`'s `hcl`.
pub fn interpolate_hcl(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
    let a = rgb_to_hcl(a);
    let b = rgb_to_hcl(b);
    let mut dh = b.h - a.h;
    if dh > 180.0 || dh < -180.0 {
        dh -= 360.0 * (dh / 360.0).round();
    }
    let h = if dh == 0.0 {
        if a.h.is_nan() { b.h } else { a.h }
    } else {
        a.h + t * dh
    };
    let c = a.c + (b.c - a.c) * t;
    let l = a.l + (b.l - a.l) * t;
    hcl_to_rgb(Hcl { h, c, l })
}

/// Interpolates two CSS colors and returns `#rrggbb`. Falls back to `a`
/// verbatim when either color cannot be parsed.
pub fn interpolate_color(a: &str, b: &str, t: f64) -> String {
    match (parse_color(a), parse_color(b)) {
        (Some(a), Some(b)) => rgb_to_hex(interpolate_rgb(a, b, t)),
        _ => a.to_string(),
    }
}

fn linearize(channel: u8) -> f64 {
    let c = channel as f64 / 255.0;
    if c <= 0.039_28 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG 2.x relative luminance of an sRGB color, in `[0, 1]`.
pub fn luminance(c: Rgb) -> f64 {
    0.2126 * linearize(c.r) + 0.7152 * linearize(c.g) + 0.0722 * linearize(c.b)
}

/// WCAG contrast ratio between two colors, in `[1, 21]`.
pub fn contrast(a: Rgb, b: Rgb) -> f64 {
    let la = luminance(a);
    let lb = luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

// ---------------------------------------------------------------------------
// Schemes
// ---------------------------------------------------------------------------

const TABLEAU10: &[&str] = &[
    "#4c78a8", "#f58518", "#e45756", "#72b7b2", "#54a24b", "#eeca3b", "#b279a2", "#ff9da6",
    "#9d755d", "#bab0ac",
];

const TABLEAU20: &[&str] = &[
    "#4c78a8", "#9ecae9", "#f58518", "#ffbf79", "#54a24b", "#88d27a", "#b79a20", "#f2cf5b",
    "#439894", "#83bcb6", "#e45756", "#ff9d98", "#79706e", "#bab0ac", "#d67195", "#fcbfd2",
    "#b279a2", "#d6a5c9", "#9e765f", "#d8b5a5",
];

const ACCENT: &[&str] = &[
    "#7fc97f", "#beaed4", "#fdc086", "#ffff99", "#386cb0", "#f0027f", "#bf5b17", "#666666",
];

const DARK2: &[&str] = &[
    "#1b9e77", "#d95f02", "#7570b3", "#e7298a", "#66a61e", "#e6ab02", "#a6761d", "#666666",
];

const PAIRED: &[&str] = &[
    "#a6cee3", "#1f78b4", "#b2df8a", "#33a02c", "#fb9a99", "#e31a1c", "#fdbf6f", "#ff7f00",
    "#cab2d6", "#6a3d9a", "#ffff99", "#b15928",
];

const SET1: &[&str] = &[
    "#e41a1c", "#377eb8", "#4daf4a", "#984ea3", "#ff7f00", "#ffff33", "#a65628", "#f781bf",
    "#999999",
];

const SET2: &[&str] = &[
    "#66c2a5", "#fc8d62", "#8da0cb", "#e78ac3", "#a6d854", "#ffd92f", "#e5c494", "#b3b3b3",
];

const SET3: &[&str] = &[
    "#8dd3c7", "#ffffb3", "#bebada", "#fb8072", "#80b1d3", "#fdb462", "#b3de69", "#fccde5",
    "#d9d9d9", "#bc80bd", "#ccebc5", "#ffed6f",
];

const BLUES: &[&str] = &[
    "#f7fbff", "#deebf7", "#c6dbef", "#9ecae1", "#6baed6", "#4292c6", "#2171b5", "#08519c",
    "#08306b",
];

const GREENS: &[&str] = &[
    "#f7fcf5", "#e5f5e0", "#c7e9c0", "#a1d99b", "#74c476", "#41ab5d", "#238b45", "#006d2c",
    "#00441b",
];

const GREYS: &[&str] = &[
    "#ffffff", "#f0f0f0", "#d9d9d9", "#bdbdbd", "#969696", "#737373", "#525252", "#252525",
    "#000000",
];

const ORANGES: &[&str] = &[
    "#fff5eb", "#fee6ce", "#fdd0a2", "#fdae6b", "#fd8d3c", "#f16913", "#d94801", "#a63603",
    "#7f2704",
];

const REDS: &[&str] = &[
    "#fff5f0", "#fee0d2", "#fcbba1", "#fc9272", "#fb6a4a", "#ef3b2c", "#cb181d", "#a50f15",
    "#67000d",
];

const PURPLES: &[&str] = &[
    "#fcfbfd", "#efedf5", "#dadaeb", "#bcbddc", "#9e9ac8", "#807dba", "#6a51a3", "#54278f",
    "#3f007d",
];

const VIRIDIS: &[&str] = &[
    "#440154", "#414487", "#2a788e", "#22a884", "#7ad151", "#fde725",
];

const MAGMA: &[&str] = &[
    "#000004", "#3b0f70", "#8c2981", "#de4968", "#fe9f6d", "#fcfdbf",
];

const INFERNO: &[&str] = &[
    "#000004", "#420a68", "#932667", "#dd513a", "#fca50a", "#fcffa4",
];

const PLASMA: &[&str] = &[
    "#0d0887", "#6a00a8", "#b12a90", "#e16462", "#fca636", "#f0f921",
];

const CIVIDIS: &[&str] = &[
    "#00204c", "#31446b", "#666870", "#958f78", "#c6b985", "#fee838",
];

const TURBO: &[&str] = &[
    "#30123b", "#4145ab", "#4675ed", "#39a2fc", "#1bcfd4", "#24eca6", "#61fc6c", "#a4fc3b",
    "#d1e834", "#f3c63a", "#fe9b2d", "#f36315", "#cb2a04", "#7a0403",
];

const YELLOW_GREEN_BLUE: &[&str] = &[
    "#ffffd9", "#edf8b1", "#c7e9b4", "#7fcdbb", "#41b6c4", "#1d91c0", "#225ea8", "#253494",
    "#081d58",
];

const BLUE_ORANGE: &[&str] = &[
    "#2166ac", "#67a9cf", "#d1e5f0", "#f7f7f7", "#fee0b6", "#f1a340", "#b35806",
];

const BLUE_GREEN: &[&str] = &[
    "#f7fcfd", "#e0f3db", "#ccebc5", "#a8ddb5", "#7bccc4", "#4eb3d3", "#2b8cbe", "#0868ac",
    "#084081",
];

const RED_BLUE: &[&str] = &[
    "#67001f", "#b2182b", "#d6604d", "#f4a582", "#fddbc7", "#f7f7f7", "#d1e5f0", "#92c5de",
    "#4393c3", "#2166ac", "#053061",
];

const RED_GREY: &[&str] = &[
    "#67001f", "#b2182b", "#d6604d", "#f4a582", "#fddbc7", "#ffffff", "#e0e0e0", "#bababa",
    "#878787", "#4d4d4d", "#1a1a1a",
];

const PURPLE_GREEN: &[&str] = &[
    "#40004b", "#762a83", "#9970ab", "#c2a5cf", "#e7d4e8", "#f7f7f7", "#d9f0d3", "#a6dba0",
    "#5aae61", "#1b7837", "#00441b",
];

const SPECTRAL: &[&str] = &[
    "#9e0142", "#d53e4f", "#f46d43", "#fdae61", "#fee08b", "#ffffbf", "#e6f598", "#abdda4",
    "#66c2a5", "#3288bd", "#5e4fa2",
];

#[derive(Clone, Copy)]
struct Scheme {
    colors: &'static [&'static str],
    continuous: bool,
}

fn normalize_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn scheme_entry(name: &str) -> Option<Scheme> {
    let key = normalize_name(name);
    let (colors, continuous): (&'static [&'static str], bool) = match key.as_str() {
        "tableau10" | "tableau" | "category" | "category10" | "cat10" | "d3category10" => {
            (TABLEAU10, false)
        }
        "tableau20" | "category20" | "cat20" | "d3category20" => (TABLEAU20, false),
        "accent" => (ACCENT, false),
        "dark2" | "dark" => (DARK2, false),
        "paired" => (PAIRED, false),
        "set1" => (SET1, false),
        "set2" => (SET2, false),
        "set3" => (SET3, false),
        "blues" | "blue" | "blues9" => (BLUES, true),
        "greens" | "green" | "greens9" => (GREENS, true),
        "greys" | "grays" | "grey" | "gray" | "greys9" | "grays9" => (GREYS, true),
        "oranges" | "orange" | "oranges9" => (ORANGES, true),
        "reds" | "red" | "reds9" => (REDS, true),
        "purples" | "purple" | "purples9" => (PURPLES, true),
        "viridis" => (VIRIDIS, true),
        "magma" => (MAGMA, true),
        "inferno" => (INFERNO, true),
        "plasma" => (PLASMA, true),
        "cividis" => (CIVIDIS, true),
        "turbo" => (TURBO, true),
        "yellowgreenblue" | "ylgnbu" | "yellowgreenblues" => (YELLOW_GREEN_BLUE, true),
        "blueorange" | "buor" | "blueoranges" => (BLUE_ORANGE, true),
        "bluegreen" | "bugn" | "bluegreens" => (BLUE_GREEN, true),
        "redblue" | "rdbu" | "redblues" => (RED_BLUE, true),
        "redgrey" | "redgray" | "rdgy" | "redgreys" | "redgrays" => (RED_GREY, true),
        "purplegreen" | "prgn" | "purplegreens" => (PURPLE_GREEN, true),
        "spectral" | "rdylbu" | "redyellowblue" => (SPECTRAL, true),
        _ => return None,
    };
    Some(Scheme { colors, continuous })
}

fn sample_continuous(colors: &[&str], t: f64) -> Option<String> {
    if colors.is_empty() {
        return None;
    }
    let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
    if colors.len() == 1 {
        return parse_color(colors[0]).map(rgb_to_hex);
    }
    let pos = t * (colors.len() - 1) as f64;
    let idx = (pos.floor() as usize).min(colors.len() - 1);
    let frac = pos - idx as f64;
    let a = parse_color(colors[idx])?;
    let b = parse_color(colors[(idx + 1).min(colors.len() - 1)])?;
    Some(rgb_to_hex(interpolate_rgb(a, b, frac)))
}

fn sample_discrete(colors: &[&str], t: f64) -> Option<String> {
    if colors.is_empty() {
        return None;
    }
    let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
    let idx = ((t * colors.len() as f64).floor() as usize).min(colors.len() - 1);
    parse_color(colors[idx]).map(rgb_to_hex)
}

/// Returns the discrete color list for a named scheme.
///
/// Categorical schemes (`tableau10`/`category`/`category10`, `accent`, ...)
/// return their fixed palettes. Sequential and diverging schemes are sampled
/// into 10 steps. Returns `None` for unknown names.
pub fn scheme(name: &str) -> Option<Vec<String>> {
    let entry = scheme_entry(name)?;
    if entry.colors.is_empty() {
        return Some(Vec::new());
    }
    if entry.continuous {
        Some(continuous_scheme(name, 10))
    } else {
        Some(entry.colors.iter().map(|c| c.to_string()).collect())
    }
}

/// Samples a continuous scheme at `t` in `[0, 1]` (clamped) as `#rrggbb`.
///
/// For categorical schemes the nearest palette entry is returned. Returns
/// `None` for unknown names.
pub fn interpolate_scheme(name: &str, t: f64) -> Option<String> {
    let entry = scheme_entry(name)?;
    if entry.continuous {
        sample_continuous(entry.colors, t)
    } else {
        sample_discrete(entry.colors, t)
    }
}

/// Samples `n` evenly spaced colors from a named scheme.
///
/// Unknown names (and `n == 0`) produce an empty vector.
pub fn continuous_scheme(name: &str, n: usize) -> Vec<String> {
    if n == 0 {
        return Vec::new();
    }
    let entry = match scheme_entry(name) {
        Some(e) => e,
        None => return Vec::new(),
    };
    if entry.continuous {
        (0..n)
            .filter_map(|i| {
                let t = if n == 1 {
                    0.5
                } else {
                    i as f64 / (n - 1) as f64
                };
                sample_continuous(entry.colors, t)
            })
            .collect()
    } else {
        (0..n)
            .map(|i| {
                let idx = if n == 1 {
                    entry.colors.len() / 2
                } else {
                    i * (entry.colors.len().saturating_sub(1)) / (n - 1).max(1)
                };
                entry.colors[idx.min(entry.colors.len() - 1)].to_string()
            })
            .collect()
    }
}

/// The Vega `range.symbol` shape list.
pub fn symbol_scheme() -> Vec<String> {
    [
        "circle",
        "square",
        "triangle-up",
        "cross",
        "diamond",
        "triangle-right",
        "triangle-down",
        "triangle-left",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn tuple(c: Rgb) -> (u8, u8, u8) {
        (c.r, c.g, c.b)
    }

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn parses_hex_forms() {
        assert_eq!(tuple(parse_color("#f00").unwrap()), (255, 0, 0));
        assert_eq!(tuple(parse_color("#00FF00").unwrap()), (0, 255, 0));
        assert_eq!(tuple(parse_color("#0000ff80").unwrap()), (0, 0, 255));
        assert!(parse_color("#12").is_none());
        assert!(parse_color("#ggg").is_none());
        assert!(parse_color("#1234567").is_none());
    }

    #[test]
    fn parses_functional_colors() {
        assert_eq!(tuple(parse_color("rgb(255, 0, 0)").unwrap()), (255, 0, 0));
        assert_eq!(tuple(parse_color("rgb(0 128 255)").unwrap()), (0, 128, 255));
        assert_eq!(
            tuple(parse_color("rgba(0,128,255,0.5)").unwrap()),
            (0, 128, 255)
        );
        assert_eq!(
            tuple(parse_color("rgb(100%, 0%, 0%)").unwrap()),
            (255, 0, 0)
        );
        assert_eq!(
            tuple(parse_color("rgb(255 0 0 / 50%)").unwrap()),
            (255, 0, 0)
        );
        assert_eq!(
            tuple(parse_color("hsl(0, 100%, 50%)").unwrap()),
            (255, 0, 0)
        );
        assert_eq!(
            tuple(parse_color("hsla(120, 100%, 50%, 0.3)").unwrap()),
            (0, 255, 0)
        );
        assert_eq!(
            tuple(parse_color("hsl(240deg, 100%, 50%)").unwrap()),
            (0, 0, 255)
        );
        assert_eq!(
            tuple(parse_color("hsl(0.5turn, 100%, 50%)").unwrap()),
            (0, 255, 255)
        );
        assert!(parse_color("rgb(1, 2)").is_none());
        assert!(parse_color("rgb(a, b, c)").is_none());
    }

    #[test]
    fn parses_named_colors() {
        assert_eq!(tuple(parse_color("black").unwrap()), (0, 0, 0));
        assert_eq!(tuple(parse_color("White").unwrap()), (255, 255, 255));
        assert_eq!(tuple(parse_color("grey").unwrap()), (128, 128, 128));
        assert_eq!(tuple(parse_color("lightgray").unwrap()), (211, 211, 211));
        assert_eq!(tuple(parse_color("steelblue").unwrap()), (70, 130, 180));
        assert_eq!(tuple(parse_color("tomato").unwrap()), (255, 99, 71));
        assert!(parse_color("none").is_none());
        assert!(parse_color("transparent").is_none());
        assert!(parse_color("not-a-color").is_none());
        assert!(parse_color("").is_none());
    }

    #[test]
    fn serializes_hex() {
        assert_eq!(rgb_to_hex(Rgb::new(255, 0, 0)), "#ff0000");
        assert_eq!(rgb_to_hex(Rgb::new(0, 128, 255)), "#0080ff");
        assert_eq!(rgb_to_hex(parse_color("#abc").unwrap()), "#aabbcc");
    }

    #[test]
    fn interpolates_rgb() {
        let black = Rgb::new(0, 0, 0);
        let white = Rgb::new(255, 255, 255);
        assert_eq!(tuple(interpolate_rgb(black, white, 0.0)), (0, 0, 0));
        assert_eq!(tuple(interpolate_rgb(black, white, 1.0)), (255, 255, 255));
        assert_eq!(tuple(interpolate_rgb(black, white, 0.5)), (128, 128, 128));
        // t is clamped.
        assert_eq!(tuple(interpolate_rgb(black, white, -5.0)), (0, 0, 0));
        assert_eq!(tuple(interpolate_rgb(black, white, 5.0)), (255, 255, 255));
    }

    #[test]
    fn interpolates_css_colors() {
        assert_eq!(interpolate_color("red", "blue", 0.0), "#ff0000");
        assert_eq!(interpolate_color("red", "blue", 1.0), "#0000ff");
        assert_eq!(interpolate_color("#000", "#fff", 0.5), "#808080");
        assert_eq!(interpolate_color("bogus", "#fff", 0.5), "bogus");
        assert_eq!(interpolate_color("#000", "bogus", 0.5), "#000");
    }

    #[test]
    fn computes_luminance_and_contrast() {
        assert!(approx(luminance(Rgb::new(0, 0, 0)), 0.0));
        assert!(approx(luminance(Rgb::new(255, 255, 255)), 1.0));
        assert!(approx(luminance(Rgb::new(255, 0, 0)), 0.2126));
        let c = contrast(Rgb::new(255, 255, 255), Rgb::new(0, 0, 0));
        assert!((c - 21.0).abs() < 1e-6);
        assert!(approx(
            contrast(Rgb::new(255, 255, 255), Rgb::new(255, 255, 255)),
            1.0
        ));
        // Symmetric.
        let a = Rgb::new(10, 200, 30);
        let b = Rgb::new(240, 20, 90);
        assert!(approx(contrast(a, b), contrast(b, a)));
    }

    #[test]
    fn provides_categorical_schemes() {
        let cat = scheme("category").unwrap();
        assert_eq!(cat.len(), 10);
        assert_eq!(cat, scheme("tableau10").unwrap());
        assert_eq!(cat, scheme("category10").unwrap());
        assert_eq!(scheme("tableau20").unwrap().len(), 20);
        assert_eq!(scheme("accent").unwrap().len(), 8);
        assert_eq!(scheme("dark2").unwrap().len(), 8);
        assert_eq!(scheme("paired").unwrap().len(), 12);
        assert_eq!(scheme("set1").unwrap().len(), 9);
        assert_eq!(scheme("set2").unwrap().len(), 8);
        assert_eq!(scheme("set3").unwrap().len(), 12);
        assert!(scheme("no-such-scheme").is_none());
    }

    #[test]
    fn provides_continuous_schemes() {
        for name in [
            "blues",
            "greens",
            "greys",
            "grays",
            "oranges",
            "reds",
            "purples",
            "viridis",
            "magma",
            "inferno",
            "plasma",
            "cividis",
            "turbo",
            "yellowgreenblue",
            "blueorange",
            "bluegreen",
            "redblue",
            "redgrey",
            "purplegreen",
            "spectral",
            "rdylbu",
        ] {
            let cols = scheme(name).unwrap_or_else(|| panic!("missing scheme {name}"));
            assert_eq!(cols.len(), 10, "scheme {name} should have 10 steps");
            assert!(cols.iter().all(|c| parse_color(c).is_some()));
            // Sequential/diverging schemes should not be flat.
            assert!(
                cols.first() != cols.last(),
                "scheme {name} endpoints should differ"
            );
            let start = interpolate_scheme(name, 0.0).unwrap();
            let end = interpolate_scheme(name, 1.0).unwrap();
            assert_eq!(start, cols[0]);
            assert_eq!(end, cols[9]);
        }
        assert!(interpolate_scheme("nope", 0.5).is_none());
    }

    #[test]
    fn interpolates_schemes_clamped() {
        assert_eq!(
            interpolate_scheme("blues", -1.0),
            interpolate_scheme("blues", 0.0)
        );
        assert_eq!(
            interpolate_scheme("blues", 2.0),
            interpolate_scheme("blues", 1.0)
        );
        // Categorical schemes return a palette entry, not a blend.
        let mid = interpolate_scheme("tableau10", 0.5).unwrap();
        assert!(scheme("tableau10").unwrap().contains(&mid));
    }

    #[test]
    fn samples_continuous_scheme() {
        assert_eq!(continuous_scheme("viridis", 5).len(), 5);
        assert_eq!(continuous_scheme("viridis", 1).len(), 1);
        assert!(continuous_scheme("viridis", 0).is_empty());
        assert!(continuous_scheme("unknown", 5).is_empty());
        assert_eq!(continuous_scheme("tableau10", 3).len(), 3);
    }

    #[test]
    fn symbolic_scheme_matches_vega() {
        let symbols = symbol_scheme();
        assert_eq!(symbols.len(), 8);
        assert_eq!(symbols[0], "circle");
        assert_eq!(symbols[1], "square");
        assert_eq!(symbols[7], "triangle-left");
    }
}

//! Bounds calculation matching `vega-scenegraph`'s headless text estimator and
//! guide layout. Used to size the SVG view and to place axes/legends.

/// Advance width of one character in thousandths of an em.
///
/// Approximates the default `sans-serif` (Noto Sans / Liberation Sans) that
/// browsers use for Vega's SVG output. Vega's headless fallback (`0.8 * length
/// * fontSize`) is much wider than the real font and makes label overlap
/// culling hide labels that browsers keep.
fn char_width(c: char) -> f64 {
    match c {
        ' ' => 260.,
        '\t' => 520.,
        '0'..='9' => 572.,
        'a' => 561.,
        'b' | 'd' | 'g' | 'h' | 'n' | 'p' | 'q' | 'u' => 588.,
        'c' => 508.,
        'e' => 553.,
        'f' | 't' => 330.,
        'i' | 'j' | 'l' => 232.,
        'k' => 545.,
        'm' => 874.,
        'o' => 580.,
        'r' => 383.,
        's' => 481.,
        'v' | 'y' => 518.,
        'w' => 780.,
        'x' => 519.,
        'z' => 480.,
        'A' => 641.,
        'B' => 641.,
        'C' => 646.,
        'D' => 675.,
        'E' => 556.,
        'F' => 531.,
        'G' => 676.,
        'H' => 699.,
        'I' => 298.,
        'J' => 478.,
        'K' => 627.,
        'L' => 516.,
        'M' => 885.,
        'N' => 714.,
        'O' => 729.,
        'P' => 605.,
        'Q' => 729.,
        'R' => 610.,
        'S' => 570.,
        'T' => 571.,
        'U' => 698.,
        'V' => 641.,
        'W' => 990.,
        'X' => 614.,
        'Y' => 592.,
        'Z' => 556.,
        '.' | ',' | ':' | ';' => 273.,
        '-' => 339.,
        '–' | '—' => 500.,
        '…' => 900.,
        '(' | ')' | '[' | ']' => 320.,
        '/' | '\\' => 400.,
        '+' | '=' | '<' | '>' => 600.,
        '%' => 813.,
        '$' => 572.,
        '#' => 620.,
        '?' => 500.,
        '!' => 300.,
        '\'' => 200.,
        '"' => 340.,
        '\n' => 0.,
        _ => 520.,
    }
}

/// Estimated rendered text width. JavaScript's `text.length` counts UTF-16
/// code units; we use per-character advances of the browser's default sans
/// font so guide layout matches the SVG rendered in a browser.
pub fn text_width(text: &str, font_size: f64) -> f64 {
    let em: f64 = text.chars().map(char_width).sum();
    // Slight calibration against canvas `measureText` for the default sans
    // font (our per-character advances round a little high).
    (em * font_size * 0.985 / 1000.0).round()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Bounds {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl Bounds {
    pub fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Self { x1, y1, x2, y2 }
    }
    pub fn empty() -> Self {
        Self {
            x1: f64::INFINITY,
            y1: f64::INFINITY,
            x2: f64::NEG_INFINITY,
            y2: f64::NEG_INFINITY,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.x1 > self.x2 || self.y1 > self.y2
    }
    pub fn add(&mut self, x: f64, y: f64) {
        self.x1 = self.x1.min(x);
        self.y1 = self.y1.min(y);
        self.x2 = self.x2.max(x);
        self.y2 = self.y2.max(y);
    }
    pub fn union(&mut self, b: Bounds) {
        if b.is_empty() {
            return;
        }
        self.add(b.x1, b.y1);
        self.add(b.x2, b.y2);
    }
    /// Restrict this bounds to the intersection with `b` (for clipped marks).
    pub fn intersect(&mut self, b: Bounds) {
        if b.is_empty() || self.is_empty() {
            *self = Bounds::empty();
            return;
        }
        self.x1 = self.x1.max(b.x1);
        self.y1 = self.y1.max(b.y1);
        self.x2 = self.x2.min(b.x2);
        self.y2 = self.y2.min(b.y2);
        if self.x1 > self.x2 || self.y1 > self.y2 {
            *self = Bounds::empty();
        }
    }
    pub fn translate(&self, dx: f64, dy: f64) -> Bounds {
        Bounds::new(self.x1 + dx, self.y1 + dy, self.x2 + dx, self.y2 + dy)
    }
    pub fn width(&self) -> f64 {
        self.x2 - self.x1
    }
    pub fn height(&self) -> f64 {
        self.y2 - self.y1
    }
}

/// Port of vega-scenegraph's `offset()` baseline correction.
pub fn baseline_offset(baseline: Option<&str>, font_size: f64) -> f64 {
    let h = font_size;
    let v = match baseline {
        Some("top") => 0.79 * h,
        Some("middle") => 0.30 * h,
        Some("bottom") => -0.21 * h,
        Some("line-top") => 0.29 * h + 0.5 * (h + 2.0),
        Some("line-bottom") => 0.29 * h - 0.5 * (h + 2.0),
        _ => 0.0,
    };
    v.round()
}

/// Port of the text mark bound calculation in `vega-scenegraph/src/marks/text.js`.
#[allow(clippy::too_many_arguments)]
pub fn text_bounds(
    x: f64,
    y: f64,
    text: &str,
    font_size: f64,
    align: Option<&str>,
    baseline: Option<&str>,
    dx: f64,
    dy: f64,
    angle: f64,
) -> Bounds {
    let h = font_size;
    let w = text_width(text, h);
    let off = baseline_offset(baseline, h);
    let mut bx = x + dx;
    let mut by = y + dy + off - (0.8 * h).round();
    match align {
        Some("center") => bx -= w / 2.0,
        Some("right") | Some("end") => bx -= w,
        _ => {}
    }
    let mut b = Bounds::new(bx, by, bx + w, by + h);
    if angle != 0.0 {
        let rot = angle.to_radians();
        let (sin, cos) = rot.sin_cos();
        let corners = [(b.x1, b.y1), (b.x2, b.y1), (b.x2, b.y2), (b.x1, b.y2)];
        let mut out = Bounds::empty();
        for (cx, cy) in corners {
            let rx = x + (cx - x) * cos - (cy - y) * sin;
            let ry = y + (cx - x) * sin + (cy - y) * cos;
            out.add(rx, ry);
        }
        b = out;
    }
    let _ = &mut by;
    b
}

/// Bounds of a rule/tick segment including a centered stroke.
pub fn rule_bounds(x1: f64, y1: f64, x2: f64, y2: f64, stroke_width: f64) -> Bounds {
    let h = stroke_width / 2.0;
    Bounds::new(
        x1.min(x2) - h,
        y1.min(y2) - h,
        x1.max(x2) + h,
        y1.max(y2) + h,
    )
}

/// Conservative bounds for an SVG path string. Handles the commands emitted by
/// Vega's path generators (including arcs and cubic/quadratic curves) by
/// including control points and arc bounding boxes.
pub fn path_bounds(d: &str) -> Bounds {
    let mut b = Bounds::empty();
    let mut cx = 0.0;
    let mut cy = 0.0;
    let mut sx = 0.0;
    let mut sy = 0.0;
    let mut cmd = ' ';
    let mut tokens: Vec<String> = Vec::new();
    let chars: Vec<char> = d.chars().collect();
    let mut k = 0;
    while k < chars.len() {
        let c = chars[k];
        if c.is_whitespace() || c == ',' {
            k += 1;
        } else if c.is_ascii_alphabetic() {
            tokens.push(c.to_string());
            k += 1;
        } else if c.is_ascii_digit() || c == '-' || c == '+' || c == '.' {
            let start = k;
            k += 1;
            while k < chars.len() {
                let ch = chars[k];
                if ch.is_ascii_digit() || ch == '.' {
                    k += 1;
                } else if (ch == 'e' || ch == 'E')
                    && k + 1 < chars.len()
                    && (chars[k + 1].is_ascii_digit()
                        || chars[k + 1] == '+'
                        || chars[k + 1] == '-')
                {
                    k += 2;
                } else {
                    break;
                }
            }
            tokens.push(chars[start..k].iter().collect());
        } else {
            k += 1;
        }
    }
    let mut i = 0;
    let num = |s: &str| s.parse::<f64>().ok();
    while i < tokens.len() {
        let token = tokens[i].as_str();
        let first = token.chars().next().unwrap_or(' ');
        if first.is_ascii_alphabetic() {
            cmd = first;
            i += 1;
            if cmd == 'Z' || cmd == 'z' {
                cx = sx;
                cy = sy;
                b.add(cx, cy);
                continue;
            }
        }
        let relative = cmd.is_ascii_lowercase();
        let upper = cmd.to_ascii_uppercase();
        let arg = |i: &mut usize| -> f64 {
            let v = tokens.get(*i).and_then(|s| num(s.as_str())).unwrap_or(0.0);
            *i += 1;
            v
        };
        match upper {
            'M' => {
                let x = arg(&mut i);
                let y = arg(&mut i);
                cx = if relative { cx + x } else { x };
                cy = if relative { cy + y } else { y };
                sx = cx;
                sy = cy;
                b.add(cx, cy);
                cmd = if relative { 'l' } else { 'L' };
            }
            'L' => {
                let x = arg(&mut i);
                let y = arg(&mut i);
                cx = if relative { cx + x } else { x };
                cy = if relative { cy + y } else { y };
                b.add(cx, cy);
            }
            'H' => {
                let x = arg(&mut i);
                cx = if relative { cx + x } else { x };
                b.add(cx, cy);
            }
            'V' => {
                let y = arg(&mut i);
                cy = if relative { cy + y } else { y };
                b.add(cx, cy);
            }
            'C' => {
                let x1 = arg(&mut i);
                let y1 = arg(&mut i);
                let x2 = arg(&mut i);
                let y2 = arg(&mut i);
                let x = arg(&mut i);
                let y = arg(&mut i);
                b.add(cx + x1, cy + y1);
                b.add(cx + x2, cy + y2);
                cx = if relative { cx + x } else { x };
                cy = if relative { cy + y } else { y };
                b.add(cx, cy);
            }
            'S' | 'Q' => {
                let x1 = arg(&mut i);
                let y1 = arg(&mut i);
                let x = arg(&mut i);
                let y = arg(&mut i);
                b.add(cx + x1, cy + y1);
                cx = if relative { cx + x } else { x };
                cy = if relative { cy + y } else { y };
                b.add(cx, cy);
            }
            'T' => {
                let x = arg(&mut i);
                let y = arg(&mut i);
                cx = if relative { cx + x } else { x };
                cy = if relative { cy + y } else { y };
                b.add(cx, cy);
            }
            'A' => {
                let rx = arg(&mut i).abs();
                let ry = arg(&mut i).abs();
                let _rot = arg(&mut i);
                let _large = arg(&mut i);
                let _sweep = arg(&mut i);
                let x = arg(&mut i);
                let y = arg(&mut i);
                let nx = if relative { cx + x } else { x };
                let ny = if relative { cy + y } else { y };
                b.add(cx - rx, cy - ry);
                b.add(cx + rx, cy + ry);
                b.add(nx - rx, ny - ry);
                b.add(nx + rx, ny + ry);
                cx = nx;
                cy = ny;
            }
            _ => {
                i += 1;
            }
        }
    }
    b
}

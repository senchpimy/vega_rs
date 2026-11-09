//! Number, date and generic value formatting for Vega/Vega-Lite.
//!
//! This module implements a useful subset of `d3-format` and `d3-time-format`
//! without pulling in any dependency other than `serde_json` and `std`.
//!
//! Numbers use the familiar d3 format spec grammar:
//!
//! ```text
//! [[fill]align][sign][symbol][0][width][,][.precision][~][type]
//! ```
//!
//! Dates are always formatted and parsed in UTC so the output is deterministic
//! regardless of the host time zone.

use serde_json::Value;

// ---------------------------------------------------------------------------
// Number formatting
// ---------------------------------------------------------------------------

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

const MONTHS_SHORT: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

const WEEKDAYS_SHORT: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

const SI_SUFFIXES: [&str; 17] = [
    "y", "z", "a", "f", "p", "n", "µ", "m", "", "k", "M", "G", "T", "P", "E", "Z", "Y",
];

/// Offset of the empty suffix inside [`SI_SUFFIXES`].
const SI_ZERO: i32 = 8;

/// d3-format's default minus sign (U+2212).
const MINUS: &str = "\u{2212}";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Align {
    Left,
    Right,
    Center,
    Numeric,
}

#[derive(Clone)]
struct FormatSpec {
    fill: char,
    align: Option<Align>,
    sign: Option<char>,
    symbol: bool,
    zero: bool,
    width: Option<usize>,
    grouping: bool,
    precision: Option<usize>,
    trim: bool,
    ty: Option<char>,
    /// True when the original spec contained no characters at all.
    empty: bool,
}

fn is_align(c: char) -> bool {
    matches!(c, '<' | '>' | '^' | '=')
}

fn parse_spec(spec: &str) -> FormatSpec {
    let chars: Vec<char> = spec.chars().collect();
    let len = chars.len();
    let mut i = 0usize;
    let mut out = FormatSpec {
        fill: ' ',
        align: None,
        sign: None,
        symbol: false,
        zero: false,
        width: None,
        grouping: false,
        precision: None,
        trim: false,
        ty: None,
        empty: len == 0,
    };

    // [[fill]align]
    if len >= 2 && is_align(chars[1]) {
        out.fill = chars[0];
        out.align = Some(match chars[1] {
            '<' => Align::Left,
            '>' => Align::Right,
            '^' => Align::Center,
            _ => Align::Numeric,
        });
        i = 2;
    } else if i < len && is_align(chars[i]) {
        out.align = Some(match chars[i] {
            '<' => Align::Left,
            '>' => Align::Right,
            '^' => Align::Center,
            _ => Align::Numeric,
        });
        i += 1;
    }

    // [sign]
    if i < len && matches!(chars[i], '+' | '-' | '(') {
        out.sign = Some(chars[i]);
        i += 1;
    }

    // [symbol]
    if i < len && chars[i] == '$' {
        out.symbol = true;
        i += 1;
    }

    // [0]
    if i < len && chars[i] == '0' {
        out.zero = true;
        i += 1;
    }

    // [width]
    let width_start = i;
    while i < len && chars[i].is_ascii_digit() {
        i += 1;
    }
    if i > width_start {
        let text: String = chars[width_start..i].iter().collect();
        out.width = text.parse::<usize>().ok();
    }

    // [,]
    if i < len && chars[i] == ',' {
        out.grouping = true;
        i += 1;
    }

    // [.precision]
    if i < len && chars[i] == '.' {
        i += 1;
        let prec_start = i;
        while i < len && chars[i].is_ascii_digit() {
            i += 1;
        }
        if i > prec_start {
            let text: String = chars[prec_start..i].iter().collect();
            out.precision = text.parse::<usize>().ok();
        } else {
            out.precision = Some(0);
        }
    }

    // [~]
    if i < len && chars[i] == '~' {
        out.trim = true;
        i += 1;
    }

    // [type]
    if i < len {
        let c = chars[i];
        if matches!(
            c,
            'd' | 'f' | 'e' | 'g' | 'n' | 's' | '%' | 'b' | 'o' | 'x' | 'X' | 'r' | 'p' | 'c' | 'a'
        ) {
            out.ty = Some(c);
            i += 1;
        }
    }

    let _ = i;
    out
}

/// Format a number using a d3-format style specification.
///
/// ```text
/// format_number(1234.5678, ",.2f") == "1,234.57"
/// format_number(0.1234, ".1%")     == "12.3%"
/// format_number(1500.0, ".2~s")    == "1.5k"
/// ```
pub fn format_number(value: f64, spec: &str) -> String {
    format_number_spec(value, &parse_spec(spec))
}

fn format_number_spec(value: f64, spec: &FormatSpec) -> String {
    let mut fs = spec.clone();

    if value.is_nan() {
        return pad_number("NaN", "", "", &fs, false);
    }
    if value.is_infinite() {
        let sign = if value.is_sign_negative() { MINUS } else { "" };
        return pad_number("Infinity", sign, "", &fs, false);
    }

    let negative = value.is_sign_negative() && value != 0.0;
    let abs = value.abs();

    // d3-format treats a missing (or invalid) type as ".12~g".
    if fs.ty.is_none() {
        if fs.precision.is_none() {
            fs.precision = Some(12);
        }
        fs.trim = true;
        fs.ty = Some('g');
    }
    let ty = fs.ty.unwrap_or('g');
    let _ = fs.empty;

    match ty {
        'd' => {
            let digits = fixed(abs, 0);
            finish(abs, negative, digits, "", "", &fs, false)
        }
        'b' => {
            let digits = format!("{:b}", abs.round() as i64);
            finish(abs, negative, digits, "", "", &fs, false)
        }
        'o' => {
            let digits = format!("{:o}", abs.round() as i64);
            finish(abs, negative, digits, "", "", &fs, false)
        }
        'x' => {
            let digits = format!("{:x}", abs.round() as i64);
            finish(abs, negative, digits, "", "", &fs, false)
        }
        'X' => {
            let digits = format!("{:X}", abs.round() as i64);
            finish(abs, negative, digits, "", "", &fs, false)
        }
        'f' => {
            let prec = fs.precision.unwrap_or(6);
            let digits = fixed(abs, prec);
            finish(abs, negative, digits, "", "", &fs, false)
        }
        '%' => {
            let prec = fs.precision.unwrap_or(6);
            let digits = fixed(abs * 100.0, prec);
            finish(abs, negative, digits, "", "%", &fs, false)
        }
        'e' => {
            let prec = fs.precision.unwrap_or(6);
            let digits = scientific(abs, prec, fs.trim);
            finish(abs, negative, digits, "", "", &fs, false)
        }
        's' => {
            let prec = fs.precision.unwrap_or(6);
            let (scaled, suffix) = si_scale(abs);
            let mut digits = significant_plain(scaled, prec, fs.trim);
            // Rounding can push the mantissa to 1000 (e.g. 999_999 -> "1000k");
            // promote to the next suffix when possible.
            if let Ok(v) = digits.parse::<f64>() {
                if v.abs() >= 1000.0 {
                    if let Some(next) = next_si_suffix(suffix) {
                        let rescale = abs / 1000f64.powi(si_power(next));
                        digits = significant_plain(rescale, prec, fs.trim);
                        return finish(abs, negative, digits, "", next, &fs, false);
                    }
                }
            }
            finish(abs, negative, digits, "", suffix, &fs, false)
        }
        'n' => {
            let prec = fs.precision.unwrap_or(6);
            let digits = general(abs, prec, fs.trim);
            let grouped = group(&digits);
            finish(abs, negative, grouped, "", "", &fs, true)
        }
        'g' => {
            let prec = fs.precision.unwrap_or(6);
            let digits = general(abs, prec, fs.trim);
            finish(abs, negative, digits, "", "", &fs, false)
        }
        'r' => {
            let prec = fs.precision.unwrap_or(6);
            let digits = significant_plain(abs, prec, false);
            finish(abs, negative, digits, "", "", &fs, false)
        }
        'p' => {
            let prec = fs.precision.unwrap_or(6);
            let digits = significant_plain(abs * 100.0, prec, false);
            finish(abs, negative, digits, "", "%", &fs, false)
        }
        'c' => finish(abs, negative, format!("{}", abs), "", "", &fs, false),
        _ => {
            let prec = fs.precision.unwrap_or(6);
            let digits = general(abs, prec, fs.trim);
            finish(abs, negative, digits, "", "", &fs, false)
        }
    }
}

/// Port of vega-format's `formatSpan` for axis labels.
///
/// Chooses a precision from the tick step when the caller did not specify one.
pub fn format_span(value: f64, start: f64, stop: f64, count: f64, spec: &str) -> String {
    let fs = parse_spec(spec);
    if fs.precision.is_some() {
        return format_number_spec(value, &fs);
    }
    let step = d3_tick_step(start, stop, count);
    let max = start.abs().max(stop.abs());
    let ty = fs.ty.unwrap_or(' ');
    match ty {
        's' => {
            let exponent = decimal_exponent(max);
            let e = (exponent as f64 / 3.0).floor() as i32;
            let e = e.clamp(-8, 8) * 3;
            let scaled = value / 10f64.powi(e);
            let precision = (e - decimal_exponent(step.abs())).max(0) as usize;
            let mut inner =
                format_number_spec(scaled, &with_type_precision(&fs, 'f', precision));
            inner.push_str(si_suffix(e));
            inner
        }
        ' ' | 'e' | 'g' | 'p' | 'r' => {
            let precision = precision_round(step, max) - if ty == 'e' { 1 } else { 0 };
            format_number_spec(value, &with_precision(&fs, precision.max(0) as usize))
        }
        'f' | '%' => {
            let precision = precision_fixed(step) - if ty == '%' { 2 } else { 0 };
            format_number_spec(value, &with_precision(&fs, precision.max(0) as usize))
        }
        _ => format_number_spec(value, &fs),
    }
}

fn with_precision(fs: &FormatSpec, precision: usize) -> FormatSpec {
    let mut out = fs.clone();
    out.precision = Some(precision.min(20));
    out
}

fn with_type_precision(fs: &FormatSpec, ty: char, precision: usize) -> FormatSpec {
    let mut out = with_precision(fs, precision);
    out.ty = Some(ty);
    out
}

fn decimal_exponent(value: f64) -> i32 {
    if value == 0.0 || !value.is_finite() {
        0
    } else {
        value.abs().log10().floor() as i32
    }
}

fn precision_fixed(step: f64) -> i32 {
    (-decimal_exponent(step.abs())).max(0)
}

fn precision_round(step: f64, max: f64) -> i32 {
    let step = step.abs();
    let max = max.abs() - step;
    (decimal_exponent(max) - decimal_exponent(step)).max(0) + 1
}

fn si_suffix(exponent: i32) -> &'static str {
    let idx = exponent / 3 + SI_ZERO;
    SI_SUFFIXES
        .get(idx.clamp(0, SI_SUFFIXES.len() as i32 - 1) as usize)
        .copied()
        .unwrap_or("")
}

fn d3_tick_step(start: f64, stop: f64, count: f64) -> f64 {
    let reverse = stop < start;
    let (a, b) = if reverse { (stop, start) } else { (start, stop) };
    let step = (b - a) / count.max(0.0);
    if !(step > 0.0) || !step.is_finite() {
        return 0.0;
    }
    let power = step.log10().floor();
    let error = step / 10f64.powf(power);
    let factor = if error >= 7.0710678118654755 {
        10.0
    } else if error >= 3.1622776601683795 {
        5.0
    } else if error >= 1.4142135623730951 {
        2.0
    } else {
        1.0
    };
    let magnitude = if power < 0.0 {
        factor / 10f64.powf(-power)
    } else {
        10f64.powf(power) * factor
    };
    magnitude
}

fn fixed(value: f64, prec: usize) -> String {
    if !value.is_finite() || prec > 15 {
        return strip_negative_zero(format!("{:.*}", prec, value));
    }
    if prec == 0 {
        return strip_negative_zero(format!("{:.0}", value.round()));
    }

    // d3-format rounds halves away from zero, whereas Rust's formatter rounds
    // halves to even. Round explicitly on the scaled value to match d3.
    let factor = 10f64.powi(prec as i32);
    let scaled = (value * factor).round();
    if !scaled.is_finite() {
        return strip_negative_zero(format!("{:.*}", prec, value));
    }

    let negative = scaled < 0.0;
    let mut digits = format!("{:.0}", scaled.abs());
    if digits.len() <= prec {
        let mut padded = String::new();
        for _ in 0..(prec + 1 - digits.len()) {
            padded.push('0');
        }
        padded.push_str(&digits);
        digits = padded;
    }

    let split = digits.len() - prec;
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    out.push_str(&digits[..split]);
    out.push('.');
    out.push_str(&digits[split..]);
    strip_negative_zero(out)
}

fn strip_negative_zero(s: String) -> String {
    if s.starts_with("-0") {
        let rest = &s[1..];
        let all_zero = rest.chars().all(|c| c == '0' || c == '.');
        if all_zero {
            return rest.to_string();
        }
    }
    s
}

/// Round `value` to `prec` significant digits and format it without exponent.
fn significant_plain(value: f64, prec: usize, trim: bool) -> String {
    if value == 0.0 {
        let s = if prec > 0 {
            fixed(0.0, prec.saturating_sub(1))
        } else {
            "0".to_string()
        };
        let s = if trim { trim_zeros(&s) } else { s };
        return s;
    }

    let prec = prec.max(1) as i32;
    let exp = value.abs().log10().floor() as i32;
    let decimals = prec - 1 - exp;
    let mut s = if decimals >= 0 {
        fixed(value, decimals as usize)
    } else {
        let factor = 10f64.powi(-decimals);
        let rounded = (value / factor).round() * factor;
        fixed(rounded, 0)
    };

    if trim {
        s = trim_zeros(&s);
    }
    s
}

/// d3 `g`-style formatting: fixed notation for moderate magnitudes, exponent
/// notation otherwise.
fn general(value: f64, prec: usize, trim: bool) -> String {
    if value == 0.0 {
        let s = if prec > 0 {
            fixed(0.0, prec.saturating_sub(1))
        } else {
            "0".to_string()
        };
        return if trim { trim_zeros(&s) } else { s };
    }

    let prec = prec.clamp(1, 21);
    let exp = value.abs().log10().floor() as i32;
    if exp < -6 || exp >= prec as i32 {
        scientific(value, prec.saturating_sub(1), trim)
    } else {
        let decimals = (prec as i32 - 1 - exp).max(0) as usize;
        let mut s = fixed(value, decimals);
        if trim {
            s = trim_zeros(&s);
        }
        s
    }
}

/// Format `abs` in scientific notation, e.g. `1.23e+3`.
fn scientific(abs: f64, prec: usize, trim: bool) -> String {
    if abs == 0.0 {
        let mantissa = fixed(0.0, prec);
        let mantissa = if trim {
            trim_zeros(&mantissa)
        } else {
            mantissa
        };
        return format!("{}e+0", mantissa);
    }

    let mut exp = abs.log10().floor() as i32;
    let mut mantissa = abs / 10f64.powi(exp);
    let rounded = round_to(mantissa, prec);
    if rounded >= 10.0 {
        exp += 1;
        mantissa = abs / 10f64.powi(exp);
    }
    let mut digits = fixed(mantissa, prec);
    if trim {
        digits = trim_zeros(&digits);
    }
    let sign = if exp < 0 { '-' } else { '+' };
    format!("{}e{}{}", digits, sign, exp.abs())
}

fn round_to(value: f64, prec: usize) -> f64 {
    let factor = 10f64.powi(prec as i32);
    (value * factor).round() / factor
}

fn trim_zeros(s: &str) -> String {
    if !s.contains('.') {
        return s.to_string();
    }
    let trimmed = s.trim_end_matches('0');
    let trimmed = trimmed.trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-" {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Insert thousands separators into the integer part of a numeric string.
fn group(s: &str) -> String {
    let (integer, fraction) = match s.find('.') {
        Some(pos) => (&s[..pos], &s[pos..]),
        None => (s, ""),
    };

    let mut out = String::new();
    let len = integer.len();
    for (idx, ch) in integer.chars().enumerate() {
        if idx > 0 && (len - idx) % 3 == 0 && ch.is_ascii_digit() {
            out.push(',');
        }
        out.push(ch);
    }
    out.push_str(fraction);
    out
}

fn pad_number(
    digits: &str,
    sign: &str,
    suffix: &str,
    fs: &FormatSpec,
    already_grouped: bool,
) -> String {
    let mut body = if already_grouped {
        digits.to_string()
    } else if fs.grouping {
        group(digits)
    } else {
        digits.to_string()
    };

    if fs.trim {
        body = trim_zeros(&body);
    }

    let mut out = String::new();
    out.push_str(sign);
    if fs.symbol {
        out.push('$');
    }
    out.push_str(&body);
    out.push_str(suffix);
    apply_padding(out, sign, fs)
}

fn finish(
    _abs: f64,
    negative: bool,
    digits: String,
    prefix: &str,
    suffix: &str,
    fs: &FormatSpec,
    _already_grouped: bool,
) -> String {
    // Suppress a sign when the value rounds to zero at the requested precision
    // (for example `.2f` of -0.0001 yields "0.00", not "-0.00").
    let negative = negative && digits.chars().any(|c| c.is_ascii_digit() && c != '0');
    // Grouping applies to the integer part for numeric types.
    let mut body = if fs.grouping && !digits.contains('e') {
        group(&digits)
    } else if fs.grouping && digits.contains('e') {
        // Group only the mantissa part.
        if let Some(pos) = digits.find('e') {
            format!("{}{}", group(&digits[..pos]), &digits[pos..])
        } else {
            digits
        }
    } else {
        digits
    };

    if fs.trim {
        // Trim after grouping; commas never appear after the decimal point, so
        // trimming is safe on the grouped string as well.
        body = trim_zeros(&body);
    }

    let sign = if negative {
        MINUS
    } else {
        match fs.sign {
            Some('+') => "+",
            Some(' ') => " ",
            _ => "",
        }
    };

    let mut out = String::new();
    match fs.sign {
        Some('(') if negative => {
            out.push('(');
            if fs.symbol {
                out.push('$');
            }
            out.push_str(prefix);
            out.push_str(&body);
            out.push_str(suffix);
            out.push(')');
            return apply_padding(out, "", fs);
        }
        _ => {}
    }

    out.push_str(sign);
    if fs.symbol {
        out.push('$');
    }
    out.push_str(prefix);
    out.push_str(&body);
    out.push_str(suffix);

    apply_padding(out, sign, fs)
}

fn apply_padding(body: String, sign: &str, fs: &FormatSpec) -> String {
    let width = match fs.width {
        Some(w) if w > body.chars().count() => w,
        _ => return body,
    };
    let missing = width - body.chars().count();
    let align = fs.align.unwrap_or(Align::Right);

    if fs.zero && align != Align::Left && fs.fill == ' ' {
        // Zero padding goes after the sign and currency symbol.
        let mut prefix_len = sign.chars().count();
        if fs.symbol {
            prefix_len += 1;
        }
        let prefix: String = body.chars().take(prefix_len).collect();
        let rest: String = body.chars().skip(prefix_len).collect();
        let mut out = String::new();
        out.push_str(&prefix);
        for _ in 0..missing {
            out.push('0');
        }
        out.push_str(&rest);
        return out;
    }

    let fill = fs.fill;
    match align {
        Align::Left => {
            let mut out = body;
            for _ in 0..missing {
                out.push(fill);
            }
            out
        }
        Align::Center => {
            let left = missing / 2;
            let right = missing - left;
            let mut out = String::new();
            for _ in 0..left {
                out.push(fill);
            }
            out.push_str(&body);
            for _ in 0..right {
                out.push(fill);
            }
            out
        }
        _ => {
            let mut out = String::new();
            for _ in 0..missing {
                out.push(fill);
            }
            out.push_str(&body);
            out
        }
    }
}

fn si_scale(abs: f64) -> (f64, &'static str) {
    if abs == 0.0 {
        return (0.0, "");
    }
    let mut group = (abs.log10() / 3.0).floor() as i32;
    group = group.clamp(-SI_ZERO, SI_ZERO);
    let scaled = abs / 1000f64.powi(group);
    (scaled, SI_SUFFIXES[(group + SI_ZERO) as usize])
}

fn si_power(suffix: &str) -> i32 {
    SI_SUFFIXES
        .iter()
        .position(|s| *s == suffix)
        .map(|p| p as i32 - SI_ZERO)
        .unwrap_or(0)
}

fn next_si_suffix(suffix: &str) -> Option<&'static str> {
    let idx = SI_SUFFIXES.iter().position(|s| *s == suffix)?;
    SI_SUFFIXES.get(idx + 1).copied()
}

// ---------------------------------------------------------------------------
// Civil date algorithms (Howard Hinnant's public-domain algorithms)
// ---------------------------------------------------------------------------

/// Days since 1970-01-01 for a proleptic Gregorian date.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Proleptic Gregorian date from days since 1970-01-01.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

#[derive(Clone, Copy)]
struct DateParts {
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
    millis: i64,
}

impl DateParts {
    fn from_ms(ms: f64) -> Self {
        let total = ms.floor() as i64;
        let days = total.div_euclid(86_400_000);
        let rem = total.rem_euclid(86_400_000);
        let (year, month, day) = civil_from_days(days);
        DateParts {
            year,
            month,
            day,
            hour: rem / 3_600_000,
            minute: (rem / 60_000) % 60,
            second: (rem / 1000) % 60,
            millis: rem % 1000,
        }
    }

    fn days(&self) -> i64 {
        days_from_civil(self.year, self.month, self.day)
    }

    fn weekday(&self) -> i64 {
        // 1970-01-01 was a Thursday; 0 = Sunday.
        (self.days() + 4).rem_euclid(7)
    }

    fn day_of_year(&self) -> i64 {
        self.days() - days_from_civil(self.year, 1, 1) + 1
    }
}

fn pad_number_value(value: i64, width: usize, pad: char, no_pad: bool) -> String {
    if no_pad {
        return value.to_string();
    }
    if value < 0 {
        let digits = value.unsigned_abs().to_string();
        let mut out = String::from("-");
        out.push_str(&pad_str(&digits, width.saturating_sub(1), pad));
        out
    } else {
        pad_str(&value.to_string(), width, pad)
    }
}

fn pad_str(text: &str, width: usize, pad: char) -> String {
    let len = text.chars().count();
    if len >= width {
        return text.to_string();
    }
    let mut out = String::new();
    for _ in 0..(width - len) {
        out.push(pad);
    }
    out.push_str(text);
    out
}

/// Format a UTC timestamp (milliseconds since the Unix epoch) with a
/// `d3-time-format` style specification.
///
/// Supported directives: `%Y %y %m %d %e %b %B %a %A %H %M %S %L %p %j %U %W
/// %x %X %%`. A `-` modifier disables zero padding (`%-d`), `_` pads with
/// spaces and `0` forces zero padding.
/// Local-time variant of [`format_date`] (Vega's `timeFormat`/`formatType:
/// "time"`), converting the instant to local wall-clock fields first.
pub fn format_date_local(ms: f64, spec: &str) -> String {
    format_date(ms + crate::data::local_offset_minutes(ms) * 60_000.0, spec)
}

pub fn format_date(ms: f64, spec: &str) -> String {
    if spec.is_empty() {
        return String::new();
    }

    let parts = DateParts::from_ms(ms);
    let chars: Vec<char> = spec.chars().collect();
    let mut out = String::new();
    let mut i = 0usize;

    while i < chars.len() {
        if chars[i] != '%' {
            out.push(chars[i]);
            i += 1;
            continue;
        }

        i += 1;
        if i >= chars.len() {
            out.push('%');
            break;
        }

        let mut pad = '0';
        let mut no_pad = false;
        match chars[i] {
            '-' => {
                no_pad = true;
                i += 1;
            }
            '_' => {
                pad = ' ';
                i += 1;
            }
            '0' => {
                pad = '0';
                i += 1;
            }
            _ => {}
        }

        if i >= chars.len() {
            out.push('%');
            break;
        }

        let directive = chars[i];
        i += 1;

        let piece = match directive {
            'Y' => pad_number_value(parts.year, 4, pad, no_pad),
            'y' => pad_number_value(parts.year.rem_euclid(100), 2, pad, no_pad),
            'm' => pad_number_value(parts.month, 2, pad, no_pad),
            'd' => pad_number_value(parts.day, 2, pad, no_pad),
            'e' => pad_number_value(parts.day, 2, ' ', no_pad),
            'b' => MONTHS_SHORT[(parts.month - 1).clamp(0, 11) as usize].to_string(),
            'B' => MONTHS[(parts.month - 1).clamp(0, 11) as usize].to_string(),
            'a' => WEEKDAYS_SHORT[parts.weekday() as usize].to_string(),
            'A' => WEEKDAYS[parts.weekday() as usize].to_string(),
            'H' => pad_number_value(parts.hour, 2, pad, no_pad),
            'I' => {
                let h = if parts.hour % 12 == 0 {
                    12
                } else {
                    parts.hour % 12
                };
                pad_number_value(h, 2, pad, no_pad)
            }
            'M' => pad_number_value(parts.minute, 2, pad, no_pad),
            'S' => pad_number_value(parts.second, 2, pad, no_pad),
            'L' => pad_number_value(parts.millis, 3, pad, no_pad),
            'f' => pad_number_value(parts.millis * 1000, 6, pad, no_pad),
            'p' => {
                if parts.hour < 12 {
                    "AM".to_string()
                } else {
                    "PM".to_string()
                }
            }
            'q' => pad_number_value((parts.month + 2) / 3, 1, pad, no_pad),
            'u' => pad_number_value((parts.weekday() + 6) % 7 + 1, 1, pad, no_pad),
            'w' => pad_number_value(parts.weekday(), 1, pad, no_pad),
            'G' => pad_number_value(iso_week(&parts).0, 4, pad, no_pad),
            'g' => pad_number_value(iso_week(&parts).0.rem_euclid(100), 2, pad, no_pad),
            'V' => pad_number_value(iso_week(&parts).1, 2, pad, no_pad),
            'Q' => {
                let ms = parts.days() * 86_400_000
                    + parts.hour * 3_600_000
                    + parts.minute * 60_000
                    + parts.second * 1000
                    + parts.millis;
                ms.to_string()
            }
            's' => {
                let ms = parts.days() * 86_400_000
                    + parts.hour * 3_600_000
                    + parts.minute * 60_000
                    + parts.second * 1000;
                (ms / 1000).to_string()
            }
            'Z' => "+0000".to_string(),
            'j' => pad_number_value(parts.day_of_year(), 3, pad, no_pad),
            'U' => {
                let yday = parts.day_of_year() - 1;
                let week = (yday + 7 - parts.weekday()) / 7;
                pad_number_value(week, 2, pad, no_pad)
            }
            'W' => {
                let yday = parts.day_of_year() - 1;
                let monday_weekday = (parts.weekday() + 6) % 7;
                let week = (yday + 7 - monday_weekday) / 7;
                pad_number_value(week, 2, pad, no_pad)
            }
            'x' => {
                let mut s = String::new();
                s.push_str(&pad_number_value(parts.month, 2, '0', false));
                s.push('/');
                s.push_str(&pad_number_value(parts.day, 2, '0', false));
                s.push('/');
                s.push_str(&pad_number_value(parts.year, 4, '0', false));
                s
            }
            'X' => {
                let mut s = String::new();
                s.push_str(&pad_number_value(parts.hour, 2, '0', false));
                s.push(':');
                s.push_str(&pad_number_value(parts.minute, 2, '0', false));
                s.push(':');
                s.push_str(&pad_number_value(parts.second, 2, '0', false));
                s
            }
            '%' => "%".to_string(),
            other => {
                out.push('%');
                other.to_string()
            }
        };

        out.push_str(&piece);
    }

    out
}

fn iso_week(parts: &DateParts) -> (i64, i64) {
    let iso_dow = (parts.weekday() + 6) % 7 + 1; // Monday = 1
    let ordinal = parts.day_of_year();
    let thursday = ordinal + (4 - iso_dow);
    let year = parts.year;
    let days_in_year = if is_leap_year(year) { 366 } else { 365 };
    if thursday < 1 {
        let prev = year - 1;
        let jan1_dow = (days_from_civil(prev, 1, 1) + 4).rem_euclid(7);
        let jan1_iso = (jan1_dow + 6) % 7 + 1;
        let weeks = if jan1_iso == 4 || (is_leap_year(prev) && jan1_iso == 3) {
            53
        } else {
            52
        };
        (prev, weeks)
    } else if thursday > days_in_year {
        (year + 1, 1)
    } else {
        (year, (thursday - 1) / 7 + 1)
    }
}

/// d3-scale's default time tick formatter (multi-format).
///
/// Selects the finest applicable calendar formatting directive for the given
/// timestamp, using the same defaults as d3's `calendar` scale.
pub fn format_date_multi(ms: f64) -> String {
    format_date_multi_span(ms, f64::INFINITY)
}

/// UTC variant of [`format_date_multi`] for `utc` scales.
pub fn format_date_multi_utc(ms: f64) -> String {
    let wall = ms;
    let parts = DateParts::from_ms(wall);
    if parts.millis != 0 {
        format_date(wall, ".%L")
    } else if parts.second != 0 {
        format_date(wall, ":%S")
    } else if parts.minute != 0 {
        format_date(wall, "%I:%M")
    } else if parts.hour != 0 {
        format_date(wall, "%I %p")
    } else if parts.day != 1 {
        if parts.weekday() != 0 {
            format_date(wall, "%a %d")
        } else {
            format_date(wall, "%b %d")
        }
    } else if parts.month != 1 {
        format_date(wall, "%B")
    } else {
        format_date(wall, "%Y")
    }
}

/// Multi-format variant that knows the scale's domain span.
///
/// Vega-Lite emits month/year time-unit specifiers as signal objects, which
/// callers cannot always read back. For sub-year spans we therefore prefer the
/// short month name over d3's full month/year defaults.
pub fn format_date_multi_span(ms: f64, _span: f64) -> String {
    // This is d3-scale's `calendar` multi-format, which depends only on the
    // timestamp's *local* calendar fields (d3's timeFormat uses local time).
    let wall = ms + crate::data::local_offset_minutes(ms) * 60_000.0;
    let parts = DateParts::from_ms(wall);
    if parts.millis != 0 {
        format_date(wall, ".%L")
    } else if parts.second != 0 {
        format_date(wall, ":%S")
    } else if parts.minute != 0 {
        format_date(wall, "%I:%M")
    } else if parts.hour != 0 {
        format_date(wall, "%I %p")
    } else if parts.day != 1 {
        // `%a %d` for days within a week, `%b %d` for the start of a week.
        if parts.weekday() != 0 {
            format_date(wall, "%a %d")
        } else {
            format_date(wall, "%b %d")
        }
    } else if parts.month != 1 {
        format_date(wall, "%B")
    } else {
        format_date(wall, "%Y")
    }
}

// ---------------------------------------------------------------------------
// Date parsing
// ---------------------------------------------------------------------------

/// Parse a date string into milliseconds since the Unix epoch (UTC).
///
/// Numeric strings are interpreted as milliseconds. Supported textual formats
/// include ISO-8601 (`YYYY-MM-DD`, optional time and zone), slash-separated
/// dates (`YYYY/MM/DD`) and common RFC-style dates such as
/// `Mon, 01 Jan 2020 00:00:00 GMT`.
pub fn parse_date(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    // Plain numeric strings are already epoch milliseconds.
    if let Ok(value) = text.parse::<f64>() {
        if value.is_finite() {
            return Some(value);
        }
        return None;
    }

    if let Some(ms) = parse_iso(text) {
        return Some(ms);
    }
    if let Some(ms) = parse_rfc(text) {
        return Some(ms);
    }
    None
}

fn parse_iso(text: &str) -> Option<f64> {
    let bytes: Vec<char> = text.chars().collect();
    if bytes.is_empty() {
        return None;
    }

    // Locate the end of the date component.
    let mut date_end = bytes.len();
    for (idx, ch) in bytes.iter().enumerate() {
        if *ch == 'T' || *ch == 't' || *ch == ' ' {
            date_end = idx;
            break;
        }
    }

    let date_part: String = bytes[..date_end].iter().collect();
    let rest: String = bytes[date_end..].iter().collect();

    let separator = if date_part.contains('-') {
        '-'
    } else if date_part.contains('/') {
        '/'
    } else {
        return None;
    };

    let mut pieces = date_part.split(separator);
    let year: i64 = pieces.next()?.parse().ok()?;
    let month: i64 = pieces.next()?.parse().ok()?;
    let day: i64 = pieces.next()?.parse().ok()?;
    if pieces.next().is_some() {
        return None;
    }
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return None;
    }

    let rest = rest.trim();
    let rest = rest
        .strip_prefix('T')
        .or_else(|| rest.strip_prefix('t'))
        .unwrap_or(rest)
        .trim();
    let (time_text, offset_minutes) = strip_zone(rest)?;
    let (hour, minute, second, millis) = parse_time_of_day(time_text)?;

    let days = days_from_civil(year, month, day);
    let ms = days * 86_400_000 + hour * 3_600_000 + minute * 60_000 + second * 1000 + millis
        - offset_minutes * 60_000;
    Some(ms as f64)
}

/// Split a trailing timezone designator off a time string. Returns the
/// remaining time and the offset in minutes east of UTC.
fn strip_zone(text: &str) -> Option<(&str, i64)> {
    let text = text.trim();
    if text.is_empty() {
        return Some(("", 0));
    }

    let upper = text.to_ascii_uppercase();
    if upper.ends_with('Z') {
        let cut = text.len() - 1;
        return Some((text[..cut].trim(), 0));
    }
    if upper.ends_with(" UTC") || upper.ends_with(" GMT") {
        let cut = text.len() - 4;
        return Some((text[..cut].trim(), 0));
    }
    if upper.ends_with("UTC") || upper.ends_with("GMT") {
        let cut = text.len() - 3;
        return Some((text[..cut].trim(), 0));
    }

    // Look for a +/-HH:MM or +/-HHMM designator after the time component.
    if let Some(colon_pos) = text.rfind(':') {
        let tail = &text[colon_pos + 1..];
        // Handle "05:30+02:00": find a sign after the minutes.
        if let Some(sign_pos) = tail.find(['+', '-']) {
            let sign_idx = colon_pos + 1 + sign_pos;
            let (time, zone) = text.split_at(sign_idx);
            let offset = parse_offset(zone)?;
            return Some((time.trim(), offset));
        }
    }
    if let Some(sign_pos) = text.find(['+', '-']) {
        if sign_pos > 0 {
            let (time, zone) = text.split_at(sign_pos);
            if let Some(offset) = parse_offset(zone) {
                return Some((time.trim(), offset));
            }
        }
    }

    Some((text, 0))
}

fn parse_offset(zone: &str) -> Option<i64> {
    let zone = zone.trim();
    let (sign, rest) = match zone.chars().next()? {
        '+' => (1i64, &zone[1..]),
        '-' => (-1i64, &zone[1..]),
        _ => return None,
    };
    let digits: String = rest.chars().filter(|c| *c != ':').collect();
    if digits.len() != 4 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let hours: i64 = digits[..2].parse().ok()?;
    let minutes: i64 = digits[2..].parse().ok()?;
    if hours > 23 || minutes > 59 {
        return None;
    }
    Some(sign * (hours * 60 + minutes))
}

fn parse_time_of_day(text: &str) -> Option<(i64, i64, i64, i64)> {
    let text = text.trim();
    if text.is_empty() {
        return Some((0, 0, 0, 0));
    }
    let mut pieces = text.split(':');
    let hour: i64 = pieces.next()?.trim().parse().ok()?;
    let minute: i64 = pieces.next().unwrap_or("0").trim().parse().ok()?;
    let sec_text = pieces.next().unwrap_or("0").trim();
    let (second, millis) = if let Some(dot) = sec_text.find('.') {
        let secs: i64 = sec_text[..dot].parse().ok()?;
        let frac = &sec_text[dot + 1..];
        let mut frac_digits = String::new();
        for ch in frac.chars().take(3) {
            if !ch.is_ascii_digit() {
                return None;
            }
            frac_digits.push(ch);
        }
        while frac_digits.len() < 3 {
            frac_digits.push('0');
        }
        (secs, frac_digits.parse().ok()?)
    } else {
        let secs: i64 = if sec_text.is_empty() {
            0
        } else {
            sec_text.parse().ok()?
        };
        (secs, 0)
    };
    if !(0..=23).contains(&hour) || !(0..=59).contains(&minute) || !(0..=60).contains(&second) {
        return None;
    }
    Some((hour, minute, second, millis))
}

fn month_from_name(name: &str) -> Option<i64> {
    let lower = name.to_ascii_lowercase();
    let lower = lower.trim_end_matches('.');
    for (idx, full) in MONTHS.iter().enumerate() {
        if lower == full.to_ascii_lowercase() || lower == MONTHS_SHORT[idx].to_ascii_lowercase() {
            return Some(idx as i64 + 1);
        }
    }
    None
}

fn is_weekday_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let lower = lower.trim_end_matches(',');
    WEEKDAYS
        .iter()
        .chain(WEEKDAYS_SHORT.iter())
        .any(|day| lower == day.to_ascii_lowercase())
}

fn parse_rfc(text: &str) -> Option<f64> {
    let cleaned = text.replace(',', " ");
    let mut year: Option<i64> = None;
    let mut month: Option<i64> = None;
    let mut day: Option<i64> = None;
    let mut hour = 0i64;
    let mut minute = 0i64;
    let mut second = 0i64;
    let mut millis = 0i64;
    let mut offset = 0i64;
    let mut pm = false;
    let mut am = false;

    for token in cleaned.split_whitespace() {
        let lower = token.to_ascii_lowercase();
        if is_weekday_name(token) {
            continue;
        }
        if matches!(lower.as_str(), "gmt" | "utc" | "z") {
            continue;
        }
        if lower == "am" {
            am = true;
            continue;
        }
        if lower == "pm" {
            pm = true;
            continue;
        }

        if token.contains(':') {
            // A time token, possibly carrying a zone suffix (handled below by
            // splitting the token before parsing).
            let (time, zone_offset) = strip_zone(token)?;
            let parsed = parse_time_of_day(time)?;
            hour = parsed.0;
            minute = parsed.1;
            second = parsed.2;
            millis = parsed.3;
            offset = zone_offset;
            continue;
        }

        if (token.starts_with('+') || token.starts_with('-'))
            && token.len() >= 5
            && token[1..].chars().all(|c| c.is_ascii_digit() || c == ':')
        {
            if let Some(value) = parse_offset(token) {
                offset = value;
                continue;
            }
        }

        if let Some(m) = month_from_name(token) {
            month = Some(m);
            continue;
        }

        if token.chars().all(|c| c.is_ascii_digit()) {
            let value: i64 = token.parse().ok()?;
            if token.len() == 4 || value > 31 {
                if year.is_none() {
                    year = Some(value);
                    continue;
                }
            }
            if day.is_none() && (1..=31).contains(&value) {
                day = Some(value);
                continue;
            }
            if year.is_none() {
                year = Some(value);
                continue;
            }
        }
    }

    if am || pm {
        if pm && hour < 12 {
            hour += 12;
        } else if am && hour == 12 {
            hour = 0;
        }
    }

    let year = year?;
    let month = month?;
    let day = day?;
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return None;
    }
    if !(0..=23).contains(&hour) || !(0..=59).contains(&minute) || !(0..=60).contains(&second) {
        return None;
    }

    let days = days_from_civil(year, month, day);
    let ms = days * 86_400_000 + hour * 3_600_000 + minute * 60_000 + second * 1000 + millis
        - offset * 60_000;
    Some(ms as f64)
}

// ---------------------------------------------------------------------------
// Generic value formatting
// ---------------------------------------------------------------------------

/// Format an arbitrary JSON value.
///
/// `format_type` follows Vega's convention: `"number"`, `"time"` and `"utc"`.
/// When it is `None` the value type decides: numbers use [`format_number`],
/// strings are returned unchanged, booleans become `true`/`false`, `null`
/// becomes the empty string and arrays are joined with `", "`.
pub fn format_value(value: &Value, spec: &str, format_type: Option<&str>) -> String {
    match value {
        Value::Array(items) => items
            .iter()
            .map(|item| format_value(item, spec, format_type))
            .collect::<Vec<_>>()
            .join(", "),
        Value::Number(number) => {
            let as_f64 = number.as_f64().unwrap_or(0.0);
            match format_type {
                Some("time") => format_date_local(as_f64, spec),
                Some("utc") => format_date(as_f64, spec),
                Some("number") => format_number(as_f64, spec),
                _ => format_number(as_f64, spec),
            }
        }
        Value::String(text) => match format_type {
            Some("time") => match parse_date(text) {
                Some(ms) => format_date_local(ms, spec),
                None => text.clone(),
            },
            Some("utc") => match parse_date(text) {
                Some(ms) => format_date(ms, spec),
                None => text.clone(),
            },
            Some("number") => match text.trim().parse::<f64>() {
                Ok(number) => format_number(number, spec),
                Err(_) => text.clone(),
            },
            _ => text.clone(),
        },
        Value::Bool(flag) => {
            if *flag {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        Value::Null => String::new(),
        Value::Object(_) => value.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn empty_spec_is_compact() {
        assert_eq!(format_number(0.0, ""), "0");
        assert_eq!(format_number(1.0, ""), "1");
        assert_eq!(format_number(1.5, ""), "1.5");
        assert_eq!(format_number(1000.0, ""), "1000");
        assert_eq!(format_number(-2.25, ""), "−2.25");
    }

    #[test]
    fn default_spec_uses_twelve_significant_digits() {
        assert_eq!(format_number(1.2345678901234, ""), "1.23456789012");
        assert_eq!(format_number(12345678901234.0, ""), "1.23456789012e+13");
    }

    #[test]
    fn span_formatting_matches_vega() {
        assert_eq!(format_span(0.0, 0.0, 12.0e6, 8.0, "s"), "0M");
        assert_eq!(format_span(12.0e6, 0.0, 12.0e6, 8.0, "s"), "12M");
        assert_eq!(format_span(1.0, 1.0, 10.0, 30.0, ",f"), "1.0");
        assert_eq!(format_span(1.5, 1.0, 10.0, 30.0, ",f"), "1.5");
        assert_eq!(format_span(0.0, 0.0, 0.1, 5.0, ",f"), "0.00");
    }

    #[test]
    fn integer_format() {
        assert_eq!(format_number(42.0, "d"), "42");
        assert_eq!(format_number(42.6, "d"), "43");
        assert_eq!(format_number(-42.6, "d"), "−43");
        assert_eq!(format_number(1234567.0, ",d"), "1,234,567");
        assert_eq!(format_number(1234.0, "08d"), "00001234");
    }

    #[test]
    fn fixed_format() {
        assert_eq!(format_number(1.23456, ".1f"), "1.2");
        assert_eq!(format_number(1234.5678, ",.2f"), "1,234.57");
        assert_eq!(format_number(0.5, ".0f"), "1");
        assert_eq!(format_number(2.0, ".2f"), "2.00");
        assert_eq!(format_number(-0.0001, ".2f"), "0.00");
    }

    #[test]
    fn percent_and_currency() {
        assert_eq!(format_number(0.1234, ".1%"), "12.3%");
        assert_eq!(format_number(0.5, ".0%"), "50%");
        assert_eq!(format_number(1234.5, "$,.2f"), "$1,234.50");
        assert_eq!(format_number(-1234.5, "$,.2f"), "−$1,234.50");
    }

    #[test]
    fn si_suffixes() {
        assert_eq!(format_number(1500.0, ".2~s"), "1.5k");
        assert_eq!(format_number(1_000_000.0, ".1~s"), "1M");
        assert_eq!(format_number(0.0015, ".2~s"), "1.5m");
        assert_eq!(format_number(999_999.0, ".2~s"), "1M");
        assert_eq!(format_number(12.0, "s"), "12.0000");
    }

    #[test]
    fn scientific_notation() {
        assert_eq!(format_number(1234.0, ".2e"), "1.23e+3");
        assert_eq!(format_number(0.00123, ".2e"), "1.23e-3");
        assert_eq!(format_number(0.0, ".1e"), "0.0e+0");
    }

    #[test]
    fn signs_alignment_and_trim() {
        assert_eq!(format_number(5.0, "+d"), "+5");
        assert_eq!(format_number(-5.0, "+d"), "−5");
        assert_eq!(format_number(1.100, ".2~f"), "1.1");
        assert_eq!(format_number(42.0, "5d"), "   42");
        assert_eq!(format_number(42.0, "<5d"), "42   ");
        assert_eq!(format_number(3.14159, ".3~f"), "3.142");
    }

    #[test]
    fn special_values() {
        assert_eq!(format_number(f64::NAN, "f"), "NaN");
        assert_eq!(format_number(f64::INFINITY, "f"), "Infinity");
        assert_eq!(format_number(f64::NEG_INFINITY, "f"), "−Infinity");
    }

    #[test]
    fn date_basics() {
        // 2020-01-02T03:04:05.006Z
        let ms = 1_577_934_245_006.0;
        assert_eq!(format_date(ms, "%Y-%m-%d"), "2020-01-02");
        assert_eq!(format_date(ms, "%H:%M:%S.%L"), "03:04:05.006");
        assert_eq!(format_date(ms, "%b %e %Y"), "Jan  2 2020");
        assert_eq!(format_date(ms, "%-d/%-m/%y"), "2/1/20");
        assert_eq!(format_date(ms, "%A, %B %d"), "Thursday, January 02");
        assert_eq!(format_date(ms, "%a %p %j"), "Thu AM 002");
        assert_eq!(format_date(ms, "%x %X"), "01/02/2020 03:04:05");
        assert_eq!(format_date(ms, "%%"), "%");
        assert_eq!(format_date(ms, "%U %W"), "00 00");
    }

    #[test]
    fn date_epoch_and_negative() {
        assert_eq!(format_date(0.0, "%Y-%m-%d %H:%M:%S"), "1970-01-01 00:00:00");
        assert_eq!(format_date(0.0, "%a"), "Thu");
        // 1969-12-31T23:59:59Z
        assert_eq!(
            format_date(-1000.0, "%Y-%m-%d %H:%M:%S"),
            "1969-12-31 23:59:59"
        );
        // A date before the epoch exercises Euclidean day handling.
        assert_eq!(format_date(-2_208_988_800_000.0, "%Y-%m-%d"), "1900-01-01");
    }

    #[test]
    fn time_multi_format_span_aware() {
        // d3-scale's multi-format uses local calendar fields, so build the
        // instants from local wall-clock components to stay timezone-agnostic.
        let jan2012 = crate::data::local_civil_to_ms(2012, 1, 1, 0, 0, 0, 0);
        assert_eq!(format_date_multi_span(jan2012, 365.0 * 86_400_000.0), "2012");
        assert_eq!(format_date_multi_span(jan2012, 5.0 * 365.0 * 86_400_000.0), "2012");
        let three_am = jan2012 + 3.0 * 3_600_000.0;
        assert_eq!(format_date_multi_span(three_am, 86_400_000.0), "03 AM");
        let three_oh_five = three_am + 5.0 * 60_000.0;
        assert_eq!(format_date_multi_span(three_oh_five, 86_400_000.0), "03:05");
        let jan15 = crate::data::local_civil_to_ms(2012, 1, 15, 0, 0, 0, 0);
        assert_eq!(format_date_multi_span(jan15, 86_400_000.0), "Jan 15");
    }

    #[test]
    fn additional_date_directives() {
        let ms = 1_577_934_245_006.0; // 2020-01-02T03:04:05.006Z
        assert_eq!(format_date(ms, "%I %p"), "03 AM");
        assert_eq!(format_date(ms, "%q %u %w"), "1 4 4");
        assert_eq!(
            format_date(parse_date("2021-01-01").unwrap(), "%G-W%V"),
            "2020-W53"
        );
        assert_eq!(format_date(ms, "%Q %s"), "1577934245006 1577934245");
    }

    #[test]
    fn date_week_numbers() {
        // 2019-01-01 was a Tuesday: week 00 for both Sunday/Monday weeks.
        let ms = 1_546_300_800_000.0;
        assert_eq!(format_date(ms, "%U %W"), "00 00");
        // 2019-01-06 was the first Sunday.
        assert_eq!(format_date(ms + 5.0 * 86_400_000.0, "%U %W"), "01 00");
        // 2019-01-07 was the first Monday.
        assert_eq!(format_date(ms + 6.0 * 86_400_000.0, "%U %W"), "01 01");
    }

    #[test]
    fn parse_iso_dates() {
        assert_eq!(parse_date("2020-01-02"), Some(1_577_923_200_000.0));
        assert_eq!(
            parse_date("2020-01-02T03:04:05.006Z"),
            Some(1_577_934_245_006.0)
        );
        assert_eq!(
            parse_date("2020-01-02T03:04:05.006"),
            Some(1_577_934_245_006.0)
        );
        assert_eq!(
            parse_date("2020-01-02T03:04:05+02:00"),
            Some(1_577_927_045_000.0)
        );
        assert_eq!(
            parse_date("2020-01-02T03:04:05-05:30"),
            Some(1_577_954_045_000.0)
        );
        assert_eq!(parse_date("2020/01/02"), Some(1_577_923_200_000.0));
        assert_eq!(parse_date("1577923200000"), Some(1_577_923_200_000.0));
    }

    #[test]
    fn parse_rfc_dates() {
        assert_eq!(
            parse_date("Thu, 02 Jan 2020 03:04:05 GMT"),
            Some(1_577_934_245_000.0)
        );
        assert_eq!(parse_date("January 2, 2020"), Some(1_577_923_200_000.0));
        assert_eq!(parse_date("Jan 2 2020 03:04:05"), Some(1_577_934_245_000.0));
        assert_eq!(parse_date("2 January 2020"), Some(1_577_923_200_000.0));
    }

    #[test]
    fn parse_invalid_dates() {
        assert_eq!(parse_date(""), None);
        assert_eq!(parse_date("not a date"), None);
        assert_eq!(parse_date("2020-13-01"), None);
        assert_eq!(parse_date("2020-02-30"), None);
        assert_eq!(parse_date("2020-01-02T25:00:00Z"), None);
    }

    #[test]
    fn value_formatting() {
        assert_eq!(format_value(&json!(1234.5), ",.2f", None), "1,234.50");
        assert_eq!(
            format_value(&json!(1234.5), ",.2f", Some("number")),
            "1,234.50"
        );
        assert_eq!(format_value(&json!("hello"), "", None), "hello");
        assert_eq!(format_value(&json!(true), "", None), "true");
        assert_eq!(format_value(&json!(false), "", None), "false");
        assert_eq!(format_value(&json!(null), "", None), "");
        assert_eq!(format_value(&json!([1, 2, 3]), "", None), "1, 2, 3");
        assert_eq!(
            format_value(&json!("2020-01-02"), "%Y/%m/%d", Some("utc")),
            "2020/01/02"
        );
        assert_eq!(
            format_value(&json!(1_577_966_400_000.0), "%Y-%m-%d", Some("time")),
            "2020-01-02"
        );
        assert_eq!(format_value(&json!("42.5"), ".1f", Some("number")), "42.5");
        assert_eq!(
            format_value(&json!("not a date"), "%Y", Some("time")),
            "not a date"
        );
    }
}

/// Parses a date string according to the subset of d3-time-format directives
/// used by the corpus (`%Y %m %d %H %M %S %L %b %B %j %p`).
pub fn parse_date_format(text: &str, format: &str) -> Option<(i64, i64, i64, i64, i64, i64, i64)> {
    let mut year = 1970i64;
    let mut month = 1i64;
    let mut day = 1i64;
    let (mut hour, mut minute, mut second, mut milli) = (0i64, 0i64, 0i64, 0i64);
    let mut pm = false;
    let chars: Vec<char> = text.trim().chars().collect();
    let mut pos = 0usize;
    let mut it = format.chars().peekable();
    let read_num = |pos: &mut usize, len: usize| -> Option<i64> {
        while *pos < chars.len() && chars[*pos].is_whitespace() {
            *pos += 1;
        }
        let start = *pos;
        while *pos < chars.len() && chars[*pos].is_ascii_digit() {
            *pos += 1;
        }
        if *pos == start {
            return None;
        }
        let end = if len > 0 { (start + len).min(*pos) } else { *pos };
        chars[start..end].iter().collect::<String>().parse().ok()
    };
    let read_word = |pos: &mut usize| -> Option<String> {
        while *pos < chars.len() && chars[*pos].is_whitespace() {
            *pos += 1;
        }
        let start = *pos;
        while *pos < chars.len() && chars[*pos].is_alphabetic() {
            *pos += 1;
        }
        if *pos == start {
            return None;
        }
        Some(chars[start..*pos].iter().collect::<String>().to_ascii_lowercase())
    };
    while let Some(c) = it.next() {
        if c != '%' {
            if c.is_whitespace() {
                while pos < chars.len() && chars[pos].is_whitespace() {
                    pos += 1;
                }
            } else {
                if chars.get(pos) != Some(&c) {
                    return None;
                }
                pos += 1;
            }
            continue;
        }
        let Some(d) = it.next() else { return None };
        match d {
            'Y' => year = read_num(&mut pos, 0)?,
            'y' => {
                let y = read_num(&mut pos, 0)?;
                year = if y < 70 { 2000 + y } else { 1900 + y };
            }
            'm' => month = read_num(&mut pos, 0)?,
            'd' | 'e' => day = read_num(&mut pos, 0)?,
            'H' => hour = read_num(&mut pos, 0)?,
            'I' => hour = read_num(&mut pos, 0)?,
            'M' => minute = read_num(&mut pos, 0)?,
            'S' => second = read_num(&mut pos, 0)?,
            'L' => milli = read_num(&mut pos, 0)?,
            'j' => {
                let ordinal = read_num(&mut pos, 0)?;
                let jan1 = days_from_civil(year, 1, 1);
                let (y2, m2, d2) = civil_from_days(jan1 + ordinal - 1);
                year = y2;
                month = m2;
                day = d2;
            }
            'b' | 'B' | 'h' => {
                let word = read_word(&mut pos)?;
                month = match word.as_str() {
                    "jan" | "january" => 1,
                    "feb" | "february" => 2,
                    "mar" | "march" => 3,
                    "apr" | "april" => 4,
                    "may" => 5,
                    "jun" | "june" => 6,
                    "jul" | "july" => 7,
                    "aug" | "august" => 8,
                    "sep" | "september" => 9,
                    "oct" | "october" => 10,
                    "nov" | "november" => 11,
                    "dec" | "december" => 12,
                    _ => return None,
                };
            }
            'p' => {
                let word = read_word(&mut pos)?;
                pm = word == "pm";
            }
            '%' => {
                if chars.get(pos) != Some(&'%') {
                    return None;
                }
                pos += 1;
            }
            _ => return None,
        }
    }
    if pm && hour < 12 {
        hour += 12;
    }
    Some((year, month, day, hour, minute, second, milli))
}

//! Port of `src/datetime.ts`.
//!
//! `DateTime` is modelled as a struct holding the (possibly non-numeric) values
//! from the upstream TypeScript interface.  This mirrors the permissive
//! behaviour of the original code, which accepts strings such as
//! `{year: '2007', month: '1'}` "by accident".

use serde_json::{json, Map, Number, Value};

use crate::log;
use crate::timeunit::TIMEUNIT_PARTS;
use crate::util::{has_property, is_numeric, js_number_to_string, stringify};

pub const MONTHS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

pub const SHORT_MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

pub const DAYS: [&str; 7] = [
    "sunday",
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
];

pub const SHORT_DAYS: [&str; 7] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];

/// Object for defining datetime in a Vega-Lite filter.
///
/// Values are kept as [`serde_json::Value`] so that numeric strings (which the
/// upstream implementation accepts) can be handled the same way.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DateTime {
    pub year: Option<Value>,
    pub quarter: Option<Value>,
    pub month: Option<Value>,
    pub date: Option<Value>,
    pub day: Option<Value>,
    pub hours: Option<Value>,
    pub minutes: Option<Value>,
    pub seconds: Option<Value>,
    pub milliseconds: Option<Value>,
    pub utc: Option<bool>,
}

fn get_prop(v: &Value, key: &str) -> Option<Value> {
    match v {
        Value::Object(map) => map.get(key).filter(|x| !x.is_null()).cloned(),
        _ => None,
    }
}

/// JavaScript truthiness for the values we care about.
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0 && !f.is_nan()).unwrap_or(false),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

impl DateTime {
    fn from_object(v: &Value) -> Self {
        let utc = match v {
            Value::Object(map) => map.get("utc").filter(|x| !x.is_null()).map(truthy),
            _ => None,
        };
        Self {
            year: get_prop(v, "year"),
            quarter: get_prop(v, "quarter"),
            month: get_prop(v, "month"),
            date: get_prop(v, "date"),
            day: get_prop(v, "day"),
            hours: get_prop(v, "hours"),
            minutes: get_prop(v, "minutes"),
            seconds: get_prop(v, "seconds"),
            milliseconds: get_prop(v, "milliseconds"),
            utc,
        }
    }

    fn remove_day(&mut self) {
        self.day = None;
    }

    /// Keys of the defined properties, in the same order as the interface in
    /// the TypeScript source.
    fn keys(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.year.is_some() {
            out.push("year");
        }
        if self.quarter.is_some() {
            out.push("quarter");
        }
        if self.month.is_some() {
            out.push("month");
        }
        if self.date.is_some() {
            out.push("date");
        }
        if self.day.is_some() {
            out.push("day");
        }
        if self.hours.is_some() {
            out.push("hours");
        }
        if self.minutes.is_some() {
            out.push("minutes");
        }
        if self.seconds.is_some() {
            out.push("seconds");
        }
        if self.milliseconds.is_some() {
            out.push("milliseconds");
        }
        if self.utc.is_some() {
            out.push("utc");
        }
        out
    }

    /// Convert back to a JSON object (used for log messages and durations).
    pub fn to_value(&self) -> Value {
        let mut map = Map::new();
        for key in self.keys() {
            let value = match key {
                "year" => self.year.clone(),
                "quarter" => self.quarter.clone(),
                "month" => self.month.clone(),
                "date" => self.date.clone(),
                "day" => self.day.clone(),
                "hours" => self.hours.clone(),
                "minutes" => self.minutes.clone(),
                "seconds" => self.seconds.clone(),
                "milliseconds" => self.milliseconds.clone(),
                "utc" => self.utc.map(Value::Bool),
                _ => None,
            };
            if let Some(value) = value {
                map.insert(key.to_string(), value);
            }
        }
        Value::Object(map)
    }
}

impl From<&Value> for DateTime {
    fn from(v: &Value) -> Self {
        DateTime::from_object(v)
    }
}

impl From<Value> for DateTime {
    fn from(v: Value) -> Self {
        DateTime::from_object(&v)
    }
}

impl From<&DateTime> for DateTime {
    fn from(d: &DateTime) -> Self {
        d.clone()
    }
}

/// Returns true if `o` is a DateTime definition object.
pub fn is_date_time(o: &Value) -> bool {
    if matches!(o, Value::Object(_)) {
        for part in TIMEUNIT_PARTS {
            if has_property(o, part) {
                return true;
            }
        }
    }
    false
}

fn value_to_number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => {
            if is_numeric(v) {
                s.trim().parse::<f64>().ok()
            } else {
                None
            }
        }
        _ => None,
    }
}

fn number_value(n: f64) -> Value {
    Number::from_f64(n).map(Value::Number).unwrap_or(Value::Null)
}

fn normalize_quarter(q: &Value) -> f64 {
    let value = value_to_number(q).unwrap_or_else(|| {
        panic!("{}", log::message::invalid_time_unit("quarter", q));
    });

    // The JavaScript version only warns (and then still computes `q - 1`).
    if value > 4.0 {
        log::warn(log::message::invalid_time_unit("quarter", &number_value(value)));
    }
    value - 1.0
}

fn normalize_month(m: &Value) -> f64 {
    if let Some(value) = value_to_number(m) {
        return value - 1.0;
    }

    if let Value::String(s) = m {
        let lower = s.to_lowercase();
        if let Some(index) = MONTHS.iter().position(|month| *month == lower) {
            return index as f64;
        }
        let short: String = lower.chars().take(3).collect();
        if let Some(index) = SHORT_MONTHS.iter().position(|month| *month == short) {
            return index as f64;
        }
    }

    panic!("{}", log::message::invalid_time_unit("month", m));
}

fn normalize_day(d: &Value) -> f64 {
    if let Some(value) = value_to_number(d) {
        // mod so that this can be both 0-based (0 = sunday) and 1-based
        // (7 = sunday)
        return value % 7.0;
    }

    if let Value::String(s) = d {
        let lower = s.to_lowercase();
        if let Some(index) = DAYS.iter().position(|day| *day == lower) {
            return index as f64;
        }
        let short: String = lower.chars().take(3).collect();
        if let Some(index) = SHORT_DAYS.iter().position(|day| *day == short) {
            return index as f64;
        }
    }

    panic!("{}", log::message::invalid_time_unit("day", d));
}

fn js_value_to_string(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n
            .as_f64()
            .map(js_number_to_string)
            .unwrap_or_else(|| n.to_string()),
        Value::String(s) => s.clone(),
        Value::Array(_) | Value::Object(_) => stringify(v),
    }
}

/// Port of the internal `dateTimeParts`.
///
/// Returns `[year, month, date, hours, minutes, seconds, milliseconds]` where
/// each part is either a number or an expression string.
fn date_time_parts(d: &DateTime, normalize: bool) -> Vec<Value> {
    let mut d = d.clone();

    if normalize && d.day.is_some() && d.keys().len() > 1 {
        log::warn(log::message::dropped_day(&d.to_value()));
        d.remove_day();
    }

    let mut parts: Vec<Value> = Vec::new();

    if let Some(year) = &d.year {
        parts.push(year.clone());
    } else {
        // Just like Vega's timeunit transform, set default year to 2012, so
        // domain conversion will be compatible with Vega.
        parts.push(json!(2012));
    }

    if let Some(month) = &d.month {
        if normalize {
            parts.push(number_value(normalize_month(month)));
        } else {
            parts.push(month.clone());
        }
    } else if let Some(quarter) = &d.quarter {
        let quarter = if normalize {
            number_value(normalize_quarter(quarter))
        } else {
            quarter.clone()
        };
        parts.push(match &quarter {
            Value::Number(_) => {
                let q = quarter.as_f64().unwrap_or(0.0);
                number_value(q * 3.0)
            }
            other => Value::String(format!("{}*3", js_value_to_string(other))),
        });
    } else {
        // months start at zero in JS
        parts.push(json!(0));
    }

    if let Some(date) = &d.date {
        parts.push(date.clone());
    } else if let Some(day) = &d.day {
        // HACK: Day only works as a standalone unit.
        let day = if normalize {
            number_value(normalize_day(day))
        } else {
            day.clone()
        };
        parts.push(match &day {
            Value::Number(_) => {
                let day = day.as_f64().unwrap_or(0.0);
                number_value(day + 1.0)
            }
            other => Value::String(format!("{}+1", js_value_to_string(other))),
        });
    } else {
        // Date starts at 1 in JS
        parts.push(json!(1));
    }

    for part in [&d.hours, &d.minutes, &d.seconds, &d.milliseconds] {
        parts.push(part.clone().unwrap_or_else(|| json!(0)));
    }

    parts
}

fn expr_from_parts(parts: &[Value], utc: bool) -> String {
    let joined: Vec<String> = parts.iter().map(js_value_to_string).collect();
    let string = joined.join(", ");
    if utc {
        format!("utc({string})")
    } else {
        format!("datetime({string})")
    }
}

/// Return a Vega expression for a date time.
pub fn date_time_to_expr<D: Into<DateTime>>(d: D) -> String {
    let d = d.into();
    let utc = d.utc.unwrap_or(false);
    let parts = date_time_parts(&d, true);
    expr_from_parts(&parts, utc)
}

/// Return a Vega expression for a date time expression.
pub fn date_time_expr_to_expr<D: Into<DateTime>>(d: D) -> String {
    let d = d.into();
    let utc = d.utc.unwrap_or(false);
    let parts = date_time_parts(&d, false);
    expr_from_parts(&parts, utc)
}

fn part_to_i64(v: &Value) -> i64 {
    match value_to_number(v) {
        Some(f) => f.trunc() as i64,
        None => 0,
    }
}

/// Days since 1970-01-01 for a civil date (Howard Hinnant's algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn utc_timestamp_from_parts(
    year: i64,
    month: i64,
    date: i64,
    hours: i64,
    minutes: i64,
    seconds: i64,
    milliseconds: i64,
) -> i64 {
    // JavaScript's `Date`/`Date.UTC` map years 0-99 to 1900-1999.
    let year = if (0..=99).contains(&year) { year + 1900 } else { year };

    // Normalize the month (values outside 0-11 roll over into years).
    let total_months = year * 12 + month;
    let normalized_year = total_months.div_euclid(12);
    let normalized_month = total_months.rem_euclid(12);

    let days = days_from_civil(normalized_year, normalized_month + 1, 1) + date - 1;

    ((((days * 24 + hours) * 60 + minutes) * 60 + seconds) * 1000) + milliseconds
}

/// Return the timestamp for a date time.
///
/// The upstream example corpus is generated with
/// `TZ=America/Los_Angeles`, so local date-times use US Pacific time
/// (UTC-8 standard / UTC-7 daylight saving). `utc: true` keeps `Date.UTC`
/// semantics.
pub fn date_time_to_timestamp<D: Into<DateTime>>(d: D) -> i64 {
    let d = d.into();
    let parts = date_time_parts(&d, true);

    let year = {
        let y = part_to_i64(&parts[0]);
        if (0..=99).contains(&y) {
            y + 1900
        } else {
            y
        }
    };
    let month = part_to_i64(&parts[1]);
    let date = part_to_i64(&parts[2]);
    let hours = part_to_i64(&parts[3]);
    let minutes = part_to_i64(&parts[4]);
    let seconds = part_to_i64(&parts[5]);
    let milliseconds = part_to_i64(&parts[6]);

    let utc = utc_timestamp_from_parts(year, month, date, hours, minutes, seconds, milliseconds);
    if d.utc == Some(true) {
        return utc;
    }

    let total_months = year * 12 + month;
    let normalized_year = total_months.div_euclid(12);
    let normalized_month = total_months.rem_euclid(12);
    let mar1 = days_from_civil(normalized_year, 3, 1);
    let mar1_weekday = (mar1 + 4).rem_euclid(7);
    let second_sunday_march = 1 + (7 - mar1_weekday) % 7 + 7;

    let nov1 = days_from_civil(normalized_year, 11, 1);
    let nov1_weekday = (nov1 + 4).rem_euclid(7);
    let first_sunday_november = 1 + (7 - nov1_weekday) % 7;

    let is_dst = (normalized_month > 2 && normalized_month < 10)
        || (normalized_month == 2
            && (date > second_sunday_march
                || (date == second_sunday_march && hours >= 2)))
        || (normalized_month == 10
            && (date < first_sunday_november
                || (date == first_sunday_november && hours < 2)));

    let offset_hours = if is_dst { -7 } else { -8 };
    utc - offset_hours * 3_600_000
}

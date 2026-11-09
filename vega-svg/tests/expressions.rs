//! Integration tests for `vega_svg::expression`.
//!
//! These tests exercise the evaluator through its public API only. They use a
//! couple of tiny `Environment` implementations to provide signals, scale
//! domains and bandwidths without depending on the rest of the runtime.

use std::collections::HashMap;

use serde_json::{json, Value};
use vega_svg::expression::{evaluate, EmptyEnv, Environment};

/// Environment backed by a fixed signal map.
struct TestEnv {
    signals: HashMap<String, Value>,
}

impl TestEnv {
    fn new() -> Self {
        Self {
            signals: HashMap::new(),
        }
    }

    fn with(mut self, name: &str, value: Value) -> Self {
        self.signals.insert(name.to_string(), value);
        self
    }
}

impl Environment for TestEnv {
    fn signal(&self, name: &str) -> Option<Value> {
        self.signals.get(name).cloned()
    }
}

/// Environment that exposes fixed scale information.
struct ScaleEnv;

impl Environment for ScaleEnv {
    fn signal(&self, name: &str) -> Option<Value> {
        match name {
            "count" => Some(json!(3)),
            _ => None,
        }
    }

    fn scale_domain(&self, name: &str) -> Option<Vec<Value>> {
        match name {
            "x" => Some(vec![json!(0), json!(10)]),
            _ => None,
        }
    }

    fn bandwidth(&self, name: &str) -> Option<f64> {
        match name {
            "x" => Some(12.5),
            _ => None,
        }
    }
}

fn eval(src: &str) -> Value {
    evaluate(src, None, &EmptyEnv).expect("expression should parse and evaluate")
}

fn eval_with(src: &str, datum: Value) -> Value {
    evaluate(src, Some(&datum), &EmptyEnv).expect("expression should parse and evaluate")
}

// ---------------------------------------------------------------------------
// Operators
// ---------------------------------------------------------------------------

#[test]
fn arithmetic_precedence_and_associativity() {
    assert_eq!(eval("1 + 2 * 3"), json!(7));
    assert_eq!(eval("(1 + 2) * 3"), json!(9));
    assert_eq!(eval("10 - 2 - 3"), json!(5));
    assert_eq!(eval("2 * 3 + 4 * 5"), json!(26));
    assert_eq!(eval("7 % 4"), json!(3));
}

#[test]
fn exponent_is_right_associative() {
    assert_eq!(eval("2 ** 3 ** 2"), json!(512));
    assert_eq!(eval("(2 ** 3) ** 2"), json!(64));
}

#[test]
fn unary_operators() {
    assert_eq!(eval("-3 + +4"), json!(1));
    assert_eq!(eval("-(2 ** 2)"), json!(-4));
    assert_eq!(eval("!0"), json!(true));
    assert_eq!(eval("!false"), json!(true));
}

#[test]
fn ternary_and_short_circuiting() {
    assert_eq!(eval("1 < 2 ? 'yes' : 'no'"), json!("yes"));
    assert_eq!(eval("1 > 2 ? 'yes' : 'no'"), json!("no"));
    assert_eq!(eval("1 < 2 ? (2 < 3 ? 'a' : 'b') : 'c'"), json!("a"));
    // && and || return operand values, as in JavaScript.
    assert_eq!(eval("0 && 'unused'"), json!(0));
    assert_eq!(eval("0 || 'fallback'"), json!("fallback"));
    assert_eq!(eval("'left' && 'right'"), json!("right"));
}

// ---------------------------------------------------------------------------
// Strings and comparisons
// ---------------------------------------------------------------------------

#[test]
fn string_concatenation() {
    assert_eq!(eval("'a' + 'b' + 'c'"), json!("abc"));
    assert_eq!(eval("'n=' + 1"), json!("n=1"));
    assert_eq!(eval("1 + '2'"), json!("12"));
}

#[test]
fn comparisons() {
    assert_eq!(eval("2 < 3"), json!(true));
    assert_eq!(eval("2 >= 2"), json!(true));
    assert_eq!(eval("3 <= 2"), json!(false));
    assert_eq!(eval("'apple' < 'banana'"), json!(true));
    assert_eq!(eval("'a' != 'b'"), json!(true));
}

#[test]
fn loose_vs_strict_equality() {
    assert_eq!(eval("1 == '1'"), json!(true));
    assert_eq!(eval("1 === '1'"), json!(false));
    assert_eq!(eval("1 != '1'"), json!(false));
    assert_eq!(eval("1 !== '1'"), json!(true));
    assert_eq!(eval("null == null"), json!(true));
    assert_eq!(eval("true == 1"), json!(true));
    assert_eq!(eval("true === 1"), json!(false));
}

// ---------------------------------------------------------------------------
// Member access, arrays and objects
// ---------------------------------------------------------------------------

#[test]
fn datum_member_access() {
    let datum = json!({"a": 4, "b": {"c": 5}, "list": [10, 20, 30]});
    assert_eq!(eval_with("datum.a", datum.clone()), json!(4));
    assert_eq!(eval_with("datum['a']", datum.clone()), json!(4));
    assert_eq!(eval_with("datum.b.c", datum.clone()), json!(5));
    assert_eq!(eval_with("datum['b']['c']", datum.clone()), json!(5));
    assert_eq!(eval_with("datum.list[1]", datum.clone()), json!(20));
    assert_eq!(eval_with("datum.list['2']", datum.clone()), json!(30));
    // A missing property is JavaScript `undefined`, not `null`.
    assert_ne!(eval_with("datum.missing", datum.clone()), Value::Null);
    assert_eq!(eval_with("datum.missing === null", datum.clone()), json!(false));
    assert_eq!(eval_with("isValid(datum.missing)", datum), json!(false));
}

#[test]
fn array_and_object_literals() {
    assert_eq!(eval("[1, 2, 3]"), json!([1, 2, 3]));
    assert_eq!(eval("[[1], [2, 3]]"), json!([[1], [2, 3]]));
    assert_eq!(eval("{a: 1, b: 'two'}"), json!({"a": 1, "b": "two"}));
    assert_eq!(eval("{'a b': 7}['a b']"), json!(7));
    assert_eq!(eval("({a: 1}).a"), json!(1));
    assert_eq!(eval("{a: [1, 2]}['a'][0]"), json!(1));
}

#[test]
fn array_and_string_length() {
    assert_eq!(eval("[1, 2, 3].length"), json!(3));
    assert_eq!(eval("length([1, 2, 3, 4])"), json!(4));
    assert_eq!(eval("'hello'.length"), json!(5));
    assert_eq!(eval("length('hello')"), json!(5));
}

// ---------------------------------------------------------------------------
// String functions
// ---------------------------------------------------------------------------

#[test]
fn string_case_and_whitespace() {
    assert_eq!(eval("upper('AbC')"), json!("ABC"));
    assert_eq!(eval("lower('AbC')"), json!("abc"));
    assert_eq!(eval("trim('  padded  ')"), json!("padded"));
}

#[test]
fn string_replace_split_and_search() {
    assert_eq!(eval("replace('a-b-a', 'a', 'X')"), json!("X-b-a"));
    assert_eq!(eval("split('a,b,c', ',')"), json!(["a", "b", "c"]));
    assert_eq!(eval("split('a,b,c', ',', 2)"), json!(["a", "b"]));
    assert_eq!(eval("split('abc', '')"), json!(["a", "b", "c"]));
    assert_eq!(eval("indexof('hello', 'll')"), json!(2));
    assert_eq!(eval("indexof('hello', 'z')"), json!(-1));
    assert_eq!(eval("lastindexof('abab', 'ab')"), json!(2));
}

#[test]
fn string_substring_and_slice() {
    assert_eq!(eval("substring('abcdef', 1, 3)"), json!("bc"));
    // substring swaps reversed indices.
    assert_eq!(eval("substring('abcdef', 4, 1)"), json!("bcd"));
    assert_eq!(eval("slice('abcdef', 1, 4)"), json!("bcd"));
    assert_eq!(eval("slice('abcdef', -3)"), json!("def"));
    assert_eq!(eval("slice([1, 2, 3, 4], 1, 3)"), json!([2, 3]));
}

#[test]
fn string_pad_truncate_and_join() {
    assert_eq!(eval("pad('7', 3, '0')"), json!("007"));
    assert_eq!(eval("pad('7', 3, '0', 'left')"), json!("700"));
    assert_eq!(eval("pad('7', 3, '0', 'center')"), json!("070"));
    assert_eq!(eval("truncate('abcdefgh', 5)"), json!("\u{2026}efgh"));
    assert_eq!(
        eval("truncate('abcdefgh', 5, 'left')"),
        json!("abcd\u{2026}")
    );
    assert_eq!(eval("join([1, 2, 3], '-')"), json!("1-2-3"));
    assert_eq!(eval("join(['a', 'b'])"), json!("a,b"));
}

// ---------------------------------------------------------------------------
// Math functions
// ---------------------------------------------------------------------------

#[test]
fn aggregate_math_functions() {
    assert_eq!(eval("min(3, 1, 2)"), json!(1));
    assert_eq!(eval("min([3, 1, 2])"), json!(1));
    assert_eq!(eval("max(3, 1, 2)"), json!(3));
    assert_eq!(eval("max([3, 1, 2])"), json!(3));
    assert_eq!(eval("sum([1, 2, 3, 4])"), json!(10));
    assert_eq!(eval("mean([1, 2, 3])"), json!(2));
    assert_eq!(eval("mean([1, 2, 3, 4])"), json!(2.5));
}

#[test]
fn scalar_math_functions() {
    assert_eq!(eval("round(2.5)"), json!(3));
    assert_eq!(eval("round(-2.5)"), json!(-3));
    assert_eq!(eval("floor(1.9)"), json!(1));
    assert_eq!(eval("floor(-1.5)"), json!(-2));
    assert_eq!(eval("ceil(1.2)"), json!(2));
    assert_eq!(eval("ceil(-1.2)"), json!(-1));
    assert_eq!(eval("abs(-4)"), json!(4));
    assert_eq!(eval("sqrt(9)"), json!(3));
    assert_eq!(eval("pow(2, 5)"), json!(32));
}

#[test]
fn interpolation_and_range_functions() {
    assert_eq!(eval("clamp(5, 0, 3)"), json!(3));
    assert_eq!(eval("clamp(-1, 0, 3)"), json!(0));
    assert_eq!(eval("clamp(2, 0, 3)"), json!(2));
    assert_eq!(eval("lerp(0, 10, 0.25)"), json!(2.5));
    assert_eq!(eval("span(2, 10)"), json!(8));
    assert_eq!(eval("inrange(5, 1, 10)"), json!(true));
    assert_eq!(eval("inrange(0, 1, 10)"), json!(false));
    // Bounds are inclusive and may be given in either order.
    assert_eq!(eval("inrange(1, 1, 10)"), json!(true));
    assert_eq!(eval("inrange(5, 10, 1)"), json!(true));
}

// ---------------------------------------------------------------------------
// Type predicates and coercions
// ---------------------------------------------------------------------------

#[test]
fn type_predicates() {
    assert_eq!(eval("isValid(0)"), json!(true));
    assert_eq!(eval("isValid(null)"), json!(false));
    assert_eq!(eval("isFinite(1)"), json!(true));
    assert_eq!(eval("isFinite('42')"), json!(true));
    assert_eq!(eval("isFinite('abc')"), json!(false));
    assert_eq!(eval("isNaN('abc')"), json!(true));
    assert_eq!(eval("isNaN(1)"), json!(false));
    assert_eq!(eval("isArray([1, 2])"), json!(true));
    assert_eq!(eval("isArray('1,2')"), json!(false));
    assert_eq!(eval("isObject({a: 1})"), json!(true));
    assert_eq!(eval("isObject([1])"), json!(false));
}

#[test]
fn coercions() {
    assert_eq!(eval("toNumber('42')"), json!(42));
    assert_eq!(eval("toNumber(' 3.5 ')"), json!(3.5));
    assert_eq!(eval("toNumber('')"), Value::Null);
    assert_eq!(eval("toNumber(null)"), Value::Null);
    assert_eq!(eval("toNumber(true)"), json!(1));
    assert_eq!(eval("toString(42)"), json!("42"));
    assert_eq!(eval("toString(true)"), json!("true"));
    assert_eq!(eval("toString(null)"), json!("null"));
    assert_eq!(eval("toString([1, 2])"), json!("1,2"));
}

// ---------------------------------------------------------------------------
// Formatting and dates
// ---------------------------------------------------------------------------

#[test]
fn number_formatting() {
    assert_eq!(eval("format(1234.5678, ',.2f')"), json!("1,234.57"));
    assert_eq!(eval("format(0.1234, '.1%')"), json!("12.3%"));
    assert_eq!(eval("format(1500, '.2~s')"), json!("1.5k"));
    assert_eq!(eval("format(42, 'd')"), json!("42"));
}

#[test]
fn time_formatting_with_known_dates() {
    assert_eq!(
        eval("timeFormat(datetime(2020, 0, 2), '%Y-%m-%d')"),
        json!("2020-01-02")
    );
    assert_eq!(
        eval("utcFormat(utcParse('2020-01-02T03:04:05.006Z'), '%H:%M:%S.%L')"),
        json!("03:04:05.006")
    );
    assert_eq!(
        eval("timeFormat(timeParse('2020-01-02'), '%Y/%m/%d')"),
        json!("2020/01/02")
    );
}

#[test]
fn date_parts() {
    // 2020-01-02 was a Thursday, so day() is 4 (Sunday = 0).
    assert_eq!(eval("year(toDate('2020-01-02 03:04:05'))"), json!(2020));
    assert_eq!(eval("month(toDate('2020-01-02 03:04:05'))"), json!(0));
    assert_eq!(eval("date(toDate('2020-01-02 03:04:05'))"), json!(2));
    assert_eq!(eval("day(toDate('2020-01-02 03:04:05'))"), json!(4));
    assert_eq!(eval("hours(toDate('2020-01-02 03:04:05'))"), json!(3));
    assert_eq!(eval("minutes(toDate('2020-01-02 03:04:05'))"), json!(4));
    assert_eq!(eval("month(toDate('2020-12-31 00:00:00'))"), json!(11));
}

// ---------------------------------------------------------------------------
// Signals and environment-provided scale information
// ---------------------------------------------------------------------------

#[test]
fn signals_come_from_the_environment() {
    let env = TestEnv::new()
        .with("threshold", json!(21))
        .with("label", json!("hi"));
    assert_eq!(evaluate("threshold * 2", None, &env).unwrap(), json!(42));
    assert_eq!(evaluate("label + '!'", None, &env).unwrap(), json!("hi!"));
    assert_eq!(
        evaluate("datum.v > threshold", Some(&json!({"v": 22})), &env).unwrap(),
        json!(true)
    );
    // Unknown identifiers resolve to null.
    assert_eq!(evaluate("missing_signal", None, &env).unwrap(), Value::Null);
}

#[test]
fn domain_and_bandwidth_come_from_the_environment() {
    assert_eq!(
        evaluate("domain('x')", None, &ScaleEnv).unwrap(),
        json!([0, 10])
    );
    assert_eq!(
        evaluate("bandwidth('x')", None, &ScaleEnv).unwrap(),
        json!(12.5)
    );
    assert_eq!(
        evaluate("domain('x')[0] + bandwidth('x')", None, &ScaleEnv).unwrap(),
        json!(12.5)
    );
    // Missing scales yield null / zero rather than failing.
    assert_eq!(
        evaluate("domain('nope')", None, &ScaleEnv).unwrap(),
        Value::Null
    );
    assert_eq!(
        evaluate("bandwidth('nope')", None, &ScaleEnv).unwrap(),
        json!(0)
    );
    assert_eq!(evaluate("count", None, &ScaleEnv).unwrap(), json!(3));
}

// ---------------------------------------------------------------------------
// Collection helpers
// ---------------------------------------------------------------------------

#[test]
fn sequence_generates_arithmetic_progressions() {
    assert_eq!(eval("sequence(0, 5, 2)"), json!([0, 2, 4]));
    assert_eq!(eval("sequence(1, 4)"), json!([1, 2, 3]));
    assert_eq!(eval("sequence(3, 0, -1)"), json!([3, 2, 1]));
    assert_eq!(eval("sequence(0, 0)"), json!([]));
}

#[test]
fn extent_and_pluck() {
    assert_eq!(eval("extent([3, 1, 4, 1, 5])"), json!([1, 5]));
    assert_eq!(eval("extent([])"), json!([null, null]));
    assert_eq!(eval("pluck([{a: 1}, {a: 2}], 'a')"), json!([1, 2]));
    assert_eq!(
        eval("pluck([{a: {b: 1}}, {a: {b: 2}}], 'a.b')"),
        json!([1, 2])
    );
}

#[test]
fn merge_combines_objects_left_to_right() {
    assert_eq!(eval("merge({a: 1}, {b: 2})"), json!({"a": 1, "b": 2}));
    assert_eq!(
        eval("merge({a: 1, b: 2}, {a: 3}, {c: 4})"),
        json!({"a": 3, "b": 2, "c": 4})
    );
    assert_eq!(eval("merge({a: 1}, 5, null)"), json!({"a": 1}));
}

// ---------------------------------------------------------------------------
// Parse errors
// ---------------------------------------------------------------------------

#[test]
fn malformed_expressions_return_errors() {
    let cases = [
        "",
        "1 +",
        "(1 + 2",
        "[1, 2",
        "{a: 1",
        "1 2",
        "1 @ 2",
        "foo(1,)",
        "foo.bar()",
        "datum.",
        "? 1 : 2",
    ];
    for src in cases {
        assert!(
            evaluate(src, None, &EmptyEnv).is_err(),
            "expected an error for malformed expression {src:?}"
        );
    }
}

//! Port of `test/compile/axis/properties.test.ts`.
//!
//! The upstream tests evaluate the compiled Vega signal expressions with
//! `vega-expression`.  This port includes a tiny evaluator for the subset of
//! JavaScript expressions produced by `properties.ts` (comparisons, `%`,
//! ternary, `&&`, `||`, string/number literals and the `a`/`o` globals).

use serde_json::{json, Number, Value};

use vega_lite::compile::axis::config::get_axis_configs;
use vega_lite::compile::axis::properties::{
    self, axis_rule, default_label_align, default_label_baseline, get_label_angle,
    is_axis_rule, AxisRuleParams,
};
use vega_lite::compile::model::Model;
use vega_lite::compile::support::init_config;
use vega_lite::util::normalize_angle;

// ---------------------------------------------------------------------------
// Tiny Vega expression evaluator
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Val {
    Num(f64),
    Str(String),
    Bool(bool),
    Null,
}

impl Val {
    fn truthy(&self) -> bool {
        match self {
            Val::Num(n) => *n != 0.0 && !n.is_nan(),
            Val::Str(s) => !s.is_empty(),
            Val::Bool(b) => *b,
            Val::Null => false,
        }
    }

    fn to_json(&self) -> Value {
        match self {
            Val::Num(n) => {
                if n.fract() == 0.0 && n.abs() < 9e15 {
                    json!(*n as i64)
                } else {
                    Number::from_f64(*n).map(Value::Number).unwrap_or(Value::Null)
                }
            }
            Val::Str(s) => json!(s),
            Val::Bool(b) => json!(b),
            Val::Null => Value::Null,
        }
    }
}

struct Parser<'a> {
    chars: Vec<char>,
    pos: usize,
    vars: &'a std::collections::HashMap<String, Val>,
}

impl<'a> Parser<'a> {
    fn new(input: &str, vars: &'a std::collections::HashMap<String, Val>) -> Self {
        Parser {
            chars: input.chars().collect(),
            pos: 0,
            vars,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while self.peek().map(char::is_whitespace).unwrap_or(false) {
            self.pos += 1;
        }
    }

    fn eat(&mut self, s: &str) -> bool {
        self.skip_ws();
        let target: Vec<char> = s.chars().collect();
        if self.chars[self.pos..].starts_with(&target) {
            self.pos += target.len();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, c: char) {
        self.skip_ws();
        assert_eq!(self.peek(), Some(c), "expected {c} at {}", self.pos);
        self.pos += 1;
    }

    fn parse(&mut self) -> Val {
        let v = self.ternary();
        self.skip_ws();
        assert!(self.pos >= self.chars.len(), "trailing input in expression");
        v
    }

    fn ternary(&mut self) -> Val {
        let cond = self.or();
        if self.eat("?") {
            let then = self.ternary();
            self.expect(':');
            let other = self.ternary();
            return if cond.truthy() { then } else { other };
        }
        cond
    }

    fn or(&mut self) -> Val {
        let mut left = self.and();
        while self.eat("||") {
            let right = self.and();
            left = if left.truthy() { left } else { right };
        }
        left
    }

    fn and(&mut self) -> Val {
        let mut left = self.equality();
        while self.eat("&&") {
            let right = self.equality();
            left = if left.truthy() { right } else { left };
        }
        left
    }

    fn equality(&mut self) -> Val {
        let mut left = self.relational();
        loop {
            if self.eat("===") {
                let right = self.relational();
                left = Val::Bool(strict_eq(&left, &right));
            } else if self.eat("!==") {
                let right = self.relational();
                left = Val::Bool(!strict_eq(&left, &right));
            } else if self.eat("==") {
                let right = self.relational();
                left = Val::Bool(strict_eq(&left, &right));
            } else if self.eat("!=") {
                let right = self.relational();
                left = Val::Bool(!strict_eq(&left, &right));
            } else {
                break;
            }
        }
        left
    }

    fn relational(&mut self) -> Val {
        let mut left = self.additive();
        loop {
            if self.eat("<=") {
                let right = self.additive();
                left = Val::Bool(num(&left) <= num(&right));
            } else if self.eat(">=") {
                let right = self.additive();
                left = Val::Bool(num(&left) >= num(&right));
            } else if self.eat("<") {
                let right = self.additive();
                left = Val::Bool(num(&left) < num(&right));
            } else if self.eat(">") {
                let right = self.additive();
                left = Val::Bool(num(&left) > num(&right));
            } else {
                break;
            }
        }
        left
    }

    fn additive(&mut self) -> Val {
        let mut left = self.multiplicative();
        loop {
            if self.eat("+") {
                let right = self.multiplicative();
                left = Val::Num(num(&left) + num(&right));
            } else if self.eat("-") {
                let right = self.multiplicative();
                left = Val::Num(num(&left) - num(&right));
            } else {
                break;
            }
        }
        left
    }

    fn multiplicative(&mut self) -> Val {
        let mut left = self.unary();
        loop {
            if self.eat("%") {
                let right = self.unary();
                left = Val::Num(num(&left) % num(&right));
            } else if self.eat("*") {
                let right = self.unary();
                left = Val::Num(num(&left) * num(&right));
            } else if self.eat("/") {
                let right = self.unary();
                left = Val::Num(num(&left) / num(&right));
            } else {
                break;
            }
        }
        left
    }

    fn unary(&mut self) -> Val {
        if self.eat("!") {
            return Val::Bool(!self.unary().truthy());
        }
        if self.eat("-") {
            return Val::Num(-num(&self.unary()));
        }
        self.primary()
    }

    fn primary(&mut self) -> Val {
        self.skip_ws();
        match self.peek() {
            Some('(') => {
                self.pos += 1;
                let v = self.ternary();
                self.expect(')');
                v
            }
            Some('"') | Some('\'') => {
                let quote = self.peek().unwrap();
                self.pos += 1;
                let mut s = String::new();
                while let Some(c) = self.peek() {
                    self.pos += 1;
                    if c == '\\' {
                        if let Some(next) = self.peek() {
                            self.pos += 1;
                            s.push(next);
                        }
                    } else if c == quote {
                        break;
                    } else {
                        s.push(c);
                    }
                }
                Val::Str(s)
            }
            Some(c) if c.is_ascii_digit() || c == '.' => {
                let start = self.pos;
                while self
                    .peek()
                    .map(|c| c.is_ascii_digit() || c == '.' || c == 'e' || c == 'E' || c == '-' || c == '+')
                    .unwrap_or(false)
                {
                    self.pos += 1;
                }
                let text: String = self.chars[start..self.pos].iter().collect();
                Val::Num(text.parse().expect("number"))
            }
            Some(c) if c.is_alphabetic() || c == '_' => {
                let start = self.pos;
                while self
                    .peek()
                    .map(|c| c.is_alphanumeric() || c == '_')
                    .unwrap_or(false)
                {
                    self.pos += 1;
                }
                let ident: String = self.chars[start..self.pos].iter().collect();
                match ident.as_str() {
                    "null" => Val::Null,
                    "true" => Val::Bool(true),
                    "false" => Val::Bool(false),
                    name => self.vars.get(name).cloned().unwrap_or(Val::Null),
                }
            }
            other => panic!("unexpected token {other:?} in expression"),
        }
    }
}

fn num(v: &Val) -> f64 {
    match v {
        Val::Num(n) => *n,
        Val::Bool(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Val::Null => 0.0,
        Val::Str(s) => s.parse().unwrap_or(f64::NAN),
    }
}

fn strict_eq(a: &Val, b: &Val) -> bool {
    match (a, b) {
        (Val::Num(x), Val::Num(y)) => x == y,
        (Val::Str(x), Val::Str(y)) => x == y,
        (Val::Bool(x), Val::Bool(y)) => x == y,
        (Val::Null, Val::Null) => true,
        _ => false,
    }
}

fn eval_signal(signal: &str, vars: &[(&str, Val)]) -> Val {
    let map: std::collections::HashMap<String, Val> = vars
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    Parser::new(signal, &map).parse()
}

fn eval_value_or_signal(value: Option<&Value>, orient: &str) -> Value {
    match value {
        None => Value::Null,
        Some(Value::Object(map)) => {
            if let Some(signal) = map.get("signal").and_then(Value::as_str) {
                eval_signal(signal, &[("o", Val::Str(orient.to_string()))]).to_json()
            } else {
                Value::Null
            }
        }
        Some(v) => v.clone(),
    }
}

fn range_angles() -> Vec<f64> {
    (0..49).map(|i| -360.0 + 15.0 * i as f64).collect()
}

fn num_json(v: &Value) -> Option<f64> {
    v.as_f64()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_unit_model_with_scale_and_layout_size(spec: &Value) -> Model {
    let config = init_config(spec.get("config"));
    let mut model = Model::new_unit(spec, None, "", None, &config);
    model.parse_scale();
    model.parse_layout_size();
    model
}

fn rule_params<'a>(model: &'a Model, channel: &'a str, axis: Value) -> AxisRuleParams<'a> {
    AxisRuleParams {
        field_or_datum_def: model
            .encoding()
            .and_then(|e| vega_lite::channeldef::get_field_or_datum_def(&e[channel])),
        axis,
        channel: channel.to_string(),
        model,
        mark: model.mark_type(),
        scale_type: model.get_scale_component(channel).and_then(|c| c.get_type()),
        orient: json!(if channel == "x" { "bottom" } else { "left" }),
        label_angle: None,
        format: None,
        format_type: None,
        config: model.config.clone(),
    }
}

// ---------------------------------------------------------------------------
// defaultGrid()
// ---------------------------------------------------------------------------

#[test]
fn default_grid_returns_true_by_default_for_continuous_scale_not_binned() {
    assert!(
        properties::default_grid(
            Some(vega_lite::scale::ScaleType::Linear),
            &json!({"field": "a", "type": "quantitative"})
        )
    );
}

#[test]
fn default_grid_returns_false_by_default_for_binned_field() {
    assert!(
        !properties::default_grid(
            Some(vega_lite::scale::ScaleType::Linear),
            &json!({"bin": true, "field": "a", "type": "quantitative"})
        )
    );
}

#[test]
fn default_grid_returns_false_by_default_for_discrete_scale() {
    assert!(
        !properties::default_grid(
            Some(vega_lite::scale::ScaleType::Point),
            &json!({"field": "a", "type": "quantitative"})
        )
    );
}

// ---------------------------------------------------------------------------
// bandPosition
// ---------------------------------------------------------------------------

#[test]
fn band_position_y_axis_is_1_for_bar_with_discrete_y_and_continuous_yoffset() {
    let model = parse_unit_model_with_scale_and_layout_size(&json!({
        "mark": "bar",
        "data": {"values": [{"a": "A", "b": 1, "c": "x"}]},
        "encoding": {
            "x": {"field": "a", "type": "nominal"},
            "y": {"field": "c", "type": "nominal"},
            "yOffset": {"field": "b", "type": "quantitative"},
        },
    }));
    let params = rule_params(&model, "y", json!({}));
    assert_eq!(axis_rule("bandPosition", &params), Some(json!(1)));
}

#[test]
fn band_position_x_axis_is_0_for_bar_with_discrete_x_and_continuous_xoffset() {
    let model = parse_unit_model_with_scale_and_layout_size(&json!({
        "mark": "bar",
        "data": {"values": [{"a": "A", "b": 1, "c": "x"}]},
        "encoding": {
            "x": {"field": "c", "type": "nominal"},
            "xOffset": {"field": "b", "type": "quantitative"},
            "y": {"field": "a", "type": "nominal"},
        },
    }));
    let params = rule_params(&model, "x", json!({}));
    assert_eq!(axis_rule("bandPosition", &params), Some(json!(0)));
}

#[test]
fn band_position_respects_explicit_axis_band_position() {
    let model = parse_unit_model_with_scale_and_layout_size(&json!({
        "mark": "bar",
        "data": {"values": [{"a": "A", "b": 1, "c": "x"}]},
        "encoding": {
            "x": {"field": "a", "type": "nominal"},
            "y": {"field": "c", "type": "nominal", "axis": {"bandPosition": 0.25}},
            "yOffset": {"field": "b", "type": "quantitative"},
        },
    }));
    let params = rule_params(&model, "y", json!({"bandPosition": 0.25}));
    assert_eq!(axis_rule("bandPosition", &params), Some(json!(0.25)));
}

// ---------------------------------------------------------------------------
// orient()
// ---------------------------------------------------------------------------

#[test]
fn orient_returns_bottom_for_x_by_default() {
    assert_eq!(properties::default_orient("x"), json!("bottom"));
}

#[test]
fn orient_returns_left_for_y_by_default() {
    assert_eq!(properties::default_orient("y"), json!("left"));
}

// ---------------------------------------------------------------------------
// defaultTickCount()
// ---------------------------------------------------------------------------

#[test]
fn default_tick_count_returns_size_over_10_by_default_for_binned_field() {
    let tick_count = properties::default_tick_count(
        Some(&json!({"bin": {"maxbins": 10}, "field": "a", "type": "quantitative"})),
        Some(vega_lite::scale::ScaleType::Linear),
        Some(&json!({"signal": "a"})),
        None,
    );
    assert_eq!(tick_count, Some(json!({"signal": "ceil(a/10)"})));
}

#[test]
fn default_tick_count_returns_undefined_if_values_specified() {
    let tick_count = properties::default_tick_count(
        Some(&json!({"bin": {"maxbins": 10}, "field": "a", "type": "quantitative"})),
        Some(vega_lite::scale::ScaleType::Linear),
        Some(&json!({"signal": "a"})),
        Some(&json!([1, 2, 3])),
    );
    assert_eq!(tick_count, None);
}

#[test]
fn default_tick_count_undefined_for_temporal_time_units() {
    for time_unit in ["month", "hours", "day", "quarter"] {
        let tick_count = properties::default_tick_count(
            Some(&json!({"timeUnit": time_unit, "field": "a", "type": "temporal"})),
            Some(vega_lite::scale::ScaleType::Linear),
            Some(&json!({"signal": "a"})),
            None,
        );
        assert_eq!(tick_count, None, "timeUnit {time_unit}");
    }
}

#[test]
fn default_tick_count_returns_size_over_40_for_linear_scale() {
    let tick_count = properties::default_tick_count(
        Some(&json!({"field": "a", "type": "quantitative"})),
        Some(vega_lite::scale::ScaleType::Linear),
        Some(&json!({"signal": "a"})),
        None,
    );
    assert_eq!(tick_count, Some(json!({"signal": "ceil(a/40)"})));
}

#[test]
fn default_tick_count_undefined_for_log_scale() {
    let tick_count = properties::default_tick_count(
        Some(&json!({"field": "a", "type": "quantitative"})),
        Some(vega_lite::scale::ScaleType::Log),
        None,
        None,
    );
    assert_eq!(tick_count, None);
}

#[test]
fn default_tick_count_undefined_for_point_scale() {
    let tick_count = properties::default_tick_count(
        Some(&json!({"field": "a", "type": "quantitative"})),
        Some(vega_lite::scale::ScaleType::Point),
        None,
        None,
    );
    assert_eq!(tick_count, None);
}

// ---------------------------------------------------------------------------
// defaultTickMinStep()
// ---------------------------------------------------------------------------

#[test]
fn default_tick_min_step_returns_1_for_integer() {
    let tick_min_step = properties::default_tick_min_step(
        Some(&json!("d")),
        Some(&json!({"field": "a", "type": "quantitative"})),
    );
    assert_eq!(tick_min_step, Some(json!(1)));
}

#[test]
fn default_tick_min_step_returns_timestamp_durations() {
    let cases: Vec<(Value, &str)> = vec![
        (
            json!("year"),
            "datetime(2002, 0, 1, 0, 0, 0, 0) - datetime(2001, 0, 1, 0, 0, 0, 0)",
        ),
        (
            json!({"unit": "year", "step": 2}),
            "datetime(2003, 0, 1, 0, 0, 0, 0) - datetime(2001, 0, 1, 0, 0, 0, 0)",
        ),
        (
            json!("yearmonth"),
            "datetime(2001, 1, 1, 0, 0, 0, 0) - datetime(2001, 0, 1, 0, 0, 0, 0)",
        ),
        (
            json!({"unit": "yearmonth", "step": 4}),
            "datetime(2001, 4, 1, 0, 0, 0, 0) - datetime(2001, 0, 1, 0, 0, 0, 0)",
        ),
        (
            json!("month"),
            "datetime(2001, 1, 1, 0, 0, 0, 0) - datetime(2001, 0, 1, 0, 0, 0, 0)",
        ),
        (
            json!("yearquarter"),
            "datetime(2001, 3, 1, 0, 0, 0, 0) - datetime(2001, 0, 1, 0, 0, 0, 0)",
        ),
        (
            json!("yearmonthdate"),
            "datetime(2001, 0, 2, 0, 0, 0, 0) - datetime(2001, 0, 1, 0, 0, 0, 0)",
        ),
    ];

    for (time_unit, signal) in cases {
        let tick_min_step = properties::default_tick_min_step(
            None,
            Some(&json!({"timeUnit": time_unit, "field": "a", "type": "temporal"})),
        );
        assert_eq!(
            tick_min_step,
            Some(json!({"signal": signal})),
            "timeUnit {time_unit}"
        );
    }
}

// ---------------------------------------------------------------------------
// values
// ---------------------------------------------------------------------------

#[test]
fn values_returns_correct_timestamp_values_for_datetimes() {
    let values = properties::values(
        &json!({"values": [{"year": 1970}, {"year": 1980}]}),
        &json!({"field": "a", "type": "temporal"}),
    );
    assert_eq!(
        values,
        Some(json!([
            {"signal": "datetime(1970, 0, 1, 0, 0, 0, 0)"},
            {"signal": "datetime(1980, 0, 1, 0, 0, 0, 0)"}
        ]))
    );
}

#[test]
fn values_simply_returns_values_for_non_datetime() {
    let values = properties::values(
        &json!({"values": [1, 2, 3, 4]}),
        &json!({"field": "a", "type": "quantitative"}),
    );
    assert_eq!(values, Some(json!([1, 2, 3, 4])));
}

#[test]
fn values_returns_signal_correctly_for_non_datetime() {
    let values = properties::values(
        &json!({"values": {"signal": "a"}}),
        &json!({"field": "a", "type": "quantitative"}),
    );
    assert_eq!(values, Some(json!({"signal": "a"})));
}

#[test]
fn values_simply_drops_values_when_domain_is_specified() {
    let values = properties::values(
        &json!({}),
        &json!({"type": "quantitative", "field": "US_Gross", "bin": {"extent": [0, 1]}}),
    );
    assert_eq!(values, None);
}

// ---------------------------------------------------------------------------
// labelAngle
// ---------------------------------------------------------------------------

#[test]
fn label_angle_returns_correct_label_angle_from_axis_definition() {
    let axis = json!({"labelAngle": 600});
    assert_eq!(
        num_json(&get_label_angle(
            Some(&json!({"type": "quantitative", "field": "US_Gross", "axis": axis})),
            &axis,
            "y",
            &json!({}),
            None
        )
        .unwrap()),
        Some(240.0)
    );
}

#[test]
fn label_angle_returns_correct_label_angle_from_axis_config_definition() {
    let config = json!({"axis": {"labelAngle": 500}});
    let axis_configs = get_axis_configs(
        "y",
        Some(vega_lite::scale::ScaleType::Linear),
        &json!("left"),
        &config,
    );
    assert_eq!(
        num_json(&get_label_angle(
            Some(&json!({"type": "quantitative", "field": "US_Gross"})),
            &json!({}),
            "y",
            &config,
            Some(&axis_configs)
        )
        .unwrap()),
        Some(140.0)
    );
}

#[test]
fn label_angle_returns_correct_default_label_angle_when_not_specified() {
    assert_eq!(
        get_label_angle(Some(&json!({"field": "a", "type": "ordinal"})), &json!({}), "x", &json!({}), None),
        Some(json!(270))
    );
}

#[test]
fn label_angle_axis_wins_over_axis_config() {
    let axis = json!({"labelAngle": 600});
    let config = json!({"axis": {"labelAngle": 500}});
    assert_eq!(
        num_json(&get_label_angle(
            Some(&json!({"type": "quantitative", "field": "US_Gross", "axis": axis})),
            &axis,
            "y",
            &config,
            None
        )
        .unwrap()),
        Some(240.0)
    );
}

#[test]
fn label_angle_returns_undefined_when_no_default_and_no_specified() {
    assert_eq!(
        get_label_angle(
            Some(&json!({"type": "quantitative", "field": "US_Gross"})),
            &json!({}),
            "y",
            &json!({}),
            None
        ),
        None
    );
}

// ---------------------------------------------------------------------------
// defaultLabelAlign
// ---------------------------------------------------------------------------

#[test]
fn default_label_align_correctly_aligns_x_labels_for_all_degrees() {
    assert_eq!(
        default_label_align(Some(&json!(0)), Some(&json!("top")), "x"),
        Some(Value::Null)
    );
    for angle in [15, 30, 45, 60, 75, 90, 105, 120, 135, 150, 165] {
        assert_eq!(
            default_label_align(Some(&json!(angle)), Some(&json!("top")), "x"),
            Some(json!("right")),
            "angle {angle}"
        );
    }
    assert_eq!(
        default_label_align(Some(&json!(180)), Some(&json!("top")), "x"),
        Some(Value::Null)
    );
    for angle in [195, 210, 225, 240, 255, 270, 285, 300, 315, 330, 345] {
        assert_eq!(
            default_label_align(Some(&json!(angle)), Some(&json!("bottom")), "x"),
            Some(json!("right")),
            "angle {angle}"
        );
    }
}

#[test]
fn default_label_align_correctly_aligns_y_labels_for_all_degrees() {
    for angle in [0, 15, 30, 45, 60, 75] {
        assert_eq!(
            default_label_align(Some(&json!(angle)), Some(&json!("left")), "y"),
            Some(json!("right")),
            "angle {angle}"
        );
    }
    assert_eq!(
        default_label_align(Some(&json!(90)), Some(&json!("left")), "y"),
        Some(json!("center"))
    );
    for angle in [105, 120, 135, 150, 165, 180] {
        assert_eq!(
            default_label_align(Some(&json!(angle)), Some(&json!("left")), "y"),
            Some(json!("left")),
            "angle {angle}"
        );
    }
    for angle in [195, 210, 225, 240, 255] {
        assert_eq!(
            default_label_align(Some(&json!(angle)), Some(&json!("right")), "y"),
            Some(json!("right")),
            "angle {angle}"
        );
    }
    assert_eq!(
        default_label_align(Some(&json!(270)), Some(&json!("right")), "y"),
        Some(json!("center"))
    );
    for angle in [285, 300, 315, 330, 345] {
        assert_eq!(
            default_label_align(Some(&json!(angle)), Some(&json!("right")), "y"),
            Some(json!("left")),
            "angle {angle}"
        );
    }
}

#[test]
fn default_label_align_returns_undefined_if_angle_undefined() {
    assert_eq!(default_label_align(None, Some(&json!("left")), "y"), None);
}

#[test]
fn default_label_align_y_labels_for_label_angle_and_orient_signals() {
    let signal = default_label_align(Some(&json!({"signal": "a"})), Some(&json!({"signal": "o"})), "y")
        .unwrap();
    let signal = signal.get("signal").unwrap().as_str().unwrap();
    for angle in range_angles() {
        for orient in ["left", "right"] {
            let actual = eval_signal(
                signal,
                &[
                    ("a", Val::Num(angle)),
                    ("o", Val::Str(orient.to_string())),
                ],
            )
            .to_json();
            let expected = default_label_align(
                Some(&json!(normalize_angle(Some(angle)).unwrap())),
                Some(&json!(orient)),
                "y",
            )
            .unwrap();
            assert_eq!(actual, expected, "angle {angle} orient {orient}");
        }
    }
}

#[test]
fn default_label_align_x_labels_for_label_angle_and_orient_signals() {
    let signal = default_label_align(Some(&json!({"signal": "a"})), Some(&json!({"signal": "o"})), "x")
        .unwrap();
    let signal = signal.get("signal").unwrap().as_str().unwrap();
    for angle in range_angles() {
        for orient in ["top", "bottom"] {
            let actual = eval_signal(
                signal,
                &[
                    ("a", Val::Num(angle)),
                    ("o", Val::Str(orient.to_string())),
                ],
            )
            .to_json();
            let expected = default_label_align(
                Some(&json!(normalize_angle(Some(angle)).unwrap())),
                Some(&json!(orient)),
                "x",
            )
            .unwrap();
            assert_eq!(actual, expected, "angle {angle} orient {orient}");
        }
    }
}

#[test]
fn default_label_align_y_labels_for_orient_signal() {
    for angle in range_angles() {
        let a = normalize_angle(Some(angle)).unwrap();
        let align = default_label_align(Some(&json!(a)), Some(&json!({"signal": "o"})), "y");
        for orient in ["left", "right"] {
            let actual = eval_value_or_signal(align.as_ref(), orient);
            let expected = default_label_align(Some(&json!(a)), Some(&json!(orient)), "y").unwrap();
            assert_eq!(actual, expected, "angle {angle} orient {orient}");
        }
    }
}

#[test]
fn default_label_align_x_labels_for_orient_signal() {
    for angle in range_angles() {
        let a = normalize_angle(Some(angle)).unwrap();
        let align = default_label_align(Some(&json!(a)), Some(&json!({"signal": "o"})), "x");
        for orient in ["top", "bottom"] {
            let actual = eval_value_or_signal(align.as_ref(), orient);
            let expected = default_label_align(Some(&json!(a)), Some(&json!(orient)), "x").unwrap();
            assert_eq!(actual, expected, "angle {angle} orient {orient}");
        }
    }
}

// ---------------------------------------------------------------------------
// defaultLabelBaseline
// ---------------------------------------------------------------------------

#[test]
fn default_label_baseline_is_middle_for_perpendiculars_horizontal_orients() {
    assert_eq!(
        default_label_baseline(Some(&json!(90)), Some(&json!("top")), "x", false),
        Some(json!("middle"))
    );
    assert_eq!(
        default_label_baseline(Some(&json!(270)), Some(&json!("bottom")), "x", false),
        Some(json!("middle"))
    );
}

#[test]
fn default_label_baseline_is_top_for_bottom_orients_first_and_fourth_quadrants() {
    assert_eq!(
        default_label_baseline(Some(&json!(45)), Some(&json!("bottom")), "x", false),
        Some(json!("top"))
    );
    assert_eq!(
        default_label_baseline(Some(&json!(180)), Some(&json!("top")), "x", false),
        Some(json!("top"))
    );
}

#[test]
fn default_label_baseline_is_bottom_for_bottom_orients_second_and_third_quadrants() {
    assert_eq!(
        default_label_baseline(Some(&json!(100)), Some(&json!("bottom")), "x", false),
        Some(json!("middle"))
    );
    assert_eq!(
        default_label_baseline(Some(&json!(260)), Some(&json!("bottom")), "x", false),
        Some(json!("middle"))
    );
}

#[test]
fn default_label_baseline_is_middle_for_0_and_180_horizontal_orients() {
    assert_eq!(
        default_label_baseline(Some(&json!(0)), Some(&json!("left")), "y", false),
        Some(Value::Null)
    );
    assert_eq!(
        default_label_baseline(Some(&json!(180)), Some(&json!("right")), "y", false),
        Some(Value::Null)
    );
}

#[test]
fn default_label_baseline_is_top_for_bottom_orients_first_and_second_quadrants() {
    assert_eq!(
        default_label_baseline(Some(&json!(80)), Some(&json!("left")), "y", false),
        Some(json!("top"))
    );
    assert_eq!(
        default_label_baseline(Some(&json!(100)), Some(&json!("left")), "y", false),
        Some(json!("top"))
    );
}

#[test]
fn default_label_baseline_is_bottom_for_bottom_orients_third_and_fourth_quadrants() {
    assert_eq!(
        default_label_baseline(Some(&json!(280)), Some(&json!("left")), "y", false),
        Some(json!("bottom"))
    );
    assert_eq!(
        default_label_baseline(Some(&json!(260)), Some(&json!("left")), "y", false),
        Some(json!("bottom"))
    );
}

#[test]
fn default_label_baseline_returns_undefined_if_angle_undefined() {
    assert_eq!(
        default_label_baseline(None, Some(&json!("left")), "y", false),
        None
    );
}

#[test]
fn default_label_baseline_y_labels_for_label_angle_and_orient_signals() {
    let signal =
        default_label_baseline(Some(&json!({"signal": "a"})), Some(&json!({"signal": "o"})), "y", false)
            .unwrap();
    let signal = signal.get("signal").unwrap().as_str().unwrap();
    for angle in range_angles() {
        for orient in ["left", "right"] {
            let actual = eval_signal(
                signal,
                &[
                    ("a", Val::Num(angle)),
                    ("o", Val::Str(orient.to_string())),
                ],
            )
            .to_json();
            let expected = default_label_baseline(
                Some(&json!(normalize_angle(Some(angle)).unwrap())),
                Some(&json!(orient)),
                "y",
                false,
            )
            .unwrap();
            assert_eq!(actual, expected, "angle {angle} orient {orient}");
        }
    }
}

#[test]
fn default_label_baseline_x_labels_for_label_angle_and_orient_signals() {
    let signal =
        default_label_baseline(Some(&json!({"signal": "a"})), Some(&json!({"signal": "o"})), "x", false)
            .unwrap();
    let signal = signal.get("signal").unwrap().as_str().unwrap();
    for angle in range_angles() {
        for orient in ["top", "bottom"] {
            let actual = eval_signal(
                signal,
                &[
                    ("a", Val::Num(angle)),
                    ("o", Val::Str(orient.to_string())),
                ],
            )
            .to_json();
            let expected = default_label_baseline(
                Some(&json!(normalize_angle(Some(angle)).unwrap())),
                Some(&json!(orient)),
                "x",
                false,
            )
            .unwrap();
            assert_eq!(actual, expected, "angle {angle} orient {orient}");
        }
    }
}

#[test]
fn default_label_baseline_y_labels_for_orient_signal() {
    for angle in range_angles() {
        let a = normalize_angle(Some(angle)).unwrap();
        let baseline = default_label_baseline(Some(&json!(a)), Some(&json!({"signal": "o"})), "y", false);
        for orient in ["left", "right"] {
            let actual = eval_value_or_signal(baseline.as_ref(), orient);
            let expected =
                default_label_baseline(Some(&json!(a)), Some(&json!(orient)), "y", false).unwrap();
            assert_eq!(actual, expected, "angle {angle} orient {orient}");
        }
    }
}

#[test]
fn default_label_baseline_x_labels_for_orient_signal() {
    for angle in range_angles() {
        let a = normalize_angle(Some(angle)).unwrap();
        let baseline = default_label_baseline(Some(&json!(a)), Some(&json!({"signal": "o"})), "x", false);
        for orient in ["top", "bottom"] {
            let actual = eval_value_or_signal(baseline.as_ref(), orient);
            let expected =
                default_label_baseline(Some(&json!(a)), Some(&json!(orient)), "x", false).unwrap();
            assert_eq!(actual, expected, "angle {angle} orient {orient}");
        }
    }
}

// ---------------------------------------------------------------------------
// defaultLabelOverlap
// ---------------------------------------------------------------------------

#[test]
fn default_label_overlap_returns_true_for_time_unit_ordinal_without_sort() {
    assert_eq!(
        properties::default_label_overlap(
            Some("ordinal"),
            Some(vega_lite::scale::ScaleType::Band),
            true,
            None
        ),
        Some(json!(true))
    );
}

#[test]
fn default_label_overlap_returns_true_for_time_unit_ordinal_with_sort() {
    assert_eq!(
        properties::default_label_overlap(
            Some("ordinal"),
            Some(vega_lite::scale::ScaleType::Band),
            true,
            Some(&json!({"field": "x", "op": "min"}))
        ),
        None
    );
}

#[test]
fn default_label_overlap_returns_undefined_for_ordinal_and_nominal() {
    assert_eq!(
        properties::default_label_overlap(
            Some("ordinal"),
            Some(vega_lite::scale::ScaleType::Band),
            false,
            None
        ),
        None
    );
    assert_eq!(
        properties::default_label_overlap(
            Some("nominal"),
            Some(vega_lite::scale::ScaleType::Band),
            false,
            None
        ),
        None
    );
}

#[test]
fn default_label_overlap_returns_greedy_for_log_and_symlog_scale() {
    assert_eq!(
        properties::default_label_overlap(
            Some("quantitative"),
            Some(vega_lite::scale::ScaleType::Log),
            false,
            None
        ),
        Some(json!("greedy"))
    );
    assert_eq!(
        properties::default_label_overlap(
            Some("quantitative"),
            Some(vega_lite::scale::ScaleType::Symlog),
            false,
            None
        ),
        Some(json!("greedy"))
    );
}

// ---------------------------------------------------------------------------
// defaultZindex
// ---------------------------------------------------------------------------

#[test]
fn default_zindex_returns_1_for_discrete_axes_of_rect_marks() {
    assert_eq!(
        properties::default_zindex("rect", &json!({"field": "a", "type": "nominal"})),
        1
    );
}

// Keep the parameter helper referenced so `is_axis_rule` is covered when the
// upstream module exports it.
#[test]
fn axis_rule_registry_covers_expected_properties() {
    assert!(is_axis_rule("grid"));
    assert!(is_axis_rule("title"));
    assert!(!is_axis_rule("labelExpr"));
}

//! A small port of the [vega-expression] language.
//!
//! Vega specifications embed JavaScript-like expressions in signals, filters,
//! encoders and transforms. This module implements the subset needed to render
//! static specifications: literals, member access, arithmetic/comparison/logical
//! operators, ternaries, array/object literals and the Vega expression function
//! library.
//!
//! [vega-expression]: https://github.com/vega/vega/tree/main/packages/vega-expression

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{Map, Number, Value};

use crate::format::{format_number, parse_date};
use crate::value::*;

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Num(f64),
    Str(String),
    Ident(String),
    Punct(char),
    Op(String),
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = input.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit())
        {
            let start = i;
            if c == '0' && i + 1 < chars.len() && (chars[i + 1] == 'x' || chars[i + 1] == 'X') {
                i += 2;
                let s = i;
                while i < chars.len() && chars[i].is_ascii_hexdigit() {
                    i += 1;
                }
                let text: String = chars[s..i].iter().collect();
                let n = i64::from_str_radix(&text, 16).unwrap_or(0) as f64;
                out.push(Token::Num(n));
                continue;
            }
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                i += 1;
                if i < chars.len() && (chars[i] == '+' || chars[i] == '-') {
                    i += 1;
                }
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let text: String = chars[start..i].iter().collect();
            out.push(Token::Num(
                text.parse::<f64>()
                    .map_err(|_| format!("invalid number literal `{text}`"))?,
            ));
            continue;
        }
        if c == '\'' || c == '"' {
            let quote = c;
            i += 1;
            let mut s = String::new();
            while i < chars.len() {
                let ch = chars[i];
                if ch == quote {
                    i += 1;
                    break;
                }
                if ch == '\\' && i + 1 < chars.len() {
                    i += 1;
                    let esc = chars[i];
                    match esc {
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        'r' => s.push('\r'),
                        'b' => s.push('\u{0008}'),
                        'f' => s.push('\u{000c}'),
                        'u' => {
                            if i + 4 < chars.len() {
                                let hex: String = chars[i + 1..i + 5].iter().collect();
                                if let Ok(code) = u32::from_str_radix(&hex, 16) {
                                    if let Some(ch) = char::from_u32(code) {
                                        s.push(ch);
                                        i += 4;
                                    }
                                }
                            }
                        }
                        other => s.push(other),
                    }
                } else {
                    s.push(ch);
                }
                i += 1;
            }
            out.push(Token::Str(s));
            continue;
        }
        if c.is_alphabetic() || c == '_' || c == '$' {
            let start = i;
            i += 1;
            while i < chars.len()
                && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '$')
            {
                i += 1;
            }
            out.push(Token::Ident(chars[start..i].iter().collect()));
            continue;
        }
        // Multi-character operators first.
        let three: String = chars[i..(i + 3).min(chars.len())].iter().collect();
        if three == "===" || three == "!==" {
            out.push(Token::Op(three));
            i += 3;
            continue;
        }
        let two: String = chars[i..(i + 2).min(chars.len())].iter().collect();
        if matches!(two.as_str(), "==" | "!=" | "<=" | ">=" | "&&" | "||" | "**") {
            out.push(Token::Op(two));
            i += 2;
            continue;
        }
        if "+-*/%<>!?:.,()[]{}".contains(c) {
            out.push(Token::Punct(c));
            i += 1;
            continue;
        }
        if c == '&' || c == '|' {
            out.push(Token::Punct(c));
            i += 1;
            continue;
        }
        return Err(format!(
            "unexpected character `{c}` in expression `{input}`"
        ));
    }
    Ok(out)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UnOp {
    Not,
    Neg,
    Pos,
    BitNot,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
    StrictEq,
    StrictNe,
    And,
    Or,
    BitAnd,
    BitOr,
}

#[derive(Clone, Debug)]
pub enum Expr {
    Literal(Value),
    Ident(String),
    Member(Box<Expr>, Box<Expr>),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Conditional(Box<Expr>, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    Array(Vec<Expr>),
    Object(Vec<(String, Expr)>),
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }
    fn next(&mut self) -> Option<Token> {
        let t = self.tokens.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }
    fn eat_punct(&mut self, c: char) -> bool {
        if matches!(self.peek(), Some(Token::Punct(p)) if *p == c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect_punct(&mut self, c: char) -> Result<(), String> {
        if self.eat_punct(c) {
            Ok(())
        } else {
            Err(format!("expected `{c}`"))
        }
    }

    fn parse_expression(&mut self) -> Result<Expr, String> {
        let cond = self.parse_binary(0)?;
        if self.eat_punct('?') {
            let a = self.parse_expression()?;
            self.expect_punct(':')?;
            let b = self.parse_expression()?;
            Ok(Expr::Conditional(Box::new(cond), Box::new(a), Box::new(b)))
        } else {
            Ok(cond)
        }
    }

    fn precedence(op: &str) -> Option<(u8, BinOp)> {
        Some(match op {
            "||" => (1, BinOp::Or),
            "&&" => (2, BinOp::And),
            "|" => (3, BinOp::BitOr),
            "&" => (4, BinOp::BitAnd),
            "==" => (5, BinOp::Eq),
            "!=" => (5, BinOp::Ne),
            "===" => (5, BinOp::StrictEq),
            "!==" => (5, BinOp::StrictNe),
            "<" => (6, BinOp::Lt),
            ">" => (6, BinOp::Gt),
            "<=" => (6, BinOp::Le),
            ">=" => (6, BinOp::Ge),
            "+" => (7, BinOp::Add),
            "-" => (7, BinOp::Sub),
            "*" => (8, BinOp::Mul),
            "/" => (8, BinOp::Div),
            "%" => (8, BinOp::Mod),
            "**" => (9, BinOp::Pow),
            _ => return None,
        })
    }

    fn token_op(t: &Token) -> Option<String> {
        match t {
            Token::Op(s) => Some(s.clone()),
            Token::Punct(c @ ('+' | '-' | '*' | '/' | '%' | '<' | '>' | '&' | '|')) => {
                Some(c.to_string())
            }
            _ => None,
        }
    }

    fn parse_binary(&mut self, min_prec: u8) -> Result<Expr, String> {
        let mut left = self.parse_unary()?;
        loop {
            let (prec, op) = match self.peek().and_then(Self::token_op) {
                Some(s) => match Self::precedence(&s) {
                    Some(v) => v,
                    None => break,
                },
                None => break,
            };
            if prec < min_prec {
                break;
            }
            self.pos += 1;
            // ** is right-associative.
            let next_min = if op == BinOp::Pow { prec } else { prec + 1 };
            let right = self.parse_binary(next_min)?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        match self.peek() {
            Some(Token::Punct('!'))
            | Some(Token::Punct('~'))
            | Some(Token::Punct('-'))
            | Some(Token::Punct('+')) => {
                let c = match self.next() {
                    Some(Token::Punct(c)) => c,
                    _ => unreachable!(),
                };
                let e = self.parse_unary()?;
                let op = match c {
                    '!' => UnOp::Not,
                    '-' => UnOp::Neg,
                    '+' => UnOp::Pos,
                    _ => UnOp::BitNot,
                };
                Ok(Expr::Unary(op, Box::new(e)))
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, String> {
        let mut e = self.parse_primary()?;
        loop {
            if self.eat_punct('.') {
                let name = match self.next() {
                    Some(Token::Ident(s)) => s,
                    Some(Token::Num(n)) => number_to_string(&Number::from_f64(n).unwrap()),
                    _ => return Err("expected property name".to_string()),
                };
                e = Expr::Member(Box::new(e), Box::new(Expr::Literal(Value::String(name))));
            } else if self.eat_punct('[') {
                let idx = self.parse_expression()?;
                self.expect_punct(']')?;
                e = Expr::Member(Box::new(e), Box::new(idx));
            } else if self.eat_punct('(') {
                // Only identifiers can be called in Vega expressions.
                let name = match &e {
                    Expr::Ident(n) => n.clone(),
                    _ => return Err("only named functions can be called".to_string()),
                };
                let mut args = Vec::new();
                if !self.eat_punct(')') {
                    loop {
                        args.push(self.parse_expression()?);
                        if self.eat_punct(')') {
                            break;
                        }
                        self.expect_punct(',')?;
                    }
                }
                e = Expr::Call(name, args);
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.next() {
            Some(Token::Num(n)) => Ok(Expr::Literal(json_number(n))),
            Some(Token::Str(s)) => Ok(Expr::Literal(Value::String(s))),
            Some(Token::Ident(s)) => match s.as_str() {
                "true" => Ok(Expr::Literal(Value::Bool(true))),
                "false" => Ok(Expr::Literal(Value::Bool(false))),
                "null" => Ok(Expr::Literal(Value::Null)),
                _ => Ok(Expr::Ident(s)),
            },
            Some(Token::Punct('(')) => {
                let e = self.parse_expression()?;
                self.expect_punct(')')?;
                Ok(e)
            }
            Some(Token::Punct('[')) => {
                let mut items = Vec::new();
                if !self.eat_punct(']') {
                    loop {
                        items.push(self.parse_expression()?);
                        if self.eat_punct(']') {
                            break;
                        }
                        self.expect_punct(',')?;
                    }
                }
                Ok(Expr::Array(items))
            }
            Some(Token::Punct('{')) => {
                let mut fields = Vec::new();
                if !self.eat_punct('}') {
                    loop {
                        let key = match self.next() {
                            Some(Token::Ident(s)) | Some(Token::Str(s)) => s,
                            Some(Token::Num(n)) => n.to_string(),
                            _ => return Err("expected object key".to_string()),
                        };
                        self.expect_punct(':')?;
                        let value = self.parse_expression()?;
                        fields.push((key, value));
                        if self.eat_punct('}') {
                            break;
                        }
                        self.expect_punct(',')?;
                    }
                }
                Ok(Expr::Object(fields))
            }
            other => Err(format!("unexpected token {other:?}")),
        }
    }
}

pub fn parse(src: &str) -> Result<Expr, String> {
    let tokens = tokenize(src)?;
    let mut p = Parser { tokens, pos: 0 };
    let e = p.parse_expression()?;
    if p.pos != p.tokens.len() {
        return Err(format!("unexpected trailing tokens in `{src}`"));
    }
    Ok(e)
}

thread_local! {
    static CACHE: std::cell::RefCell<HashMap<String, Arc<Expr>>> =
        std::cell::RefCell::new(HashMap::new());
}

/// Parse an expression, sharing the parsed AST between evaluations.
pub fn parse_cached(src: &str) -> Result<Arc<Expr>, String> {
    CACHE.with(|cache| {
        if let Some(e) = cache.borrow().get(src) {
            return Ok(e.clone());
        }
        let e = Arc::new(parse(src)?);
        cache.borrow_mut().insert(src.to_string(), e.clone());
        Ok(e)
    })
}

/// Environment used while evaluating an expression.
pub trait Environment {
    fn signal(&self, name: &str) -> Option<Value>;
    fn dataset(&self, name: &str) -> Option<Vec<Value>> {
        let _ = name;
        None
    }
    fn scale(&self, name: &str, value: &Value) -> Option<Value> {
        let _ = (name, value);
        None
    }
    fn scale_domain(&self, name: &str) -> Option<Vec<Value>> {
        let _ = name;
        None
    }
    fn scale_range(&self, name: &str) -> Option<Vec<Value>> {
        let _ = name;
        None
    }
    fn bandwidth(&self, name: &str) -> Option<f64> {
        let _ = name;
        None
    }
    fn invert(&self, name: &str, value: f64) -> Option<Value> {
        let _ = (name, value);
        None
    }
}

#[derive(Default)]
pub struct EmptyEnv;

impl Environment for EmptyEnv {
    fn signal(&self, _name: &str) -> Option<Value> {
        None
    }
}

pub fn evaluate(src: &str, datum: Option<&Value>, env: &dyn Environment) -> Result<Value, String> {
    let e = parse_cached(src)?;
    Ok(eval(&e, datum, None, env))
}

pub fn eval(
    expr: &Expr,
    datum: Option<&Value>,
    item: Option<&Value>,
    env: &dyn Environment,
) -> Value {
    match expr {
        Expr::Literal(v) => v.clone(),
        Expr::Ident(name) => match name.as_str() {
            "datum" => datum.cloned().unwrap_or(Value::Null),
            "item" => item.cloned().unwrap_or(Value::Null),
            "event" | "parent" => Value::Null,
            "NaN" => json_number(f64::NAN),
            "Infinity" => json_number(f64::INFINITY),
            "PI" => json_number(std::f64::consts::PI),
            "E" => json_number(std::f64::consts::E),
            _ => env.signal(name).unwrap_or(Value::Null),
        },
        Expr::Member(target, key) => {
            let k = eval(key, datum, item, env);
            // Fast path: `datum[...]` and `item[...]` are by far the most
            // common member accesses. Index into the row directly instead of
            // cloning it just to look up one property.
            if let Expr::Ident(name) = &**target {
                let direct = match name.as_str() {
                    "datum" => Some(datum),
                    "item" => Some(item),
                    _ => None,
                };
                if let Some(value) = direct {
                    return member_value(value, &k);
                }
            }
            let t = eval(target, datum, item, env);
            member_value(Some(&t), &k)
        }
        Expr::Unary(op, e) => {
            let v = eval(e, datum, item, env);
            match op {
                UnOp::Not => Value::Bool(!truthy(&v)),
                UnOp::Neg => {
                    let n = -to_number(&v);
                    if n.is_nan() {
                        Value::String("NaN".into())
                    } else {
                        json_number(n)
                    }
                }
                UnOp::Pos => {
                    let n = to_number(&v);
                    if n.is_nan() {
                        Value::String("NaN".into())
                    } else {
                        json_number(n)
                    }
                }
                UnOp::BitNot => json_number(!to_int(&v) as f64),
            }
        }
        Expr::Binary(op, a, b) => {
            if *op == BinOp::And {
                let av = eval(a, datum, item, env);
                return if truthy(&av) {
                    eval(b, datum, item, env)
                } else {
                    av
                };
            }
            if *op == BinOp::Or {
                let av = eval(a, datum, item, env);
                return if truthy(&av) {
                    av
                } else {
                    eval(b, datum, item, env)
                };
            }
            let av = eval(a, datum, item, env);
            let bv = eval(b, datum, item, env);
            eval_binary(*op, &av, &bv)
        }
        Expr::Conditional(c, a, b) => {
            if truthy(&eval(c, datum, item, env)) {
                eval(a, datum, item, env)
            } else {
                eval(b, datum, item, env)
            }
        }
        Expr::Array(items) => {
            Value::Array(items.iter().map(|e| eval(e, datum, item, env)).collect())
        }
        Expr::Object(fields) => {
            let mut map = Map::new();
            for (k, e) in fields {
                map.insert(k.clone(), eval(e, datum, item, env));
            }
            Value::Object(map)
        }
        Expr::Call(name, args) => {
            let argv: Vec<Value> = args.iter().map(|e| eval(e, datum, item, env)).collect();
            call(name, &argv, datum, item, env)
        }
    }
}

/// Property/index access shared by `Expr::Member` (string and numeric keys).
fn member_value(target: Option<&Value>, key: &Value) -> Value {
    match key {
        Value::String(s) => match target {
            Some(Value::Object(map)) => map.get(s).cloned().unwrap_or_else(undefined),
            Some(Value::Array(a)) => s
                .parse::<usize>()
                .ok()
                .and_then(|i| a.get(i).cloned())
                .or_else(|| match s.as_str() {
                    "length" => Some(json_number(a.len() as f64)),
                    _ => None,
                })
                .unwrap_or_else(undefined),
            Some(Value::String(sv)) => {
                if s == "length" {
                    json_number(sv.chars().count() as f64)
                } else {
                    undefined()
                }
            }
            _ => undefined(),
        },
        Value::Number(n) => {
            let idx = n.as_i64().unwrap_or(0).max(0) as usize;
            match target {
                Some(Value::String(sv)) => sv
                    .chars()
                    .nth(idx)
                    .map(|c| Value::String(c.to_string()))
                    .unwrap_or_else(undefined),
                Some(t) => t.get(idx).cloned().unwrap_or_else(undefined),
                None => undefined(),
            }
        }
        _ => Value::Null,
    }
}

fn eval_binary(op: BinOp, a: &Value, b: &Value) -> Value {
    match op {
        BinOp::Add => {
            if a.is_string() || b.is_string() {
                Value::String(format!("{}{}", to_string_js(a), to_string_js(b)))
            } else {
                json_number(to_number(a) + to_number(b))
            }
        }
        BinOp::Sub => json_number(to_number(a) - to_number(b)),
        BinOp::Mul => json_number(to_number(a) * to_number(b)),
        BinOp::Div => json_number(to_number(a) / to_number(b)),
        BinOp::Mod => json_number(to_number(a) % to_number(b)),
        BinOp::Pow => json_number(to_number(a).powf(to_number(b))),
        BinOp::Lt => Value::Bool(compare(a, b) == std::cmp::Ordering::Less),
        BinOp::Gt => Value::Bool(compare(a, b) == std::cmp::Ordering::Greater),
        BinOp::Le => Value::Bool(compare(a, b) != std::cmp::Ordering::Greater),
        BinOp::Ge => Value::Bool(compare(a, b) != std::cmp::Ordering::Less),
        BinOp::Eq => Value::Bool(loose_eq(a, b)),
        BinOp::Ne => Value::Bool(!loose_eq(a, b)),
        BinOp::StrictEq => Value::Bool(strict_eq(a, b)),
        BinOp::StrictNe => Value::Bool(!strict_eq(a, b)),
        BinOp::And | BinOp::Or => unreachable!(),
        BinOp::BitAnd => json_number((to_int(a) & to_int(b)) as f64),
        BinOp::BitOr => json_number((to_int(a) | to_int(b)) as f64),
    }
}

fn arg(args: &[Value], i: usize) -> Value {
    args.get(i).cloned().unwrap_or(Value::Null)
}

fn call(
    name: &str,
    args: &[Value],
    datum: Option<&Value>,
    item: Option<&Value>,
    env: &dyn Environment,
) -> Value {
    match name {
        "if" => {
            if truthy(&arg(args, 0)) {
                arg(args, 1)
            } else {
                arg(args, 2)
            }
        }
        "isValid" => {
            let v = arg(args, 0);
            Value::Bool(!v.is_null() && !crate::value::is_undefined(&v))
        }
        "isFinite" => Value::Bool(to_number(&arg(args, 0)).is_finite()),
        "isNaN" => Value::Bool(to_number(&arg(args, 0)).is_nan()),
        "isArray" => Value::Bool(arg(args, 0).is_array()),
        "isObject" => Value::Bool(arg(args, 0).is_object()),
        "isString" => Value::Bool(arg(args, 0).is_string()),
        "isNumber" => Value::Bool(arg(args, 0).is_number()),
        "isBoolean" => Value::Bool(arg(args, 0).is_boolean()),
        "isDate" => Value::Bool(match arg(args, 0) {
            Value::String(s) => parse_date(&s).is_some(),
            _ => false,
        }),
        "toNumber" => {
            let v = arg(args, 0);
            if v.is_null()
                || crate::value::is_undefined(&v)
                || v.as_str().map(|s| s.is_empty()).unwrap_or(false)
            {
                Value::Null
            } else {
                json_number(to_number(&v))
            }
        }
        "toString" => Value::String(to_string_js(&arg(args, 0))),
        "toBoolean" => Value::Bool(truthy(&arg(args, 0))),
        "length" => {
            let v = arg(args, 0);
            let n = match &v {
                Value::Array(a) => a.len(),
                Value::String(s) => s.chars().count(),
                Value::Object(m) => m.len(),
                _ => 0,
            };
            json_number(n as f64)
        }
        "join" => {
            let sep = match args.get(1) {
                Some(s) => to_string_js(s),
                None => ",".to_string(),
            };
            match arg(args, 0) {
                Value::Array(a) => {
                    Value::String(a.iter().map(to_string_js).collect::<Vec<_>>().join(&sep))
                }
                v => Value::String(to_string_js(&v)),
            }
        }
        "indexof" | "lastindexof" => {
            let hay = arg(args, 0);
            let needle = arg(args, 1);
            let found = match &hay {
                Value::String(s) => {
                    let n = to_string_js(&needle);
                    if name == "indexof" {
                        s.find(&n).map(|b| s[..b].chars().count() as i64)
                    } else {
                        s.rfind(&n).map(|b| s[..b].chars().count() as i64)
                    }
                }
                Value::Array(a) => {
                    let idx = if name == "indexof" {
                        a.iter().position(|x| loose_eq(x, &needle))
                    } else {
                        a.iter().rposition(|x| loose_eq(x, &needle))
                    };
                    idx.map(|i| i as i64)
                }
                _ => None,
            };
            json_number(found.map(|i| i as f64).unwrap_or(-1.0))
        }
        "slice" => {
            let v = arg(args, 0);
            let len = match &v {
                Value::Array(a) => a.len() as i64,
                Value::String(s) => s.chars().count() as i64,
                _ => 0,
            };
            let mut start = to_int(&arg(args, 1));
            let mut end = if args.len() > 2 {
                to_int(&args[2])
            } else {
                len
            };
            if start < 0 {
                start += len;
            }
            if end < 0 {
                end += len;
            }
            let start = start.max(0).min(len);
            let end = end.max(0).min(len);
            match v {
                Value::Array(a) => Value::Array(
                    a.into_iter()
                        .skip(start as usize)
                        .take((end - start).max(0) as usize)
                        .collect(),
                ),
                Value::String(s) => Value::String(
                    s.chars()
                        .skip(start as usize)
                        .take((end - start).max(0) as usize)
                        .collect(),
                ),
                _ => Value::Null,
            }
        }
        "substring" => {
            let s = to_string_js(&arg(args, 0));
            let len = s.chars().count() as i64;
            let a = to_int(&arg(args, 1)).max(0).min(len);
            let b = if args.len() > 2 {
                to_int(&args[2]).max(0).min(len)
            } else {
                len
            };
            let (a, b) = if a <= b { (a, b) } else { (b, a) };
            Value::String(s.chars().skip(a as usize).take((b - a) as usize).collect())
        }
        "upper" => Value::String(to_string_js(&arg(args, 0)).to_uppercase()),
        "lower" => Value::String(to_string_js(&arg(args, 0)).to_lowercase()),
        "trim" => Value::String(to_string_js(&arg(args, 0)).trim().to_string()),
        "replace" => {
            let s = to_string_js(&arg(args, 0));
            let pat = to_string_js(&arg(args, 1));
            let repl = to_string_js(&arg(args, 2));
            let out = if pat.starts_with('/') {
                // A very small subset of regular expressions: /pattern/g and /pattern/i.
                let (body, flags) = pat[1..]
                    .split_once('/')
                    .map(|(b, f)| (b.to_string(), f.to_string()))
                    .unwrap_or((pat[1..].to_string(), String::new()));
                regex_lite_replace(&s, &body, &repl, flags.contains('g'), flags.contains('i'))
            } else {
                s.replacen(&pat, &repl, 1)
            };
            Value::String(out)
        }
        "split" => {
            let s = to_string_js(&arg(args, 0));
            let sep = to_string_js(&arg(args, 1));
            let limit = if args.len() > 2 {
                to_int(&args[2]).max(0) as usize
            } else {
                usize::MAX
            };
            let parts: Vec<Value> = if sep.is_empty() {
                s.chars()
                    .map(|c| Value::String(c.to_string()))
                    .take(limit)
                    .collect()
            } else {
                s.split(&sep)
                    .take(limit)
                    .map(|p| Value::String(p.to_string()))
                    .collect()
            };
            Value::Array(parts)
        }
        "pad" => {
            let s = to_string_js(&arg(args, 0));
            let width = to_int(&arg(args, 1)).max(0) as usize;
            let ch = if args.len() > 2 {
                to_string_js(&args[2]).chars().next().unwrap_or(' ')
            } else {
                ' '
            };
            let align = if args.len() > 3 {
                to_string_js(&args[3])
            } else {
                "right".to_string()
            };
            let count = width.saturating_sub(s.chars().count());
            let padding: String = std::iter::repeat(ch).take(count).collect();
            Value::String(match align.as_str() {
                "left" => format!("{s}{padding}"),
                "center" => {
                    let l = count / 2;
                    let r = count - l;
                    format!(
                        "{}{}{}",
                        std::iter::repeat(ch).take(l).collect::<String>(),
                        s,
                        std::iter::repeat(ch).take(r).collect::<String>()
                    )
                }
                _ => format!("{padding}{s}"),
            })
        }
        "truncate" => {
            let s = to_string_js(&arg(args, 0));
            let len = to_int(&arg(args, 1)).max(0) as usize;
            let align = if args.len() > 2 {
                to_string_js(&args[2])
            } else {
                "right".to_string()
            };
            let ellipsis = if args.len() > 3 {
                to_string_js(&args[3])
            } else {
                "\u{2026}".to_string()
            };
            let count = s.chars().count();
            if count <= len {
                return Value::String(s);
            }
            let keep = len.saturating_sub(ellipsis.chars().count());
            let chars: Vec<char> = s.chars().collect();
            let text: String = match align.as_str() {
                "left" => chars[..keep].iter().collect(),
                "center" => {
                    let left = keep / 2;
                    let right = keep - left;
                    format!(
                        "{}{}{}",
                        chars[..left].iter().collect::<String>(),
                        ellipsis,
                        chars[count - right..].iter().collect::<String>()
                    )
                }
                _ => chars[count - keep..].iter().collect(),
            };
            Value::String(if align == "right" {
                format!("{ellipsis}{text}")
            } else if align == "center" {
                text
            } else {
                format!("{text}{ellipsis}")
            })
        }
        "format" => {
            let spec = if args.len() > 1 {
                to_string_js(&args[1])
            } else {
                String::new()
            };
            Value::String(format_number(to_number(&arg(args, 0)), &spec))
        }
        "timeFormat" | "utcFormat" => {
            let ms = to_number(&arg(args, 0));
            let spec = if args.len() > 1 {
                to_string_js(&args[1])
            } else {
                "%b %d, %Y".to_string()
            };
            let expanded = expand_time_spec(&spec, ms);
            Value::String(if name == "utcFormat" {
                crate::format::format_date(ms, &expanded)
            } else {
                crate::format::format_date_local(ms, &expanded)
            })
        }
        "timeUnitSpecifier" => {
            let units = match arg(args, 0) {
                Value::Array(a) => a
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>(),
                Value::String(s) => vec![s],
                _ => Vec::new(),
            };
            let extra = match args.get(1) {
                Some(Value::Object(m)) => m.clone(),
                _ => Map::new(),
            };
            Value::String(time_unit_specifier(&units, &extra))
        }
        "timeOffset" | "utcOffset" => {
            let unit = to_string_js(&arg(args, 0));
            let ms = to_ms(&arg(args, 1));
            let step = to_number(&arg(args, 2));
            Value::Number(
                Number::from_f64(offset_time(&unit, ms, step))
                    .unwrap_or_else(|| Number::from(0)),
            )
        }
        "toDate" => match arg(args, 0) {
            Value::String(s) => match js_date_ms(&s) {
                Some(ms) => json_number(ms),
                None => Value::Null,
            },
            Value::Number(n) => Value::Number(n),
            Value::Null => Value::Null,
            other => json_number(to_number(&other)),
        },
        "timeParse" | "utcParse" => {
            let text = to_string_js(&arg(args, 0));
            let format = args
                .get(1)
                .filter(|v| !v.is_null())
                .map(to_string_js)
                .unwrap_or_default();
            let parsed = if !format.is_empty() {
                crate::format::parse_date_format(&text, &format).map(
                    |(y, m, d, h, mi, s, milli)| {
                        if name == "utcParse" {
                            days_from_civil(y, m, d) as f64 * 86_400_000.0
                                + h as f64 * 3_600_000.0
                                + mi as f64 * 60_000.0
                                + s as f64 * 1000.0
                                + milli as f64
                        } else {
                            local_datetime_ms(
                                y as f64,
                                (m - 1) as f64,
                                d as f64,
                                h as f64,
                                mi as f64,
                                s as f64,
                                milli as f64,
                            )
                        }
                    },
                )
            } else if name == "utcParse" {
                parse_date(&text)
            } else {
                // d3's `timeParse` builds an instant from *local* calendar
                // fields, so reinterpret the parsed civil parts locally.
                parse_date(&text).map(|ms| {
                    let (y, m, d, h, mi, s, milli, _) = date_parts(&json_number(ms));
                    local_datetime_ms(
                        y as f64,
                        (m - 1) as f64,
                        d as f64,
                        h as f64,
                        mi as f64,
                        s as f64,
                        milli as f64,
                    )
                })
            };
            match parsed {
                Some(ms) => json_number(ms),
                None => Value::Null,
            }
        }
        "datetime" => {
            // Vega's `datetime` is the JavaScript `Date` constructor:
            // `new Date(ms)` with a single argument (also a parseable string),
            // or local calendar fields (`year, month, day, ...`) otherwise.
            if args.len() <= 1 {
                return match args.first() {
                    None => json_number(current_millis()),
                    Some(Value::String(s)) => match js_date_ms(s) {
                        Some(ms) => json_number(ms),
                        None => Value::Null,
                    },
                    Some(Value::Number(n)) => Value::Number(n.clone()),
                    Some(Value::Null) => Value::Null,
                    Some(other) => json_number(to_number(other)),
                };
            }
            let nums: Vec<f64> = args.iter().map(to_number).collect();
            let get = |i: usize, d: f64| nums.get(i).copied().unwrap_or(d);
            Value::Number(
                Number::from_f64(local_datetime_ms(
                    get(0, 1970.0),
                    get(1, 0.0),
                    get(2, 1.0),
                    get(3, 0.0),
                    get(4, 0.0),
                    get(5, 0.0),
                    get(6, 0.0),
                ))
                .unwrap_or_else(|| Number::from(0)),
            )
        }
        "utc" => {
            let nums: Vec<f64> = args.iter().map(to_number).collect();
            let get = |i: usize, d: f64| nums.get(i).copied().unwrap_or(d);
            Value::Number(
                Number::from_f64(datetime(
                    get(0, 1970.0),
                    get(1, 0.0),
                    get(2, 1.0),
                    get(3, 0.0),
                    get(4, 0.0),
                    get(5, 0.0),
                    get(6, 0.0),
                ))
                .unwrap_or_else(|| Number::from(0)),
            )
        }
        "now" => json_number(current_millis()),
        "random" => json_number(next_random()),
        "quantileUniform" => {
            let p = to_number(&arg(args, 0));
            let min = args.get(1).map(to_number).unwrap_or(0.0);
            let max = args.get(2).map(to_number).unwrap_or(1.0);
            json_number(if (0.0..=1.0).contains(&p) {
                min + p * (max - min)
            } else {
                f64::NAN
            })
        }
        "quantileNormal" => {
            let p = to_number(&arg(args, 0));
            let mean = args.get(1).map(to_number).unwrap_or(0.0);
            let stdev = args.get(2).map(to_number).unwrap_or(1.0);
            json_number(if (0.0..=1.0).contains(&p) {
                mean + stdev * std::f64::consts::SQRT_2 * erfinv(2.0 * p - 1.0)
            } else {
                f64::NAN
            })
        }
        "abs" => json_number(to_number(&arg(args, 0)).abs()),
        "ceil" => json_number(to_number(&arg(args, 0)).ceil()),
        "floor" => json_number(to_number(&arg(args, 0)).floor()),
        "round" => json_number(to_number(&arg(args, 0)).round()),
        "trunc" => json_number(to_number(&arg(args, 0)).trunc()),
        "sqrt" => json_number(to_number(&arg(args, 0)).sqrt()),
        "exp" => json_number(to_number(&arg(args, 0)).exp()),
        "log" => json_number(to_number(&arg(args, 0)).ln()),
        "log10" => json_number(to_number(&arg(args, 0)).log10()),
        "pow" => json_number(to_number(&arg(args, 0)).powf(to_number(&arg(args, 1)))),
        "sin" => json_number(to_number(&arg(args, 0)).sin()),
        "cos" => json_number(to_number(&arg(args, 0)).cos()),
        "tan" => json_number(to_number(&arg(args, 0)).tan()),
        "asin" => json_number(to_number(&arg(args, 0)).asin()),
        "acos" => json_number(to_number(&arg(args, 0)).acos()),
        "atan" => json_number(to_number(&arg(args, 0)).atan()),
        "atan2" => json_number(to_number(&arg(args, 0)).atan2(to_number(&arg(args, 1)))),
        "min" => {
            let vals = flatten_numbers(args);
            json_number(vals.into_iter().fold(f64::INFINITY, f64::min))
        }
        "max" => {
            let vals = flatten_numbers(args);
            json_number(vals.into_iter().fold(f64::NEG_INFINITY, f64::max))
        }
        "sum" => json_number(flatten_numbers(args).into_iter().sum()),
        "mean" | "average" => {
            let vals = flatten_numbers(args);
            if vals.is_empty() {
                Value::Null
            } else {
                json_number(vals.iter().sum::<f64>() / vals.len() as f64)
            }
        }
        "clamp" => json_number(
            to_number(&arg(args, 0))
                .max(to_number(&arg(args, 1)))
                .min(to_number(&arg(args, 2))),
        ),
        "span" => json_number(to_number(&arg(args, 1)) - to_number(&arg(args, 0))),
        "lerp" => json_number(
            to_number(&arg(args, 0)) * (1.0 - to_number(&arg(args, 2)))
                + to_number(&arg(args, 1)) * to_number(&arg(args, 2)),
        ),
        "inrange" => {
            let x = to_number(&arg(args, 0));
            let (a, b) = match args.get(1) {
                Some(Value::Array(range)) if range.len() >= 2 => {
                    (to_number(&range[0]), to_number(&range[1]))
                }
                _ => (to_number(&arg(args, 1)), to_number(&arg(args, 2))),
            };
            Value::Bool((x >= a && x <= b) || (x >= b && x <= a))
        }
        "peek" => match arg(args, 0) {
            Value::Array(a) => a.last().cloned().unwrap_or(Value::Null),
            _ => Value::Null,
        },
        "pluck" => {
            let field = to_string_js(&arg(args, 1));
            match arg(args, 0) {
                Value::Array(a) => {
                    Value::Array(a.iter().map(|v| get_field_owned(v, &field)).collect())
                }
                _ => Value::Array(vec![]),
            }
        }
        "extent" => match arg(args, 0) {
            Value::Array(a) => {
                if a.is_empty() {
                    Value::Array(vec![Value::Null, Value::Null])
                } else {
                    let mut min = a[0].clone();
                    let mut max = a[0].clone();
                    for v in &a[1..] {
                        if compare(v, &min) == std::cmp::Ordering::Less {
                            min = v.clone();
                        }
                        if compare(v, &max) == std::cmp::Ordering::Greater {
                            max = v.clone();
                        }
                    }
                    Value::Array(vec![min, max])
                }
            }
            _ => Value::Array(vec![Value::Null, Value::Null]),
        },
        "sequence" => {
            let start = to_number(&arg(args, 0));
            let stop = to_number(&arg(args, 1));
            let step = if args.len() > 2 {
                to_number(&args[2])
            } else {
                1.0
            };
            let mut out = Vec::new();
            if step != 0.0 {
                if step > 0.0 {
                    let mut x = start;
                    while x < stop - 1e-9 {
                        out.push(json_number(x));
                        x += step;
                        if out.len() > 100_000 {
                            break;
                        }
                    }
                } else {
                    let mut x = start;
                    while x > stop + 1e-9 {
                        out.push(json_number(x));
                        x += step;
                        if out.len() > 100_000 {
                            break;
                        }
                    }
                }
            }
            Value::Array(out)
        }
        "merge" => {
            let mut map = Map::new();
            for v in args {
                if let Value::Object(m) = v {
                    for (k, v) in m {
                        map.insert(k.clone(), v.clone());
                    }
                }
            }
            Value::Object(map)
        }
        "bandspace" => {
            let count = to_number(&arg(args, 0));
            let inner = to_number(&arg(args, 1));
            let outer = to_number(&arg(args, 2));
            json_number(count - inner + 2.0 * outer)
        }
        "bandwidth" => json_number(env.bandwidth(&to_string_js(&arg(args, 0))).unwrap_or(0.0)),
        "domain" => env
            .scale_domain(&to_string_js(&arg(args, 0)))
            .map(Value::Array)
            .unwrap_or(Value::Null),
        "range" => env
            .scale_range(&to_string_js(&arg(args, 0)))
            .map(Value::Array)
            .unwrap_or(Value::Null),
        "scale" => {
            let name = to_string_js(&arg(args, 0));
            env.scale(&name, &arg(args, 1)).unwrap_or(Value::Null)
        }
        "invert" => {
            let name = to_string_js(&arg(args, 0));
            env.invert(&name, to_number(&arg(args, 1)))
                .unwrap_or(Value::Null)
        }
        "data" => env
            .dataset(&to_string_js(&arg(args, 0)))
            .map(Value::Array)
            .unwrap_or(Value::Null),
        "luminance" => {
            let c = crate::color::parse_color(&to_string_js(&arg(args, 0)));
            json_number(c.map(crate::color::luminance).unwrap_or(f64::NAN))
        }
        "contrast" => {
            let a = crate::color::parse_color(&to_string_js(&arg(args, 0)));
            let b = crate::color::parse_color(&to_string_js(&arg(args, 1)));
            match (a, b) {
                (Some(a), Some(b)) => json_number(crate::color::contrast(a, b)),
                _ => Value::Null,
            }
        }
        "rgb" | "hsl" | "lab" | "hcl" => {
            let r = to_number(&arg(args, 0));
            let g = to_number(&arg(args, 1));
            let b = to_number(&arg(args, 2));
            let (r, g, b) = if name == "rgb" {
                (r, g, b)
            } else {
                hsl_to_rgb(r, g, b)
            };
            Value::String(format!(
                "#{:02x}{:02x}{:02x}",
                r.round().clamp(0.0, 255.0) as u8,
                g.round().clamp(0.0, 255.0) as u8,
                b.round().clamp(0.0, 255.0) as u8
            ))
        }
        "year" => date_part(&arg(args, 0), "year"),
        "quarter" => date_part(&arg(args, 0), "quarter"),
        "month" => date_part(&arg(args, 0), "month"),
        "date" | "dayofmonth" => date_part(&arg(args, 0), "date"),
        "day" | "dayofweek" => date_part(&arg(args, 0), "day"),
        "hours" => date_part(&arg(args, 0), "hours"),
        "minutes" => date_part(&arg(args, 0), "minutes"),
        "seconds" => date_part(&arg(args, 0), "seconds"),
        "milliseconds" => date_part(&arg(args, 0), "milliseconds"),
        "time" => json_number(wall_ms(to_ms(&arg(args, 0)))),
        "timezoneoffset" => json_number(0.0),
        "vlSelectionTest" => selection_test(
            &to_string_js(&arg(args, 0)),
            &arg(args, 1),
            args.get(2),
            env,
        ),
        "vlSelectionIdTest" => selection_id_test(
            &to_string_js(&arg(args, 0)),
            &arg(args, 1),
            env,
        ),
        "vlSelectionResolve" => selection_resolve(
            &to_string_js(&arg(args, 0)),
            &arg(args, 1),
            truthy(&arg(args, 2)),
            truthy(&arg(args, 3)),
            env,
        ),
        "vlSelectionTuples" => selection_tuples(&arg(args, 0), &arg(args, 1)),
        "warn" => {
            eprintln!("vega-svg warning: {}", to_string_js(&arg(args, 0)));
            Value::Null
        }
        "info" | "debug" => Value::Null,
        "item" => item.cloned().unwrap_or(Value::Null),
        _ => {
            // Unknown functions evaluate to null, matching a missing binding in
            // a permissive expression evaluator.
            let _ = datum;
            Value::Null
        }
    }
}

// ---------------------------------------------------------------------------
// Selection functions (port of vega-selections).
//
// Statically rendered specifications can still contain a selection store with
// initial tuples (for example when a fixture was generated from a view with an
// active selection). These helpers evaluate the `vlSelection*` functions
// against the dataset exposed by the environment so that filters and resolved
// signals reproduce Vega's initial state.
// ---------------------------------------------------------------------------

const SELECTION_ID: &str = "_vgsid_";

enum SetOp {
    Union,
    Intersect,
}

fn selection_op(op: Option<&Value>) -> SetOp {
    match op {
        Some(Value::String(s)) if s == "intersect" => SetOp::Intersect,
        _ => SetOp::Union,
    }
}

/// Field accessor used by selection tuples: `{field: "name"}` (or a plain
/// string for convenience).
fn tuple_field_name(field: &Value) -> Option<String> {
    match field {
        Value::String(s) => Some(s.clone()),
        other => other
            .get("field")
            .and_then(|f| f.as_str())
            .map(String::from),
    }
}

fn range_contains(value: &Value, range: &Value, left: bool, right: bool) -> bool {
    let Some(bounds) = range.as_array() else {
        return false;
    };
    if bounds.len() < 2 {
        return false;
    }
    let x = to_number(value);
    let a = to_number(&bounds[0]);
    let b = to_number(&bounds[1]);
    if !x.is_finite() || !a.is_finite() || !b.is_finite() {
        return false;
    }
    let in_left = if left { x >= a } else { x > a };
    let in_right = if right { x <= b } else { x < b };
    (in_left && in_right) || {
        // Reversed range.
        let in_left_rev = if left { x >= b } else { x > b };
        let in_right_rev = if right { x <= a } else { x < a };
        in_left_rev && in_right_rev
    }
}

fn test_point(datum: &Value, entry: &Value) -> bool {
    let Some(fields) = entry.get("fields").and_then(|f| f.as_array()) else {
        return false;
    };
    let Some(values) = entry.get("values").and_then(|v| v.as_array()) else {
        return false;
    };
    let n = fields.len().min(values.len());
    for i in 0..n {
        let Some(name) = tuple_field_name(&fields[i]) else {
            return false;
        };
        let dval = get_field_owned(datum, &name);
        let value = &values[i];
        let ty = fields[i]
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("E");
        let ok = match ty {
            "E" => match value {
                Value::Array(a) => a.iter().any(|v| strict_eq(v, &dval)),
                other => strict_eq(other, &dval),
            },
            "R" => range_contains(&dval, value, true, true),
            "R-RE" => range_contains(&dval, value, true, false),
            "R-E" | "R-LE" => range_contains(&dval, value, false, false),
            "E-LT" => compare(&dval, value) == std::cmp::Ordering::Less,
            "E-LTE" => compare(&dval, value) != std::cmp::Ordering::Greater,
            "E-GT" => compare(&dval, value) == std::cmp::Ordering::Greater,
            "E-GTE" => compare(&dval, value) != std::cmp::Ordering::Less,
            "E-VALID" => !dval.is_null() && !to_number(&dval).is_nan(),
            "E-ONE" => match value {
                Value::Array(a) => a.iter().any(|v| strict_eq(v, &dval)),
                _ => false,
            },
            _ => false,
        };
        if !ok {
            return false;
        }
    }
    true
}

fn selection_test(
    name: &str,
    datum: &Value,
    op: Option<&Value>,
    env: &dyn Environment,
) -> Value {
    let entries = env.dataset(name).unwrap_or_default();
    if entries.is_empty() {
        return Value::Bool(false);
    }
    let intersect = matches!(selection_op(op), SetOp::Intersect);
    for entry in &entries {
        let matched = test_point(datum, entry);
        if intersect && !matched {
            return Value::Bool(false);
        }
        if !intersect && matched {
            return Value::Bool(true);
        }
    }
    Value::Bool(intersect)
}

fn selection_id_test(name: &str, datum: &Value, env: &dyn Environment) -> Value {
    let entries = env.dataset(name).unwrap_or_default();
    let value = get_field_owned(datum, SELECTION_ID);
    let found = entries
        .iter()
        .any(|entry| strict_eq(&get_field_owned(entry, SELECTION_ID), &value));
    Value::Bool(found)
}

fn unique_push(base: &mut Vec<Value>, value: &Value) {
    if !base.iter().any(|v| strict_eq(v, value)) {
        base.push(value.clone());
    }
}

fn range_bounds(value: &Value) -> Option<(Value, Value)> {
    let a = value.as_array()?;
    if a.len() < 2 {
        return None;
    }
    let lo = to_number(&a[0]);
    let hi = to_number(&a[1]);
    if lo > hi {
        Some((a[1].clone(), a[0].clone()))
    } else {
        Some((a[0].clone(), a[1].clone()))
    }
}

fn enum_set_op(op: &SetOp, base: &mut Vec<Value>, value: &[Value]) {
    match op {
        SetOp::Union => {
            for v in value {
                unique_push(base, v);
            }
        }
        SetOp::Intersect => {
            if base.is_empty() {
                base.extend(value.iter().cloned());
            } else {
                base.retain(|v| value.iter().any(|w| strict_eq(v, w)));
            }
        }
    }
}

fn range_set_op(op: &SetOp, base: &mut Vec<Value>, value: &Value) {
    let Some((lo, hi)) = range_bounds(value) else {
        return;
    };
    if base.is_empty() {
        base.push(lo);
        base.push(hi);
        return;
    }
    let base_lo = to_number(&base[0]);
    let base_hi = to_number(&base[1]);
    let lo_n = to_number(&lo);
    let hi_n = to_number(&hi);
    match op {
        SetOp::Union => {
            if base_lo > lo_n {
                base[0] = lo;
            }
            if base_hi < hi_n {
                base[1] = hi;
            }
        }
        SetOp::Intersect => {
            if hi_n < base_lo || base_hi < lo_n {
                base.clear();
            } else {
                if base_lo < lo_n {
                    base[0] = lo;
                }
                if base_hi > hi_n {
                    base[1] = hi;
                }
            }
        }
    }
}

fn flatten_ids(values: &[Value], op: &SetOp) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for value in values {
        match value {
            Value::Array(items) => match op {
                SetOp::Union => {
                    for item in items {
                        unique_push(&mut out, item);
                    }
                }
                SetOp::Intersect => {
                    if out.is_empty() {
                        out.extend(items.iter().cloned());
                    } else {
                        out.retain(|v| items.iter().any(|w| strict_eq(v, w)));
                    }
                }
            },
            other => unique_push(&mut out, other),
        }
    }
    out
}

fn selection_resolve(
    name: &str,
    op: &Value,
    is_multi: bool,
    vl5: bool,
    env: &dyn Environment,
) -> Value {
    let entries = env.dataset(name).unwrap_or_default();
    let set_op = selection_op(Some(op));
    // field -> unit -> resolved value
    let mut resolved: Map<String, Value> = Map::new();
    let mut types: HashMap<String, char> = HashMap::new();
    // unit -> multi-selection tuple
    let mut multi: Map<String, Value> = Map::new();

    for entry in &entries {
        let unit = entry
            .get("unit")
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string();
        let fields = entry.get("fields").and_then(|f| f.as_array());
        let values = entry.get("values").and_then(|v| v.as_array());
        if let (Some(fields), Some(values)) = (fields, values) {
            for (i, field) in fields.iter().enumerate() {
                let Some(field_name) = tuple_field_name(field) else {
                    continue;
                };
                let ty = field
                    .get("type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("E")
                    .chars()
                    .next()
                    .unwrap_or('E');
                types.insert(field_name.clone(), ty);
                let units = resolved
                    .entry(field_name)
                    .or_insert_with(|| Value::Object(Map::new()));
                let Some(unit_map) = units.as_object_mut() else {
                    continue;
                };
                let cell = unit_map
                    .entry(unit.clone())
                    .or_insert_with(|| Value::Array(Vec::new()));
                if let Value::Array(base) = cell {
                    let value = values.get(i).cloned().unwrap_or(Value::Null);
                    if ty == 'R' {
                        range_set_op(&set_op, base, &value);
                    } else {
                        let items = match value {
                            Value::Array(a) => a,
                            other => vec![other],
                        };
                        enum_set_op(&set_op, base, &items);
                    }
                }
            }
            if is_multi {
                let mut tuple = Map::new();
                for (i, field) in fields.iter().enumerate() {
                    if let Some(field_name) = tuple_field_name(field) {
                        tuple.insert(
                            field_name,
                            values.get(i).cloned().unwrap_or(Value::Null),
                        );
                    }
                }
                let unit_list = multi
                    .entry(unit)
                    .or_insert_with(|| Value::Array(Vec::new()));
                if let Value::Array(list) = unit_list {
                    list.push(Value::Object(tuple));
                }
            }
        } else {
            let value = get_field_owned(entry, SELECTION_ID);
            let units = resolved
                .entry(SELECTION_ID.to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            if let Some(unit_map) = units.as_object_mut() {
                let cell = unit_map
                    .entry(unit.clone())
                    .or_insert_with(|| Value::Array(Vec::new()));
                if let Value::Array(base) = cell {
                    base.push(value.clone());
                }
            }
            if is_multi {
                let mut tuple = Map::new();
                tuple.insert(SELECTION_ID.to_string(), value);
                let unit_list = multi
                    .entry(unit)
                    .or_insert_with(|| Value::Array(Vec::new()));
                if let Value::Array(list) = unit_list {
                    list.push(Value::Object(tuple));
                }
            }
        }
    }

    // Resolve fields across units using the requested set operation.
    let mut out = Map::new();
    if let Some(units) = resolved.get(SELECTION_ID) {
        let values: Vec<Value> = units
            .as_object()
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default();
        out.insert(
            SELECTION_ID.to_string(),
            Value::Array(flatten_ids(&values, &set_op)),
        );
    } else {
        for (field, units) in &resolved {
            let ty = types.get(field).copied().unwrap_or('E');
            let values: Vec<Value> = units
                .as_object()
                .map(|m| m.values().cloned().collect())
                .unwrap_or_default();
            let mut acc: Vec<Value> = Vec::new();
            for value in &values {
                if let Value::Array(items) = value {
                    if ty == 'R' {
                        range_set_op(&set_op, &mut acc, &Value::Array(items.clone()));
                    } else {
                        enum_set_op(&set_op, &mut acc, items);
                    }
                }
            }
            out.insert(field.clone(), Value::Array(acc));
        }
    }

    if is_multi && !multi.is_empty() {
        let key = if vl5 { "vlPoint" } else { "vlMulti" };
        let units: Vec<Value> = multi.values().cloned().collect();
        let value = match set_op {
            SetOp::Union => {
                let mut all = Vec::new();
                for unit in &units {
                    if let Value::Array(items) = unit {
                        all.extend(items.iter().cloned());
                    }
                }
                let mut m = Map::new();
                m.insert("or".to_string(), Value::Array(all));
                Value::Object(m)
            }
            SetOp::Intersect => {
                let and: Vec<Value> = units
                    .iter()
                    .map(|u| {
                        let mut m = Map::new();
                        m.insert("or".to_string(), u.clone());
                        Value::Object(m)
                    })
                    .collect();
                let mut m = Map::new();
                m.insert("and".to_string(), Value::Array(and));
                Value::Object(m)
            }
        };
        out.insert(key.to_string(), value);
    }

    Value::Object(out)
}

fn selection_tuples(values: &Value, base: &Value) -> Value {
    let Some(items) = values.as_array() else {
        return Value::Array(Vec::new());
    };
    let base = base.clone();
    let fields = base.get("fields").and_then(|f| f.as_array());
    let out: Vec<Value> = items
        .iter()
        .map(|item| {
            let datum = item.get("datum").unwrap_or(item);
            let mut tuple = match &base {
                Value::Object(m) => m.clone(),
                _ => Map::new(),
            };
            match fields {
                Some(fields) => {
                    let mut resolved = Vec::new();
                    for field in fields {
                        let value = match field {
                            Value::String(name) => get_field_owned(datum, name),
                            other => other
                                .get("field")
                                .and_then(|f| f.as_str())
                                .map(|name| get_field_owned(datum, name))
                                .unwrap_or(Value::Null),
                        };
                        resolved.push(value);
                    }
                    tuple.insert("values".to_string(), Value::Array(resolved));
                }
                None => {
                    tuple.insert(
                        SELECTION_ID.to_string(),
                        get_field_owned(datum, SELECTION_ID),
                    );
                }
            }
            Value::Object(tuple)
        })
        .collect();
    Value::Array(out)
}

/// Order of time units used by `timeUnitSpecifier` (matches vega-time).
const TIME_UNIT_ORDER: [&str; 12] = [
    "year",
    "quarter",
    "month",
    "week",
    "isoweek",
    "date",
    "day",
    "dayofyear",
    "hours",
    "minutes",
    "seconds",
    "milliseconds",
];

fn time_unit_specifier(units: &[String], extra: &Map<String, Value>) -> String {
    let mut sorted: Vec<&str> = units
        .iter()
        .map(String::as_str)
        .filter(|u| TIME_UNIT_ORDER.contains(u))
        .collect();
    sorted.sort_by_key(|u| TIME_UNIT_ORDER.iter().position(|x| x == u).unwrap_or(0));
    let defaults = [
        ("year", "%Y "),
        ("quarter", "Q%q "),
        ("month", "%b "),
        ("date", "%d "),
        ("week", "W%U "),
        ("isoweek", "W%V "),
        ("day", "%a "),
        ("dayofyear", "%j "),
        ("hours", "%H:00"),
        ("minutes", "00:%M"),
        ("seconds", ":%S"),
        ("milliseconds", ".%L"),
        ("year-isoweek", "%G W%V "),
        ("year-month", "%Y-%m "),
        ("year-month-date", "%Y-%m-%d "),
        ("hours-minutes", "%H:%M"),
    ];
    let lookup = |key: &str| -> Option<String> {
        if let Some(v) = extra.get(key) {
            return v.as_str().map(String::from);
        }
        defaults
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| (*v).to_string())
    };
    let n = sorted.len();
    let mut fmt = String::new();
    let mut start = 0;
    while start < n {
        let mut matched = false;
        for end in (start + 1..=n).rev() {
            let key = sorted[start..end].join("-");
            if let Some(s) = lookup(&key) {
                fmt.push_str(&s);
                start = end;
                matched = true;
                break;
            }
        }
        if !matched {
            start += 1;
        }
    }
    fmt.trim().to_string()
}

/// Fill in directives that `format_date` does not implement (%q, %V, %G).
fn expand_time_spec(spec: &str, ms: f64) -> String {
    if !spec.contains("%q") && !spec.contains("%V") && !spec.contains("%G") {
        return spec.to_string();
    }
    let (_y, m, _d, _h, _mi, _s, _ms, weekday) = date_parts(&json_number(ms));
    let quarter = ((m - 1) / 3 + 1).to_string();
    // ISO week number and ISO week-numbering year.
    let isoyear = {
        let week_day = (weekday + 6) % 7; // Monday = 0
        let thursday_offset = 3 - week_day;
        let th = ms + thursday_offset as f64 * 86_400_000.0;
        civil_from_days((th / 86_400_000.0).floor() as i64).0
    };
    let jan4 = days_from_civil(isoyear, 1, 4);
    let jan4_weekday = (jan4 + 4).rem_euclid(7); // 1970-01-01 Thursday
    let jan4_monday = jan4 - (jan4_weekday + 6) % 7;
    let day = (ms / 86_400_000.0).floor() as i64;
    let isoweek = ((day - jan4_monday) / 7 + 1).to_string();
    spec.replace("%q", &quarter)
        .replace("%V", &isoweek)
        .replace("%G", &isoyear.to_string())
}

fn days_in_month_expr(y: i64, m: i64) -> i64 {
    match m {
        2 => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn offset_time(unit: &str, ms: f64, step: f64) -> f64 {
    let (y, m, d, h, mi, s, milli, _wd) = date_parts(&json_number(ms));
    match unit {
        "year" | "quarter" => {
            let months = if unit == "quarter" { step * 3.0 } else { step * 12.0 };
            let total = y * 12 + (m - 1) + months as i64;
            let ny = total.div_euclid(12);
            let nm = total.rem_euclid(12) + 1;
            let nd = d.min(days_in_month_expr(ny, nm));
            civil_to_ms(ny, nm, nd, h, mi, s, milli)
        }
        "month" => {
            let total = y * 12 + (m - 1) + step as i64;
            let ny = total.div_euclid(12);
            let nm = total.rem_euclid(12) + 1;
            let nd = d.min(days_in_month_expr(ny, nm));
            civil_to_ms(ny, nm, nd, h, mi, s, milli)
        }
        "week" | "isoweek" => ms + step * 7.0 * 86_400_000.0,
        "date" | "day" | "dayofyear" => ms + step * 86_400_000.0,
        "hours" => ms + step * 3_600_000.0,
        "minutes" => ms + step * 60_000.0,
        "seconds" => ms + step * 1000.0,
        "milliseconds" => ms + step,
        _ => ms,
    }
}

fn civil_to_ms(y: i64, m: i64, d: i64, h: i64, mi: i64, s: i64, ms: i64) -> f64 {
    days_from_civil(y, m, d) as f64 * 86_400_000.0
        + h as f64 * 3_600_000.0
        + mi as f64 * 60_000.0
        + s as f64 * 1000.0
        + ms as f64
}

fn flatten_numbers(args: &[Value]) -> Vec<f64> {
    let mut out = Vec::new();
    fn walk(v: &Value, out: &mut Vec<f64>) {
        match v {
            Value::Array(a) => {
                for x in a {
                    walk(x, out);
                }
            }
            Value::Null => {}
            other => {
                let n = to_number(other);
                if !n.is_nan() {
                    out.push(n);
                }
            }
        }
    }
    for a in args {
        walk(a, &mut out);
    }
    out
}

fn regex_lite_replace(
    s: &str,
    pat: &str,
    repl: &str,
    global: bool,
    case_insensitive: bool,
) -> String {
    // Handles the tiny regexes emitted by Vega-Lite in practice (typically
    // anchored suffix/prefix replacements) without pulling in a regex crate.
    let mut p = pat.to_string();
    if p.starts_with('^') {
        p.remove(0);
        if case_insensitive {
            let lower = s.to_lowercase();
            if lower.starts_with(&p.to_lowercase()) {
                return format!("{}{}", repl, &s[p.len()..]);
            }
            return s.to_string();
        }
        return s.replacen(&p, repl, 1);
    }
    if p.ends_with('$') {
        p.pop();
        if case_insensitive {
            let lower = s.to_lowercase();
            if lower.ends_with(&p.to_lowercase()) {
                return format!("{}{}", &s[..s.len() - p.len()], repl);
            }
            return s.to_string();
        }
        if s.ends_with(&p) {
            return format!("{}{}", &s[..s.len() - p.len()], repl);
        }
        return s.to_string();
    }
    if global {
        s.replace(&p, repl)
    } else {
        s.replacen(&p, repl, 1)
    }
}

// Civil date <-> Unix epoch helpers.

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn datetime(y: f64, mo: f64, d: f64, h: f64, mi: f64, s: f64, ms: f64) -> f64 {
    let days = days_from_civil(y as i64, mo as i64 + 1, d as i64);
    days as f64 * 86_400_000.0 + h * 3_600_000.0 + mi * 60_000.0 + s * 1000.0 + ms
}

fn to_ms(v: &Value) -> f64 {
    match v {
        Value::String(s) => parse_date(s).unwrap_or(0.0),
        other => to_number(other),
    }
}

// ---------------------------------------------------------------------------
// Local time helpers (Vega time expressions use local time; `utc*` variants
// use UTC). The offset is queried from libc so `TZ` is honoured.
// ---------------------------------------------------------------------------

#[cfg(unix)]
mod localtime {
    #[repr(C)]
    struct Tm {
        tm_sec: i32,
        tm_min: i32,
        tm_hour: i32,
        tm_mday: i32,
        tm_mon: i32,
        tm_year: i32,
        tm_wday: i32,
        tm_yday: i32,
        tm_isdst: i32,
        tm_gmtoff: i64,
        tm_zone: *const std::os::raw::c_char,
    }

    extern "C" {
        fn localtime_r(timep: *const i64, result: *mut Tm) -> *mut Tm;
    }

    pub fn offset_minutes(ms: f64) -> i64 {
        let secs = (ms / 1000.0).floor() as i64;
        let mut tm = Tm {
            tm_sec: 0,
            tm_min: 0,
            tm_hour: 0,
            tm_mday: 0,
            tm_mon: 0,
            tm_year: 0,
            tm_wday: 0,
            tm_yday: 0,
            tm_isdst: 0,
            tm_gmtoff: 0,
            tm_zone: std::ptr::null(),
        };
        let ok = unsafe { !localtime_r(&secs, &mut tm).is_null() };
        if ok {
            tm.tm_gmtoff / 60
        } else {
            0
        }
    }
}

#[cfg(not(unix))]
mod localtime {
    pub fn offset_minutes(_ms: f64) -> i64 {
        0
    }
}

fn local_offset_minutes(ms: f64) -> f64 {
    localtime::offset_minutes(ms) as f64
}

/// Wall-clock representation of an instant (epoch milliseconds shifted by the
/// local UTC offset). This is what the UTC-based formatter displays as local
/// time.
fn wall_ms(ms: f64) -> f64 {
    ms + local_offset_minutes(ms) * 60_000.0
}

fn local_datetime_ms(y: f64, mo: f64, d: f64, h: f64, mi: f64, s: f64, ms: f64) -> f64 {
    let days = days_from_civil(y as i64, mo as i64 + 1, d as i64);
    let wall = days as f64 * 86_400_000.0 + h * 3_600_000.0 + mi * 60_000.0 + s * 1000.0 + ms;
    let guess = wall - local_offset_minutes(wall) * 60_000.0;
    wall - local_offset_minutes(guess) * 60_000.0
}

fn local_date_parts(v: &Value) -> (i64, i64, i64, i64, i64, i64, i64, i64) {
    let ms = wall_ms(to_ms(v));
    let days = (ms / 86_400_000.0).floor() as i64;
    let rem = ms - days as f64 * 86_400_000.0;
    let (y, m, d) = civil_from_days(days);
    let h = (rem / 3_600_000.0).floor() as i64;
    let mi = ((rem % 3_600_000.0) / 60_000.0).floor() as i64;
    let s = ((rem % 60_000.0) / 1000.0).floor() as i64;
    let milli = (rem % 1000.0).floor() as i64;
    let weekday = (days + 4).rem_euclid(7);
    (y, m, d, h, mi, s, milli, weekday)
}

fn date_parts(v: &Value) -> (i64, i64, i64, i64, i64, i64, i64, i64) {
    let ms = to_ms(v);
    let days = (ms / 86_400_000.0).floor() as i64;
    let rem = ms - days as f64 * 86_400_000.0;
    let (y, m, d) = civil_from_days(days);
    let h = (rem / 3_600_000.0).floor() as i64;
    let mi = ((rem % 3_600_000.0) / 60_000.0).floor() as i64;
    let s = ((rem % 60_000.0) / 1000.0).floor() as i64;
    let milli = (rem % 1000.0).floor() as i64;
    // 1970-01-01 was a Thursday (4).
    let weekday = (days + 4).rem_euclid(7);
    (y, m, d, h, mi, s, milli, weekday)
}

/// JavaScript `new Date(text)` semantics: ISO date-only strings are UTC, while
/// other unzoned strings are parsed in local time.
fn js_date_ms(text: &str) -> Option<f64> {
    let t = text.trim();
    let ms = parse_date(t)?;
    if t.is_empty() {
        return Some(ms);
    }
    let upper = t.to_ascii_uppercase();
    let has_zone = upper.ends_with('Z')
        || upper.ends_with("GMT")
        || upper.ends_with("UTC")
        || {
            let bytes: Vec<char> = t.chars().collect();
            let mut found = false;
            for i in 0..bytes.len().saturating_sub(5) {
                if (bytes[i] == '+' || bytes[i] == '-')
                    && bytes[i + 1].is_ascii_digit()
                    && bytes[i + 2].is_ascii_digit()
                    && bytes[i + 3] == ':'
                    && bytes[i + 4].is_ascii_digit()
                    && bytes[i + 5].is_ascii_digit()
                {
                    found = true;
                }
            }
            found
        };
    if has_zone {
        return Some(ms);
    }
    let is_iso_date_only = t.len() == 10
        && t.as_bytes()[4] == b'-'
        && t.as_bytes()[7] == b'-'
        && t.chars().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                c == '-'
            } else {
                c.is_ascii_digit()
            }
        });
    if is_iso_date_only {
        return Some(ms);
    }
    let (y, m, d, h, mi, s, milli, _) = date_parts(&json_number(ms));
    Some(local_datetime_ms(
        y as f64,
        (m - 1) as f64,
        d as f64,
        h as f64,
        mi as f64,
        s as f64,
        milli as f64,
    ))
}

fn date_part(v: &Value, part: &str) -> Value {
    let (y, m, d, h, mi, s, ms, weekday) = local_date_parts(v);
    let n = match part {
        "year" => y,
        "quarter" => (m - 1) / 3 + 1,
        "month" => m - 1,
        "date" => d,
        "day" => weekday,
        "hours" => h,
        "minutes" => mi,
        "seconds" => s,
        "milliseconds" => ms,
        _ => 0,
    };
    json_number(n as f64)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (f64, f64, f64) {
    let h = h.rem_euclid(360.0) / 360.0;
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    if s == 0.0 {
        let v = l * 255.0;
        return (v, v, v);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let f = |t: f64| -> f64 {
        let t = if t < 0.0 {
            t + 1.0
        } else if t > 1.0 {
            t - 1.0
        } else {
            t
        };
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    (
        f(h + 1.0 / 3.0) * 255.0,
        f(h) * 255.0,
        f(h - 1.0 / 3.0) * 255.0,
    )
}

/// xorshift64* PRNG: `Math.random()` semantics (a fresh value per call).
fn next_random() -> f64 {
    use std::cell::Cell;
    thread_local! {
        static STATE: Cell<u64> = Cell::new(0);
    }
    STATE.with(|state| {
        let mut x = state.get();
        if x == 0 {
            let seed = current_millis() as u64 ^ 0x9e37_79b9_7f4a_7c15;
            x = if seed == 0 { 1 } else { seed };
        }
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        state.set(x);
        // 53 random bits -> [0, 1).
        ((x >> 11) as f64) / ((1u64 << 53) as f64)
    })
}

/// Inverse error function (Giles' approximation, as used by vega-statistics).
fn erfinv(x: f64) -> f64 {
    let w = -((1.0 - x) * (1.0 + x)).ln();
    let mut p;
    if w < 6.25 {
        let w = w - 3.125;
        p = -3.6444120640178196996e-21;
        for c in [
            -1.685059138182016589e-19,
            1.2858480715256400167e-18,
            1.115787767802518096e-17,
            -1.333171662854620906e-16,
            2.0972767875968561637e-17,
            6.6376381343583238325e-15,
            -4.0545662729752068639e-14,
            -8.1519341976054721522e-14,
            2.6335093153082322977e-12,
            -1.2975133253453532498e-11,
            -5.4154120542946279317e-11,
            1.051212273321532285e-09,
            -4.1126339803469836976e-09,
            -2.9070369957882005086e-08,
            4.2347877827932403518e-07,
            -1.3654692000834678645e-06,
            -1.3882523362786468719e-05,
            0.0001867342080340571352,
            -0.00074070253416626697512,
            -0.0060336708714301490533,
            0.24015818242558961693,
            1.6536545626831027356,
        ] {
            p = c + p * w;
        }
    } else if w < 16.0 {
        let w = w.sqrt() - 3.25;
        p = 2.2137376921775787049e-09;
        for c in [
            9.0756561938885390979e-08,
            -2.7517406297064545428e-07,
            1.8239629214389227755e-08,
            1.5027403968909827627e-06,
            -4.013867526981545969e-06,
            2.9234449089955446044e-06,
            1.2475304481671778723e-05,
            -4.7318229009055733981e-05,
            6.8284851459573175448e-05,
            2.4031110387097893999e-05,
            -0.0003550375203628474796,
            0.00095328937973738049703,
            -0.0016882755560235047313,
            0.0024914420961078508066,
            -0.0037512085075692412107,
            0.005370914553590063617,
            1.0052589676941592334,
            3.0838856104922207635,
        ] {
            p = c + p * w;
        }
    } else if w.is_finite() {
        let w = w.sqrt() - 5.0;
        p = -2.7109920616438573243e-11;
        for c in [
            -2.5556418169965252055e-10,
            1.5076572693500548083e-09,
            -3.7894654401267369937e-09,
            7.6157012080783393804e-09,
            -1.4960026627149240478e-08,
            2.9147953450901080826e-08,
            -6.7711997758452339498e-08,
            2.2900482228026654717e-07,
            -9.9298272942317002539e-07,
            4.5260625972231537039e-06,
            -1.9681778105531670567e-05,
            7.5995277030017761139e-05,
            -0.00021503011930044477347,
            -0.00013871931833623122026,
            1.0103004648645343977,
            4.8499064014085844221,
        ] {
            p = c + p * w;
        }
    } else {
        return f64::INFINITY;
    }
    p * x
}

fn current_millis() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_and_precedence() {
        let env = EmptyEnv;
        assert_eq!(evaluate("1 + 2 * 3", None, &env).unwrap(), json_number(7.0));
        assert_eq!(
            evaluate("2 ** 3 ** 2", None, &env).unwrap(),
            json_number(512.0)
        );
        assert_eq!(
            evaluate("(1 + 2) * 3", None, &env).unwrap(),
            json_number(9.0)
        );
    }

    #[test]
    fn datum_access() {
        let env = EmptyEnv;
        let d = serde_json::json!({"a": {"b": 4}, "c": 2});
        assert_eq!(
            evaluate("datum.a.b + datum['c']", Some(&d), &env).unwrap(),
            json_number(6.0)
        );
    }

    #[test]
    fn ternaries_and_logic() {
        let env = EmptyEnv;
        assert_eq!(
            evaluate(
                "datum.x > 2 ? 'hi' : 'lo'",
                Some(&serde_json::json!({"x": 3})),
                &env
            )
            .unwrap(),
            Value::String("hi".into())
        );
        assert_eq!(
            evaluate("!false && true", None, &env).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn string_ops() {
        let env = EmptyEnv;
        assert_eq!(
            evaluate(
                "upper(datum.a) + '-' + lower('X')",
                Some(&serde_json::json!({"a": "ok"})),
                &env
            )
            .unwrap(),
            Value::String("OK-x".into())
        );
        assert_eq!(
            evaluate("join([1,2,3], '-')", None, &env).unwrap(),
            Value::String("1-2-3".into())
        );
    }

    #[test]
    fn date_functions() {
        let env = EmptyEnv;
        assert_eq!(
            evaluate("year(datetime(2020, 0, 2))", None, &env).unwrap(),
            json_number(2020.0)
        );
        assert_eq!(
            evaluate("timeFormat(datetime(2020, 0, 2), '%Y-%m-%d')", None, &env).unwrap(),
            Value::String("2020-01-02".into())
        );
    }
}


//! Port of `vega-event-selector`'s `parseSelector`.
//!
//! Vega-Lite uses this to normalise the `on`/`clear`/`translate`/`zoom`
//! properties of selections into Vega event-stream objects.

use serde_json::{json, Map, Value};

const COMMA: char = ',';
const COLON: char = ':';
const GT: char = '>';
const NAME: char = '@';
const LBRACK: char = '[';
const RBRACK: char = ']';
const LBRACE: char = '{';
const RBRACE: char = '}';

const DEFAULT_MARKS: [&str; 12] = [
    "*", "arc", "area", "group", "image", "line", "path", "rect", "rule", "shape", "symbol", "text",
];

fn is_mark_type(t: &str) -> bool {
    DEFAULT_MARKS.contains(&t) || t == "trail"
}

fn find(s: &[char], mut i: usize, end_char: char, push: &str, pop: &str) -> usize {
    let n = s.len();
    let mut count: i64 = 0;
    while i < n {
        let c = s[i];
        if count == 0 && c == end_char {
            return i;
        } else if pop.contains(c) {
            count -= 1;
        } else if push.contains(c) {
            count += 1;
        }
        i += 1;
    }
    i
}

fn parse_merge(s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let mut output = Vec::new();
    let mut i = 0usize;
    let mut start = 0usize;
    while i < n {
        i = find(&chars, i, COMMA, "[{", "]}");
        let piece: String = chars[start..i.min(n)].iter().collect();
        output.push(piece.trim().to_string());
        i += 1;
        start = i;
    }
    output
}

/// Parse an event selector string into a list of Vega event streams.
pub fn parse_selector(selector: &str, source: &str) -> Vec<Value> {
    parse_merge(selector.trim())
        .iter()
        .map(|s| parse_selector_one(s, source))
        .collect()
}

fn parse_selector_one(s: &str, source: &str) -> Value {
    if s.starts_with(LBRACK) {
        parse_between(s, source)
    } else {
        parse_stream(s, source)
    }
}

fn parse_between(s: &str, source: &str) -> Value {
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let i = find(&chars, 1, RBRACK, "[", "]");
    if i == n {
        panic!("Empty between selector: {s}");
    }
    let b = parse_merge(&chars[1..i].iter().collect::<String>());
    if b.len() != 2 {
        panic!("Between selector must have two elements: {s}");
    }
    let rest: String = chars[i + 1..].iter().collect();
    let rest = rest.trim().to_string();
    if !rest.starts_with(GT) {
        panic!("Expected '>' after between selector: {s}");
    }

    let between: Vec<Value> = b.iter().map(|x| parse_selector_one(x, source)).collect();

    let after = rest[1..].trim().to_string();
    let mut stream = parse_selector_one(&after, source);
    let obj = stream.as_object_mut().unwrap();
    if obj.contains_key("between") {
        return json!({"between": between, "stream": stream});
    }
    obj.insert("between".to_string(), Value::Array(between));
    stream
}

fn parse_stream(s: &str, source: &str) -> Value {
    let mut s: Vec<char> = s.chars().collect();
    let mut n = s.len();

    let mut throttle: Vec<f64> = Vec::new();
    let mut markname = false;

    if n > 0 && s[n - 1] == RBRACE {
        let i = s.iter().rposition(|&c| c == LBRACE);
        match i {
            Some(i) => {
                let inner: String = s[i + 1..n - 1].iter().collect();
                throttle = parse_throttle(&inner);
                s = s[..i].to_vec();
                let trimmed: String = s.iter().collect::<String>().trim().to_string();
                s = trimmed.chars().collect();
                n = s.len();
            }
            None => panic!("Unmatched right brace"),
        }
    }

    if n == 0 {
        panic!("Empty event stream");
    }

    let mut i = 0usize;
    if s[0] == NAME {
        markname = true;
        i = 1;
    }

    let mut source_parts: Vec<String> = Vec::new();
    let mut start = 0usize;

    let j = find(&s, i, COLON, "", "");
    if j < n {
        source_parts.push(s[start..j].iter().collect::<String>().trim().to_string());
        i = j + 1;
        start = i;
    }

    let mut filter: Option<Vec<String>> = None;
    i = find(&s, i, LBRACK, "", "");
    if i == n {
        source_parts.push(s[start..n].iter().collect::<String>().trim().to_string());
    } else {
        source_parts.push(s[start..i].iter().collect::<String>().trim().to_string());
        filter = Some(Vec::new());
        i += 1;
        start = i;
        if start == n {
            panic!("Unmatched left bracket");
        }
    }

    while i < n {
        i = find(&s, i, RBRACK, "", "");
        if i == n {
            panic!("Unmatched left bracket");
        }
        filter
            .as_mut()
            .unwrap()
            .push(s[start..i].iter().collect::<String>().trim().to_string());
        if i < n - 1 {
            i += 1;
            if s[i] != LBRACK {
                panic!("Expected left bracket");
            }
        }
        i += 1;
        start = i;
    }

    let mut stream = Map::new();
    stream.insert("source".to_string(), json!(source));

    let count = source_parts.len();
    if count == 0 || source_parts[count - 1].contains(['[', ']', '{', '}']) {
        panic!("Invalid event selector");
    }

    let mut type_: String;
    if count > 1 {
        type_ = source_parts[1].clone();
        if markname {
            let first = source_parts[0].clone();
            let name: String = first.chars().skip(1).collect();
            stream.insert("markname".to_string(), json!(name));
        } else if is_mark_type(&source_parts[0]) {
            stream.insert("marktype".to_string(), json!(source_parts[0]));
        } else {
            stream.insert("source".to_string(), json!(source_parts[0]));
        }
    } else {
        type_ = source_parts[0].clone();
    }

    if type_.ends_with('!') {
        stream.insert("consume".to_string(), Value::Bool(true));
        type_.pop();
    }

    stream.insert("type".to_string(), json!(type_));

    if let Some(filter) = filter {
        stream.insert("filter".to_string(), json!(filter));
    }
    if !throttle.is_empty() && throttle[0] != 0.0 {
        stream.insert("throttle".to_string(), json!(throttle[0]));
    }
    if throttle.len() > 1 && throttle[1] != 0.0 {
        stream.insert("debounce".to_string(), json!(throttle[1]));
    }

    Value::Object(stream)
}

fn parse_throttle(s: &str) -> Vec<f64> {
    if s.is_empty() {
        panic!("Invalid throttle specification");
    }
    let parts: Vec<&str> = s.split(COMMA).collect();
    if parts.len() > 2 {
        panic!("Invalid throttle specification");
    }
    parts
        .iter()
        .map(|p| {
            p.trim()
                .parse::<f64>()
                .unwrap_or_else(|_| panic!("Invalid throttle specification"))
        })
        .collect()
}

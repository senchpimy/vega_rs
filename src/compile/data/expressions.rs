//! Port of `src/compile/data/expressions.ts`.
//!
//! Upstream parses the expression with `vega-expression` and visits every
//! `MemberExpression` that starts with `datum`.  This module implements the
//! same dependency extraction without a full JavaScript parser; it recognises
//! `datum.foo`, `datum["foo"]`, `datum['foo']` and nested combinations.

use indexmap::IndexSet;

/// Port of `getDependentFields`.
pub fn get_dependent_fields(expression: &str) -> IndexSet<String> {
    let bytes = expression.as_bytes();
    let mut dependents = IndexSet::new();
    let mut i = 0;

    while i + 5 <= bytes.len() {
        if &bytes[i..i + 5] != b"datum" {
            i += 1;
            continue;
        }
        let prev_ok = i == 0
            || !(bytes[i - 1].is_ascii_alphanumeric()
                || bytes[i - 1] == b'_'
                || bytes[i - 1] == b'.');
        if !prev_ok {
            i += 1;
            continue;
        }

        let mut j = i + 5;
        let mut parts: Vec<String> = Vec::new();
        loop {
            if j < bytes.len() && bytes[j] == b'.' {
                j += 1;
                let start = j;
                while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                    j += 1;
                }
                if j > start {
                    parts.push(expression[start..j].to_string());
                    dependents.insert(parts.join("."));
                } else {
                    break;
                }
            } else if j < bytes.len() && bytes[j] == b'[' {
                j += 1;
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && (bytes[j] == b'"' || bytes[j] == b'\'') {
                    let quote = bytes[j];
                    j += 1;
                    let start = j;
                    while j < bytes.len() && bytes[j] != quote {
                        j += 1;
                    }
                    let mut s = expression[start..j].to_string();
                    s = s.replace("\\'", "'").replace("\\\"", "\"");
                    parts.push(s);
                    dependents.insert(parts.join("."));
                    j += 1;
                    while j < bytes.len() && bytes[j] != b']' {
                        j += 1;
                    }
                    j += 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        i = j;
    }

    dependents
}

//! Stdin/stdout compile helper used by the local Python viewer.
//!
//! Reads a Vega-Lite specification as JSON from stdin and writes
//! `{"vega", "normalized", "ms"}` on success or `{"error", "ms"}` on failure.
//! This keeps the web server out of the Rust workspace while still exercising
//! the actual compiler.
//!
//! ```sh
//! echo '{"mark":"point"}' | cargo run -q --bin vl-compile
//! ```

use std::io::{Read, Write};
use std::time::Instant;

use serde_json::{json, Value};

fn main() {
    let mut input = String::new();
    if let Err(err) = std::io::stdin().read_to_string(&mut input) {
        print_json(&json!({ "error": format!("failed to read stdin: {err}") }));
        std::process::exit(2);
    }

    let spec: Value = match serde_json::from_str(&input) {
        Ok(spec) => spec,
        Err(err) => {
            print_json(&json!({ "error": format!("invalid JSON: {err}") }));
            std::process::exit(2);
        }
    };

    let started = Instant::now();
    match vega_lite::compile(&spec) {
        Ok(result) => print_json(&json!({
            "vega": result.spec,
            "normalized": result.normalized,
            "ms": started.elapsed().as_secs_f64() * 1000.0,
        })),
        Err(err) => {
            print_json(&json!({
                "error": err,
                "ms": started.elapsed().as_secs_f64() * 1000.0,
            }));
            std::process::exit(1);
        }
    }
}

fn print_json(value: &Value) {
    // Tolerate a closed downstream pipe (for example `... | head`).
    let _ = writeln!(std::io::stdout(), "{value}");
}

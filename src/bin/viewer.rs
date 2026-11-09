//! Local side-by-side viewer for Vega-Lite specs and their compiled Vega output.
//!
//! Run with:
//!   cargo run --bin viewer
//!
//! Options (environment variables):
//!   VEGA_LITE_REPO  path to a checkout of vega/vega-lite
//!                   (default: /tmp/opencode/vega-lite)
//!   PORT            port to listen on (default: 8080)

use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use serde_json::{json, Value};

const INDEX_HTML: &str = include_str!("viewer.html");

fn main() {
    let repo = env::var("VEGA_LITE_REPO")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/opencode/vega-lite"));
    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    if !repo.join("examples/specs").is_dir() {
        eprintln!(
            "warning: {} does not look like a Vega-Lite checkout (examples/specs missing)",
            repo.display()
        );
    }

    let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind");
    println!("Vega-Lite viewer: http://127.0.0.1:{port}");
    println!("examples: {}", repo.join("examples").display());

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(err) = handle(stream, &repo) {
                    eprintln!("request error: {err}");
                }
            }
            Err(err) => eprintln!("connection error: {err}"),
        }
    }
}

fn handle(mut stream: TcpStream, repo: &Path) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Ok(());
    }

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");

    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }

    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }

    let (path, query) = split_target(target);

    match (method, path.as_str()) {
        ("GET", "/") | ("GET", "/index.html") => {
            respond(&mut stream, 200, "text/html; charset=utf-8", INDEX_HTML.as_bytes())
        }
        ("GET", path) if path.starts_with("/data/") => {
            let file = percent_decode(path.trim_start_matches("/data/"));
            match data_dir(repo).and_then(|dir| serve_data_file(&dir, &file)) {
                Some((content_type, bytes)) => respond(&mut stream, 200, content_type, &bytes),
                None => respond(&mut stream, 404, "text/plain; charset=utf-8", b"data not found"),
            }
        }
        ("GET", "/api/examples") => {
            let list = list_examples(repo, &query);
            let payload = json!({ "examples": list });
            respond_json(&mut stream, 200, &payload)
        }
        ("GET", "/api/example") => {
            let name = query_param(&query, "name").unwrap_or_default();
            match load_example(repo, &name) {
                Some(payload) => respond_json(&mut stream, 200, &payload),
                None => respond_json(
                    &mut stream,
                    404,
                    &json!({ "error": format!("example not found: {name}") }),
                ),
            }
        }
        ("POST", "/api/compile") => {
            let spec: Value = match serde_json::from_slice(&body) {
                Ok(v) => v,
                Err(err) => {
                    return respond_json(
                        &mut stream,
                        400,
                        &json!({ "error": format!("invalid JSON: {err}") }),
                    )
                }
            };
            let started = std::time::Instant::now();
            match vega_lite::compile::compile::compile(&spec) {
                Ok(result) => respond_json(
                    &mut stream,
                    200,
                    &json!({
                        "vega": result.spec,
                        "normalized": result.normalized,
                        "ms": started.elapsed().as_secs_f64() * 1000.0,
                    }),
                ),
                Err(err) => respond_json(
                    &mut stream,
                    501,
                    &json!({
                        "error": err,
                        "ms": started.elapsed().as_secs_f64() * 1000.0,
                        "hint": "The end-to-end Rust compiler is still being assembled. Use the expected output from examples meanwhile."
                    }),
                ),
            }
        }
        ("POST", "/api/compile_ts") => {
            let spec: Value = match serde_json::from_slice(&body) {
                Ok(v) => v,
                Err(err) => {
                    return respond_json(
                        &mut stream,
                        400,
                        &json!({ "error": format!("invalid JSON: {err}") }),
                    )
                }
            };
            match run_ts_compiler(&spec) {
                Ok(value) => respond_json(&mut stream, 200, &value),
                Err(err) => respond_json(
                    &mut stream,
                    503,
                    &json!({
                        "error": err,
                        "hint": "Run `bun add vega-lite@6.5.0` in the project or set COMPILE_TS_SCRIPT."
                    }),
                ),
            }
        }
        ("POST", "/api/render_svg") => {
            let spec: Value = match serde_json::from_slice(&body) {
                Ok(v) => v,
                Err(err) => {
                    return respond_json(
                        &mut stream,
                        400,
                        &json!({ "error": format!("invalid JSON: {err}") }),
                    )
                }
            };
            let started = std::time::Instant::now();
            match vega_lite::compile::compile::compile(&spec) {
                Ok(result) => match run_vega_svg(&result.spec, repo) {
                    Ok(svg) => {
                        let interactive = has_interactivity(&result.spec);
                        respond_json(
                            &mut stream,
                            200,
                            &json!({
                                "vega": result.spec,
                                "svg": svg,
                                "interactive": interactive,
                                "ms": started.elapsed().as_secs_f64() * 1000.0,
                            }),
                        )
                    }
                    Err(err) => respond_json(
                        &mut stream,
                        502,
                        &json!({
                            "vega": result.spec,
                            "error": err,
                            "ms": started.elapsed().as_secs_f64() * 1000.0,
                            "hint": "Build the Rust renderer with `cargo build -p vega-svg --bin vega-svg` or set VEGA_SVG_BIN."
                        }),
                    ),
                },
                Err(err) => respond_json(
                    &mut stream,
                    501,
                    &json!({
                        "error": err,
                        "ms": started.elapsed().as_secs_f64() * 1000.0,
                    }),
                ),
            }
        }
        _ => respond(&mut stream, 404, "text/plain; charset=utf-8", b"not found"),
    }
}

fn has_interactivity(vega: &Value) -> bool {
    vega.get("signals")
        .and_then(|s| s.as_array())
        .map(|signals| {
            signals.iter().any(|s| {
                s.get("on").is_some()
                    || s.get("bind").is_some()
                    || s.get("react").is_some()
            })
        })
        .unwrap_or(false)
}

fn vega_svg_binary() -> Result<PathBuf, String> {
    if let Ok(path) = env::var("VEGA_SVG_BIN") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!("VEGA_SVG_BIN no existe: {}", path.display()));
    }
    static BIN: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    BIN.get_or_init(|| {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        // Prefer optimized builds: rendering in a debug build is ~6x slower and
        // dominates the viewer's response time.
        let mut candidates = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                candidates.push(dir.join("release").join("vega-svg"));
                candidates.push(dir.join("vega-svg"));
                candidates.push(dir.join("vega-svg.exe"));
            }
        }
        candidates.push(manifest.join("target/release/vega-svg"));
        candidates.push(manifest.join("target/debug/vega-svg"));
        for candidate in &candidates {
            if candidate.is_file() {
                return Ok(candidate.clone());
            }
        }
        // Always rebuild once per viewer process so the Todo Rust panel uses the
        // current source, even when an older target/debug/vega-svg is present.
        let output = Command::new("cargo")
            .args(["build", "-q", "--release", "-p", "vega-svg", "--bin", "vega-svg"])
            .current_dir(manifest)
            .output()
            .map_err(|e| format!("no se pudo ejecutar cargo: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "no se pudo compilar el renderizador vega-svg: {}",
                stderr.lines().rev().take(8).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n")
            ));
        }
        candidates
            .into_iter()
            .find(|p| p.is_file())
            .ok_or_else(|| "no se encontró el binario vega-svg después de compilar".to_string())
    })
    .clone()
}

fn datasets_root(repo: &Path) -> Option<PathBuf> {
    data_dir(repo).and_then(|d| d.parent().map(|p| p.to_path_buf()))
}

fn run_vega_svg(vega: &Value, repo: &Path) -> Result<String, String> {
    let bin = vega_svg_binary()?;
    let mut command = Command::new(bin);
    command.arg("-");
    if let Some(root) = datasets_root(repo) {
        command.arg("--base").arg(root);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("no se pudo ejecutar vega-svg: {e}"))?;
    let input = serde_json::to_vec(vega).map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(&input)
            .map_err(|e| format!("no se pudo escribir al renderizador Rust: {e}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("el renderizador Rust falló: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let svg = String::from_utf8_lossy(&output.stdout).into_owned();
    let trimmed = svg.trim_start();
    if !(trimmed.starts_with("<svg") || (trimmed.starts_with("<?xml") && trimmed.contains("<svg"))) {
        return Err(format!(
            "salida inesperada de vega-svg: {}",
            svg.chars().take(200).collect::<String>()
        ));
    }
    Ok(svg)
}

/// Runs the upstream TypeScript compiler through bun. The returned `ms` is the
/// time measured *inside* `compile()`; `wall_ms` includes bun's startup.
fn run_ts_compiler(spec: &Value) -> Result<Value, String> {
    let script = env::var("COMPILE_TS_SCRIPT").unwrap_or_else(|_| "scripts/compile_ts.mjs".to_string());
    let bun = env::var("BUN").unwrap_or_else(|_| "bun".to_string());
    let started = std::time::Instant::now();

    let mut child = Command::new(&bun)
        .arg(&script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("no se pudo ejecutar `{bun}`: {err}"))?;

    let input = serde_json::to_vec(spec).map_err(|err| err.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(&input)
            .map_err(|err| format!("no se pudo escribir al compilador TS: {err}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|err| format!("el compilador TS falló: {err}"))?;
    let wall_ms = started.elapsed().as_secs_f64() * 1000.0;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut value: Value = serde_json::from_str(stdout.trim())
        .map_err(|err| format!("respuesta inválida del compilador TS: {err}"))?;

    if value.get("ok").and_then(|v| v.as_bool()) == Some(false) {
        return Err(value
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("error desconocido en el compilador TS")
            .to_string());
    }

    if let Some(obj) = value.as_object_mut() {
        obj.insert("wall_ms".to_string(), json!(wall_ms));
    }
    Ok(value)
}

fn split_target(target: &str) -> (String, String) {    match target.split_once('?') {
        Some((path, query)) => (path.to_string(), query.to_string()),
        None => (target.to_string(), String::new()),
    }
}

fn query_param(query: &str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        if k == key {
            return Some(percent_decode(v));
        }
    }
    None
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    out.push(byte);
                    i += 3;
                    continue;
                }
                out.push(bytes[i]);
                i += 1;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn list_examples(repo: &Path, query: &str) -> Vec<String> {
    let filter = query_param(query, "q").unwrap_or_default().to_lowercase();
    let specs = repo.join("examples/specs");
    let mut names = Vec::new();
    collect_specs(&specs, "", &mut names);
    let normalized = specs.join("normalized");
    if normalized.is_dir() {
        collect_specs(&normalized, "normalized/", &mut names);
    }
    names.sort();
    if filter.is_empty() {
        names
    } else {
        names
            .into_iter()
            .filter(|n| n.to_lowercase().contains(&filter))
            .collect()
    }
}

fn collect_specs(dir: &Path, prefix: &str, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !file_name.ends_with(".vl.json") {
            continue;
        }
        let stem = file_name.trim_end_matches(".vl.json");
        out.push(format!("{prefix}{stem}"));
    }
}

fn data_dir(repo: &Path) -> Option<PathBuf> {
    if let Ok(dir) = env::var("VEGA_DATASETS_DIR") {
        let dir = PathBuf::from(dir);
        if dir.is_dir() {
            return Some(dir);
        }
    }
    let candidates = [
        PathBuf::from("node_modules/vega-datasets/data"),
        repo.join("site/data"),
        repo.join("node_modules/vega-datasets/data"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("node_modules/vega-datasets/data"),
    ];
    candidates.into_iter().find(|p| p.is_dir())
}

fn serve_data_file(dir: &Path, file: &str) -> Option<(&'static str, Vec<u8>)> {
    if file.is_empty() || file.contains("..") || file.contains('\\') || file.contains('\0') || file.starts_with('/') {
        return None;
    }
    let path = dir.join(file);
    let bytes = fs::read(&path).ok()?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let content_type = match ext.to_ascii_lowercase().as_str() {
        "json" => "application/json",
        "csv" => "text/csv; charset=utf-8",
        "tsv" => "text/tab-separated-values; charset=utf-8",
        "txt" => "text/plain; charset=utf-8",
        "topojson" => "application/json",
        "geojson" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    };
    Some((content_type, bytes))
}

fn load_example(repo: &Path, name: &str) -> Option<Value> {
    if name.is_empty()
        || name.contains("..")
        || name.contains('\\')
        || name.contains('\0')
        || name.starts_with('/')
    {
        return None;
    }
    if !list_examples(repo, "").iter().any(|n| n == name) {
        return None;
    }

    let spec_path = repo.join(format!("examples/specs/{name}.vl.json"));
    let spec_text = fs::read_to_string(&spec_path).ok()?;
    let spec: Value = serde_json::from_str(&spec_text).ok()?;

    let compiled_path = repo.join(format!("examples/compiled/{name}.vg.json"));
    let compiled = fs::read_to_string(compiled_path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok());

    Some(json!({
        "name": name,
        "spec": spec,
        "compiled": compiled,
    }))
}

fn respond_json(stream: &mut TcpStream, status: u16, value: &Value) -> std::io::Result<()> {
    let body = serde_json::to_vec(value).unwrap_or_default();
    respond(stream, status, "application/json", &body)
}

fn respond(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        501 => "Not Implemented",
        _ => "Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()
}

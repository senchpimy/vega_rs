//! Command line interface for rendering Vega / Vega-Lite specifications to SVG.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
vega-svg [OPTIONS] <INPUT>

Arguments:
  <INPUT>  Input JSON file, or \"-\" for stdin

Options:
  -o, --output <FILE>        Output SVG file (default: stdout)
      --vega-lite            Treat input as Vega-Lite (compiled in memory)
      --base <DIR>           Base directory for relative data URLs (default: cwd / input dir)
      --data-dir <DIR>       Extra data directory (repeatable)
      --scale <N>            Output scale factor (default 1)
      --background <COLOR>   Override background color
  -h, --help                 Print help
";

#[derive(Debug)]
struct Args {
    input: String,
    output: Option<PathBuf>,
    vega_lite: bool,
    base: Option<PathBuf>,
    data_dirs: Vec<PathBuf>,
    scale: f64,
    background: Option<String>,
}

enum Parsed {
    Help,
    Run(Args),
}

fn parse_args(argv: Vec<String>) -> Result<Parsed, String> {
    let mut input: Option<String> = None;
    let mut output: Option<PathBuf> = None;
    let mut vega_lite = false;
    let mut base: Option<PathBuf> = None;
    let mut data_dirs: Vec<PathBuf> = Vec::new();
    let mut scale: f64 = 1.0;
    let mut background: Option<String> = None;

    let mut iter = argv.into_iter().peekable();
    while let Some(arg) = iter.next() {
        // Split `--flag=value` / `-o=value` forms.
        let (flag, inline_value) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with('-') => (f.to_string(), Some(v.to_string())),
            _ => (arg.clone(), None),
        };

        match flag.as_str() {
            "-h" | "--help" => return Ok(Parsed::Help),
            "--vega-lite" => {
                if inline_value.is_some() {
                    return Err(format!("option '{}' does not take a value", flag));
                }
                vega_lite = true;
            }
            "-o" | "--output" => {
                let value = take_value(&flag, inline_value, &mut iter)?;
                output = Some(PathBuf::from(value));
            }
            "--base" => {
                let value = take_value(&flag, inline_value, &mut iter)?;
                base = Some(PathBuf::from(value));
            }
            "--data-dir" => {
                let value = take_value(&flag, inline_value, &mut iter)?;
                data_dirs.push(PathBuf::from(value));
            }
            "--scale" => {
                let value = take_value(&flag, inline_value, &mut iter)?;
                scale = value
                    .parse::<f64>()
                    .map_err(|_| format!("invalid value for '--scale': {}", value))?;
                if !scale.is_finite() || scale <= 0.0 {
                    return Err(format!("invalid value for '--scale': {}", value));
                }
            }
            "--background" => {
                let value = take_value(&flag, inline_value, &mut iter)?;
                background = Some(value);
            }
            "-" => {
                if input.is_some() {
                    return Err("unexpected extra positional argument".to_string());
                }
                input = Some("-".to_string());
            }
            _ if flag.starts_with('-') && flag != "-" => {
                return Err(format!("unknown option: {}", flag));
            }
            _ => {
                if input.is_some() {
                    return Err(format!("unexpected argument: {}", arg));
                }
                input = Some(arg);
            }
        }
    }

    let input = input.ok_or_else(|| "missing required argument: <INPUT>".to_string())?;

    Ok(Parsed::Run(Args {
        input,
        output,
        vega_lite,
        base,
        data_dirs,
        scale,
        background,
    }))
}

fn take_value<I>(
    flag: &str,
    inline: Option<String>,
    iter: &mut std::iter::Peekable<I>,
) -> Result<String, String>
where
    I: Iterator<Item = String>,
{
    if let Some(value) = inline {
        return Ok(value);
    }
    match iter.next() {
        Some(value) => Ok(value),
        None => Err(format!("option '{}' requires a value", flag)),
    }
}

fn default_base(input: &str) -> PathBuf {
    if input == "-" {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    } else {
        let path = Path::new(input);
        match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => {
                if parent.is_absolute() {
                    parent.to_path_buf()
                } else {
                    std::env::current_dir()
                        .map(|cwd| cwd.join(parent))
                        .unwrap_or_else(|_| parent.to_path_buf())
                }
            }
            _ => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }
}

fn read_input(input: &str) -> Result<String, String> {
    if input == "-" {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("failed to read stdin: {}", e))?;
        Ok(buf)
    } else {
        std::fs::read_to_string(input).map_err(|e| format!("failed to read '{}': {}", input, e))
    }
}

fn run() -> Result<(), String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse_args(argv)? {
        Parsed::Help => {
            print!("{}", USAGE);
            return Ok(());
        }
        Parsed::Run(args) => args,
    };

    let text = read_input(&args.input)?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("invalid JSON: {}", e))?;

    let opts = vega_svg::RenderOptions {
        data_base: args
            .base
            .clone()
            .unwrap_or_else(|| default_base(&args.input)),
        data_dirs: args.data_dirs.clone(),
        scale: args.scale,
        background: args.background.clone(),
    };

    let svg = if args.vega_lite {
        vega_svg::vega_lite_to_svg_with_options(&value, &opts)
    } else {
        vega_svg::vega_to_svg_with_options(&value, &opts)
    }
    .map_err(|e| format!("{}", e))?;

    let mut output = String::with_capacity(svg.len() + 1);
    output.push_str(&svg);
    if !output.ends_with('\n') {
        output.push('\n');
    }

    match &args.output {
        Some(path) => {
            std::fs::write(path, output)
                .map_err(|e| format!("failed to write '{}': {}", path.display(), e))?;
        }
        None => {
            let stdout = io::stdout();
            let mut lock = stdout.lock();
            lock.write_all(output.as_bytes())
                .map_err(|e| format!("failed to write stdout: {}", e))?;
            lock.flush()
                .map_err(|e| format!("failed to flush stdout: {}", e))?;
        }
    }

    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {}", message);
            ExitCode::FAILURE
        }
    }
}

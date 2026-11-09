//! Profile a single spec for N seconds (useful for hangs / very slow specs).
//!
//! ```sh
//! cargo run --release -p vega-svg --example profile_hang -- spec.vg.json out.svg 5
//! ```

use std::env;
use std::fs;
use std::time::Duration;

use serde_json::Value;

fn main() {
    let mut args = env::args().skip(1);
    let spec_path = args.next().expect("spec path");
    let out = args.next().unwrap_or_else(|| "/tmp/opencode/hang.svg".into());
    let secs: u64 = args.next().and_then(|v| v.parse().ok()).unwrap_or(5);

    let spec: Value = serde_json::from_str(&fs::read_to_string(&spec_path).expect("read spec"))
        .expect("parse spec");
    let opts = vega_svg::RenderOptions {
        data_base: "node_modules/vega-datasets".into(),
        ..Default::default()
    };

    let guard = pprof::ProfilerGuardBuilder::default()
        .frequency(997)
        .blocklist(&["libc", "libgcc", "pthread", "vdso"])
        .build()
        .expect("start profiler");

    std::thread::spawn(move || {
        let result = vega_svg::vega_to_svg_with_options(&spec, &opts);
        match result {
            Ok(svg) => {
                fs::write(&out, svg).ok();
                eprintln!("finished (wrote {out})");
            }
            Err(err) => eprintln!("error: {err}"),
        }
        std::process::exit(0);
    });

    std::thread::sleep(Duration::from_secs(secs));
    let report = guard.report().build().expect("build report");
    let file = fs::File::create("/tmp/opencode/hang-flame.svg").expect("create");
    report.flamegraph(file).expect("write flamegraph");
    eprintln!("still running after {secs}s; flamegraph at /tmp/opencode/hang-flame.svg");
    std::process::exit(2);
}

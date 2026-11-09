use std::path::PathBuf;

use serde_json::json;
use vega_svg::{
    vega_lite_to_svg, vega_lite_to_svg_with_options, vega_to_svg, vega_to_svg_with_options,
    RenderOptions,
};

fn data_base() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("node_modules")
        .join("vega-datasets")
}

fn opts() -> RenderOptions {
    RenderOptions {
        data_base: data_base(),
        ..Default::default()
    }
}

#[test]
fn renders_a_bar_chart_to_svg() {
    let spec = json!({
        "width": 200,
        "height": 100,
        "padding": 5,
        "data": [
            {"name": "t", "values": [
                {"a": "A", "b": 28},
                {"a": "B", "b": 55},
                {"a": "C", "b": 43}
            ]}
        ],
        "scales": [
            {"name": "x", "type": "band", "domain": {"data": "t", "field": "a"},
             "range": [0, 200], "paddingInner": 0.1, "paddingOuter": 0.05},
            {"name": "y", "type": "linear", "domain": {"data": "t", "field": "b"},
             "range": [100, 0], "nice": true, "zero": true}
        ],
        "axes": [
            {"scale": "x", "orient": "bottom", "title": "a"},
            {"scale": "y", "orient": "left", "title": "b"}
        ],
        "marks": [{
            "type": "rect",
            "from": {"data": "t"},
            "encode": {"update": {
                "fill": {"value": "#4c78a8"},
                "x": {"scale": "x", "field": "a"},
                "width": {"signal": "max(0.25, bandwidth('x'))"},
                "y": {"scale": "y", "field": "b"},
                "y2": {"scale": "y", "value": 0}
            }}
        }]
    });
    let svg = vega_to_svg(&spec).expect("render");
    assert!(svg.starts_with("<svg"), "{svg}");
    assert!(svg.ends_with("</svg>"), "{svg}");
    assert!(svg.contains("mark-rect"), "{svg}");
    assert!(svg.contains("fill=\"#4c78a8\""), "{svg}");
    assert!(svg.contains(">A</text>"), "{svg}");
    assert!(svg.contains(">B</text>"), "{svg}");
    // The plot has three bars.
    assert_eq!(svg.matches("fill=\"#4c78a8\"").count() >= 3, true, "{svg}");
}

#[test]
fn renders_a_vega_lite_bar_directly() {
    let spec = json!({
        "data": {"values": [{"a": "A", "b": 1}, {"a": "B", "b": 2}]},
        "mark": "bar",
        "encoding": {
            "x": {"field": "a", "type": "nominal"},
            "y": {"field": "b", "type": "quantitative"}
        }
    });
    let svg = vega_lite_to_svg(&spec).expect("render");
    assert!(svg.starts_with("<svg"), "{svg}");
    assert!(svg.contains("mark-rect"), "{svg}");
    assert!(svg.contains("#4c78a8"), "{svg}");
}

#[test]
fn supports_grouped_concat_layout() {
    let spec = json!({
        "data": {"values": [{"a": "A", "b": 1}, {"a": "B", "b": 2}]},
        "hconcat": [
            {"mark": "bar", "encoding": {
                "x": {"field": "a", "type": "nominal"},
                "y": {"field": "b", "type": "quantitative"}
            }},
            {"mark": "point", "encoding": {
                "x": {"field": "a", "type": "nominal"},
                "y": {"field": "b", "type": "quantitative"}
            }}
        ]
    });
    let svg = vega_lite_to_svg(&spec).expect("render");
    assert!(svg.starts_with("<svg"), "{svg}");
    assert!(svg.matches("role-mark").count() >= 2, "{svg}");
}

#[test]
fn encodes_text_and_gradients() {
    let spec = json!({
        "width": 100,
        "height": 100,
        "data": [{"name": "t", "values": [{"a": "left <&>", "b": 3}]}],
        "scales": [
            {"name": "x", "type": "linear", "domain": [0, 3], "range": [0, 100]},
            {"name": "y", "type": "linear", "domain": [0, 3], "range": [100, 0]}
        ],
        "marks": [{
            "type": "text",
            "from": {"data": "t"},
            "encode": {"update": {
                "x": {"scale": "x", "field": "b"},
                "y": {"scale": "y", "field": "b"},
                "text": {"field": "a"},
                "angle": {"value": 45},
                "fill": {"value": {
                    "gradient": "linear",
                    "stops": [{"offset": 0, "color": "white"}, {"offset": 1, "color": "red"}]
                }}
            }}
        }]
    });
    let svg = vega_to_svg(&spec).expect("render");
    assert!(svg.contains("<linearGradient"), "{svg}");
    assert!(svg.contains("left &lt;&amp;&gt;"), "{svg}");
    assert!(svg.contains("rotate(45"), "{svg}");
}

#[test]
fn renders_every_compiled_example() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("examples")
        .join("compiled");
    let mut count = 0;
    for entry in std::fs::read_dir(&dir).expect("fixtures") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read fixture");
        let spec: serde_json::Value = serde_json::from_str(&text).expect("parse fixture");
        let svg = vega_to_svg_with_options(&spec, &opts())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(svg.starts_with("<svg"), "{}", path.display());
        assert!(svg.ends_with("</svg>"), "{}", path.display());
        assert!(svg.len() > 200, "{}", path.display());
        count += 1;
    }
    assert!(count >= 30, "expected fixtures, got {count}");
}

#[test]
fn vega_lite_with_local_data_url() {
    let spec = json!({
        "data": {"url": "data/cars.json"},
        "mark": "point",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative"}
        }
    });
    let svg = vega_lite_to_svg_with_options(&spec, &opts()).expect("render");
    assert!(svg.contains("mark-symbol"), "{svg}");
    assert!(svg.contains("<circle") || svg.contains("<path"), "{svg}");
}

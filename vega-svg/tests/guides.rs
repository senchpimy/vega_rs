//! Integration tests for axes, legends, scales and data transforms.
//!
//! These tests intentionally use small inline specs and assert only the guide
//! and mark properties described in `AGENT_GUIDE_TESTS.md`.

use serde_json::{json, Value};
use vega_svg::{
    vega_lite_to_svg, vega_lite_to_svg_with_options, vega_to_svg, vega_to_svg_with_options,
    RenderOptions,
};

fn render_vega(spec: &Value) -> String {
    vega_to_svg_with_options(spec, &RenderOptions::default())
        .unwrap_or_else(|err| panic!("Vega spec should render: {err}\n{spec}"))
}

fn render_vega_lite(spec: &Value) -> String {
    vega_lite_to_svg_with_options(spec, &RenderOptions::default())
        .unwrap_or_else(|err| panic!("Vega-Lite spec should render: {err}\n{spec}"))
}

/// Return the `<text>` children of the first group whose class contains
/// `class_name`, as `(x, text)` pairs. Guide text groups contain no nested
/// groups, so scanning up to the first `</g>` is sufficient.
fn guide_texts(svg: &str, class_name: &str) -> Vec<(f64, String)> {
    let mut result = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = svg[cursor..].find(class_name) {
        let start = cursor + relative;
        let Some(end_relative) = svg[start..].find("</g>") else {
            break;
        };
        let group = &svg[start..start + end_relative];
        let mut rest = group;
        while let Some(text_start) = rest.find("<text") {
            rest = &rest[text_start + "<text".len()..];
            let Some(tag_end) = rest.find('>') else {
                break;
            };
            let tag = &rest[..tag_end];
            let x = tag
                .split("x=\"")
                .nth(1)
                .and_then(|value| value.split('"').next())
                .and_then(|value| value.parse::<f64>().ok())
                .unwrap_or(f64::NAN);
            let content_start = tag_end + 1;
            let Some(content_end) = rest[content_start..].find("</text>") else {
                break;
            };
            let content = &rest[content_start..content_start + content_end];
            // Text may be wrapped in <tspan> elements; the test guides do not.
            let text = content
                .rfind('>')
                .map(|index| content[index + 1..].to_string())
                .unwrap_or_else(|| content.to_string());
            result.push((x, text));
            rest = &rest[content_start + content_end + "</text>".len()..];
        }
        cursor = start + end_relative + "</g>".len();
    }
    result
}

fn group_containing<'a>(svg: &'a str, class_name: &str) -> &'a str {
    let start = svg
        .find(class_name)
        .unwrap_or_else(|| panic!("missing {class_name} in SVG:\n{svg}"));
    let end = svg[start..]
        .find("</g>")
        .unwrap_or_else(|| panic!("unterminated group for {class_name}:\n{svg}"));
    &svg[start..start + end]
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.01,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn linear_y_axis_with_grid_labels_and_title() {
    let spec = json!({
        "width": 300,
        "height": 200,
        "padding": 5,
        "data": [{"name": "t", "values": [
            {"a": "A", "b": 0},
            {"a": "B", "b": 50},
            {"a": "C", "b": 100}
        ]}],
        "scales": [
            {"name": "x", "type": "band",
             "domain": {"data": "t", "field": "a"}, "range": [0, 300]},
            {"name": "y", "type": "linear", "domain": [0, 100], "nice": true,
             "range": [200, 0]}
        ],
        "axes": [
            // The grid-only axis draws horizontal rules.
            {"scale": "y", "orient": "left", "grid": true,
             "labels": false, "ticks": false, "domain": false,
             "tickCount": 2},
            // The regular axis draws the labels and title.
            {"scale": "y", "orient": "left", "grid": false,
             "title": "Value", "tickCount": 2},
            {"scale": "x", "orient": "bottom", "title": "Category"}
        ],
        "marks": [{
            "type": "rect",
            "from": {"data": "t"},
            "encode": {"update": {
                "x": {"scale": "x", "field": "a"},
                "width": {"signal": "bandwidth('x')"},
                "y": {"scale": "y", "field": "b"},
                "y2": {"scale": "y", "value": 0},
                "fill": {"value": "#4c78a8"}
            }}
        }]
    });

    let svg = render_vega(&spec);
    assert!(svg.starts_with("<svg"), "{svg}");
    assert!(svg.ends_with("</svg>"), "{svg}");

    // Grid lines are present.
    assert!(svg.contains("role-axis-grid"), "{svg}");
    let grid = group_containing(&svg, "role-axis-grid");
    assert!(grid.matches("<line").count() >= 3, "grid group:\n{grid}");

    // Tick labels for the nice 0..100 domain and both axis titles.
    assert!(svg.contains(">0</text>"), "{svg}");
    assert!(svg.contains(">50</text>"), "{svg}");
    assert!(svg.contains(">100</text>"), "{svg}");
    assert!(svg.contains(">Value</text>"), "{svg}");
    assert!(svg.contains(">Category</text>"), "{svg}");
}

#[test]
fn band_x_axis_has_one_centered_tick_per_category() {
    let spec = json!({
        "width": 300,
        "height": 120,
        "padding": 0,
        "data": [{"name": "t", "values": [
            {"a": "A", "b": 1},
            {"a": "B", "b": 2},
            {"a": "C", "b": 3}
        ]}],
        "scales": [{
            "name": "x", "type": "band",
            "domain": {"data": "t", "field": "a"},
            "range": [0, 300],
            "paddingInner": 0,
            "paddingOuter": 0
        }],
        "axes": [{"scale": "x", "orient": "bottom", "ticks": true, "labels": true}],
        "marks": [{
            "type": "rect",
            "from": {"data": "t"},
            "encode": {"update": {
                "x": {"scale": "x", "field": "a"},
                "width": {"signal": "bandwidth('x')"},
                "y": {"value": 0},
                "height": {"value": 10},
                "fill": {"value": "#4c78a8"}
            }}
        }]
    });

    let svg = render_vega(&spec);
    let labels = guide_texts(&svg, "role-axis-label");
    assert_eq!(
        labels
            .iter()
            .map(|(_, label)| label.as_str())
            .collect::<Vec<_>>(),
        vec!["A", "B", "C"],
        "{svg}"
    );

    // With a 300px range, centered band ticks apply Vega's `axisBand`
    // tickOffset of -0.5, landing at 49.5, 149.5 and 249.5.
    let positions: Vec<f64> = labels.iter().map(|(x, _)| *x).collect();
    assert_eq!(positions.len(), 3, "{svg}");
    assert_close(positions[0], 49.5);
    assert_close(positions[1], 149.5);
    assert_close(positions[2], 249.5);
}

#[test]
fn time_scale_accepts_iso_dates_and_renders_labels() {
    let spec = json!({
        "width": 300,
        "height": 150,
        "padding": 5,
        "data": [{"name": "t", "values": [
            {"date": "2020-01-01", "v": 1},
            {"date": "2020-02-01", "v": 3},
            {"date": "2020-03-01", "v": 2},
            {"date": "2020-04-01", "v": 5}
        ]}],
        "scales": [
            {"name": "x", "type": "time",
             "domain": {"data": "t", "field": "date"}, "range": [0, 300]},
            {"name": "y", "type": "linear", "domain": [0, 6], "range": [150, 0]}
        ],
        "axes": [{"scale": "x", "orient": "bottom", "tickCount": 4}],
        "marks": [{
            "type": "line",
            "from": {"data": "t"},
            "encode": {"update": {
                "x": {"scale": "x", "field": "date"},
                "y": {"scale": "y", "field": "v"},
                "stroke": {"value": "#4c78a8"},
                "strokeWidth": {"value": 2}
            }}
        }]
    });

    let svg = render_vega(&spec);
    assert!(svg.starts_with("<svg"), "{svg}");
    assert!(svg.ends_with("</svg>"), "{svg}");
    assert!(svg.contains("role-axis-label"), "{svg}");

    let labels = guide_texts(&svg, "role-axis-label");
    assert!(
        labels.len() >= 2,
        "expected time tick labels, got {labels:?}\n{svg}"
    );
    assert!(
        labels.iter().all(|(_, label)| !label.trim().is_empty()),
        "time tick labels must be non-empty: {labels:?}\n{svg}"
    );
}

#[test]
fn ordinal_fill_legend_has_one_symbol_per_domain_value_and_a_title() {
    let spec = json!({
        "width": 150,
        "height": 100,
        "padding": 5,
        "data": [{"name": "t", "values": [
            {"c": "A", "v": 1},
            {"c": "B", "v": 2},
            {"c": "C", "v": 3}
        ]}],
        "scales": [{
            "name": "color", "type": "ordinal",
            "domain": {"data": "t", "field": "c"},
            "range": {"scheme": "tableau10"}
        }],
        "legends": [{"fill": "color", "title": "Category"}],
        "marks": [{
            "type": "rect",
            "from": {"data": "t"},
            "encode": {"update": {
                "x": {"value": 0},
                "width": {"value": 20},
                "y": {"value": 0},
                "height": {"value": 20},
                "fill": {"scale": "color", "field": "c"}
            }}
        }]
    });

    let svg = render_vega(&spec);
    assert!(svg.contains("role-legend"), "{svg}");

    // Only the legend emits symbol entries; the chart marks are rectangles.
    assert_eq!(
        svg.matches("role-legend-symbol").count(),
        3,
        "expected one legend symbol per domain value:\n{svg}"
    );

    let texts = guide_texts(&svg, "role-legend");
    let labels: Vec<&str> = texts.iter().map(|(_, text)| text.as_str()).collect();
    assert!(
        labels.contains(&"Category"),
        "legend title missing: {labels:?}\n{svg}"
    );
    assert!(
        labels.contains(&"A"),
        "legend label A missing: {labels:?}\n{svg}"
    );
    assert!(
        labels.contains(&"B"),
        "legend label B missing: {labels:?}\n{svg}"
    );
    assert!(
        labels.contains(&"C"),
        "legend label C missing: {labels:?}\n{svg}"
    );
}

#[test]
fn continuous_color_scale_uses_a_gradient_legend() {
    let spec = json!({
        "width": 150,
        "height": 100,
        "padding": 5,
        "data": [{"name": "t", "values": [
            {"v": 10},
            {"v": 50},
            {"v": 90}
        ]}],
        "scales": [{
            "name": "color", "type": "linear",
            "domain": [0, 100],
            "range": {"scheme": "blues"}
        }],
        "legends": [{"fill": "color", "title": "Value"}],
        "marks": [{
            "type": "rect",
            "from": {"data": "t"},
            "encode": {"update": {
                "x": {"value": 0},
                "width": {"value": 20},
                "y": {"value": 0},
                "height": {"value": 20},
                "fill": {"scale": "color", "field": "v"}
            }}
        }]
    });

    let svg = render_vega(&spec);
    assert!(svg.contains("<linearGradient"), "{svg}");
    assert!(svg.contains("role-legend"), "{svg}");
    assert!(svg.contains("fill=\"url(#gradient_"), "{svg}");
}

#[test]
fn hconcat_renders_multiple_group_marks() {
    let spec = json!({
        "hconcat": [
            {
                "data": {"values": [{"a": "A", "b": 1}, {"a": "B", "b": 2}]},
                "mark": "bar",
                "encoding": {
                    "x": {"field": "a", "type": "nominal"},
                    "y": {"field": "b", "type": "quantitative"}
                }
            },
            {
                "data": {"values": [{"a": "C", "b": 3}, {"a": "D", "b": 4}]},
                "mark": "bar",
                "encoding": {
                    "x": {"field": "a", "type": "nominal"},
                    "y": {"field": "b", "type": "quantitative"}
                }
            }
        ]
    });

    let svg = render_vega_lite(&spec);
    assert!(
        svg.matches("role-mark").count() >= 2,
        "expected one mark group per concatenated view:\n{svg}"
    );
}

#[test]
fn vega_filter_aggregate_stack_pipeline_produces_expected_bars() {
    let spec = json!({
        "width": 300,
        "height": 200,
        "padding": 5,
        "data": [
            {"name": "raw", "values": [
                {"c": "X", "g": "A", "v": 1},
                {"c": "X", "g": "B", "v": 2},
                {"c": "Y", "g": "A", "v": 3},
                {"c": "Y", "g": "B", "v": 4},
                {"c": "Y", "g": "A", "v": 5}
            ]},
            {"name": "filtered", "source": "raw", "transform": [
                {"type": "filter", "expr": "datum.v > 1"}
            ]},
            {"name": "agg", "source": "filtered", "transform": [
                {"type": "aggregate",
                 "groupby": ["c", "g"],
                 "fields": ["v"],
                 "ops": ["sum"],
                 "as": ["total"]}
            ]},
            {"name": "stacked", "source": "agg", "transform": [
                {"type": "stack",
                 "groupby": ["c"],
                 "field": "total",
                 "sort": {"field": "g"},
                 "as": ["y0", "y1"]}
            ]}
        ],
        "scales": [
            {"name": "x", "type": "band",
             "domain": {"data": "stacked", "field": "c"},
             "range": [0, 300], "paddingInner": 0.1},
            {"name": "y", "type": "linear", "domain": [0, 10], "range": [200, 0]},
            {"name": "color", "type": "ordinal",
             "domain": {"data": "agg", "field": "g"},
             "range": {"scheme": "tableau10"}}
        ],
        "marks": [{
            "type": "rect",
            "from": {"data": "stacked"},
            "encode": {"update": {
                "x": {"scale": "x", "field": "c"},
                "width": {"signal": "bandwidth('x')"},
                "y": {"scale": "y", "field": "y0"},
                "y2": {"scale": "y", "field": "y1"},
                "fill": {"scale": "color", "field": "g"}
            }}
        }]
    });

    let svg = render_vega(&spec);
    assert!(svg.contains("class=\"mark-rect role-mark\""), "{svg}");
    // Filter leaves (X,B)=2, (Y,A)=3+5, (Y,B)=4; aggregation yields three
    // category/group bars, and stacking does not change that count.
    assert_eq!(
        svg.matches("fill=\"#4c78a8\"").count() + svg.matches("fill=\"#f58518\"").count(),
        3,
        "expected exactly three stacked bars:\n{svg}"
    );
}

#[test]
fn categorical_color_scheme_uses_tableau10_palette() {
    let spec = json!({
        "width": 150,
        "height": 100,
        "padding": 5,
        "data": [{"name": "t", "values": [
            {"c": "A", "v": 1},
            {"c": "B", "v": 2}
        ]}],
        "scales": [{
            "name": "color", "type": "ordinal",
            "domain": {"data": "t", "field": "c"},
            "range": {"scheme": "tableau10"}
        }],
        "marks": [{
            "type": "rect",
            "from": {"data": "t"},
            "encode": {"update": {
                "x": {"value": 0},
                "width": {"value": 20},
                "y": {"value": 0},
                "height": {"value": 20},
                "fill": {"scale": "color", "field": "c"}
            }}
        }]
    });

    let svg = vega_to_svg(&spec).expect("render categorical color spec");
    assert!(
        svg.contains("#4c78a8"),
        "tableau10 first color missing:\n{svg}"
    );
    assert!(
        svg.contains("#f58518"),
        "tableau10 second color missing:\n{svg}"
    );
}

#[test]
fn vega_lite_entry_point_renders_a_basic_bar_chart() {
    // A small smoke test for the plain (non-options) Vega-Lite entry point.
    let spec = json!({
        "data": {"values": [{"a": "A", "b": 1}, {"a": "B", "b": 2}]},
        "mark": "bar",
        "encoding": {
            "x": {"field": "a", "type": "nominal"},
            "y": {"field": "b", "type": "quantitative"}
        }
    });

    let svg = vega_lite_to_svg(&spec).expect("render Vega-Lite spec");
    assert!(svg.starts_with("<svg"), "{svg}");
    assert!(svg.contains("role-mark"), "{svg}");
}

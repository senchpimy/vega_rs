//! Regression tests for runtime fixes: marks without `from`, axis title
//! truncation defaults, encoded group clip/corner radius, and rect defaults.

use serde_json::json;
use vega_svg::vega_to_svg;

#[test]
fn renders_single_item_for_mark_without_from() {
    // Selection brush rectangles have no `from` data but still render one
    // zero-size item; the SVG output must keep them.
    let spec = json!({
        "width": 100,
        "height": 100,
        "marks": [{
            "type": "rect",
            "encode": {
                "enter": {"fill": {"value": "#333"}, "fillOpacity": {"value": 0.125}},
                "update": {
                    "x": {"value": 0},
                    "y": {"value": 0},
                    "x2": {"value": 0},
                    "y2": {"value": 0}
                }
            }
        }]
    });
    let svg = vega_to_svg(&spec).unwrap();
    assert!(svg.contains("fill=\"#333\""), "brush fill missing: {svg}");
    assert!(
        svg.contains("fill-opacity=\"0.125\""),
        "brush opacity missing: {svg}"
    );
}

#[test]
fn axis_title_is_not_truncated_without_explicit_limit() {
    // Vega's axis config has no default titleLimit; only legends do.
    let spec = json!({
        "width": 100,
        "height": 60,
        "data": [{"name": "t", "values": [{"a": "A"}, {"a": "B"}]}],
        "scales": [{
            "name": "x",
            "type": "band",
            "domain": {"data": "t", "field": "a"},
            "range": "width"
        }],
        "axes": [{
            "scale": "x",
            "orient": "bottom",
            "title": "precipitation (binned)"
        }]
    });
    let svg = vega_to_svg(&spec).unwrap();
    assert!(
        svg.contains("precipitation (binned)"),
        "axis title was truncated: {svg}"
    );
    assert!(!svg.contains('…'), "unexpected ellipsis: {svg}");
}

#[test]
fn encoded_group_clip_and_corner_radius_are_applied() {
    let spec = json!({
        "width": 100,
        "height": 100,
        "marks": [{
            "type": "group",
            "encode": {
                "update": {
                    "x": {"value": 10},
                    "y": {"value": 10},
                    "width": {"value": 40},
                    "height": {"value": 30},
                    "clip": {"value": true},
                    "cornerRadius": {"value": 5}
                }
            },
            "marks": [{
                "type": "rect",
                "encode": {
                    "update": {
                        "x": {"value": 0},
                        "y": {"value": 0},
                        "width": {"value": 10},
                        "height": {"value": 10}
                    }
                }
            }]
        }]
    });
    let svg = vega_to_svg(&spec).unwrap();
    assert!(svg.contains("clip-path=\"url(#clip1)\""), "clip missing: {svg}");
    assert!(svg.contains("<clipPath id=\"clip1\">"), "clip def missing: {svg}");
    // The rounded clip path is local to the group and starts 5px in.
    assert!(
        svg.contains("<clipPath id=\"clip1\"><path d=\"M5,0L35,0C"),
        "corner radius missing: {svg}"
    );
}

#[test]
fn rect_with_width_but_no_x_starts_at_zero() {
    let spec = json!({
        "width": 100,
        "height": 100,
        "marks": [{
            "type": "rect",
            "encode": {
                "update": {
                    "width": {"value": 20},
                    "height": {"value": 10}
                }
            }
        }]
    });
    let svg = vega_to_svg(&spec).unwrap();
    assert!(svg.contains("d=\"M0,0h20v10h-20Z\""), "unexpected path: {svg}");
}

//! Port of `test/normalize/pathoverlay.test.ts`.

use serde_json::{json, Value};
use vega_lite::normalize::normalize;
use vega_lite::normalize::pathoverlay::has_matching_type;

fn x_date() -> Value {
    json!({"field": "date", "type": "temporal"})
}

fn y_price() -> Value {
    json!({"field": "price", "type": "quantitative"})
}

#[test]
fn correctly_normalizes_line_with_overlayed_point() {
    let spec = json!({
        "data": {"url": "data/stocks.csv"},
        "mark": "line",
        "encoding": {
            "x": {"field": "date", "type": "temporal"},
            "y": {"field": "price", "type": "quantitative"}
        },
        "config": {"line": {"point": {}}}
    });

    assert!(has_matching_type(&spec, spec.get("config").unwrap()));

    assert_eq!(
        normalize(&spec, None).unwrap(),
        json!({
            "data": {"url": "data/stocks.csv"},
            "layer": [
                {
                    "mark": "line",
                    "encoding": {
                        "x": {"field": "date", "type": "temporal"},
                        "y": {"field": "price", "type": "quantitative"}
                    }
                },
                {
                    "mark": {"type": "point", "opacity": 1, "filled": true},
                    "encoding": {
                        "x": {"field": "date", "type": "temporal"},
                        "y": {"field": "price", "type": "quantitative"}
                    }
                }
            ],
            "config": {"line": {"point": {}}}
        })
    );
}

#[test]
fn correctly_normalizes_line_with_transparent_point_overlayed() {
    let spec = json!({
        "data": {"url": "data/stocks.csv"},
        "mark": {"type": "line", "point": "transparent", "tooltip": ""},
        "encoding": {
            "x": {"field": "date", "type": "temporal"},
            "y": {"field": "price", "type": "quantitative"}
        }
    });
    assert_eq!(
        normalize(&spec, None).unwrap(),
        json!({
            "data": {"url": "data/stocks.csv"},
            "layer": [
                {
                    "mark": {"type": "line", "tooltip": ""},
                    "encoding": {
                        "x": {"field": "date", "type": "temporal"},
                        "y": {"field": "price", "type": "quantitative"}
                    }
                },
                {
                    "mark": {"type": "point", "opacity": 0, "filled": true, "tooltip": ""},
                    "encoding": {
                        "x": {"field": "date", "type": "temporal"},
                        "y": {"field": "price", "type": "quantitative"}
                    }
                }
            ]
        })
    );
}

#[test]
fn correctly_normalizes_line_with_point_overlayed_via_mark_definition() {
    let spec = json!({
        "data": {"url": "data/stocks.csv"},
        "mark": {"type": "line", "point": {"color": "red"}},
        "encoding": {
            "x": {"field": "date", "type": "temporal"},
            "y": {"field": "price", "type": "quantitative"}
        }
    });
    assert_eq!(
        normalize(&spec, None).unwrap(),
        json!({
            "data": {"url": "data/stocks.csv"},
            "layer": [
                {
                    "mark": "line",
                    "encoding": {
                        "x": {"field": "date", "type": "temporal"},
                        "y": {"field": "price", "type": "quantitative"}
                    }
                },
                {
                    "mark": {"type": "point", "opacity": 1, "filled": true, "color": "red"},
                    "encoding": {
                        "x": {"field": "date", "type": "temporal"},
                        "y": {"field": "price", "type": "quantitative"}
                    }
                }
            ]
        })
    );
}

#[test]
fn correctly_normalizes_faceted_line_plots_with_overlayed_point() {
    let spec = json!({
        "data": {"url": "data/stocks.csv"},
        "mark": "line",
        "encoding": {
            "row": {"field": "symbol", "type": "nominal"},
            "x": {"field": "date", "type": "temporal"},
            "y": {"field": "price", "type": "quantitative"}
        },
        "config": {"line": {"point": {}}}
    });
    assert_eq!(
        normalize(&spec, None).unwrap(),
        json!({
            "data": {"url": "data/stocks.csv"},
            "facet": {"row": {"field": "symbol", "type": "nominal"}},
            "spec": {
                "layer": [
                    {
                        "mark": "line",
                        "encoding": {
                            "x": {"field": "date", "type": "temporal"},
                            "y": {"field": "price", "type": "quantitative"}
                        }
                    },
                    {
                        "mark": {"type": "point", "opacity": 1, "filled": true},
                        "encoding": {
                            "x": {"field": "date", "type": "temporal"},
                            "y": {"field": "price", "type": "quantitative"}
                        }
                    }
                ]
            },
            "config": {"line": {"point": {}}}
        })
    );
}

#[test]
fn correctly_normalizes_area_with_overlay_line_and_point() {
    let spec = json!({
        "data": {"url": "data/stocks.csv"},
        "mark": "area",
        "encoding": {"x": x_date(), "y": y_price()},
        "config": {"area": {"line": {}, "point": {}}}
    });
    assert_eq!(
        normalize(&spec, None).unwrap(),
        json!({
            "data": {"url": "data/stocks.csv"},
            "layer": [
                {
                    "mark": {"type": "area", "opacity": 0.7},
                    "encoding": {"x": x_date(), "y": y_price()}
                },
                {
                    "mark": {"type": "line"},
                    "encoding": {
                        "x": x_date(),
                        "y": {"field": "price", "type": "quantitative", "stack": "zero"}
                    }
                },
                {
                    "mark": {"type": "point", "opacity": 1, "filled": true},
                    "encoding": {
                        "x": x_date(),
                        "y": {"field": "price", "type": "quantitative", "stack": "zero"}
                    }
                }
            ],
            "config": {"area": {"line": {}, "point": {}}}
        })
    );
}

#[test]
fn correctly_normalizes_area_using_y2_with_overlay_line() {
    let spec = json!({
        "data": {"url": "data/stocks.csv"},
        "mark": "area",
        "encoding": {
            "x": x_date(),
            "y": y_price(),
            "y2": {"value": 0, "type": "quantitative"}
        },
        "config": {"area": {"line": {}, "point": {}}}
    });
    assert_eq!(
        normalize(&spec, None).unwrap(),
        json!({
            "data": {"url": "data/stocks.csv"},
            "layer": [
                {
                    "mark": {"type": "area", "opacity": 0.7},
                    "encoding": {
                        "x": x_date(),
                        "y": y_price(),
                        "y2": {"value": 0, "type": "quantitative"}
                    }
                },
                {
                    "mark": {"type": "line"},
                    "encoding": {
                        "x": x_date(),
                        "y": {"field": "price", "type": "quantitative", "stack": "zero"}
                    }
                },
                {
                    "mark": {"type": "point", "opacity": 1, "filled": true},
                    "encoding": {
                        "x": x_date(),
                        "y": {"field": "price", "type": "quantitative", "stack": "zero"}
                    }
                }
            ],
            "config": {"area": {"line": {}, "point": {}}}
        })
    );
}

#[test]
fn correctly_normalizes_interpolated_area_with_overlay_line() {
    let spec = json!({
        "data": {"url": "data/stocks.csv"},
        "mark": {"type": "area", "interpolate": "monotone"},
        "encoding": {"x": x_date(), "y": y_price()},
        "config": {"area": {"line": {}}}
    });
    assert_eq!(
        normalize(&spec, None).unwrap(),
        json!({
            "data": {"url": "data/stocks.csv"},
            "layer": [
                {
                    "mark": {"type": "area", "opacity": 0.7, "interpolate": "monotone"},
                    "encoding": {"x": x_date(), "y": y_price()}
                },
                {
                    "mark": {"type": "line", "interpolate": "monotone"},
                    "encoding": {
                        "x": x_date(),
                        "y": {"field": "price", "type": "quantitative", "stack": "zero"}
                    }
                }
            ],
            "config": {"area": {"line": {}}}
        })
    );
}

#[test]
fn correctly_normalizes_area_with_overlay_point_and_line_disabled_in_config() {
    for overlay in [Value::Null, Value::Bool(false)] {
        let spec = json!({
            "data": {"url": "data/stocks.csv"},
            "mark": "area",
            "encoding": {"x": x_date(), "y": y_price()},
            "config": {"area": {"point": overlay.clone(), "line": overlay.clone()}}
        });
        assert_eq!(
            normalize(&spec, None).unwrap(),
            json!({
                "data": {"url": "data/stocks.csv"},
                "mark": "area",
                "encoding": {"x": x_date(), "y": y_price()},
                "config": {"area": {"point": overlay.clone(), "line": overlay}}
            })
        );
    }
}

#[test]
fn correctly_normalizes_stacked_area_with_overlay_line() {
    let spec = json!({
        "data": {"url": "data/stocks.csv"},
        "mark": "area",
        "encoding": {
            "x": x_date(),
            "y": {"aggregate": "sum", "field": "price", "type": "quantitative"},
            "color": {"field": "symbol", "type": "nominal"}
        },
        "config": {"area": {"line": {}}}
    });
    assert_eq!(
        normalize(&spec, None).unwrap(),
        json!({
            "data": {"url": "data/stocks.csv"},
            "layer": [
                {
                    "mark": {"type": "area", "opacity": 0.7},
                    "encoding": {
                        "x": x_date(),
                        "y": {"aggregate": "sum", "field": "price", "type": "quantitative"},
                        "color": {"field": "symbol", "type": "nominal"}
                    }
                },
                {
                    "mark": {"type": "line"},
                    "encoding": {
                        "x": x_date(),
                        "y": {"aggregate": "sum", "field": "price", "type": "quantitative", "stack": "zero"},
                        "color": {"field": "symbol", "type": "nominal"}
                    }
                }
            ],
            "config": {"area": {"line": {}}}
        })
    );
}

#[test]
fn correctly_normalizes_default_stacked_area_with_overlay_line() {
    let spec = json!({
        "data": {"url": "data/movies.json"},
        "mark": "area",
        "encoding": {
            "x": {"field": "IMDB Rating", "type": "quantitative"},
            "y": {"field": "Rotten Tomatoes Rating", "type": "quantitative"},
            "color": {"field": "MPAA RATING", "type": "nominal"}
        },
        "config": {"area": {"line": {}}}
    });
    assert_eq!(
        normalize(&spec, None).unwrap(),
        json!({
            "data": {"url": "data/movies.json"},
            "layer": [
                {
                    "mark": {"type": "area", "opacity": 0.7},
                    "encoding": {
                        "x": {"field": "IMDB Rating", "type": "quantitative"},
                        "y": {"field": "Rotten Tomatoes Rating", "type": "quantitative"},
                        "color": {"field": "MPAA RATING", "type": "nominal"}
                    }
                },
                {
                    "mark": {"type": "line"},
                    "encoding": {
                        "x": {"field": "IMDB Rating", "type": "quantitative"},
                        "y": {"field": "Rotten Tomatoes Rating", "type": "quantitative", "stack": "zero"},
                        "color": {"field": "MPAA RATING", "type": "nominal"}
                    }
                }
            ],
            "config": {"area": {"line": {}}}
        })
    );
}

#[test]
fn correctly_normalizes_streamgraph_with_overlay_line() {
    let spec = json!({
        "data": {"url": "data/stocks.csv"},
        "mark": "area",
        "encoding": {
            "x": x_date(),
            "y": {"aggregate": "sum", "field": "price", "type": "quantitative", "stack": "center"},
            "color": {"field": "symbol", "type": "nominal"}
        },
        "config": {"area": {"line": {}}}
    });
    assert_eq!(
        normalize(&spec, None).unwrap(),
        json!({
            "data": {"url": "data/stocks.csv"},
            "layer": [
                {
                    "mark": {"type": "area", "opacity": 0.7},
                    "encoding": {
                        "x": x_date(),
                        "y": {"aggregate": "sum", "field": "price", "type": "quantitative", "stack": "center"},
                        "color": {"field": "symbol", "type": "nominal"}
                    }
                },
                {
                    "mark": {"type": "line"},
                    "encoding": {
                        "x": x_date(),
                        "y": {"aggregate": "sum", "field": "price", "type": "quantitative", "stack": "center"},
                        "color": {"field": "symbol", "type": "nominal"}
                    }
                }
            ],
            "config": {"area": {"line": {}}}
        })
    );
}

#[test]
fn correctly_normalizes_opacity_for_area_with_line() {
    let spec = json!({
        "data": {"url": "data/stocks.csv"},
        "mark": {"type": "area", "line": true, "opacity": 1, "color": "red"},
        "encoding": {"x": x_date(), "y": y_price()}
    });
    assert_eq!(
        normalize(&spec, None).unwrap(),
        json!({
            "data": {"url": "data/stocks.csv"},
            "layer": [
                {
                    "mark": {"type": "area", "opacity": 1, "color": "red"},
                    "encoding": {"x": x_date(), "y": y_price()}
                },
                {
                    "mark": {"type": "line"},
                    "encoding": {
                        "x": x_date(),
                        "y": {"field": "price", "type": "quantitative", "stack": "zero"}
                    }
                }
            ]
        })
    );
}

//! Port of `test/compile/compile.test.ts`.

use serde_json::{json, Value};

use vega_lite::compile::compile::compile as compile_spec;
use vega_lite::log;

fn compile(spec: &Value) -> Value {
    compile_spec(spec)
        .unwrap_or_else(|e| panic!("compile failed: {e}"))
        .spec
}

fn warns<T>(f: impl FnOnce() -> T) -> (T, log::LocalLogger) {
    log::capture(f)
}

#[test]
fn should_throw_error_for_invalid_spec() {
    assert!(compile_spec(&json!({})).is_err());
}

#[test]
fn should_return_a_spec_with_default_top_level_properties_size_signals_data_marks_and_title() {
    let spec = compile(&json!({
        "data": {"values": [{"a": "A", "b": 28}]},
        "title": {"text": "test"},
        "mark": "point",
        "encoding": {}
    }));

    assert_eq!(spec["padding"], json!(5));
    assert!(spec.get("autosize").is_none());
    assert_eq!(spec["width"], json!(20));
    assert_eq!(spec["height"], json!(20));
    assert_eq!(spec["title"], json!({"text": "test", "frame": "group"}));
    assert_eq!(spec["data"].as_array().unwrap().len(), 1);
    assert_eq!(spec["marks"].as_array().unwrap().len(), 1);
}

#[test]
fn should_return_a_spec_with_specified_top_level_properties_size_signals_data_and_marks() {
    let spec = compile(&json!({
        "padding": 123,
        "data": {"values": [{"a": "A", "b": 28}]},
        "mark": "point",
        "encoding": {}
    }));

    assert_eq!(spec["padding"], json!(123));
    assert!(spec.get("autosize").is_none());
    assert_eq!(spec["width"], json!(20));
    assert_eq!(spec["height"], json!(20));
    assert_eq!(spec["data"].as_array().unwrap().len(), 1);
    assert_eq!(spec["marks"].as_array().unwrap().len(), 1);
}

#[test]
fn should_drop_fit_in_top_level_properties_for_discrete_x_discrete_chart() {
    let (spec, logger) = warns(|| {
        compile(&json!({
            "data": {"values": [{"x": "foo", "y": "bar"}]},
            "autosize": "fit",
            "mark": "point",
            "encoding": {
                "x": {"field": "x", "type": "nominal"},
                "y": {"field": "y", "type": "nominal"}
            }
        }))
    });
    assert_eq!(
        logger.warns[0],
        json!(log::message::dropping_fit(None))
    );
    assert!(spec.get("autosize").is_none());
}

#[test]
fn should_drop_fit_y_for_quantitative_x_discrete_y_chart() {
    let (spec, logger) = warns(|| {
        compile(&json!({
            "data": {"values": [{"x": 1, "y": "bar"}]},
            "autosize": "fit",
            "mark": "point",
            "encoding": {
                "x": {"field": "x", "type": "quantitative"},
                "y": {"field": "y", "type": "nominal"}
            }
        }))
    });
    assert_eq!(logger.warns[0], json!(log::message::dropping_fit(Some("y"))));
    assert_eq!(spec["autosize"], json!("fit-x"));
}

#[test]
fn should_drop_fit_x_for_discrete_x_quantitative_y_chart() {
    let (spec, logger) = warns(|| {
        compile(&json!({
            "data": {"values": [{"x": "foo", "y": 1}]},
            "autosize": "fit",
            "mark": "point",
            "encoding": {
                "x": {"field": "x", "type": "nominal"},
                "y": {"field": "y", "type": "quantitative"}
            }
        }))
    });
    assert_eq!(logger.warns[0], json!(log::message::dropping_fit(Some("x"))));
    assert_eq!(spec["autosize"], json!("fit-y"));
}

#[test]
fn should_not_drop_fit_in_top_level_properties_for_specified_width_height_chart() {
    let (spec, logger) = warns(|| {
        compile(&json!({
            "data": {"values": [{"x": "foo", "y": "bar"}]},
            "autosize": "fit",
            "width": 400,
            "height": 400,
            "mark": "point",
            "encoding": {
                "x": {"field": "x", "type": "nominal"},
                "y": {"field": "y", "type": "nominal"}
            }
        }))
    });
    assert_eq!(logger.warns.len(), 0);
    assert_eq!(spec["autosize"], json!("fit"));
}

#[test]
fn should_not_drop_fit_y_in_top_level_properties_for_specified_height_chart() {
    let (spec, logger) = warns(|| {
        compile(&json!({
            "data": {"values": [{"x": 1, "y": "bar"}]},
            "height": 400,
            "autosize": "fit",
            "mark": "point",
            "encoding": {
                "x": {"field": "x", "type": "quantitative"},
                "y": {"field": "y", "type": "nominal"}
            }
        }))
    });
    assert_eq!(logger.warns.len(), 0);
    assert_eq!(spec["autosize"], json!("fit"));
}

#[test]
fn should_not_drop_fit_x_in_top_level_properties_for_specified_width_chart() {
    let (spec, logger) = warns(|| {
        compile(&json!({
            "data": {"values": [{"x": "foo", "y": 1}]},
            "width": 400,
            "autosize": "fit",
            "mark": "point",
            "encoding": {
                "x": {"field": "x", "type": "nominal"},
                "y": {"field": "y", "type": "quantitative"}
            }
        }))
    });
    assert_eq!(logger.warns.len(), 0);
    assert_eq!(spec["autosize"], json!("fit"));
}

#[test]
fn should_use_size_signal_for_bar_chart_width() {
    let spec = compile(&json!({
        "data": {"values": [{"a": "A", "b": 28}]},
        "mark": "bar",
        "encoding": {
            "x": {"field": "a", "type": "ordinal"},
            "y": {"field": "b", "type": "quantitative"}
        }
    }));

    assert_eq!(
        spec["signals"],
        json!([
            {"name": "x_step", "value": 20},
            {"name": "width", "update": "bandspace(domain('x').length, 0.1, 0.05) * x_step"}
        ])
    );
    assert_eq!(spec["height"], json!(300));
}

#[test]
fn should_set_resize_to_true_if_requested() {
    let spec = compile(&json!({
        "autosize": {"resize": true},
        "data": {"url": "foo.csv"},
        "mark": "point",
        "encoding": {}
    }));
    assert_eq!(spec["autosize"]["resize"], json!(true));
}

#[test]
fn should_set_autosize_to_fit_and_containment_if_requested() {
    let spec = compile(&json!({
        "autosize": {"type": "fit", "contains": "content"},
        "data": {"url": "foo.csv"},
        "mark": "point",
        "encoding": {}
    }));
    assert_eq!(spec["autosize"], json!({"type": "fit", "contains": "content"}));
}

#[test]
fn should_set_autosize_to_fit_if_requested() {
    let spec = compile(&json!({
        "autosize": "fit",
        "data": {"url": "foo.csv"},
        "mark": "point",
        "encoding": {}
    }));
    assert_eq!(spec["autosize"], json!("fit"));
}

#[test]
fn should_align_layered_bar_line_point_marks_on_shared_quantitative_y_offset_baseline() {
    let spec = compile(&json!({
        "data": {"values": [{"cat": "A", "v": 10, "g": "x"}, {"cat": "B", "v": 30, "g": "x"}]},
        "layer": [
            {"mark": "bar", "encoding": {
                "x": {"field": "cat", "type": "nominal"},
                "y": {"field": "g", "type": "nominal"},
                "yOffset": {"field": "v", "type": "quantitative"}
            }},
            {"mark": "line", "encoding": {
                "x": {"field": "cat", "type": "nominal"},
                "y": {"field": "g", "type": "nominal"},
                "yOffset": {"field": "v", "type": "quantitative"}
            }},
            {"mark": "point", "encoding": {
                "x": {"field": "cat", "type": "nominal"},
                "y": {"field": "g", "type": "nominal"},
                "yOffset": {"field": "v", "type": "quantitative"}
            }}
        ]
    }));

    let y_offset_scale = spec["scales"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == json!("yOffset"))
        .expect("yOffset scale");
    assert_eq!(y_offset_scale["zero"], json!(true));

    let bar = &spec["marks"][0]["encode"]["update"];
    let line = &spec["marks"][1]["encode"]["update"];
    let point = &spec["marks"][2]["encode"]["update"];

    assert_eq!(
        bar["y"],
        json!({"scale": "y", "field": "g", "offset": {"scale": "yOffset", "field": "v"}})
    );
    assert_eq!(
        bar["y2"],
        json!({"scale": "y", "field": "g", "offset": {"scale": "yOffset", "value": 0}})
    );
    assert_eq!(
        line["y"],
        json!({"scale": "y", "field": "g", "offset": {"scale": "yOffset", "field": "v"}})
    );
    assert_eq!(
        point["y"],
        json!({"scale": "y", "field": "g", "offset": {"scale": "yOffset", "field": "v"}})
    );
}

#[test]
fn should_include_zero_in_the_offset_scale_of_a_ranged_bar_but_not_a_jittered_point() {
    let encoding = json!({
        "x": {"field": "cat", "type": "nominal"},
        "y": {"field": "g", "type": "nominal"},
        "yOffset": {"field": "v", "type": "quantitative"}
    });
    let data = json!({"values": [{"cat": "A", "v": 10, "g": "x"}]});

    let bar_spec = compile(&json!({"data": data, "mark": "bar", "encoding": encoding}));
    assert_eq!(
        bar_spec["scales"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["name"] == json!("yOffset"))
            .unwrap()["zero"],
        json!(true)
    );

    let point_spec = compile(&json!({"data": data, "mark": "point", "encoding": encoding}));
    assert_eq!(
        point_spec["scales"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["name"] == json!("yOffset"))
            .unwrap()["zero"],
        json!(false)
    );
}

#[test]
fn should_group_line_paths_by_an_explicit_detail_field_when_y_offset_is_aggregated() {
    let spec = compile(&json!({
        "data": {"values": [
            {"a": "A", "b": 28, "c": "x", "d": "m"},
            {"a": "B", "b": 55, "c": "x", "d": "m"}
        ]},
        "mark": "line",
        "encoding": {
            "x": {"field": "a"},
            "y": {"field": "c"},
            "yOffset": {"field": "b", "aggregate": "sum", "type": "quantitative"},
            "detail": {"field": "c", "type": "nominal"}
        }
    }));

    assert_eq!(spec["marks"][0]["type"], json!("group"));
    assert_eq!(spec["marks"][0]["from"]["facet"]["groupby"], json!(["c"]));
}

#[test]
fn should_not_default_stack_x_when_only_y_offset_is_present_for_density_like_plots() {
    let data = json!({"values": [
        {"value": 3000, "density": 0.0002},
        {"value": 3500, "density": 0.0005}
    ]});

    for mark in ["bar", "area"] {
        let spec = compile(&json!({
            "data": data,
            "mark": mark,
            "encoding": {
                "x": {"field": "value", "type": "quantitative"},
                "yOffset": {"field": "density", "type": "quantitative"}
            }
        }));

        let update = &spec["marks"][0]["encode"]["update"];
        if mark == "bar" {
            assert_eq!(update["xc"], json!({"scale": "x", "field": "value"}));
            assert!(update.get("x").is_none());
        } else {
            assert_eq!(update["orient"], json!({"value": "vertical"}));
            assert_eq!(update["x"], json!({"scale": "x", "field": "value"}));
        }
        if update.get("x2").and_then(|v| v.get("field")).is_some() {
            assert_ne!(update["x2"]["field"], json!("value_start"));
        }
    }
}

#[test]
fn should_use_container_size_for_width_and_autosize_to_fit_x_padding() {
    let spec = compile(&json!({
        "width": "container",
        "data": {"url": "foo.csv"},
        "mark": "point",
        "encoding": {}
    }));

    assert_eq!(spec["autosize"], json!({"type": "fit-x", "contains": "padding"}));
    assert_eq!(
        spec["signals"],
        json!([{
            "name": "width",
            "init": "isFinite(containerSize()[0]) ? containerSize()[0] : 300",
            "on": [
                {"events": "window:resize", "update": "isFinite(containerSize()[0]) ? containerSize()[0] : 300"},
                {"events": "container:resize", "update": "isFinite(containerSize()[0]) ? containerSize()[0] : 300"}
            ]
        }])
    );
    assert!(spec.get("width").is_none());
}

#[test]
fn should_compile_area_with_discrete_y_quantitative_y_offset_and_detail_as_faceted_ranged_paths() {
    let spec = compile(&json!({
        "data": {"values": [
            {"a": "A", "b": 28, "c": "x"},
            {"a": "B", "b": 55, "c": "x"}
        ]},
        "mark": "area",
        "encoding": {
            "x": {"field": "a"},
            "y": {"field": "c"},
            "yOffset": {"field": "b", "type": "quantitative"},
            "detail": {"field": "c", "type": "nominal"}
        }
    }));

    assert_eq!(spec["marks"][0]["type"], json!("group"));
    assert_eq!(spec["marks"][0]["from"]["facet"]["groupby"], json!(["c"]));

    let update = &spec["marks"][0]["marks"][0]["encode"]["update"];
    assert_eq!(
        update["y"],
        json!({"scale": "y", "field": "c", "offset": {"scale": "yOffset", "field": "b"}})
    );
    assert_eq!(
        update["y2"],
        json!({"scale": "y", "field": "c", "offset": {"scale": "yOffset", "value": 0}})
    );
}

#[test]
fn should_compile_area_with_aggregated_y_offset_and_no_y_as_ranged_geometry() {
    let spec = compile(&json!({
        "data": {"values": [
            {"a": "A", "b": 28, "c": "x", "d": "m"},
            {"a": "B", "b": 55, "c": "x", "d": "m"}
        ]},
        "mark": "area",
        "encoding": {
            "x": {"field": "a"},
            "yOffset": {"field": "b", "aggregate": "sum", "type": "quantitative"}
        }
    }));

    let update = &spec["marks"][0]["encode"]["update"];
    assert_eq!(
        update["y"],
        json!({"value": 0, "offset": {"scale": "yOffset", "field": "sum_b"}})
    );
    assert_eq!(update["y2"], json!({"field": {"group": "height"}}));
}

#[test]
fn should_use_container_size_for_width_and_autosize_to_fit_y_padding() {
    let spec = compile(&json!({
        "height": "container",
        "data": {"url": "foo.csv"},
        "mark": "point",
        "encoding": {}
    }));

    assert_eq!(spec["autosize"], json!({"type": "fit-y", "contains": "padding"}));
    assert_eq!(
        spec["signals"],
        json!([{
            "name": "height",
            "init": "isFinite(containerSize()[1]) ? containerSize()[1] : 300",
            "on": [
                {"events": "window:resize", "update": "isFinite(containerSize()[1]) ? containerSize()[1] : 300"},
                {"events": "container:resize", "update": "isFinite(containerSize()[1]) ? containerSize()[1] : 300"}
            ]
        }])
    );
    assert!(spec.get("height").is_none());
}

#[test]
fn should_use_container_size_for_width_height_and_respect_default_view_size() {
    let spec = compile(&json!({
        "width": "container",
        "height": "container",
        "data": {"url": "foo.csv"},
        "mark": "point",
        "encoding": {},
        "config": {"view": {"continuousWidth": 500, "continuousHeight": 300}}
    }));

    assert_eq!(spec["autosize"], json!({"type": "fit", "contains": "padding"}));
    assert_eq!(
        spec["signals"],
        json!([
            {
                "name": "width",
                "init": "isFinite(containerSize()[0]) ? containerSize()[0] : 500",
                "on": [
                    {"events": "window:resize", "update": "isFinite(containerSize()[0]) ? containerSize()[0] : 500"},
                    {"events": "container:resize", "update": "isFinite(containerSize()[0]) ? containerSize()[0] : 500"}
                ]
            },
            {
                "name": "height",
                "init": "isFinite(containerSize()[1]) ? containerSize()[1] : 300",
                "on": [
                    {"events": "window:resize", "update": "isFinite(containerSize()[1]) ? containerSize()[1] : 300"},
                    {"events": "container:resize", "update": "isFinite(containerSize()[1]) ? containerSize()[1] : 300"}
                ]
            }
        ])
    );
    assert!(spec.get("width").is_none());
    assert!(spec.get("height").is_none());
}

#[test]
fn should_warn_if_use_container_for_width_and_pad_for_autosize() {
    let (_, logger) = warns(|| {
        compile(&json!({
            "width": "container",
            "height": "container",
            "autosize": "pad",
            "data": {"url": "foo.csv"},
            "mark": "point",
            "encoding": {}
        }));
    });
    assert_eq!(
        logger.warns[0],
        json!(log::message::container_size_not_compatible_with_autosize("width"))
    );
    assert_eq!(
        logger.warns[1],
        json!(log::message::container_size_not_compatible_with_autosize("height"))
    );
}

#[test]
fn should_warn_if_use_container_for_width_for_composed_spec() {
    let (spec, logger) = warns(|| {
        compile(&json!({
            "width": "container",
            "vconcat": [{"mark": "point", "encoding": {}}]
        }))
    });
    assert_eq!(
        logger.warns[0],
        json!(log::message::container_size_non_single("width"))
    );
    assert!(spec.get("autosize").is_none());
}

#[test]
fn should_warn_if_use_container_for_height_for_composed_spec() {
    let (spec, logger) = warns(|| {
        compile(&json!({
            "height": "container",
            "vconcat": [{"mark": "point", "encoding": {}}]
        }))
    });
    assert_eq!(
        logger.warns[0],
        json!(log::message::container_size_non_single("height"))
    );
    assert!(spec.get("autosize").is_none());
}

#[test]
fn should_warn_if_trying_to_fit_composed_spec() {
    let (spec, logger) = warns(|| {
        compile(&json!({
            "data": {"values": [{"a": "A", "b": 28}]},
            "autosize": "fit",
            "vconcat": [{"mark": "point", "encoding": {}}]
        }))
    });
    assert_eq!(logger.warns[0], json!(log::message::FIT_NON_SINGLE));
    assert_eq!(spec["autosize"], json!("fit"));
}

#[test]
fn should_return_title_for_a_layered_spec() {
    let spec = compile(&json!({
        "data": {"values": [{"a": "A", "b": 28}]},
        "title": {"text": "test"},
        "layer": [{"mark": "point", "encoding": {}}]
    }));
    assert_eq!(spec["title"], json!({"text": "test", "frame": "group"}));
}

#[test]
fn should_return_title_string_for_a_layered_spec() {
    let spec = compile(&json!({
        "data": {"values": [{"a": "A", "b": 28}]},
        "title": "test",
        "layer": [{"mark": "point", "encoding": {}}]
    }));
    assert_eq!(spec["title"], json!({"text": "test", "frame": "group"}));
}

#[test]
fn should_return_title_from_a_child_of_a_layer_spec_if_parent_has_no_title() {
    let spec = compile(&json!({
        "data": {"values": [{"a": "A", "b": 28}]},
        "layer": [{"title": {"text": "test"}, "mark": "point", "encoding": {}}]
    }));
    assert_eq!(spec["title"], json!({"text": "test", "frame": "group"}));
}

#[test]
fn should_return_a_title_for_a_concat_spec() {
    let spec = compile(&json!({
        "data": {"values": [{"a": "A", "b": 28}]},
        "title": {"text": "test"},
        "hconcat": [{"mark": "point", "encoding": {}}],
        "config": {"title": {"anchor": "middle"}}
    }));
    assert_eq!(spec["title"], json!({"text": "test", "anchor": "middle"}));
}

#[test]
fn should_return_a_title_for_a_concat_spec_and_set_anchor_to_start() {
    let spec = compile(&json!({
        "data": {"values": [{"a": "A", "b": 28}]},
        "title": {"text": "test"},
        "hconcat": [{"mark": "point", "encoding": {}}],
        "config": {"title": {"offset": 5}}
    }));
    assert_eq!(
        spec["title"],
        json!({"text": "test", "anchor": "start", "offset": 5})
    );
}

#[test]
fn should_not_have_title_if_there_is_no_title() {
    let spec = compile(&json!({
        "data": {"values": [{"a": "A", "b": 28}]},
        "hconcat": [{"mark": "point", "encoding": {}}],
        "config": {"title": {"offset": 5}}
    }));
    assert!(spec.get("title").is_none());
}

#[test]
fn should_use_provided_config() {
    let spec = compile(&json!({
        "mark": "point",
        "data": {"url": "foo.csv"},
        "encoding": {},
        "config": {"background": "blue"}
    }));
    assert_eq!(spec["background"], json!("blue"));
}

#[test]
fn should_apply_expr_in_top_level_property() {
    let spec = compile(&json!({
        "background": {"expr": "'red"},
        "mark": "point",
        "data": {"url": "foo.csv"},
        "encoding": {}
    }));
    assert_eq!(spec["background"], json!({"signal": "'red"}));
}

#[test]
fn should_merge_spec_and_provided_config() {
    // `compile(opt.config)` is not part of the Rust API; the spec config wins
    // and is the behaviour validated here.
    let spec = compile(&json!({
        "mark": "point",
        "data": {"url": "foo.csv"},
        "encoding": {},
        "config": {"background": "red"}
    }));
    assert_eq!(spec["background"], json!("red"));
}

#[test]
fn should_return_a_spec_with_implicit_projections() {
    let spec = compile(&json!({
        "mark": "geoshape",
        "data": {"url": "data/us-10m.json", "format": {"type": "topojson", "feature": "states"}},
        "encoding": {}
    }));
    assert!(spec.get("projections").is_some());
}

#[test]
fn should_return_a_spec_with_explicit_projections() {
    let spec = compile(&json!({
        "mark": "geoshape",
        "projection": {"type": "albersUsa"},
        "data": {"url": "data/us-10m.json", "format": {"type": "topojson", "feature": "states"}},
        "encoding": {}
    }));
    assert!(spec.get("projections").is_some());
}

#[test]
fn should_set_resize_true_for_spec_with_axis_orient_signal() {
    let spec = compile(&json!({
        "data": {"values": [{"a": "A", "b": 28}]},
        "mark": "point",
        "encoding": {
            "x": {"field": "a", "type": "nominal", "axis": {"orient": {"signal": "\"top\""}}},
            "y": {"field": "b", "type": "quantitative", "axis": {"orient": {"signal": "\"right\""}}}
        }
    }));
    assert_eq!(spec["autosize"]["resize"], json!(true));
}

// Composite `errorband` normalization is now ported, so this upstream case
// compiles end to end.
#[test]
fn should_generate_right_tooltip() {
    let spec = compile(&json!({
        "data": {"url": "data/population.json"},
        "mark": "errorband",
        "encoding": {
            "x": {"field": "age", "type": "ordinal"},
            "y": {"field": "people", "type": "quantitative"},
            "tooltip": {"field": "people", "aggregate": "mean"}
        }
    }));
    assert_eq!(
        spec["data"][0]["transform"][0]["as"],
        json!(["mean_people", "center_people", "extent_people"])
    );
}

#[test]
fn should_generate_right_cursor_for_point_selection() {
    let spec = compile(&json!({
        "data": {"url": "data/population.json"},
        "params": [{"name": "select", "select": "point"}],
        "mark": "bar",
        "encoding": {
            "x": {"field": "a", "type": "ordinal"},
            "y": {"field": "b", "type": "quantitative"}
        }
    }));
    assert_eq!(spec["marks"][0]["encode"]["update"]["cursor"], json!({"value": "pointer"}));
}

#[test]
fn should_not_generate_cursor_for_point_selection_with_binding() {
    let spec = compile(&json!({
        "data": {"url": "data/population.json"},
        "params": [{
            "name": "highlight",
            "value": [{"Cylinders": 4, "Year": 1977}],
            "select": {"type": "point", "fields": ["Cylinders", "Year"]},
            "bind": {
                "Cylinders": {"input": "range", "min": 3, "max": 8, "step": 1},
                "Year": {"input": "range", "min": 1969, "max": 1981, "step": 1}
            }
        }],
        "mark": "bar",
        "encoding": {
            "x": {"field": "a", "type": "ordinal"},
            "y": {"field": "b", "type": "quantitative"}
        }
    }));
    assert!(spec["marks"][0]["encode"]["update"].get("cursor").is_none());
}

#[test]
fn should_not_generate_cursor_for_interval_selection() {
    let spec = compile(&json!({
        "data": {"url": "data/population.json"},
        "params": [{"name": "brush", "select": {"type": "interval", "encodings": ["x"]}}],
        "mark": "bar",
        "encoding": {
            "x": {"field": "a", "type": "ordinal"},
            "y": {"field": "b", "type": "quantitative"}
        }
    }));
    assert!(spec["marks"][0]["encode"]["update"].get("cursor").is_none());
}

#[test]
fn drops_x_offset_for_binned_x_and_compiles_without_an_x_offset_scale() {
    let (spec, logger) = warns(|| {
        compile(&json!({
            "data": {"values": [{"a": 1, "b": "P"}]},
            "mark": "bar",
            "encoding": {
                "x": {"field": "a", "type": "quantitative", "bin": true},
                "y": {"aggregate": "count", "type": "quantitative"},
                "xOffset": {"field": "b", "type": "nominal"}
            }
        }))
    });
    assert!(logger.warns.iter().any(|w| w
        == &json!(log::message::offset_nested_inside_continuous_position_scale_dropped("x"))));
    let names: Vec<Value> = spec["scales"]
        .as_array()
        .map(|a| a.iter().map(|s| s["name"].clone()).collect())
        .unwrap_or_default();
    assert!(!names.contains(&json!("xOffset")));
}

#[test]
fn drops_x_offset_for_pre_binned_x_and_compiles_without_an_x_offset_scale() {
    let (spec, logger) = warns(|| {
        compile(&json!({
            "data": {"values": [{"a": 1, "a_end": 2, "b": "P"}]},
            "mark": "bar",
            "encoding": {
                "x": {"field": "a", "bin": "binned", "type": "quantitative"},
                "x2": {"field": "a_end"},
                "y": {"aggregate": "count", "type": "quantitative"},
                "xOffset": {"field": "b", "type": "nominal"}
            }
        }))
    });
    assert!(logger.warns.iter().any(|w| w
        == &json!(log::message::offset_nested_inside_continuous_position_scale_dropped("x"))));
    let names: Vec<Value> = spec["scales"]
        .as_array()
        .map(|a| a.iter().map(|s| s["name"].clone()).collect())
        .unwrap_or_default();
    assert!(!names.contains(&json!("xOffset")));
}

#[test]
fn drops_y_offset_for_binned_y_and_compiles_without_a_y_offset_scale() {
    let (spec, logger) = warns(|| {
        compile(&json!({
            "data": {"values": [{"a": 1, "b": "P"}]},
            "mark": "bar",
            "encoding": {
                "y": {"field": "a", "type": "quantitative", "bin": true},
                "x": {"aggregate": "count", "type": "quantitative"},
                "yOffset": {"field": "b", "type": "nominal"}
            }
        }))
    });
    assert!(logger.warns.iter().any(|w| w
        == &json!(log::message::offset_nested_inside_continuous_position_scale_dropped("y"))));
    let names: Vec<Value> = spec["scales"]
        .as_array()
        .map(|a| a.iter().map(|s| s["name"].clone()).collect())
        .unwrap_or_default();
    assert!(!names.contains(&json!("yOffset")));
}

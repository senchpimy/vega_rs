//! Port of `test/compile/selection/interval.test.ts`.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use vega_lite::compile::model::Model;
use vega_lite::compile::selection::assemble::assemble_unit_selection_signals;
use vega_lite::compile::selection::component::SelectionComponent;
use vega_lite::compile::selection::interval::{self, GEO_INIT_TICK};
use vega_lite::compile::selection::parse::parse_unit_selection;
use vega_lite::compile::selection::selector::parse_selector;
use vega_lite::compile::support::init_config;

fn parse_unit_model(spec: &Value) -> Model {
    let config = init_config(spec.get("config"));
    Model::new_unit(spec, None, "", None, &config)
}

fn parse_unit_model_with_scale(spec: &Value) -> Model {
    let mut model = parse_unit_model(spec);
    model.parse_scale();
    model
}

fn set_selections(model: &mut Model, defs: &[Value]) -> BTreeMap<String, SelectionComponent> {
    let components = parse_unit_selection(model, defs);
    model.component.selection = serde_json::to_value(&components).unwrap();
    components
}

fn scaled_model() -> (Model, BTreeMap<String, SelectionComponent>) {
    let mut model = parse_unit_model_with_scale(&json!({
        "mark": "circle",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Miles-per-Gallon", "type": "quantitative"},
            "color": {"field": "Origin", "type": "nominal"}
        }
    }));

    let defs = vec![
        json!({"name": "one", "select": {"type": "interval", "encodings": ["x"], "clear": false, "translate": false, "zoom": false}}),
        json!({"name": "two", "select": {"type": "interval", "encodings": ["y"], "clear": false, "translate": false, "zoom": false}, "bind": "scales"}),
        json!({"name": "thr-ee", "select": {
            "type": "interval",
            "on": "[pointerdown, pointerup] > pointermove, [keydown, keyup] > keypress",
            "clear": false, "translate": false, "zoom": false, "resolve": "intersect",
            "mark": {"fill": "red", "fillOpacity": 0.75, "stroke": "black", "strokeWidth": 4,
                     "strokeDash": [10, 5], "strokeDashOffset": 3, "strokeOpacity": 0.25}
        }}),
        json!({"name": "four", "value": {"x": [50, 70]}, "select": {"type": "interval", "encodings": ["x"], "clear": false, "translate": false, "zoom": false}}),
        json!({"name": "five", "value": {"x": [50, 60], "y": [23, 54]}, "select": {"type": "interval", "clear": false, "translate": false, "zoom": false}}),
        json!({"name": "six", "value": {"x": [{"year": 2000, "month": 10, "date": 5}, {"year": 2001, "month": 1, "date": 13}]},
               "select": {"type": "interval", "clear": false, "translate": false, "zoom": false, "encodings": ["x"]}}),
    ];

    let components = set_selections(&mut model, &defs);
    (model, components)
}

fn evt(selector: &str) -> Value {
    parse_selector(selector, "scope")[0].clone()
}

#[test]
fn builds_projection_signals() {
    let (model, sel_cmpts) = scaled_model();

    let one_sg = interval::signals(&model, &sel_cmpts["one"], vec![]);
    for expected in [
        json!({
            "name": "one_x",
            "value": [],
            "on": [
                {"events": evt("pointerdown"), "update": "[x(unit), x(unit)]"},
                {"events": evt("[pointerdown, window:pointerup] > window:pointermove!"), "update": "[one_x[0], clamp(x(unit), 0, width)]"},
                {"events": {"signal": "one_scale_trigger"}, "update": "[scale(\"x\", one_Horsepower[0]), scale(\"x\", one_Horsepower[1])]"},
            ],
        }),
        json!({
            "name": "one_Horsepower",
            "on": [{"events": {"signal": "one_x"}, "update": "one_x[0] === one_x[1] ? null : invert(\"x\", one_x)"}],
        }),
        json!({
            "name": "one_scale_trigger",
            "value": {},
            "on": [{
                "events": [{"scale": "x"}],
                "update": "(!isArray(one_Horsepower) || (+invert(\"x\", one_x)[0] === +one_Horsepower[0] && +invert(\"x\", one_x)[1] === +one_Horsepower[1])) ? one_scale_trigger : {}",
            }],
        }),
    ] {
        assert!(
            one_sg.contains(&expected),
            "missing {expected}\n{one_sg:#?}"
        );
    }

    let two_sg = interval::signals(&model, &sel_cmpts["two"], vec![]);
    assert!(two_sg.contains(&json!({"name": "two_Miles_per_Gallon", "on": []})));

    let three_sg = interval::signals(&model, &sel_cmpts["thr_ee"], vec![]);
    for expected in [
        json!({
            "name": "thr_ee_x",
            "value": [],
            "on": [
                {"events": evt("pointerdown"), "update": "[x(unit), x(unit)]"},
                {"events": evt("[pointerdown, pointerup] > pointermove"), "update": "[thr_ee_x[0], clamp(x(unit), 0, width)]"},
                {"events": evt("keydown"), "update": "[x(unit), x(unit)]"},
                {"events": evt("[keydown, keyup] > keypress"), "update": "[thr_ee_x[0], clamp(x(unit), 0, width)]"},
                {"events": {"signal": "thr_ee_scale_trigger"}, "update": "[scale(\"x\", thr_ee_Horsepower[0]), scale(\"x\", thr_ee_Horsepower[1])]"},
            ],
        }),
        json!({
            "name": "thr_ee_Horsepower",
            "on": [{"events": {"signal": "thr_ee_x"}, "update": "thr_ee_x[0] === thr_ee_x[1] ? null : invert(\"x\", thr_ee_x)"}],
        }),
        json!({
            "name": "thr_ee_y",
            "value": [],
            "on": [
                {"events": evt("pointerdown"), "update": "[y(unit), y(unit)]"},
                {"events": evt("[pointerdown, pointerup] > pointermove"), "update": "[thr_ee_y[0], clamp(y(unit), 0, height)]"},
                {"events": evt("keydown"), "update": "[y(unit), y(unit)]"},
                {"events": evt("[keydown, keyup] > keypress"), "update": "[thr_ee_y[0], clamp(y(unit), 0, height)]"},
                {"events": {"signal": "thr_ee_scale_trigger"}, "update": "[scale(\"y\", thr_ee_Miles_per_Gallon[0]), scale(\"y\", thr_ee_Miles_per_Gallon[1])]"},
            ],
        }),
        json!({
            "name": "thr_ee_Miles_per_Gallon",
            "on": [{"events": {"signal": "thr_ee_y"}, "update": "thr_ee_y[0] === thr_ee_y[1] ? null : invert(\"y\", thr_ee_y)"}],
        }),
        json!({
            "name": "thr_ee_scale_trigger",
            "value": {},
            "on": [{
                "events": [{"scale": "x"}, {"scale": "y"}],
                "update": "(!isArray(thr_ee_Horsepower) || (+invert(\"x\", thr_ee_x)[0] === +thr_ee_Horsepower[0] && +invert(\"x\", thr_ee_x)[1] === +thr_ee_Horsepower[1])) && (!isArray(thr_ee_Miles_per_Gallon) || (+invert(\"y\", thr_ee_y)[0] === +thr_ee_Miles_per_Gallon[0] && +invert(\"y\", thr_ee_y)[1] === +thr_ee_Miles_per_Gallon[1])) ? thr_ee_scale_trigger : {}",
            }],
        }),
    ] {
        assert!(
            three_sg.contains(&expected),
            "missing {expected}\n{three_sg:#?}"
        );
    }

    let four_sg = interval::signals(&model, &sel_cmpts["four"], vec![]);
    for expected in [
        json!({
            "name": "four_x",
            "init": "[scale(\"x\", 50), scale(\"x\", 70)]",
            "on": [
                {"events": evt("pointerdown"), "update": "[x(unit), x(unit)]"},
                {"events": evt("[pointerdown, window:pointerup] > window:pointermove!"), "update": "[four_x[0], clamp(x(unit), 0, width)]"},
                {"events": {"signal": "four_scale_trigger"}, "update": "[scale(\"x\", four_Horsepower[0]), scale(\"x\", four_Horsepower[1])]"},
            ],
        }),
        json!({
            "name": "four_Horsepower",
            "init": "[50, 70]",
            "on": [{"events": {"signal": "four_x"}, "update": "four_x[0] === four_x[1] ? null : invert(\"x\", four_x)"}],
        }),
    ] {
        assert!(
            four_sg.contains(&expected),
            "missing {expected}\n{four_sg:#?}"
        );
    }

    let five_sg = interval::signals(&model, &sel_cmpts["five"], vec![]);
    for expected in [
        json!({
            "name": "five_x",
            "init": "[scale(\"x\", 50), scale(\"x\", 60)]",
            "on": [
                {"events": evt("pointerdown"), "update": "[x(unit), x(unit)]"},
                {"events": evt("[pointerdown, window:pointerup] > window:pointermove!"), "update": "[five_x[0], clamp(x(unit), 0, width)]"},
                {"events": {"signal": "five_scale_trigger"}, "update": "[scale(\"x\", five_Horsepower[0]), scale(\"x\", five_Horsepower[1])]"},
            ],
        }),
        json!({
            "name": "five_Horsepower",
            "init": "[50, 60]",
            "on": [{"events": {"signal": "five_x"}, "update": "five_x[0] === five_x[1] ? null : invert(\"x\", five_x)"}],
        }),
        json!({
            "name": "five_y",
            "init": "[scale(\"y\", 23), scale(\"y\", 54)]",
            "on": [
                {"events": evt("pointerdown"), "update": "[y(unit), y(unit)]"},
                {"events": evt("[pointerdown, window:pointerup] > window:pointermove!"), "update": "[five_y[0], clamp(y(unit), 0, height)]"},
                {"events": {"signal": "five_scale_trigger"}, "update": "[scale(\"y\", five_Miles_per_Gallon[0]), scale(\"y\", five_Miles_per_Gallon[1])]"},
            ],
        }),
        json!({
            "name": "five_Miles_per_Gallon",
            "init": "[23, 54]",
            "on": [{"events": {"signal": "five_y"}, "update": "five_y[0] === five_y[1] ? null : invert(\"y\", five_y)"}],
        }),
    ] {
        assert!(
            five_sg.contains(&expected),
            "missing {expected}\n{five_sg:#?}"
        );
    }

    let six_sg = interval::signals(&model, &sel_cmpts["six"], vec![]);
    for expected in [
        json!({
            "name": "six_x",
            "init": "[scale(\"x\", datetime(2000, 9, 5, 0, 0, 0, 0)), scale(\"x\", datetime(2001, 0, 13, 0, 0, 0, 0))]",
            "on": [
                {"events": evt("pointerdown"), "update": "[x(unit), x(unit)]"},
                {"events": evt("[pointerdown, window:pointerup] > window:pointermove!"), "update": "[six_x[0], clamp(x(unit), 0, width)]"},
                {"events": {"signal": "six_scale_trigger"}, "update": "[scale(\"x\", six_Horsepower[0]), scale(\"x\", six_Horsepower[1])]"},
            ],
        }),
        json!({
            "name": "six_Horsepower",
            "init": "[datetime(2000, 9, 5, 0, 0, 0, 0), datetime(2001, 0, 13, 0, 0, 0, 0)]",
            "on": [{"events": {"signal": "six_x"}, "update": "six_x[0] === six_x[1] ? null : invert(\"x\", six_x)"}],
        }),
    ] {
        assert!(
            six_sg.contains(&expected),
            "missing {expected}\n{six_sg:#?}"
        );
    }
}

#[test]
fn builds_trigger_signals() {
    let (model, sel_cmpts) = scaled_model();

    assert!(interval::signals(&model, &sel_cmpts["one"], vec![]).contains(&json!({
        "name": "one_tuple",
        "on": [{"events": [{"signal": "one_Horsepower"}], "update": "one_Horsepower ? {unit: \"\", fields: one_tuple_fields, values: [one_Horsepower]} : null"}],
    })));
    assert!(interval::signals(&model, &sel_cmpts["two"], vec![]).contains(&json!({
        "name": "two_tuple",
        "on": [{"events": [{"signal": "two_Miles_per_Gallon"}], "update": "two_Miles_per_Gallon ? {unit: \"\", fields: two_tuple_fields, values: [two_Miles_per_Gallon]} : null"}],
    })));
    assert!(interval::signals(&model, &sel_cmpts["thr_ee"], vec![]).contains(&json!({
        "name": "thr_ee_tuple",
        "on": [{"events": [{"signal": "thr_ee_Horsepower || thr_ee_Miles_per_Gallon"}], "update": "thr_ee_Horsepower && thr_ee_Miles_per_Gallon ? {unit: \"\", fields: thr_ee_tuple_fields, values: [thr_ee_Horsepower,thr_ee_Miles_per_Gallon]} : null"}],
    })));
    assert!(interval::signals(&model, &sel_cmpts["four"], vec![]).contains(&json!({
        "name": "four_tuple",
        "init": "{unit: \"\", fields: four_tuple_fields, values: [[50, 70]]}",
        "on": [{"events": [{"signal": "four_Horsepower"}], "update": "four_Horsepower ? {unit: \"\", fields: four_tuple_fields, values: [four_Horsepower]} : null"}],
    })));
    assert!(interval::signals(&model, &sel_cmpts["five"], vec![]).contains(&json!({
        "name": "five_tuple",
        "init": "{unit: \"\", fields: five_tuple_fields, values: [[50, 60], [23, 54]]}",
        "on": [{"events": [{"signal": "five_Horsepower || five_Miles_per_Gallon"}], "update": "five_Horsepower && five_Miles_per_Gallon ? {unit: \"\", fields: five_tuple_fields, values: [five_Horsepower,five_Miles_per_Gallon]} : null"}],
    })));
}

#[test]
fn namespaces_signals_when_encoding_fields_collide() {
    let mut model2 = parse_unit_model(&json!({
        "mark": "circle",
        "encoding": {
            "x": {"field": "x", "type": "quantitative"},
            "y": {"field": "y", "type": "quantitative"}
        }
    }));
    model2.parse_scale();

    let sel_cmpts2 = set_selections(
        &mut model2,
        &[
            json!({"name": "one", "select": {"type": "interval", "encodings": ["x"], "translate": false, "zoom": false}}),
        ],
    );

    let sg = interval::signals(&model2, &sel_cmpts2["one"], vec![]);
    assert_eq!(sg[0]["name"], json!("one_x_1"));
    assert_eq!(sg[1]["name"], json!("one_x"));
}

#[test]
fn builds_interval_modify_signals() {
    let (model, _) = scaled_model();
    let signals = assemble_unit_selection_signals(&model, vec![]);
    for expected in [
        json!({"name": "one_modify", "on": [{"events": {"signal": "one_tuple"}, "update": "modify(\"one_store\", one_tuple, true)"}]}),
        json!({"name": "two_modify", "on": [{"events": {"signal": "two_tuple"}, "update": "modify(\"two_store\", two_tuple, true)"}]}),
        json!({"name": "thr_ee_modify", "on": [{"events": {"signal": "thr_ee_tuple"}, "update": "modify(\"thr_ee_store\", thr_ee_tuple, {unit: \"\"})"}]}),
    ] {
        assert!(signals.contains(&expected), "missing {expected}");
    }
}

#[test]
fn builds_brush_mark() {
    let (model, sel_cmpts) = scaled_model();
    let marks: Vec<Value> = vec![json!({"hello": "world"})];

    let one = interval::marks(&model, &sel_cmpts["one"], marks.clone());
    assert_eq!(
        one,
        vec![
            json!({
                "name": "one_brush_bg",
                "type": "rect",
                "clip": true,
                "encode": {
                    "enter": {"fill": {"value": "#333"}, "fillOpacity": {"value": 0.125}},
                    "update": {
                        "x": [{"test": "data(\"one_store\").length && data(\"one_store\")[0].unit === \"\"", "signal": "one_x[0]"}, {"value": 0}],
                        "y": [{"test": "data(\"one_store\").length && data(\"one_store\")[0].unit === \"\"", "value": 0}, {"value": 0}],
                        "x2": [{"test": "data(\"one_store\").length && data(\"one_store\")[0].unit === \"\"", "signal": "one_x[1]"}, {"value": 0}],
                        "y2": [{"test": "data(\"one_store\").length && data(\"one_store\")[0].unit === \"\"", "field": {"group": "height"}}, {"value": 0}],
                    },
                },
            }),
            json!({"hello": "world"}),
            json!({
                "name": "one_brush",
                "type": "rect",
                "clip": true,
                "encode": {
                    "enter": {"fill": {"value": "transparent"}},
                    "update": {
                        "stroke": [{"test": "one_x[0] !== one_x[1]", "value": "white"}, {"value": null}],
                        "x": [{"test": "data(\"one_store\").length && data(\"one_store\")[0].unit === \"\"", "signal": "one_x[0]"}, {"value": 0}],
                        "y": [{"test": "data(\"one_store\").length && data(\"one_store\")[0].unit === \"\"", "value": 0}, {"value": 0}],
                        "x2": [{"test": "data(\"one_store\").length && data(\"one_store\")[0].unit === \"\"", "signal": "one_x[1]"}, {"value": 0}],
                        "y2": [{"test": "data(\"one_store\").length && data(\"one_store\")[0].unit === \"\"", "field": {"group": "height"}}, {"value": 0}],
                    },
                },
            }),
        ]
    );

    // Scale-bound interval selections should not add a brush mark.
    assert_eq!(
        interval::marks(&model, &sel_cmpts["two"], marks.clone()),
        marks
    );

    let three = interval::marks(&model, &sel_cmpts["thr_ee"], marks.clone());
    assert_eq!(
        three,
        vec![
            json!({
                "name": "thr_ee_brush_bg",
                "type": "rect",
                "clip": true,
                "encode": {
                    "enter": {"fill": {"value": "red"}, "fillOpacity": {"value": 0.75}},
                    "update": {
                        "x": {"signal": "thr_ee_x[0]"},
                        "y": {"signal": "thr_ee_y[0]"},
                        "x2": {"signal": "thr_ee_x[1]"},
                        "y2": {"signal": "thr_ee_y[1]"},
                    },
                },
            }),
            json!({"hello": "world"}),
            json!({
                "name": "thr_ee_brush",
                "type": "rect",
                "clip": true,
                "encode": {
                    "enter": {"fill": {"value": "transparent"}},
                    "update": {
                        "stroke": [{"test": "thr_ee_x[0] !== thr_ee_x[1] && thr_ee_y[0] !== thr_ee_y[1]", "value": "black"}, {"value": null}],
                        "strokeWidth": [{"test": "thr_ee_x[0] !== thr_ee_x[1] && thr_ee_y[0] !== thr_ee_y[1]", "value": 4}, {"value": null}],
                        "strokeDash": [{"test": "thr_ee_x[0] !== thr_ee_x[1] && thr_ee_y[0] !== thr_ee_y[1]", "value": [10, 5]}, {"value": null}],
                        "strokeDashOffset": [{"test": "thr_ee_x[0] !== thr_ee_x[1] && thr_ee_y[0] !== thr_ee_y[1]", "value": 3}, {"value": null}],
                        "strokeOpacity": [{"test": "thr_ee_x[0] !== thr_ee_x[1] && thr_ee_y[0] !== thr_ee_y[1]", "value": 0.25}, {"value": null}],
                        "x": {"signal": "thr_ee_x[0]"},
                        "y": {"signal": "thr_ee_y[0]"},
                        "x2": {"signal": "thr_ee_x[1]"},
                        "y2": {"signal": "thr_ee_y[1]"},
                    },
                },
            }),
        ]
    );
}

#[test]
fn is_robust_to_same_channel_field_names() {
    let mut name_model = parse_unit_model(&json!({
        "mark": "circle",
        "encoding": {
            "x": {"field": "x", "type": "quantitative"},
            "y": {"field": "y", "type": "quantitative"}
        }
    }));
    name_model.parse_scale();

    let name_sel_cmpts = set_selections(
        &mut name_model,
        &[json!({"name": "brush", "select": "interval"})],
    );

    let signals = interval::signals(&name_model, &name_sel_cmpts["brush"], vec![]);
    let names: Vec<&str> = signals
        .iter()
        .filter_map(|s| s.get("name").and_then(Value::as_str))
        .collect();
    for expected in ["brush_x_1", "brush_x", "brush_y_1", "brush_y"] {
        assert!(names.contains(&expected), "missing {expected} in {names:?}");
    }

    let brush_marks: Vec<Value> = vec![json!({"hello": "world"})];
    let marks = interval::marks(&name_model, &name_sel_cmpts["brush"], brush_marks);
    assert_eq!(
        marks,
        vec![
            json!({
                "name": "brush_brush_bg",
                "type": "rect",
                "clip": true,
                "encode": {
                    "enter": {"fill": {"value": "#333"}, "fillOpacity": {"value": 0.125}},
                    "update": {
                        "x": [{"test": "data(\"brush_store\").length && data(\"brush_store\")[0].unit === \"\"", "signal": "brush_x_1[0]"}, {"value": 0}],
                        "y": [{"test": "data(\"brush_store\").length && data(\"brush_store\")[0].unit === \"\"", "signal": "brush_y_1[0]"}, {"value": 0}],
                        "x2": [{"test": "data(\"brush_store\").length && data(\"brush_store\")[0].unit === \"\"", "signal": "brush_x_1[1]"}, {"value": 0}],
                        "y2": [{"test": "data(\"brush_store\").length && data(\"brush_store\")[0].unit === \"\"", "signal": "brush_y_1[1]"}, {"value": 0}],
                    },
                },
            }),
            json!({"hello": "world"}),
            json!({
                "name": "brush_brush",
                "type": "rect",
                "clip": true,
                "encode": {
                    "enter": {"cursor": {"value": "move"}, "fill": {"value": "transparent"}},
                    "update": {
                        "x": [{"test": "data(\"brush_store\").length && data(\"brush_store\")[0].unit === \"\"", "signal": "brush_x_1[0]"}, {"value": 0}],
                        "y": [{"test": "data(\"brush_store\").length && data(\"brush_store\")[0].unit === \"\"", "signal": "brush_y_1[0]"}, {"value": 0}],
                        "x2": [{"test": "data(\"brush_store\").length && data(\"brush_store\")[0].unit === \"\"", "signal": "brush_x_1[1]"}, {"value": 0}],
                        "y2": [{"test": "data(\"brush_store\").length && data(\"brush_store\")[0].unit === \"\"", "signal": "brush_y_1[1]"}, {"value": 0}],
                        "stroke": [{"test": "brush_x_1[0] !== brush_x_1[1] && brush_y_1[0] !== brush_y_1[1]", "value": "white"}, {"value": null}],
                    },
                },
            }),
        ]
    );
}

fn brush_sel_cmpts(model: &mut Model) -> BTreeMap<String, SelectionComponent> {
    set_selections(
        model,
        &[
            json!({"name": "crosshair", "select": {"type": "interval", "clear": false, "translate": true, "zoom": false, "mark": {"cursor": "crosshair"}}}),
            json!({"name": "disabled", "select": {"type": "interval", "clear": false, "translate": false, "zoom": false}}),
        ],
    )
}

#[test]
fn should_not_override_manual_cursor_assignment() {
    let mut name_model = parse_unit_model(&json!({
        "mark": "circle",
        "encoding": {
            "x": {"field": "x", "type": "quantitative"},
            "y": {"field": "y", "type": "quantitative"}
        }
    }));
    name_model.parse_scale();
    let sel_cmpts = brush_sel_cmpts(&mut name_model);

    assert_eq!(
        interval::marks(&name_model, &sel_cmpts["crosshair"], vec![])[1]["encode"]["enter"]
            ["cursor"],
        json!({"value": "crosshair"})
    );
}

#[test]
fn should_not_change_brush_cursor_when_translate_false() {
    let mut name_model = parse_unit_model(&json!({
        "mark": "circle",
        "encoding": {
            "x": {"field": "x", "type": "quantitative"},
            "y": {"field": "y", "type": "quantitative"}
        }
    }));
    name_model.parse_scale();
    let sel_cmpts = brush_sel_cmpts(&mut name_model);

    assert_eq!(
        interval::marks(&name_model, &sel_cmpts["disabled"], vec![])[1]["encode"]["enter"],
        json!({"fill": {"value": "transparent"}})
    );
}

fn geo_model() -> (Model, BTreeMap<String, SelectionComponent>) {
    let mut model = parse_unit_model_with_scale(&json!({
        "data": {"url": "data/airports.csv", "format": {"type": "csv"}},
        "mark": "circle",
        "projection": {"type": "albersUsa"},
        "encoding": {
            "longitude": {"field": "longitude", "type": "quantitative"},
            "latitude": {"field": "latitude", "type": "quantitative"}
        }
    }));
    model.parse_projection();

    let defs = vec![
        json!({"name": "one", "select": {"type": "interval", "clear": false, "translate": false, "zoom": false}}),
        json!({"name": "two", "select": {"type": "interval", "encodings": ["longitude"], "clear": false, "translate": false, "zoom": false}}),
        json!({"name": "three", "select": {"type": "interval", "clear": false, "translate": false, "zoom": false},
               "value": {"latitude": [30, 40], "longitude": [-86, -118]}}),
        json!({"name": "four", "select": {"type": "interval", "clear": false, "translate": false, "zoom": false},
               "value": {"latitude": [30, 40]}}),
    ];
    let components = set_selections(&mut model, &defs);
    (model, components)
}

#[test]
fn builds_geo_projection_signals() {
    let (model, sel_cmpts) = geo_model();

    let one_sg = interval::signals(&model, &sel_cmpts["one"], vec![]);
    for expected in [
        json!({
            "name": "one_latitude_1",
            "value": [],
            "on": [
                {"events": evt("pointerdown"), "update": "[y(unit), y(unit)]"},
                {"events": evt("[pointerdown, window:pointerup] > window:pointermove!"), "update": "[one_latitude_1[0], clamp(y(unit), 0, height)]"},
            ],
        }),
        json!({
            "name": "one_longitude_1",
            "value": [],
            "on": [
                {"events": evt("pointerdown"), "update": "[x(unit), x(unit)]"},
                {"events": evt("[pointerdown, window:pointerup] > window:pointermove!"), "update": "[one_longitude_1[0], clamp(x(unit), 0, width)]"},
            ],
        }),
    ] {
        assert!(
            one_sg.contains(&expected),
            "missing {expected}\n{one_sg:#?}"
        );
    }

    let two_sg = interval::signals(&model, &sel_cmpts["two"], vec![]);
    assert!(two_sg.contains(&json!({
        "name": "two_longitude_1",
        "value": [],
        "on": [
            {"events": evt("pointerdown"), "update": "[x(unit), x(unit)]"},
            {"events": evt("[pointerdown, window:pointerup] > window:pointermove!"), "update": "[two_longitude_1[0], clamp(x(unit), 0, width)]"},
        ],
    })));

    let three_sg = interval::signals(&model, &sel_cmpts["three"], vec![]);
    for expected in [
        json!({"name": "three_init", "init": "[scale(\"projection\", [-86, 30]), scale(\"projection\", [-118, 40])]"}),
        json!({
            "name": "three_latitude_1",
            "init": "[three_init[0][1], three_init[1][1]]",
            "on": [
                {"events": evt("pointerdown"), "update": "[y(unit), y(unit)]"},
                {"events": evt("[pointerdown, window:pointerup] > window:pointermove!"), "update": "[three_latitude_1[0], clamp(y(unit), 0, height)]"},
            ],
        }),
        json!({
            "name": "three_longitude_1",
            "init": "[three_init[0][0], three_init[1][0]]",
            "on": [
                {"events": evt("pointerdown"), "update": "[x(unit), x(unit)]"},
                {"events": evt("[pointerdown, window:pointerup] > window:pointermove!"), "update": "[three_longitude_1[0], clamp(x(unit), 0, width)]"},
            ],
        }),
    ] {
        assert!(
            three_sg.contains(&expected),
            "missing {expected}\n{three_sg:#?}"
        );
    }

    let four_sg = interval::signals(&model, &sel_cmpts["four"], vec![]);
    for expected in [
        json!({"name": "projection_center", "update": "invert(\"projection\", [width/2, height/2])"}),
        json!({"name": "four_init", "init": "[scale(\"projection\", [projection_center[0], 30]), scale(\"projection\", [projection_center[0], 40])]"}),
        json!({
            "name": "four_latitude_1",
            "init": "[four_init[0][1], four_init[1][1]]",
            "on": [
                {"events": evt("pointerdown"), "update": "[y(unit), y(unit)]"},
                {"events": evt("[pointerdown, window:pointerup] > window:pointermove!"), "update": "[four_latitude_1[0], clamp(y(unit), 0, height)]"},
            ],
        }),
    ] {
        assert!(
            four_sg.contains(&expected),
            "missing {expected}\n{four_sg:#?}"
        );
    }
}

#[test]
fn builds_geo_trigger_signals() {
    let (model, sel_cmpts) = geo_model();

    assert!(interval::signals(&model, &sel_cmpts["one"], vec![]).contains(&json!({
        "name": "one_tuple",
        "on": [{
            "events": [{"signal": "one_longitude_1 || one_latitude_1"}],
            "update": "vlSelectionTuples(intersect([[one_longitude_1[0], one_latitude_1[0]],[one_longitude_1[1], one_latitude_1[1]]], {markname: \"marks\"}, unit.mark), {unit: \"\"})",
        }],
    })));

    assert!(interval::signals(&model, &sel_cmpts["two"], vec![]).contains(&json!({
        "name": "two_tuple",
        "on": [{
            "events": [{"signal": "two_longitude_1"}],
            "update": "vlSelectionTuples(intersect([[two_longitude_1[0], 0],[two_longitude_1[1], height]], {markname: \"marks\"}, unit.mark), {unit: \"\"})",
        }],
    })));

    let three_sg = interval::signals(&model, &sel_cmpts["three"], vec![]);
    assert!(three_sg.contains(&json!({
        "name": "three_tuple",
        "on": [{
            "events": [{"signal": "three_latitude_1 || three_longitude_1"}, {"signal": GEO_INIT_TICK}],
            "update": "vlSelectionTuples(intersect([[three_longitude_1[0], three_latitude_1[0]],[three_longitude_1[1], three_latitude_1[1]]], {markname: \"marks\"}, unit.mark), {unit: \"\"})",
        }],
    })));

    let four_sg = interval::signals(&model, &sel_cmpts["four"], vec![]);
    assert!(four_sg.contains(&json!({
        "name": "four_tuple",
        "on": [{
            "events": [{"signal": "four_latitude_1"}, {"signal": GEO_INIT_TICK}],
            "update": "vlSelectionTuples(intersect([[0, four_latitude_1[0]],[width, four_latitude_1[1]]], {markname: \"marks\"}, unit.mark), {unit: \"\"})",
        }],
    })));
}

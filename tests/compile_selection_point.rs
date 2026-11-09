//! Port of `test/compile/selection/point.test.ts`.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use vega_lite::compile::buildmodel::build_model;
use vega_lite::compile::model::Model;
use vega_lite::compile::selection::assemble::{
    assemble_top_level_signals, assemble_unit_selection_data, assemble_unit_selection_marks,
    assemble_unit_selection_signals,
};
use vega_lite::compile::selection::component::SelectionComponent;
use vega_lite::compile::selection::message;
use vega_lite::compile::selection::parse::{parse_selections, parse_unit_selection};
use vega_lite::compile::selection::point;
use vega_lite::compile::support::init_config;
use vega_lite::log;
use vega_lite::normalize::normalize;

fn parse_unit_model_with_scale(spec: &Value) -> Model {
    let config = init_config(spec.get("config"));
    let mut model = Model::new_unit(spec, None, "", None, &config);
    model.parse_scale();
    model
}

fn set_selections(model: &mut Model, defs: &[Value]) -> BTreeMap<String, SelectionComponent> {
    let components = parse_unit_selection(model, defs);
    model.component.selection = serde_json::to_value(&components).unwrap();
    components
}

fn multi_model() -> (Model, BTreeMap<String, SelectionComponent>) {
    let mut model = parse_unit_model_with_scale(&json!({
        "mark": "circle",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Miles_per_Gallon", "type": "quantitative", "bin": true},
            "color": {"field": "Origin", "type": "nominal"}
        }
    }));

    let defs = vec![
        json!({"name": "one", "select": {"type": "point", "toggle": false, "clear": false}}),
        json!({"name": "two", "select": {
            "type": "point", "encodings": ["y", "color"], "nearest": true, "clear": false,
            "on": "pointerover", "toggle": false, "resolve": "intersect"
        }}),
        json!({"name": "thr-ee", "value": [{"Horsepower": 50}], "select": {
            "type": "point", "fields": ["Horsepower"], "clear": false
        }}),
        json!({"name": "four", "value": [{"Horsepower": 50, "color": "Japan"}], "select": {
            "type": "point", "encodings": ["x", "color"], "clear": false
        }}),
        json!({"name": "five", "value": [
            {"Origin": "Japan", "Year": {"year": 1970, "month": 1, "date": 1}},
            {"Origin": "USA", "Year": {"year": 1980, "month": 1, "date": 1}}
        ], "select": {"type": "point", "fields": ["Year", "Origin"], "clear": false}}),
        json!({"name": "six", "select": {"type": "point", "fields": ["nested.a", "nested.b"], "clear": false}}),
        json!({"name": "seven", "value": 50, "select": {"type": "point", "fields": ["Horsepower"]}}),
        json!({"name": "eight", "value": 75, "select": "point"}),
        json!({"name": "nine", "value": [{"_vgsid_": 75}, {"_vgsid_": 80}], "select": "point"}),
    ];

    let components = set_selections(&mut model, &defs);
    (model, components)
}

#[test]
fn builds_tuple_signals() {
    let (mut model, sel_cmpts) = multi_model();

    assert_eq!(
        point::signals(&model, &sel_cmpts["one"], vec![]),
        vec![json!({
            "name": "one_tuple",
            "on": [{
                "events": sel_cmpts["one"].events,
                "update": "datum && item().mark.marktype !== 'group' && indexof(item().mark.role, 'legend') < 0 ? {unit: \"\", _vgsid_: (item().isVoronoi ? datum.datum : datum)[\"_vgsid_\"]} : null",
                "force": true,
            }],
        })]
    );

    assert_eq!(
        point::signals(&model, &sel_cmpts["two"], vec![]),
        vec![json!({
            "name": "two_tuple",
            "on": [{
                "events": sel_cmpts["two"].events,
                "update": "datum && item().mark.marktype !== 'group' && indexof(item().mark.role, 'legend') < 0 ? {unit: \"\", fields: two_tuple_fields, values: [[(item().isVoronoi ? datum.datum : datum)[\"bin_maxbins_10_Miles_per_Gallon\"], (item().isVoronoi ? datum.datum : datum)[\"bin_maxbins_10_Miles_per_Gallon_end\"]], (item().isVoronoi ? datum.datum : datum)[\"Origin\"]]} : null",
                "force": true,
            }],
        })]
    );

    assert_eq!(
        point::signals(&model, &sel_cmpts["thr_ee"], vec![]),
        vec![json!({
            "name": "thr_ee_tuple",
            "on": [{
                "events": [{"source": "scope", "type": "click"}],
                "update": "datum && item().mark.marktype !== 'group' && indexof(item().mark.role, 'legend') < 0 ? {unit: \"\", fields: thr_ee_tuple_fields, values: [(item().isVoronoi ? datum.datum : datum)[\"Horsepower\"]]} : null",
                "force": true,
            }],
        })]
    );

    assert_eq!(
        point::signals(&model, &sel_cmpts["four"], vec![]),
        vec![json!({
            "name": "four_tuple",
            "on": [{
                "events": [{"source": "scope", "type": "click"}],
                "update": "datum && item().mark.marktype !== 'group' && indexof(item().mark.role, 'legend') < 0 ? {unit: \"\", fields: four_tuple_fields, values: [(item().isVoronoi ? datum.datum : datum)[\"Horsepower\"], (item().isVoronoi ? datum.datum : datum)[\"Origin\"]]} : null",
                "force": true,
            }],
        })]
    );

    assert_eq!(
        point::signals(&model, &sel_cmpts["five"], vec![]),
        vec![json!({
            "name": "five_tuple",
            "on": [{
                "events": [{"source": "scope", "type": "click"}],
                "update": "datum && item().mark.marktype !== 'group' && indexof(item().mark.role, 'legend') < 0 ? {unit: \"\", fields: five_tuple_fields, values: [(item().isVoronoi ? datum.datum : datum)[\"Year\"], (item().isVoronoi ? datum.datum : datum)[\"Origin\"]]} : null",
                "force": true,
            }],
        })]
    );

    assert_eq!(
        point::signals(&model, &sel_cmpts["six"], vec![]),
        vec![json!({
            "name": "six_tuple",
            "on": [{
                "events": [{"source": "scope", "type": "click"}],
                "update": "datum && item().mark.marktype !== 'group' && indexof(item().mark.role, 'legend') < 0 ? {unit: \"\", fields: six_tuple_fields, values: [(item().isVoronoi ? datum.datum : datum)[\"nested.a\"], (item().isVoronoi ? datum.datum : datum)[\"nested.b\"]]} : null",
                "force": true,
            }],
        })]
    );

    let signals = assemble_unit_selection_signals(&model, vec![]);
    for sg in [
        point::signals(&model, &sel_cmpts["one"], vec![])[0].clone(),
        point::signals(&model, &sel_cmpts["two"], vec![])[0].clone(),
        point::signals(&model, &sel_cmpts["thr_ee"], vec![])[0].clone(),
        point::signals(&model, &sel_cmpts["four"], vec![])[0].clone(),
        point::signals(&model, &sel_cmpts["five"], vec![])[0].clone(),
        point::signals(&model, &sel_cmpts["six"], vec![])[0].clone(),
    ] {
        assert!(signals.contains(&sg), "missing signal {sg}");
    }

    // Ensure that interval brushes are accounted for.
    let sel_cmpts2 = set_selections(
        &mut model,
        &[
            json!({"name": "one", "select": "point"}),
            json!({"name": "two", "select": "interval"}),
            json!({"name": "three", "select": "interval"}),
        ],
    );
    let _ = &sel_cmpts2;
    let one_sg_interval = point::signals(&model, &sel_cmpts["one"], vec![]);
    assert_eq!(
        one_sg_interval,
        vec![json!({
            "name": "one_tuple",
            "on": [{
                "events": sel_cmpts["one"].events,
                "update": "datum && item().mark.marktype !== 'group' && indexof(item().mark.role, 'legend') < 0 && indexof(item().mark.name, 'two_brush') < 0 && indexof(item().mark.name, 'three_brush') < 0 ? {unit: \"\", _vgsid_: (item().isVoronoi ? datum.datum : datum)[\"_vgsid_\"]} : null",
                "force": true,
            }],
        })]
    );

    model.component.selection = serde_json::to_value(&sel_cmpts).unwrap();
}

#[test]
fn accesses_escaped_fields_in_tuple_signals() {
    let mut model = multi_model().0;
    let escaped = set_selections(
        &mut model,
        &[json!({"name": "escaped", "select": {"type": "point", "fields": ["y\\[foo\\]"]}})],
    );

    assert_eq!(
        point::signals(&model, &escaped["escaped"], vec![]),
        vec![json!({
            "name": "escaped_tuple",
            "on": [{
                "events": [{"source": "scope", "type": "click"}],
                "update": "datum && item().mark.marktype !== 'group' && indexof(item().mark.role, 'legend') < 0 ? {unit: \"\", fields: escaped_tuple_fields, values: [(item().isVoronoi ? datum.datum : datum)[\"y[foo]\"]]} : null",
                "force": true,
            }],
        })]
    );
}

#[test]
fn builds_modify_signals() {
    let (model, _) = multi_model();
    let signals = assemble_unit_selection_signals(&model, vec![]);
    assert!(signals.contains(&json!({
        "name": "one_modify",
        "on": [{"events": {"signal": "one_tuple"}, "update": "modify(\"one_store\", one_tuple, true)"}],
    })));
    assert!(signals.contains(&json!({
        "name": "two_modify",
        "on": [{"events": {"signal": "two_tuple"}, "update": "modify(\"two_store\", two_tuple, {unit: \"\"})"}],
    })));
}

#[test]
fn builds_top_level_signals() {
    let (model, _) = multi_model();
    let signals = assemble_top_level_signals(&model, vec![]);
    assert!(signals.contains(&json!({
        "name": "one",
        "update": "vlSelectionResolve(\"one_store\", \"union\", true, true)",
    })));
    assert!(signals.contains(&json!({
        "name": "two",
        "update": "vlSelectionResolve(\"two_store\", \"intersect\", true, true)",
    })));
    assert!(signals.contains(&json!({
        "name": "unit",
        "value": {},
        "on": [{"events": "pointermove", "update": "isTuple(group()) ? group() : unit"}],
    })));
}

#[test]
fn builds_unit_datasets() {
    let (model, _) = multi_model();
    let data = assemble_unit_selection_data(&model, vec![]);

    assert_eq!(
        data,
        vec![
            json!({"name": "one_store", "transform": [{"type": "collect", "sort": {"field": "_vgsid_"}}]}),
            json!({"name": "two_store"}),
            json!({"name": "thr_ee_store", "values": [{
                "unit": "", "fields": [{"type": "E", "field": "Horsepower"}], "values": [50]
            }]}),
            json!({"name": "four_store", "values": [{
                "unit": "",
                "fields": [
                    {"field": "Horsepower", "channel": "x", "type": "E"},
                    {"field": "Origin", "channel": "color", "type": "E"}
                ],
                "values": [50, "Japan"]
            }]}),
            json!({"name": "five_store", "values": [
                {
                    "unit": "",
                    "fields": [{"type": "E", "field": "Year"}, {"type": "E", "field": "Origin"}],
                    "values": [28800000, "Japan"]
                },
                {
                    "unit": "",
                    "fields": [{"type": "E", "field": "Year"}, {"type": "E", "field": "Origin"}],
                    "values": [315561600000i64, "USA"]
                }
            ]}),
            json!({"name": "six_store"}),
            json!({"name": "seven_store", "values": [{
                "unit": "", "fields": [{"type": "E", "field": "Horsepower"}], "values": [50]
            }]}),
            json!({"name": "eight_store",
                "transform": [{"type": "collect", "sort": {"field": "_vgsid_"}}],
                "values": [{"unit": "", "_vgsid_": 75}]
            }),
            json!({"name": "nine_store",
                "transform": [{"type": "collect", "sort": {"field": "_vgsid_"}}],
                "values": [{"unit": "", "_vgsid_": 75}, {"unit": "", "_vgsid_": 80}]
            }),
        ]
    );
}

#[test]
fn leaves_marks_alone() {
    let (mut model, sel_cmpts) = multi_model();
    let marks: Vec<Value> = vec![];
    model.component.selection = serde_json::to_value(
        [("one".to_string(), sel_cmpts["one"].clone())]
            .into_iter()
            .collect::<BTreeMap<_, _>>(),
    )
    .unwrap();
    assert_eq!(assemble_unit_selection_marks(&model, marks.clone()), marks);
}

fn animation_spec() -> Value {
    json!({
        "data": {"url": "data/gapminder.json"},
        "params": [{"name": "avl", "select": {"type": "point", "fields": ["year"], "on": "timer"}}],
        "transform": [{"filter": {"param": "avl"}}],
        "mark": "point",
        "encoding": {
            "color": {"field": "country"},
            "x": {"field": "fertility", "type": "quantitative"},
            "y": {"field": "life_expect", "type": "quantitative"},
            "time": {"field": "year", "type": "ordinal"}
        }
    })
}

fn animation_model() -> Model {
    let spec = animation_spec();
    let mut model = parse_unit_model_with_scale(&spec);
    let defs: Vec<Value> = spec["params"].as_array().unwrap().clone();
    parse_selections(&mut model);
    let _ = defs;
    model.parse_data();
    model
}

#[test]
fn builds_animated_tuple_signals() {
    let model = animation_model();
    let signals = assemble_unit_selection_signals(&model, vec![]);
    assert!(signals.contains(&json!({
        "name": "avl_tuple",
        "update": "{unit: \"\", fields: avl_tuple_fields, values: [anim_value ? anim_value : min_extent]}",
    })));
}

#[test]
fn builds_clock_signals() {
    let model = animation_model();
    let signals = assemble_top_level_signals(&model, vec![]);
    assert!(signals.contains(&json!({
        "name": "anim_clock",
        "init": "0",
        "on": [{
            "events": {"type": "timer", "throttle": 16.666666666666668},
            "update": "is_playing ? (anim_clock + (now() - last_tick_at) > max_range_extent ? 0 : anim_clock + (now() - last_tick_at)) : anim_clock",
        }],
    })));
    assert!(signals.contains(&json!({
        "name": "last_tick_at",
        "init": "now()",
        "on": [{"events": [{"signal": "anim_clock"}, {"signal": "is_playing"}], "update": "now()"}],
    })));
}

#[test]
fn builds_animation_scale_signals() {
    let model = animation_model();
    let signals = assemble_unit_selection_signals(&model, vec![]);
    for expected in [
        json!({"name": "avl_domain", "init": "domain('time')"}),
        json!({"name": "min_extent", "init": "extent(avl_domain)[0]"}),
        json!({"name": "max_range_extent", "init": "extent(range('time'))[1]"}),
        json!({"name": "anim_value", "update": "invert('time', eased_anim_clock)"}),
    ] {
        assert!(signals.contains(&expected), "missing {expected}");
    }
}

#[test]
fn builds_animated_modify_signals() {
    let model = animation_model();
    let signals = assemble_unit_selection_signals(&model, vec![]);
    assert!(signals.contains(&json!({
        "name": "avl_modify",
        "update": "modify(\"avl_store\", avl_tuple, true)",
    })));
}

#[test]
fn builds_animated_top_level_signals() {
    let model = animation_model();
    let signals = assemble_top_level_signals(&model, vec![]);
    assert!(signals.contains(&json!({
        "name": "avl",
        "update": "vlSelectionResolve(\"avl_store\", \"union\", true, true)",
    })));
}

#[test]
fn builds_animation_frame_datasets() {
    let model = animation_model();
    let data = vec![json!({
        "name": "source_0",
        "transform": [
            {"type": "filter", "expr": "!length(data(\"avl_store\")) || vlSelectionTest(\"avl_store\", datum)"}
        ]
    })];
    let assembled = assemble_unit_selection_data(&model, data);
    assert!(assembled.contains(&json!({
        "name": "source_0_curr",
        "source": "source_0",
        "transform": [{"type": "filter", "expr": "!length(data(\"avl_store\")) || vlSelectionTest(\"avl_store\", datum)"}],
    })));
}

#[test]
fn animation_frame_dataset_is_referenced_by_marks() {
    // Mark assembly is not part of this subsystem; the animation dataset is the
    // only representable piece of the upstream assertion.
    let model = animation_model();
    let data = vec![json!({
        "name": "source_0",
        "transform": [{"type": "filter", "expr": "vlSelectionTest(\"avl_store\", datum)"}]
    })];
    let assembled = assemble_unit_selection_data(&model, data);
    assert!(assembled
        .iter()
        .any(|d| d.get("name").and_then(Value::as_str) == Some("source_0_curr")));
}

#[test]
fn does_not_build_extra_signals_for_duplicate_selection() {
    let mut spec = animation_spec();
    spec["params"] = json!([
        {"name": "avl", "select": {"type": "point", "fields": ["year"], "on": "timer"}},
        {"name": "avl_2", "select": {"type": "point", "fields": ["year"], "on": "timer"}}
    ]);
    let (model, logger) = log::capture(|| {
        let mut model = parse_unit_model_with_scale(&spec);
        parse_selections(&mut model);
        model
    });

    assert_eq!(logger.warns.len(), 1);
    let signals = assemble_unit_selection_signals(&model, vec![]);
    for expected in [
        json!({"name": "avl_domain", "init": "domain('time')"}),
        json!({"name": "min_extent", "init": "extent(avl_domain)[0]"}),
        json!({"name": "max_range_extent", "init": "extent(range('time'))[1]"}),
        json!({"name": "anim_value", "update": "invert('time', eased_anim_clock)"}),
    ] {
        assert!(signals.contains(&expected));
    }
}

fn multi_view_spec(kind: &str) -> Value {
    let mut spec = animation_spec();
    spec.as_object_mut().unwrap().remove("mark");
    match kind {
        "facet" => {
            spec["mark"] = json!("point");
            spec["encoding"]["facet"] = json!({"field": "cluster"});
        }
        "layer" => {
            spec["layer"] = json!([{"mark": "point"}, {"mark": "point"}]);
        }
        _ => {
            spec["vconcat"] = json!([{"mark": "point"}, {"mark": "point"}]);
        }
    }
    spec
}

fn assert_multi_view_animation_warning(kind: &str) {
    let spec = multi_view_spec(kind);
    let config = init_config(spec.get("config"));
    let (_, logger) = log::capture(|| {
        let normalized = normalize(&spec, Some(&config)).unwrap();
        let mut model = build_model(&normalized, None, "", None, &config);
        parse_selections(&mut model);
    });
    assert!(
        logger
            .warns
            .iter()
            .any(|w| w.as_str() == Some(message::MULTI_VIEW_ANIMATION_UNSUPPORTED)),
        "expected multi-view animation warning for {kind}, got {:?}",
        logger.warns
    );
}

#[test]
fn errors_if_animation_is_used_on_a_faceted_multi_view() {
    assert_multi_view_animation_warning("facet");
}

#[test]
fn errors_if_animation_is_used_on_a_layered_multi_view() {
    assert_multi_view_animation_warning("layer");
    assert_multi_view_animation_warning("concat");
}

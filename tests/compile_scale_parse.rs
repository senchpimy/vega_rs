//! Port of `test/compile/scale/parse.test.ts`.

use serde_json::{json, Value};

use vega_lite::compile::buildmodel::parse_model;
use vega_lite::compile::model::Model;
use vega_lite::compile::scale::parse::{parse_scale_core, parse_scales};
use vega_lite::compile::support::init_config;
use vega_lite::log;
use vega_lite::scale::{NON_TYPE_DOMAIN_RANGE_VEGA_SCALE_PROPERTIES, SCALE_PROPERTIES};

fn parse_model_with_scale(input_spec: &Value) -> Model {
    let config = init_config(input_spec.get("config"));
    let mut model = parse_model(input_spec, &config);
    model.parse_scale();
    model
}

fn parse_unit_model_with_scale(spec: &Value) -> Model {
    let config = init_config(spec.get("config"));
    let mut model = Model::new_unit(spec, None, "", None, &config);
    model.parse_scale();
    model
}

fn without<T: PartialEq + Clone>(array: &[T], excluded: &[T]) -> Vec<T> {
    array
        .iter()
        .filter(|item| !excluded.contains(item))
        .cloned()
        .collect()
}

fn scale_explicit_type(model: &Model, channel: &str) -> Option<Value> {
    model
        .get_scale_component(channel)
        .and_then(|c| c.explicit.get("type").cloned())
}

#[test]
fn non_type_range_scale_properties_should_match_scale_properties_without_type_domain_range() {
    let expected = without(
        &SCALE_PROPERTIES,
        &["type", "domain", "range", "rangeMax", "rangeMin", "scheme"],
    );
    let actual: Vec<&str> = NON_TYPE_DOMAIN_RANGE_VEGA_SCALE_PROPERTIES.to_vec();
    assert_eq!(
        actual.iter().map(|s| s.to_string()).collect::<std::collections::BTreeSet<_>>(),
        expected.into_iter().map(|s| s.to_string()).collect::<std::collections::BTreeSet<_>>()
    );
}

#[test]
fn respects_explicit_scale_type_from_second_layer() {
    let config = init_config(None);
    let mut model = parse_model(
        &json!({
            "data": {"url": "data/seattle-weather.csv"},
            "layer": [
                {"mark": "bar", "encoding": {"y": {"aggregate": "mean", "field": "precipitation", "type": "quantitative"}}},
                {"mark": "rule", "encoding": {"y": {"aggregate": "mean", "field": "precipitation", "type": "quantitative", "scale": {"type": "log"}}}}
            ]
        }),
        &config,
    );
    parse_scale_core(&mut model);
    assert_eq!(scale_explicit_type(&model, "y"), Some(json!("log")));
}

#[test]
fn respects_explicit_scale_type_from_first_layer() {
    let config = init_config(None);
    let mut model = parse_model(
        &json!({
            "data": {"url": "data/seattle-weather.csv"},
            "layer": [
                {"mark": "bar", "encoding": {"y": {"aggregate": "mean", "field": "precipitation", "type": "quantitative", "scale": {"type": "log"}}}},
                {"mark": "rule", "encoding": {"y": {"aggregate": "mean", "field": "precipitation", "type": "quantitative"}}}
            ]
        }),
        &config,
    );
    parse_scale_core(&mut model);
    assert_eq!(scale_explicit_type(&model, "y"), Some(json!("log")));
}

#[test]
fn favors_the_first_explicit_scale_type() {
    let (_, logger) = log::capture(|| {
        let config = init_config(None);
        let mut model = parse_model(
            &json!({
                "data": {"url": "data/seattle-weather.csv"},
                "layer": [
                    {"mark": "bar", "encoding": {"y": {"aggregate": "mean", "field": "precipitation", "type": "quantitative", "scale": {"type": "log"}}}},
                    {"mark": "rule", "encoding": {"y": {"aggregate": "mean", "field": "precipitation", "type": "quantitative", "scale": {"type": "pow"}}}}
                ]
            }),
            &config,
        );
        parse_scale_core(&mut model);
        assert_eq!(scale_explicit_type(&model, "y"), Some(json!("log")));
    });
    assert_eq!(
        logger.warns[0].as_str().unwrap(),
        "Conflicting scale property \"type\" (\"log\" and \"pow\"). Using \"log\"."
    );
}

#[test]
fn favors_the_band_over_point() {
    let config = init_config(None);
    let mut model = parse_model(
        &json!({
            "data": {"url": "data/seattle-weather.csv"},
            "layer": [
                {"mark": "point", "encoding": {
                    "y": {"aggregate": "mean", "field": "precipitation", "type": "quantitative"},
                    "x": {"field": "weather", "type": "nominal"}
                }},
                {"mark": "bar", "encoding": {
                    "y": {"aggregate": "mean", "field": "precipitation", "type": "quantitative"},
                    "x": {"field": "weather", "type": "nominal"}
                }}
            ]
        }),
        &config,
    );
    parse_scale_core(&mut model);
    assert_eq!(
        model
            .get_scale_component("x")
            .unwrap()
            .implicit
            .get("type")
            .cloned(),
        Some(json!("band"))
    );
}

#[test]
fn correctly_ignores_x_y_when_lon_lat() {
    let config = init_config(None);
    let mut model = parse_model(
        &json!({
            "data": {"url": "data/zipcodes.csv", "format": {"type": "csv"}},
            "mark": "point",
            "encoding": {
                "longitude": {"field": "longitude", "type": "quantitative"},
                "latitude": {"field": "latitude", "type": "quantitative"}
            }
        }),
        &config,
    );
    parse_scale_core(&mut model);
    assert!(model.get_scale_component("x").is_none());
    assert!(model.get_scale_component("y").is_none());
}

#[test]
fn correctly_ignores_shape_when_geojson() {
    let config = init_config(None);
    let mut model = parse_model(
        &json!({
            "mark": "geoshape",
            "data": {"url": "data/income.json"},
            "transform": [{
                "lookup": "id",
                "from": {"data": {"url": "data/us-10m.json", "format": {"type": "topojson", "feature": "states"}}, "key": "id"},
                "as": "geo"
            }],
            "encoding": {"shape": {"field": "geo", "type": "geojson"}}
        }),
        &config,
    );
    parse_scale_core(&mut model);
    assert!(model.get_scale_component("shape").is_none());
}

#[test]
fn does_not_throw_warning_when_two_equivalent_objects_are_specified() {
    let (_, logger) = log::capture(|| {
        let config = init_config(None);
        let mut model = parse_model(
            &json!({
                "data": {"url": "data/seattle-weather.csv"},
                "layer": [
                    {"mark": "circle", "encoding": {"y": {"field": "a", "type": "nominal", "scale": {"padding": 0.2}}}},
                    {"mark": "point", "encoding": {"y": {"field": "a", "type": "nominal", "scale": {"padding": 0.2}}}}
                ]
            }),
            &config,
        );
        parse_scales(&mut model, false);
        assert_eq!(
            model
                .get_scale_component("y")
                .unwrap()
                .explicit
                .get("padding")
                .cloned(),
            Some(json!(0.2))
        );
    });
    assert_eq!(logger.warns.len(), 0);
}

#[test]
fn should_convert_date_time_object_in_domain_min_max_to_signal() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"x": {"field": "date", "type": "temporal", "scale": {"domainMin": {"year": 2002}, "domainMax": {"year": 2012}}}}
    }));
    let scale = model.get_scale_component("x").unwrap();
    assert_eq!(
        scale.explicit.get("domainMin").cloned(),
        Some(json!({"signal": "datetime(2002, 0, 1, 0, 0, 0, 0)"}))
    );
    assert_eq!(
        scale.explicit.get("domainMax").cloned(),
        Some(json!({"signal": "datetime(2012, 0, 1, 0, 0, 0, 0)"}))
    );
}

#[test]
fn should_create_an_x_point_scale_with_a_step_based_range() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"x": {"field": "origin", "type": "nominal"}}
    }));
    let scale = model.get_scale_component("x").unwrap();
    assert_eq!(scale.implicit.get("type").cloned(), Some(json!("point")));
    assert_eq!(
        scale.implicit.get("range").cloned(),
        Some(json!({"step": 20}))
    );
}

#[test]
fn should_output_only_padding_for_band_scale_with_padding() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "bar",
        "encoding": {"x": {"field": "origin", "type": "nominal", "scale": {"type": "band", "padding": 0.6}}}
    }));
    let scale = model.get_scale_component("x").unwrap();
    assert_eq!(scale.explicit.get("padding").cloned(), Some(json!(0.6)));
    assert_eq!(scale.get("paddingInner"), None);
    assert_eq!(scale.get("paddingOuter"), None);
}

#[test]
fn should_output_default_padding_inner_and_outer_for_band_scale() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "bar",
        "encoding": {"x": {"field": "origin", "type": "nominal", "scale": {"type": "band"}}},
        "config": {"scale": {"bandPaddingInner": 0.3}}
    }));
    let scale = model.get_scale_component("x").unwrap();
    assert_eq!(
        scale.implicit.get("paddingInner").cloned(),
        Some(json!(0.3))
    );
    assert_eq!(
        scale.implicit.get("paddingOuter").cloned(),
        Some(json!(0.15))
    );
    assert_eq!(scale.get("padding"), None);
}

#[test]
fn nominal_with_color_should_create_correct_color_scale() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"color": {"field": "origin", "type": "nominal"}}
    }));
    let scale = model.get_scale_component("color").unwrap();
    assert_eq!(
        scale.implicit.get("name").cloned(),
        Some(json!("color"))
    );
    assert_eq!(
        scale.implicit.get("type").cloned(),
        Some(json!("ordinal"))
    );
    assert_eq!(
        scale.get("domains"),
        Some(json!([{"data": "main", "field": "origin", "sort": true}]))
    );
    assert_eq!(
        scale.implicit.get("range").cloned(),
        Some(json!("category"))
    );
}

#[test]
fn ordinal_with_color_should_create_linear_color_scale() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"color": {"field": "origin", "type": "ordinal"}}
    }));
    let scale = model.get_scale_component("color").unwrap();
    assert_eq!(
        scale.implicit.get("name").cloned(),
        Some(json!("color"))
    );
    assert_eq!(
        scale.implicit.get("type").cloned(),
        Some(json!("ordinal"))
    );
    assert_eq!(
        scale.get("domains"),
        Some(json!([{"data": "main", "field": "origin", "sort": true}]))
    );
}

#[test]
fn quantitative_with_color_should_create_linear_color_scale() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"color": {"field": "origin", "type": "quantitative"}}
    }));
    let scale = model.get_scale_component("color").unwrap();
    assert_eq!(
        scale.implicit.get("name").cloned(),
        Some(json!("color"))
    );
    assert_eq!(
        scale.implicit.get("type").cloned(),
        Some(json!("linear"))
    );
    assert_eq!(
        scale.implicit.get("range").cloned(),
        Some(json!("ramp"))
    );
    assert_eq!(
        scale.get("domains"),
        Some(json!([{"data": "main", "field": "origin"}]))
    );
}

#[test]
fn color_with_bin_should_add_correct_scales() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"color": {"field": "origin", "type": "quantitative", "bin": true}}
    }));
    let scale = model.get_scale_component("color").unwrap();
    assert_eq!(
        scale.implicit.get("name").cloned(),
        Some(json!("color"))
    );
    assert_eq!(
        scale.implicit.get("type").cloned(),
        Some(json!("bin-ordinal"))
    );
    assert_eq!(
        scale.implicit.get("bins").cloned(),
        Some(json!({"signal": "bin_maxbins_6_origin_bins"}))
    );
}

#[test]
fn ordinal_color_with_bin_should_add_correct_scales() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"color": {"field": "origin", "type": "ordinal", "bin": true}}
    }));
    let scale = model.get_scale_component("color").unwrap();
    assert_eq!(
        scale.implicit.get("name").cloned(),
        Some(json!("color"))
    );
    assert_eq!(
        scale.implicit.get("type").cloned(),
        Some(json!("ordinal"))
    );
    assert_eq!(scale.implicit.get("bins"), None);
}

#[test]
fn opacity_with_bin_should_add_correct_scales() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"opacity": {"field": "origin", "type": "quantitative", "bin": true}}
    }));
    let scale = model.get_scale_component("opacity").unwrap();
    assert_eq!(
        scale.implicit.get("name").cloned(),
        Some(json!("opacity"))
    );
    assert_eq!(
        scale.implicit.get("type").cloned(),
        Some(json!("linear"))
    );
    assert_eq!(
        scale.implicit.get("bins").cloned(),
        Some(json!({"signal": "bin_maxbins_6_origin_bins"}))
    );
}

#[test]
fn size_with_bin_should_add_correct_scales() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"size": {"field": "origin", "type": "quantitative", "bin": true}}
    }));
    let scale = model.get_scale_component("size").unwrap();
    assert_eq!(
        scale.implicit.get("name").cloned(),
        Some(json!("size"))
    );
    assert_eq!(
        scale.implicit.get("type").cloned(),
        Some(json!("linear"))
    );
    assert_eq!(
        scale.implicit.get("bins").cloned(),
        Some(json!({"signal": "bin_maxbins_6_origin_bins"}))
    );
}

#[test]
fn color_with_time_unit_should_add_correct_scales() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "point",
        "encoding": {"color": {"field": "origin", "type": "temporal", "timeUnit": "year"}}
    }));
    let scale = model.get_scale_component("color").unwrap();
    assert_eq!(
        scale.implicit.get("name").cloned(),
        Some(json!("color"))
    );
    assert_eq!(
        scale.implicit.get("type").cloned(),
        Some(json!("time"))
    );
}

#[test]
fn selection_domain_should_add_a_selection_extent() {
    let model = parse_unit_model_with_scale(&json!({
        "mark": "area",
        "encoding": {
            "x": {"field": "date", "type": "temporal", "scale": {"domain": {"param": "brush", "encoding": "x"}}},
            "y": {"field": "date", "type": "temporal", "scale": {"domain": {"param": "foobar", "field": "Miles_per_Gallon"}}}
        }
    }));
    let x_scale = model.get_scale_component("x").unwrap();
    let y_scale = model.get_scale_component("y").unwrap();
    assert!(x_scale.explicit.contains_key("selectionExtent"));
    assert_eq!(
        x_scale.explicit["selectionExtent"].get("param").cloned(),
        Some(json!("brush"))
    );
    assert_eq!(
        x_scale.explicit["selectionExtent"].get("encoding").cloned(),
        Some(json!("x"))
    );
    assert!(y_scale.explicit.contains_key("selectionExtent"));
    assert_eq!(
        y_scale.explicit["selectionExtent"].get("param").cloned(),
        Some(json!("foobar"))
    );
    assert_eq!(
        y_scale.explicit["selectionExtent"].get("field").cloned(),
        Some(json!("Miles_per_Gallon"))
    );
}

#[test]
fn faceted_domains_should_use_cloned_subtree() {
    let model = parse_model_with_scale(&json!({
        "facet": {"row": {"field": "symbol", "type": "nominal"}},
        "data": {"url": "foo.csv"},
        "spec": {"mark": "point", "encoding": {"x": {"field": "a", "type": "quantitative"}}}
    }));
    assert_eq!(
        model.component.scales["x"].get("domains"),
        Some(json!([{"data": "scale_child_main", "field": "a"}]))
    );
}

#[test]
fn faceted_domains_should_not_use_cloned_subtree_if_data_is_not_faceted() {
    let model = parse_model_with_scale(&json!({
        "facet": {"row": {"field": "symbol", "type": "nominal"}},
        "data": {"url": "foo.csv"},
        "spec": {"data": {"url": "foo"}, "mark": "point", "encoding": {"x": {"field": "a", "type": "quantitative"}}}
    }));
    assert_eq!(
        model.component.scales["x"].get("domains"),
        Some(json!([{"data": "child_main", "field": "a"}]))
    );
}

#[test]
fn faceted_domains_should_not_use_cloned_subtree_if_scale_is_independent() {
    let model = parse_model_with_scale(&json!({
        "facet": {"row": {"field": "symbol", "type": "nominal"}},
        "data": {"url": "foo.csv"},
        "spec": {"mark": "point", "encoding": {"x": {"field": "a", "type": "quantitative"}}},
        "resolve": {"scale": {"x": "independent"}}
    }));
    assert_eq!(
        model.children[0].component.scales["x"].get("domains"),
        Some(json!([{"data": "child_main", "field": "a"}]))
    );
}

#[test]
fn faceted_domains_should_show_warning_if_two_domains_are_merged() {
    let (_, logger) = log::capture(|| {
        let model = parse_model_with_scale(&json!({
            "layer": [
                {"mark": "point", "encoding": {"y": {"field": "foo", "type": "nominal", "scale": {"domain": [1, 2, 3]}}}},
                {"mark": "point", "encoding": {"y": {"field": "foo", "type": "nominal", "scale": {"domain": [2, 3, 4]}}}}
            ]
        }));
        assert_eq!(
            model.children[0].component.scales["y"].get("domains"),
            Some(json!([[1, 2, 3]]))
        );
        assert_eq!(
            model.children[1].component.scales["y"].get("domains"),
            Some(json!([[2, 3, 4]]))
        );
    });
    assert_eq!(
        logger.warns[0].as_str().unwrap(),
        "Conflicting scale property \"domains\" ([[1,2,3]] and [[2,3,4]]). Using the union of the two domains."
    );
}

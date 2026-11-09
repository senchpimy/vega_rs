//! Port of `test/compile/mark/mark.test.ts`.

#[path = "mark_util.rs"]
mod mark_util;

use serde_json::json;

use mark_util::{
    assert_eq_json, assert_match_object, parse_concat_model, parse_facet_model,
    parse_unit_model, parse_unit_model_with_scale, parse_unit_model_with_scale_and_layout,
    parse_unit_model_with_scale_and_selection,
};
use vega_lite::compile::mark::mark::{get_sort, parse_mark_groups};

#[test]
fn multi_series_line_facet_directive() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "line", "style": "trend"},
        "encoding": {
            "x": {"field": "date", "type": "temporal", "axis": {"format": "%Y"}},
            "y": {"field": "price", "type": "quantitative"},
            "color": {"field": "symbol", "type": "nominal"}
        }
    }));
    let mark_group = &parse_mark_groups(&model)[0];
    assert_eq_json(json!("pathgroup"), mark_group["name"].clone());
    assert_eq_json(
        json!({"facet": {"name": "faceted_path_main", "data": "main", "groupby": ["symbol"]}}),
        mark_group["from"].clone(),
    );
    let submark = &mark_group["marks"][0];
    assert_eq_json(json!("marks"), submark["name"].clone());
    assert_eq_json(json!("line"), submark["type"].clone());
    assert_eq_json(json!(["line", "trend"]), submark["style"].clone());
    assert_eq_json(json!("faceted_path_main"), submark["from"]["data"].clone());
}

#[test]
fn multi_series_line_no_post_encoding_transform() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "line", "style": "trend"},
        "encoding": {
            "x": {"field": "date", "type": "temporal", "axis": {"format": "%Y"}},
            "y": {"field": "price", "type": "quantitative"},
            "color": {"field": "symbol", "type": "nominal"}
        }
    }));
    let mark_group = &parse_mark_groups(&model)[0];
    assert!(mark_group["marks"][0].get("transform").is_none());
}

#[test]
fn single_line_mark_group() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "line",
        "encoding": {
            "x": {"field": "date", "type": "temporal", "axis": {"format": "%Y"}},
            "y": {"field": "price", "type": "quantitative"}
        }
    }));
    let mark_group = &parse_mark_groups(&model)[0];
    assert_eq_json(json!("marks"), mark_group["name"].clone());
    assert_eq_json(json!("line"), mark_group["type"].clone());
    assert_eq_json(json!("main"), mark_group["from"]["data"].clone());
}

#[test]
fn single_line_no_post_encoding_transform() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "line",
        "encoding": {
            "x": {"field": "date", "type": "temporal", "axis": {"format": "%Y"}},
            "y": {"field": "price", "type": "quantitative"}
        }
    }));
    assert!(parse_mark_groups(&model)[0].get("transform").is_none());
}

#[test]
fn points_with_key() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "point",
        "encoding": {
            "x": {"field": "date", "type": "temporal", "axis": {"format": "%Y"}},
            "y": {"field": "price", "type": "quantitative"},
            "key": {"field": "k", "type": "quantitative"}
        }
    }));
    let mark_group = &parse_mark_groups(&model)[0];
    assert_eq_json(json!("symbol"), mark_group["type"].clone());
    assert_eq_json(json!("k"), mark_group["key"].clone());
    assert_eq_json(json!("main"), mark_group["from"]["data"].clone());
    assert!(parse_mark_groups(&model)[0].get("transform").is_none());
}

#[test]
fn geoshape_has_post_encoding_transform() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "geoshape",
        "projection": {"type": "albersUsa"},
        "data": {"url": "data/us-10m.json", "format": {"type": "topojson", "feature": "states"}},
        "encoding": {}
    }));
    let mark_group = &parse_mark_groups(&model)[0];
    assert!(mark_group.get("transform").is_some());
    assert_eq_json(
        json!("geoshape"),
        mark_group["transform"][0]["type"].clone(),
    );
}

#[test]
fn aggregated_bar_color_binned_x() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "bar",
        "encoding": {
            "x": {"type": "quantitative", "field": "Cost__Other", "aggregate": "sum"},
            "y": {"bin": true, "type": "quantitative", "field": "Cost__Total_$"},
            "color": {"type": "ordinal", "field": "Effect__Amount_of_damage"}
        }
    }));
    let mark_group = &parse_mark_groups(&model)[0];
    assert_eq_json(json!("main"), mark_group["from"]["data"].clone());
    assert_eq_json(json!(["bar"]), mark_group["style"].clone());
    assert!(mark_group.get("transform").is_none());
}

#[test]
fn faceted_aggregated_bar_color_binned_x() {
    let mut model = parse_facet_model(&json!({
        "facet": {"row": {"field": "a", "type": "nominal"}},
        "spec": {
            "mark": "bar",
            "encoding": {
                "x": {"type": "quantitative", "field": "Cost__Other", "aggregate": "sum"},
                "y": {"bin": true, "type": "quantitative", "field": "Cost__Total_$"},
                "color": {"type": "ordinal", "field": "Effect__Amount_of_damage"}
            }
        }
    }));
    model.parse_scale();
    model.parse_layout_size();
    let child = &model.children[0];
    let mark_group = &parse_mark_groups(child)[0];
    assert_eq_json(json!("child_main"), mark_group["from"]["data"].clone());
    assert!(mark_group.get("transform").is_none());
}

#[test]
fn aggregated_bar_uses_main_data_source() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "bar",
        "encoding": {
            "x": {"type": "quantitative", "field": "Cost__Other", "aggregate": "sum"},
            "y": {"bin": true, "type": "quantitative", "field": "Cost__Total_$"}
        }
    }));
    let mark_group = &parse_mark_groups(&model)[0];
    assert_eq_json(json!("main"), mark_group["from"]["data"].clone());
    assert!(mark_group.get("transform").is_none());
}

#[test]
fn aria_false_for_bar_mark() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "bar", "aria": false},
        "encoding": {"x": {"type": "quantitative", "field": "foo"}}
    }));
    assert_eq_json(json!(false), parse_mark_groups(&model)[0]["aria"].clone());
}

#[test]
fn aria_false_for_rounded_bar_mark() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "bar", "aria": false, "cornerRadius": 2},
        "encoding": {"x": {"type": "quantitative", "field": "foo"}}
    }));
    let mark_group = parse_mark_groups(&model);
    assert_eq_json(
        json!(false),
        mark_group[0]["marks"][0]["marks"][0]["aria"].clone(),
    );
}

#[test]
fn aria_false_for_line_mark() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "line", "aria": false},
        "encoding": {"x": {"type": "quantitative", "field": "foo"}}
    }));
    assert_eq_json(json!(false), parse_mark_groups(&model)[0]["aria"].clone());
}

#[test]
fn group_mark_with_corner_radius_by_nominal_field() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "bar", "cornerRadius": 2},
        "encoding": {
            "x": {"type": "quantitative", "field": "foo"},
            "y": {"type": "nominal", "field": "bar"}
        }
    }));
    let mark_group = parse_mark_groups(&model);
    assert_eq_json(
        json!(["bar"]),
        mark_group[0]["from"]["facet"]["groupby"].clone(),
    );
}

#[test]
fn round_left_or_right_end_of_horizontal_stacked_bar_groups() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "bar", "cornerRadiusEnd": 5},
        "encoding": {
            "x": {"type": "quantitative", "field": "foo", "aggregate": "sum"},
            "y": {"type": "nominal", "field": "bar"},
            "color": {"type": "nominal", "field": "series"}
        }
    }));
    let mark_group = parse_mark_groups(&model);
    let update = &mark_group[0]["encode"]["update"];

    let right_end = "scale(\"x\", datum[\"max_sum_foo_end\"]) > scale(\"x\", datum[\"max_sum_foo_start\"])";
    let left_end = "scale(\"x\", datum[\"min_sum_foo_end\"]) < scale(\"x\", datum[\"min_sum_foo_start\"])";
    let round_right = json!([{"test": right_end, "value": 5}, {"value": 0}]);
    let round_left = json!([{"test": left_end, "value": 5}, {"value": 0}]);

    assert_match_object(
        &json!({
            "cornerRadiusTopRight": round_right,
            "cornerRadiusBottomRight": round_right,
            "cornerRadiusTopLeft": round_left,
            "cornerRadiusBottomLeft": round_left,
        }),
        update,
    );
}

#[test]
fn round_top_or_bottom_end_of_vertical_stacked_bar_groups() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "bar", "cornerRadiusEnd": 5},
        "encoding": {
            "x": {"type": "nominal", "field": "bar"},
            "y": {"type": "quantitative", "field": "foo", "aggregate": "sum"},
            "color": {"type": "nominal", "field": "series"}
        }
    }));
    let update = &parse_mark_groups(&model)[0]["encode"]["update"];

    let top_end = "scale(\"y\", datum[\"max_sum_foo_end\"]) < scale(\"y\", datum[\"max_sum_foo_start\"])";
    let bottom_end = "scale(\"y\", datum[\"min_sum_foo_end\"]) > scale(\"y\", datum[\"min_sum_foo_start\"])";
    let round_top = json!([{"test": top_end, "value": 5}, {"value": 0}]);
    let round_bottom = json!([{"test": bottom_end, "value": 5}, {"value": 0}]);

    assert_match_object(
        &json!({
            "cornerRadiusTopLeft": round_top,
            "cornerRadiusTopRight": round_top,
            "cornerRadiusBottomLeft": round_bottom,
            "cornerRadiusBottomRight": round_bottom,
        }),
        update,
    );
}

#[test]
fn config_corner_radius_end_does_not_override_mark_corner_radius_on_stacked_groups() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "bar", "cornerRadius": 2},
        "encoding": {
            "x": {"type": "nominal", "field": "bar"},
            "y": {"type": "quantitative", "field": "foo", "aggregate": "sum"},
            "color": {"type": "nominal", "field": "series"}
        },
        "config": {"bar": {"cornerRadiusEnd": 5}}
    }));
    let update = &parse_mark_groups(&model)[0]["encode"]["update"];
    assert_eq_json(json!({"value": 2}), update["cornerRadius"].clone());
    assert!(update.get("cornerRadiusTopLeft").is_none());
    assert!(update.get("cornerRadiusBottomRight").is_none());
}

#[test]
fn no_raw_tooltip_directive_when_generated_tooltip_empty() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "point", "tooltip": true},
        "encoding": {
            "x": {"field": "a", "type": "quantitative", "tooltip": false},
            "y": {"field": "b", "type": "quantitative", "tooltip": false}
        }
    }));
    let update = &parse_mark_groups(&model)[0]["encode"]["update"];
    assert!(update.get("tooltip").is_none());
}

#[test]
fn static_mark_def_tooltip_values_pass_through() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "point", "tooltip": "static tooltip"},
        "encoding": {"x": {"field": "a", "type": "quantitative"}}
    }));
    let update = &parse_mark_groups(&model)[0]["encode"]["update"];
    assert_eq_json(json!({"value": "static tooltip"}), update["tooltip"].clone());
}

#[test]
fn interactive_no_flag_without_selections() {
    let model = parse_unit_model_with_scale_and_selection(&json!({
        "mark": "point",
        "encoding": {
            "x": {"type": "quantitative", "field": "foo"},
            "y": {"type": "nominal", "field": "bar"}
        }
    }));
    assert!(parse_mark_groups(&model)[0].get("interactive").is_none());
}

#[test]
fn interactive_true_for_unit_with_selection() {
    let model = parse_concat_model(&json!({
        "vconcat": [
            {
                "params": [{"name": "brush", "select": "interval"}],
                "mark": "point",
                "encoding": {
                    "x": {"type": "quantitative", "field": "foo"},
                    "y": {"type": "nominal", "field": "bar"}
                }
            },
            {
                "mark": "point",
                "encoding": {
                    "x": {"type": "ordinal", "field": "baz"},
                    "y": {"type": "quantitative", "field": "world"}
                }
            }
        ]
    }));
    let mut parent_selection = model.component.clone();
    let _ = &mut parent_selection;
    let mut second_model = model.children[1].clone();
    second_model.component.selection = json!({"brush": {}});
    let first = &parse_mark_groups(&model.children[0])[0];
    let second = &parse_mark_groups(&second_model)[0];
    assert_eq_json(json!(true), first["interactive"].clone());
    assert_eq_json(json!(false), second["interactive"].clone());
}

#[test]
fn interactive_true_for_selection_or_tooltip() {
    let model = parse_concat_model(&json!({
        "vconcat": [
            {
                "params": [{"name": "brush", "select": {"type": "interval"}}],
                "mark": "point",
                "encoding": {
                    "x": {"type": "quantitative", "field": "foo"},
                    "y": {"type": "nominal", "field": "bar"}
                }
            },
            {
                "mark": "point",
                "encoding": {
                    "x": {"type": "ordinal", "field": "baz"},
                    "y": {"type": "quantitative", "field": "world"},
                    "tooltip": {"type": "nominal", "field": "hello"}
                }
            }
        ]
    }));
    let mut second_model = model.children[1].clone();
    second_model.component.selection = json!({"brush": {}});
    let first = &parse_mark_groups(&model.children[0])[0];
    let second = &parse_mark_groups(&second_model)[0];
    assert_eq_json(json!(true), first["interactive"].clone());
    assert_eq_json(json!(true), second["interactive"].clone());
}

#[test]
fn sort_by_order_field() {
    let model = parse_unit_model(&json!({
        "data": {"url": "data/driving.json"},
        "mark": "line",
        "encoding": {
            "x": {"field": "miles", "type": "quantitative", "scale": {"zero": false}},
            "y": {"field": "gas", "type": "quantitative", "scale": {"zero": false}},
            "order": {"field": "year", "type": "temporal"}
        }
    }));
    assert_eq_json(
        json!({"field": ["datum[\"year\"]"], "order": ["ascending"]}),
        get_sort(&model).unwrap(),
    );
}

#[test]
fn no_sort_if_order_value_null() {
    let model = parse_unit_model(&json!({
        "data": {"url": "data/driving.json"},
        "mark": "line",
        "encoding": {
            "x": {"field": "miles", "type": "quantitative", "scale": {"zero": false}},
            "y": {"field": "gas", "type": "quantitative", "scale": {"zero": false}},
            "order": {"value": null}
        }
    }));
    assert!(get_sort(&model).is_none());
}

#[test]
fn order_by_x_if_x_is_dimension() {
    let model = parse_unit_model_with_scale(&json!({
        "data": {"url": "data/movies.json"},
        "mark": "line",
        "encoding": {
            "x": {"bin": {"maxbins": 10}, "field": "IMDB_Rating", "type": "quantitative"},
            "color": {"field": "Source", "type": "nominal"},
            "y": {"aggregate": "count", "type": "quantitative"}
        }
    }));
    assert_eq_json(json!({"field": "x"}), get_sort(&model).unwrap());
}

#[test]
fn order_by_x_when_dimension_sort_null() {
    let model = parse_unit_model_with_scale(&json!({
        "data": {"url": "data/movies.json"},
        "mark": "line",
        "encoding": {
            "x": {"field": "a", "type": "nominal", "sort": null},
            "color": {"field": "Source", "type": "nominal"},
            "y": {"aggregate": "count", "type": "quantitative"}
        }
    }));
    assert_eq_json(json!({"field": "x"}), get_sort(&model).unwrap());
}

#[test]
fn order_by_x_custom_sort_order() {
    let model = parse_unit_model_with_scale(&json!({
        "data": {"url": "data/movies.json"},
        "mark": "line",
        "encoding": {
            "x": {"type": "nominal", "field": "Name", "sort": ["Peter", "Mary", "Paul"]},
            "y": {"type": "quantitative", "field": "Score"}
        }
    }));
    assert_eq_json(json!({"field": "x"}), get_sort(&model).unwrap());
}

#[test]
fn order_by_right_channel_when_sort_by_encoding() {
    let model = parse_unit_model_with_scale(&json!({
        "data": {"url": "data/movies.json"},
        "mark": "line",
        "encoding": {
            "x": {"type": "nominal", "field": "Name", "sort": "y"},
            "y": {"type": "quantitative", "field": "Score"}
        }
    }));
    assert_eq_json(json!({"field": "x"}), get_sort(&model).unwrap());
}

#[test]
fn no_order_by_a_missing_dimension() {
    let model = parse_unit_model_with_scale(&json!({
        "data": {"url": "data/movies.json"},
        "mark": "line",
        "encoding": {
            "color": {"field": "Source", "type": "nominal"},
            "y": {"aggregate": "count", "type": "quantitative"}
        }
    }));
    assert!(get_sort(&model).is_none());
}

#[test]
fn projection_clip_not_clipped_if_auto_fit() {
    let mut model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "geoshape",
        "projection": {"type": "albersUsa"},
        "data": {"url": "data/us-10m.json", "format": {"type": "topojson", "feature": "states"}},
        "encoding": {"color": {"value": "black"}, "opacity": {"value": 0.8}},
        "config": {"mark": {"tooltip": null}}
    }));
    model.parse();
    let mark = parse_mark_groups(&model);
    assert!(mark[0].get("clip").is_none());
}

#[test]
fn projection_clip_clipped_if_not_auto_fit() {
    let mut model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "geoshape",
        "projection": {"type": "albersUsa", "scale": 1000},
        "data": {"url": "data/us-10m.json", "format": {"type": "topojson", "feature": "states"}},
        "encoding": {"color": {"value": "black"}, "opacity": {"value": 0.8}},
        "config": {"mark": {"tooltip": null}}
    }));
    model.parse();
    let mark = parse_mark_groups(&model);
    assert_eq_json(json!(true), mark[0]["clip"].clone());
}

#[test]
fn clip_as_signal_if_clip_is_expression() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "bar", "clip": {"expr": "datum['foo'] > 10"}},
        "encoding": {"x": {"type": "quantitative", "field": "foo"}}
    }));
    assert_eq_json(
        json!({"signal": "datum['foo'] > 10"}),
        parse_mark_groups(&model)[0]["clip"].clone(),
    );
}

#[test]
fn clip_as_signal_if_clip_is_signal() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "bar", "clip": {"signal": "datum['foo'] > 10"}},
        "encoding": {"x": {"type": "quantitative", "field": "foo"}}
    }));
    assert_eq_json(
        json!({"signal": "datum['foo'] > 10"}),
        parse_mark_groups(&model)[0]["clip"].clone(),
    );
}

#[test]
fn clip_as_boolean_if_true() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "bar", "clip": true},
        "encoding": {"x": {"type": "quantitative", "field": "foo"}}
    }));
    assert_eq_json(json!(true), parse_mark_groups(&model)[0]["clip"].clone());
}

#[test]
fn clip_undefied_if_false() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "bar", "clip": false},
        "encoding": {"x": {"type": "quantitative", "field": "foo"}}
    }));
    assert!(parse_mark_groups(&model)[0].get("clip").is_none());
}

#[test]
fn points_with_key_no_post_encoding_transform() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "point",
        "encoding": {
            "x": {"field": "date", "type": "temporal", "axis": {"format": "%Y"}},
            "y": {"field": "price", "type": "quantitative"},
            "key": {"field": "k", "type": "quantitative"}
        }
    }));
    assert!(parse_mark_groups(&model)[0].get("transform").is_none());
}

#[test]
fn aggregated_bar_color_binned_x_no_post_encoding_transform() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "bar",
        "encoding": {
            "x": {"type": "quantitative", "field": "Cost__Other", "aggregate": "sum"},
            "y": {"bin": true, "type": "quantitative", "field": "Cost__Total_$"},
            "color": {"type": "ordinal", "field": "Effect__Amount_of_damage"}
        }
    }));
    assert!(parse_mark_groups(&model)[0].get("transform").is_none());
}

#[test]
fn faceted_aggregated_bar_color_binned_x_no_post_encoding_transform() {
    let mut model = parse_facet_model(&json!({
        "facet": {"row": {"field": "a", "type": "nominal"}},
        "spec": {
            "mark": "bar",
            "encoding": {
                "x": {"type": "quantitative", "field": "Cost__Other", "aggregate": "sum"},
                "y": {"bin": true, "type": "quantitative", "field": "Cost__Total_$"},
                "color": {"type": "ordinal", "field": "Effect__Amount_of_damage"}
            }
        }
    }));
    model.parse_scale();
    model.parse_layout_size();
    assert!(parse_mark_groups(&model.children[0])[0].get("transform").is_none());
}

#[test]
fn aggregated_bar_uses_main_data_source_no_post_encoding_transform() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "bar",
        "encoding": {
            "x": {"type": "quantitative", "field": "Cost__Other", "aggregate": "sum"},
            "y": {"bin": true, "type": "quantitative", "field": "Cost__Total_$"}
        }
    }));
    assert!(parse_mark_groups(&model)[0].get("transform").is_none());
}

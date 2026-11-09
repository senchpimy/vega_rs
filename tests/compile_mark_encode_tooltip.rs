//! Port of `test/compile/mark/encode/tooltip.test.ts`.

#[path = "mark_util.rs"]
mod mark_util;

use serde_json::{json, Value};

use mark_util::{assert_eq_json, parse_unit_model_with_scale_and_layout};
use vega_lite::compile::mark::encode::tooltip::{
    tooltip, tooltip_ref_for_encoding, tooltip_with_options,
};
use vega_lite::compile::support::default_config;
use vega_lite::compile_common::BIN_RANGE_DELIMITER;

fn tooltip_signal(props: &Value) -> &str {
    props["tooltip"]["signal"].as_str().unwrap()
}

fn filtered_tooltip_signal(field: &str, test: &str) -> String {
    format!("({test}) ? {{\"{field}\": format(datum[\"{field}\"], \"\")}} : null")
}

fn stack_normalize(channel: &str) -> Value {
    json!({
        "fieldChannel": channel,
        "groupbyChannels": [],
        "groupbyFields": [],
        "offset": "normalize",
        "impute": false,
        "stackBy": []
    })
}

#[test]
fn generates_tooltip_object_signal_for_all_encoding_fields() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "point",
        "encoding": {
            "tooltip": [
                {"field": "Horsepower", "type": "quantitative"},
                {"field": "Acceleration", "type": "quantitative"}
            ]
        }
    }));
    assert_eq_json(
        json!({"tooltip": {"signal": "{\"Horsepower\": format(datum[\"Horsepower\"], \"\"), \"Acceleration\": format(datum[\"Acceleration\"], \"\")}"}}),
        tooltip(&model),
    );
}

#[test]
fn generates_tooltip_object_signal_when_explicitly_enabled() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "point", "tooltip": true},
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Acceleration", "type": "quantitative"}
        }
    }));
    assert_eq_json(
        json!({"tooltip": {"signal": "{\"Horsepower\": format(datum[\"Horsepower\"], \"\"), \"Acceleration\": format(datum[\"Acceleration\"], \"\")}"}}),
        tooltip(&model),
    );
}

#[test]
fn omits_encoding_fields_with_tooltip_false() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "point", "tooltip": true},
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "detail": {"field": "__internal_key", "type": "nominal", "tooltip": false}
        }
    }));
    assert_eq_json(
        json!({"tooltip": {"signal": "{\"Horsepower\": format(datum[\"Horsepower\"], \"\")}"}}),
        tooltip(&model),
    );
}

#[test]
fn filters_explicit_tooltip_fields_by_unformatted_values() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "point",
        "encoding": {
            "tooltip": [
                {"field": "Date", "type": "nominal"},
                {"field": "type1", "type": "quantitative", "filter": {"not": {"equal": 0}}},
                {"field": "type2", "type": "quantitative", "filter": {"valid": true}},
                {"field": "type3", "type": "quantitative", "filter": {"gt": 0}}
            ]
        }
    }));
    let expected = "merge({\"Date\": isValid(datum[\"Date\"]) ? isArray(datum[\"Date\"]) ? join(datum[\"Date\"], '\\n') : datum[\"Date\"] : \"\"+datum[\"Date\"]}, (!(datum[\"type1\"]===0)) ? {\"type1\": format(datum[\"type1\"], \"\")} : {}, (isValid(datum[\"type2\"]) && isFinite(+datum[\"type2\"])) ? {\"type2\": format(datum[\"type2\"], \"\")} : {}, (datum[\"type3\"]>0) ? {\"type3\": format(datum[\"type3\"], \"\")} : {})";
    assert_eq_json(json!({"tooltip": {"signal": expected}}), tooltip(&model));
}

#[test]
fn no_tooltip_if_encoding_tooltip_null() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "point",
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Acceleration", "type": "quantitative"},
            "tooltip": null
        }
    }));
    assert!(tooltip(&model).get("tooltip").is_none());
}

#[test]
fn uses_mark_tooltip_strings_as_static_values() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "point", "tooltip": "static tooltip"},
        "encoding": {"x": {"field": "Horsepower", "type": "quantitative"}}
    }));
    assert_eq_json(json!({"tooltip": {"value": "static tooltip"}}), tooltip(&model));
}

#[test]
fn tooltip_content_data() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "point", "tooltip": {"content": "data"}},
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Acceleration", "type": "quantitative"}
        }
    }));
    assert_eq_json(json!({"tooltip": {"signal": "datum"}}), tooltip(&model));
}

#[test]
fn tooltip_signal_specified() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "point", "tooltip": {"signal": "a"}},
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Acceleration", "type": "quantitative"}
        }
    }));
    assert_eq_json(json!({"tooltip": {"signal": "a"}}), tooltip(&model));
}

#[test]
fn tooltip_content_data_reactive_geom() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "line", "tooltip": {"content": "data"}},
        "encoding": {
            "x": {"field": "Horsepower", "type": "quantitative"},
            "y": {"field": "Acceleration", "type": "quantitative"}
        }
    }));
    assert_eq_json(
        json!({"tooltip": {"signal": "datum.datum"}}),
        tooltip_with_options(&model, true),
    );
}

#[test]
fn array_check_for_discrete_encodings_without_format() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "bar", "tooltip": true},
        "encoding": {"x": {"field": "Foo", "type": "nominal"}}
    }));
    let expected = "{\"Foo\": isValid(datum[\"Foo\"]) ? isArray(datum[\"Foo\"]) ? join(datum[\"Foo\"], '\\n') : datum[\"Foo\"] : \"\"+datum[\"Foo\"]}";
    assert_eq_json(json!({"tooltip": {"signal": expected}}), tooltip(&model));
}

#[test]
fn no_array_check_for_non_discrete_or_formatted() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "circle"},
        "encoding": {
            "tooltip": [
                {"field": "Foo", "format": "."},
                {"field": "Bar", "type": "quantitative"},
                {"field": "Baz", "formatType": "customFormat"}
            ]
        },
        "config": {"customFormatTypes": true}
    }));
    let expected = "{\"Foo\": format(datum[\"Foo\"], \".\"), \"Bar\": format(datum[\"Bar\"], \"\"), \"Baz\": customFormat(datum[\"Baz\"])}";
    assert_eq_json(json!({"tooltip": {"signal": expected}}), tooltip(&model));
}

#[test]
fn discrete_reactive_geom() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "circle", "tooltip": true},
        "encoding": {"color": {"field": "Foobar", "type": "nominal"}}
    }));
    let expected = "{\"Foobar\": isValid(datum.datum[\"Foobar\"]) ? isArray(datum.datum[\"Foobar\"]) ? join(datum.datum[\"Foobar\"], '\\n') : datum.datum[\"Foobar\"] : \"\"+datum.datum[\"Foobar\"]}";
    assert_eq_json(
        json!({"tooltip": {"signal": expected}}),
        tooltip_with_options(&model, true),
    );
}

#[test]
fn filters_generated_tooltip_reactive_geom() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "circle"},
        "encoding": {"tooltip": [{"field": "Foobar", "type": "quantitative", "filter": {"not": {"equal": 0}}}]}
    }));
    let expected = "(!(datum.datum[\"Foobar\"]===0)) ? {\"Foobar\": format(datum.datum[\"Foobar\"], \"\")} : null";
    assert_eq_json(
        json!({"tooltip": {"signal": expected}}),
        tooltip_with_options(&model, true),
    );
}

#[test]
fn prioritizes_tooltip_field_def() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "point", "tooltip": {"content": "data"}},
        "encoding": {
            "x": {"field": "Cylinders", "type": "quantitative"},
            "y": {"field": "Displacement", "type": "quantitative"},
            "tooltip": [
                {"field": "Horsepower", "type": "quantitative"},
                {"field": "Acceleration", "type": "quantitative"}
            ]
        }
    }));
    assert_eq_json(
        json!({"tooltip": {"signal": "{\"Horsepower\": format(datum[\"Horsepower\"], \"\"), \"Acceleration\": format(datum[\"Acceleration\"], \"\")}"}}),
        tooltip(&model),
    );
}

#[test]
fn prioritizes_tooltip_value_def() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "point", "tooltip": {"content": "data"}},
        "encoding": {
            "x": {"field": "Cylinders", "type": "quantitative"},
            "y": {"field": "Displacement", "type": "quantitative"},
            "tooltip": {"value": "haha"}
        }
    }));
    assert_eq_json(json!({"tooltip": {"value": "haha"}}), tooltip(&model));
}

#[test]
fn keys_and_values_for_channels_with_axis() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "point", "tooltip": true},
        "encoding": {
            "x": {"field": "Date", "type": "quantitative", "axis": {"title": "foo", "format": "%y"}},
            "y": {"field": "Displacement", "type": "quantitative", "axis": {"title": "bar"}}
        }
    }));
    let expected = "{\"Date\": format(datum[\"Date\"], \"%y\"), \"Displacement\": format(datum[\"Displacement\"], \"\")}";
    assert_eq_json(json!({"tooltip": {"signal": expected}}), tooltip(&model));
}

#[test]
fn keys_and_values_with_legend_title() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "point", "tooltip": true},
        "encoding": {"color": {"field": "Foobar", "type": "nominal", "legend": {"title": "baz"}}}
    }));
    let expected = "{\"Foobar\": isValid(datum[\"Foobar\"]) ? isArray(datum[\"Foobar\"]) ? join(datum[\"Foobar\"], '\\n') : datum[\"Foobar\"] : \"\"+datum[\"Foobar\"]}";
    assert_eq_json(json!({"tooltip": {"signal": expected}}), tooltip(&model));
}

#[test]
fn keys_and_values_with_title() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "point", "tooltip": true},
        "encoding": {"color": {"field": "Foobar", "type": "nominal", "title": "baz"}}
    }));
    let expected = "{\"baz\": isValid(datum[\"Foobar\"]) ? isArray(datum[\"Foobar\"]) ? join(datum[\"Foobar\"], '\\n') : datum[\"Foobar\"] : \"\"+datum[\"Foobar\"]}";
    assert_eq_json(json!({"tooltip": {"signal": expected}}), tooltip(&model));
}

#[test]
fn tooltip_via_text_ref_for_time_unit() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "bar", "tooltip": true},
        "encoding": {
            "x": {"field": "date", "timeUnit": "yearmonth", "type": "nominal"},
            "y": {"aggregate": "count", "type": "quantitative"}
        }
    }));
    let sig = tooltip_signal(&tooltip(&model)).to_string();
    assert!(!sig.contains("datum[\"date\"]"), "{sig}");
    assert!(sig.contains("yearmonth_date"), "{sig}");
}

#[test]
fn keys_and_values_with_title_with_quotes() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "point", "tooltip": true},
        "encoding": {"color": {"field": "Foobar", "type": "nominal", "title": "\"baz\""}}
    }));
    let expected = "{\"\\\"baz\\\"\": isValid(datum[\"Foobar\"]) ? isArray(datum[\"Foobar\"]) ? join(datum[\"Foobar\"], '\\n') : datum[\"Foobar\"] : \"\"+datum[\"Foobar\"]}";
    assert_eq_json(json!({"tooltip": {"signal": expected}}), tooltip(&model));
}

#[test]
fn no_tooltip_when_all_fields_hide_tooltip() {
    let result = tooltip_ref_for_encoding(
        &json!({"x": {"field": "Horsepower", "type": "quantitative", "tooltip": false}}),
        None,
        &default_config(),
        false,
    );
    assert!(result.is_none());
}

#[test]
fn tooltip_signal_for_binned_field() {
    let result = tooltip_ref_for_encoding(
        &json!({"x": {"bin": true, "field": "IMDB_Rating", "type": "quantitative"}}),
        None,
        &default_config(),
        false,
    )
    .unwrap();
    assert_eq_json(
        json!({"signal": format!(
            "{{\"IMDB_Rating (binned)\": !isValid(datum[\"bin_maxbins_10_IMDB_Rating\"]) || !isFinite(+datum[\"bin_maxbins_10_IMDB_Rating\"]) ? \"null\" : format(datum[\"bin_maxbins_10_IMDB_Rating\"], \"\") + \"{BIN_RANGE_DELIMITER}\" + format(datum[\"bin_maxbins_10_IMDB_Rating_end\"], \"\")}}"
        )}),
        result,
    );
}

#[test]
fn tooltip_signal_for_normalized_x_stacked_field() {
    let encoding = json!({"x": {"aggregate": "sum", "field": "IMDB_Rating", "type": "quantitative"}});
    let stack = stack_normalize("x");
    let result = tooltip_ref_for_encoding(&encoding, Some(&stack), &default_config(), false).unwrap();
    assert_eq_json(
        json!({"signal": "{\"Sum of IMDB_Rating\": format(datum[\"sum_IMDB_Rating_end\"]-datum[\"sum_IMDB_Rating_start\"], \".0%\")}"}),
        result,
    );
}

#[test]
fn tooltip_signal_for_normalized_theta_stacked_field() {
    let encoding = json!({"theta": {"aggregate": "sum", "field": "IMDB_Rating", "type": "quantitative"}});
    let stack = stack_normalize("theta");
    let result = tooltip_ref_for_encoding(&encoding, Some(&stack), &default_config(), false).unwrap();
    assert_eq_json(
        json!({"signal": "{\"Sum of IMDB_Rating\": format(datum[\"sum_IMDB_Rating_end\"]-datum[\"sum_IMDB_Rating_start\"], \".0%\")}"}),
        result,
    );
}

#[test]
fn tooltip_signal_for_formatted_normalized_stacked_field() {
    let encoding = json!({"x": {"aggregate": "sum", "field": "IMDB_Rating", "type": "quantitative"}});
    let stack = stack_normalize("x");
    let mut config = default_config();
    config["normalizedNumberFormat"] = json!(".4%");
    config["customFormatTypes"] = json!(true);
    let result = tooltip_ref_for_encoding(&encoding, Some(&stack), &config, false).unwrap();
    assert_eq_json(
        json!({"signal": "{\"Sum of IMDB_Rating\": format(datum[\"sum_IMDB_Rating_end\"]-datum[\"sum_IMDB_Rating_start\"], \".4%\")}"}),
        result,
    );
}

#[test]
fn tooltip_signal_normalized_uses_tooltip_format() {
    let encoding = json!({"x": {"aggregate": "sum", "field": "IMDB_Rating", "type": "quantitative"}});
    let stack = stack_normalize("x");
    let mut config = default_config();
    config["tooltipFormat"] = json!({"normalizedNumberFormat": ".4%"});
    config["customFormatTypes"] = json!(true);
    let result = tooltip_ref_for_encoding(&encoding, Some(&stack), &config, false).unwrap();
    assert_eq_json(
        json!({"signal": "{\"Sum of IMDB_Rating\": format(datum[\"sum_IMDB_Rating_end\"]-datum[\"sum_IMDB_Rating_start\"], \".4%\")}"}),
        result,
    );
}

#[test]
fn tooltip_signal_normalized_prefers_tooltip_format() {
    let encoding = json!({"x": {"aggregate": "sum", "field": "IMDB_Rating", "type": "quantitative"}});
    let stack = stack_normalize("x");
    let mut config = default_config();
    config["tooltipFormat"] = json!({"normalizedNumberFormat": ".4%"});
    config["normalizedNumberFormat"] = json!(".2%");
    config["customFormatTypes"] = json!(true);
    let result = tooltip_ref_for_encoding(&encoding, Some(&stack), &config, false).unwrap();
    assert_eq_json(
        json!({"signal": "{\"Sum of IMDB_Rating\": format(datum[\"sum_IMDB_Rating_end\"]-datum[\"sum_IMDB_Rating_start\"], \".4%\")}"}),
        result,
    );
}

#[test]
fn tooltip_signal_for_binned_field_with_custom_title() {
    let result = tooltip_ref_for_encoding(
        &json!({
            "x": {"bin": "binned", "field": "bin_IMDB_rating", "type": "quantitative", "title": "IMDB_Rating (binned)"},
            "x2": {"field": "bin_IMDB_rating_end"}
        }),
        None,
        &default_config(),
        false,
    )
    .unwrap();
    assert_eq_json(
        json!({"signal": format!(
            "{{\"IMDB_Rating (binned)\": !isValid(datum[\"bin_IMDB_rating\"]) || !isFinite(+datum[\"bin_IMDB_rating\"]) ? \"null\" : format(datum[\"bin_IMDB_rating\"], \"\") + \"{BIN_RANGE_DELIMITER}\" + format(datum[\"bin_IMDB_rating_end\"], \"\")}}"
        )}),
        result,
    );
}

#[test]
fn excludes_secondary_binned_field_when_primary_hides_tooltip() {
    let result = tooltip_ref_for_encoding(
        &json!({
            "x": {"bin": "binned", "field": "bin_IMDB_rating", "type": "quantitative", "tooltip": false},
            "x2": {"field": "bin_IMDB_rating_end"},
            "y": {"field": "count", "type": "quantitative"}
        }),
        None,
        &default_config(),
        false,
    )
    .unwrap();
    assert_eq_json(json!({"signal": "{\"count\": format(datum[\"count\"], \"\")}"}), result);
}

#[test]
fn null_when_all_tooltip_fields_filtered_out_runtime() {
    let result = tooltip_ref_for_encoding(
        &json!({
            "tooltip": [
                {"field": "type1", "type": "quantitative", "filter": {"not": {"equal": 0}}},
                {"field": "type2", "type": "quantitative", "filter": {"valid": true}}
            ]
        }),
        None,
        &default_config(),
        false,
    )
    .unwrap();
    assert_eq_json(
        json!({"signal": "((!(datum[\"type1\"]===0)) || (isValid(datum[\"type2\"]) && isFinite(+datum[\"type2\"]))) ? merge((!(datum[\"type1\"]===0)) ? {\"type1\": format(datum[\"type1\"], \"\")} : {}, (isValid(datum[\"type2\"]) && isFinite(+datum[\"type2\"])) ? {\"type2\": format(datum[\"type2\"], \"\")} : {}) : null"}),
        result,
    );
}

#[test]
fn comparison_operator_filters() {
    let cases = [
        ("eq", json!({"equal": 10}), "datum[\"eq\"]===10"),
        ("lt", json!({"lt": 10}), "datum[\"lt\"]<10"),
        ("lte", json!({"lte": 10}), "datum[\"lte\"]<=10"),
        ("gte", json!({"gte": 10}), "datum[\"gte\"]>=10"),
    ];
    for (field, filter, test) in cases {
        let result = tooltip_ref_for_encoding(
            &json!({"tooltip": [{"field": field, "type": "quantitative", "filter": filter}]}),
            None,
            &default_config(),
            false,
        )
        .unwrap();
        assert_eq_json(
            json!({"signal": filtered_tooltip_signal(field, test)}),
            result,
        );
    }
}

#[test]
fn null_value_filters() {
    let cases = [
        ("missing", json!({"equal": null}), "datum[\"missing\"]===null"),
        ("present", json!({"not": {"equal": null}}), "!(datum[\"present\"]===null)"),
    ];
    for (field, filter, test) in cases {
        let result = tooltip_ref_for_encoding(
            &json!({"tooltip": [{"field": field, "type": "quantitative", "filter": filter}]}),
            None,
            &default_config(),
            false,
        )
        .unwrap();
        assert_eq_json(
            json!({"signal": filtered_tooltip_signal(field, test)}),
            result,
        );
    }
}

#[test]
fn composed_predicates() {
    let cases = [
        ("between", json!({"and": [{"gt": 0}, {"lt": 10}]}), "(datum[\"between\"]>0) && (datum[\"between\"]<10)"),
        ("oneOrTwo", json!({"or": [{"equal": 1}, {"equal": 2}]}), "(datum[\"oneOrTwo\"]===1) || (datum[\"oneOrTwo\"]===2)"),
    ];
    for (field, filter, test) in cases {
        let result = tooltip_ref_for_encoding(
            &json!({"tooltip": [{"field": field, "type": "quantitative", "filter": filter}]}),
            None,
            &default_config(),
            false,
        )
        .unwrap();
        assert_eq_json(
            json!({"signal": filtered_tooltip_signal(field, test)}),
            result,
        );
    }
}

#[test]
fn range_one_of_and_valid_predicates() {
    let cases = [
        ("ranged", json!({"range": [1, 10]}), "inrange(datum[\"ranged\"], [1, 10])"),
        ("bucket", json!({"oneOf": ["A", "B"]}), "indexof([\"A\",\"B\"], datum[\"bucket\"]) !== -1"),
        ("invalid", json!({"valid": false}), "!isValid(datum[\"invalid\"]) || !isFinite(+datum[\"invalid\"])"),
    ];
    for (field, filter, test) in cases {
        let result = tooltip_ref_for_encoding(
            &json!({"tooltip": [{"field": field, "type": "quantitative", "filter": filter}]}),
            None,
            &default_config(),
            false,
        )
        .unwrap();
        assert_eq_json(
            json!({"signal": filtered_tooltip_signal(field, test)}),
            result,
        );
    }
}

#[test]
fn aggregate_field_filters_use_aggregated_datum() {
    let result = tooltip_ref_for_encoding(
        &json!({"tooltip": [{"aggregate": "mean", "field": "agg", "type": "quantitative", "filter": {"gt": 10}}]}),
        None,
        &default_config(),
        false,
    )
    .unwrap();
    assert_eq_json(
        json!({"signal": "(datum[\"mean_agg\"]>10) ? {\"Mean of agg\": format(datum[\"mean_agg\"], \"\")} : null"}),
        result,
    );
}

#[test]
fn binned_field_filters_use_bin_start_field() {
    let result = tooltip_ref_for_encoding(
        &json!({"tooltip": [{"bin": true, "field": "binval", "type": "quantitative", "filter": {"lt": 5}}]}),
        None,
        &default_config(),
        false,
    )
    .unwrap();
    assert_eq_json(
        json!({"signal": format!(
            "(datum[\"bin_maxbins_10_binval\"]<5) ? {{\"binval (binned)\": !isValid(datum[\"bin_maxbins_10_binval\"]) || !isFinite(+datum[\"bin_maxbins_10_binval\"]) ? \"null\" : format(datum[\"bin_maxbins_10_binval\"], \"\") + \"{BIN_RANGE_DELIMITER}\" + format(datum[\"bin_maxbins_10_binval_end\"], \"\")}} : null"
        )}),
        result,
    );
}

#[test]
fn time_unit_filter_uses_time_unit_datum_field() {
    let result = tooltip_ref_for_encoding(
        &json!({"tooltip": [{"field": "date", "timeUnit": "month", "type": "ordinal", "filter": {"equal": {"month": "jan"}}}]}),
        None,
        &default_config(),
        false,
    )
    .unwrap();
    let signal = result["signal"].as_str().unwrap();
    assert!(signal.contains("(time(datum[\"month_date\"])===time(datetime(2012, 0, 1, 0, 0, 0, 0))) ? {\"date (month)\": "), "{signal}");
    assert!(!signal.contains("datum[\"date\"]"));
}

#[test]
fn omits_tooltip_array_entry_with_tooltip_false_even_with_filter() {
    let result = tooltip_ref_for_encoding(
        &json!({
            "tooltip": [
                {"field": "shown", "type": "quantitative"},
                {"field": "hidden", "type": "quantitative", "tooltip": false, "filter": {"gt": 0}}
            ]
        }),
        None,
        &default_config(),
        false,
    )
    .unwrap();
    assert_eq_json(json!({"signal": "{\"shown\": format(datum[\"shown\"], \"\")}"}), result);
}

#[test]
fn unsupported_tooltip_filters_warn_and_are_dropped() {
    let (result, logger) = vega_lite::log::capture(|| {
        tooltip_ref_for_encoding(
            &json!({
                "tooltip": [
                    {"field": "unsupported", "type": "quantitative", "filter": "missing"},
                    {"field": "operator", "type": "quantitative", "filter": {"operator": "===", "value": 0}},
                    {"field": "emptyAnd", "type": "quantitative", "filter": {"and": []}},
                    {"field": "emptyOr", "type": "quantitative", "filter": {"or": []}}
                ]
            }),
            None,
            &default_config(),
            false,
        )
    });
    assert_eq_json(
        json!({"signal": "{\"unsupported\": format(datum[\"unsupported\"], \"\"), \"operator\": format(datum[\"operator\"], \"\"), \"emptyAnd\": format(datum[\"emptyAnd\"], \"\"), \"emptyOr\": format(datum[\"emptyOr\"], \"\")}"}),
        result.unwrap(),
    );
    assert_eq!(logger.warns.len(), 4);
}

#[test]
fn count_aggregate_filters_use_count_field() {
    let result = tooltip_ref_for_encoding(
        &json!({"tooltip": [{"aggregate": "count", "type": "quantitative", "filter": {"gt": 100}}]}),
        None,
        &default_config(),
        false,
    )
    .unwrap();
    assert_eq_json(
        json!({"signal": "(datum[\"__count\"]>100) ? {\"Count of Records\": format(datum[\"__count\"], \"\")} : null"}),
        result,
    );
}

#[test]
fn argmax_filter_warns() {
    let (result, logger) = vega_lite::log::capture(|| {
        tooltip_ref_for_encoding(
            &json!({"tooltip": [{"aggregate": {"argmax": "y"}, "field": "x", "type": "quantitative", "filter": {"gt": 0}}]}),
            None,
            &default_config(),
            false,
        )
    });
    assert_eq_json(
        json!({"signal": "{\"x for max y\": format(datum[\"argmax_y\"][\"x\"], \"\")}"}),
        result.unwrap(),
    );
    assert_eq!(logger.warns.len(), 1);
}

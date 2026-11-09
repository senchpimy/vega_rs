//! Port of `test/channeldef.test.ts`.

use serde_json::{json, Value};
use vega_lite::channeldef::*;
use vega_lite::channeldef_support::{default_config, COUNTING_OPS, CHANNELS};
use vega_lite::log;

fn vg_field_default(field_def: Value) -> String {
    vg_field(&field_def)
}

fn vg_field_expr(field_def: Value, expr: &str) -> String {
    vg_field_with(
        &field_def,
        &FieldRefOption {
            expr: Some(expr.to_string()),
            ..Default::default()
        },
    )
}

// ---------------------------------------------------------------------------
// fieldDef / vgField()
// ---------------------------------------------------------------------------

#[test]
fn vg_field_should_access_flattened_fields() {
    assert_eq!(vg_field_default(json!({"field": "foo.bar\\.baz"})), "foo\\.bar\\.baz");
}

#[test]
fn vg_field_should_access_flattened_fields_in_expression() {
    assert_eq!(
        vg_field_expr(json!({"field": "foo.bar\\.baz"}), "datum"),
        "datum[\"foo.bar.baz\"]"
    );
}

#[test]
fn vg_field_should_access_argmin_argmax_fields_in_expression() {
    assert_eq!(
        vg_field_default(json!({"aggregate": {"argmin": "b"}, "field": "a"})),
        "argmin_b[\"a\"]"
    );
    assert_eq!(
        vg_field_default(json!({"aggregate": {"argmax": "b"}, "field": "a"})),
        "argmax_b[\"a\"]"
    );
}

#[test]
fn vg_field_should_support_argmin_argmax_field_names_with_space() {
    assert_eq!(
        vg_field_expr(json!({"aggregate": {"argmin": "foo bar"}, "field": "bar baz"}), "datum"),
        "datum[\"argmin_foo bar\"][\"bar baz\"]"
    );
    assert_eq!(
        vg_field_expr(json!({"aggregate": {"argmax": "foo bar"}, "field": "bar baz"}), "datum"),
        "datum[\"argmax_foo bar\"][\"bar baz\"]"
    );
}

#[test]
fn vg_field_should_support_exponential_operations() {
    assert_eq!(
        vg_field_expr(json!({"aggregate": {"exponential": 0.23}, "field": "a"}), "datum"),
        "datum[\"exponential_a\"]"
    );
    assert_eq!(
        vg_field_default(json!({"op": {"exponential": 0.54}, "field": "a", "as": "b"})),
        "exponential_a"
    );
}

#[test]
fn vg_field_should_support_exponentialb_operations() {
    assert_eq!(
        vg_field_expr(json!({"aggregate": {"exponentialb": 0.23}, "field": "a"}), "datum"),
        "datum[\"exponentialb_a\"]"
    );
    assert_eq!(
        vg_field_default(json!({"op": {"exponentialb": 0.54}, "field": "a", "as": "b"})),
        "exponentialb_a"
    );
}

#[test]
fn vg_field_should_support_prefix_and_field_names_with_space() {
    assert_eq!(
        vg_field_with(
            &json!({"field": "foo bar"}),
            &FieldRefOption {
                prefix: Some("prefix".to_string()),
                ..Default::default()
            }
        ),
        "prefix_foo bar"
    );
}

#[test]
fn vg_field_should_support_suffix_and_field_names_with_space() {
    assert_eq!(
        vg_field_with(
            &json!({"field": "foo bar"}),
            &FieldRefOption {
                suffix: Some("suffix".to_string()),
                ..Default::default()
            }
        ),
        "foo bar_suffix"
    );
}

#[test]
fn vg_field_should_support_fields_with_space_in_datum() {
    assert_eq!(
        vg_field_expr(json!({"field": "foo bar"}), "datum"),
        "datum[\"foo bar\"]"
    );
}

// ---------------------------------------------------------------------------
// defaultType()
// ---------------------------------------------------------------------------

#[test]
fn default_type_should_return_temporal_if_there_is_time_unit() {
    assert_eq!(
        default_type(&json!({"timeUnit": "month", "field": "a"}), "x"),
        "temporal"
    );
}

#[test]
fn default_type_should_return_ordinal_if_there_is_a_custom_sort_order() {
    assert_eq!(default_type(&json!({"field": "a", "sort": [1, 2, 3]}), "x"), "ordinal");
    assert_eq!(
        default_type(&json!({"timeUnit": "month", "field": "a", "sort": [1, 2, 3]}), "x"),
        "ordinal"
    );
}

#[test]
fn default_type_should_return_nominal_if_the_field_is_sorted_by_another_field() {
    assert_eq!(default_type(&json!({"field": "a", "sort": "-x"}), "x"), "nominal");
    assert_eq!(
        default_type(&json!({"field": "a", "sort": {"encoding": "x"}}), "x"),
        "nominal"
    );
    assert_eq!(
        default_type(&json!({"field": "a", "sort": {"field": "x"}}), "x"),
        "nominal"
    );
}

#[test]
fn default_type_should_return_quantitative_if_there_is_bin() {
    assert_eq!(default_type(&json!({"bin": true, "field": "a"}), "x"), "quantitative");
}

#[test]
fn default_type_should_return_quantitative_for_latitude_longitude() {
    assert_eq!(default_type(&json!({"field": "a"}), "latitude"), "quantitative");
    assert_eq!(default_type(&json!({"field": "a"}), "longitude"), "quantitative");
}

#[test]
fn default_type_should_return_nominal_for_row_column_facet_even_with_bin_or_time_unit() {
    for c in ["row", "column", "facet"] {
        assert_eq!(default_type(&json!({"field": "a"}), c), "nominal");
        assert_eq!(default_type(&json!({"bin": true, "field": "a"}), c), "nominal");
        assert_eq!(
            default_type(&json!({"timeUnit": "month", "field": "a"}), c),
            "nominal"
        );
    }
}

#[test]
fn default_type_should_return_quantitative_for_quantitative_or_discretizing_scale_type() {
    assert_eq!(
        default_type(&json!({"field": "a", "scale": {"type": "log"}}), "x"),
        "quantitative"
    );
    assert_eq!(
        default_type(&json!({"field": "a", "scale": {"type": "pow"}}), "x"),
        "quantitative"
    );
    assert_eq!(
        default_type(&json!({"field": "a", "scale": {"type": "quantile"}}), "x"),
        "quantitative"
    );
}

#[test]
fn default_type_should_return_temporal_for_time_scale_type() {
    assert_eq!(
        default_type(&json!({"field": "a", "scale": {"type": "utc"}}), "x"),
        "temporal"
    );
    assert_eq!(
        default_type(&json!({"field": "a", "scale": {"type": "time"}}), "x"),
        "temporal"
    );
}

#[test]
fn default_type_should_return_quantitative_if_there_is_aggregate() {
    assert_eq!(
        default_type(&json!({"aggregate": "mean", "field": "a"}), "x"),
        "quantitative"
    );
}

#[test]
fn default_type_should_return_nominal_if_there_is_argmin() {
    assert_eq!(
        default_type(&json!({"aggregate": {"argmax": "b"}, "field": "a"}), "x"),
        "nominal"
    );
}

#[test]
fn default_type_should_return_ordinal_for_order_channels() {
    assert_eq!(default_type(&json!({"field": "foo"}), "order"), "ordinal");
}

#[test]
fn default_type_should_return_nominal_by_default() {
    assert_eq!(default_type(&json!({"field": "a"}), "x"), "nominal");
}

// ---------------------------------------------------------------------------
// initChannelDef()
// ---------------------------------------------------------------------------

#[test]
fn init_channel_def_should_convert_primitive_type_to_value_def() {
    let (result, logger) = log::capture(|| init_channel_def(&json!(5), "x", &default_config()));
    assert_eq!(result, json!({"value": 5}));
    assert_eq!(logger.warns.len(), 1);
}

#[test]
fn init_channel_def_should_return_field_def_with_full_type_name() {
    let field_def = json!({"field": "a", "type": "q"});
    assert_eq!(
        init_channel_def(&field_def, "x", &default_config()),
        json!({"field": "a", "type": "quantitative"})
    );
}

#[test]
fn init_channel_def_should_standardize_non_string_field_to_string() {
    let field_def = json!({"field": 1, "type": "q"});
    assert_eq!(
        init_channel_def(&field_def, "x", &default_config()),
        json!({"field": "1", "type": "quantitative"})
    );
}

#[test]
fn init_channel_def_converts_header_orient_to_label_orient_and_title_orient() {
    let field_def = json!({"field": 1, "type": "nominal", "header": {"orient": "bottom"}});
    assert_eq!(
        init_channel_def(&field_def, "row", &default_config()),
        json!({
            "field": "1",
            "type": "nominal",
            "header": {"labelOrient": "bottom", "titleOrient": "bottom"}
        })
    );
}

#[test]
fn init_channel_def_should_replace_other_type_with_quantitative_for_counting_aggregate() {
    let (result, logger) = log::capture(|| {
        let mut results = Vec::new();
        for aggregate in COUNTING_OPS {
            let field_def = json!({"aggregate": aggregate, "field": "a", "type": "nominal"});
            results.push(init_channel_def(&field_def, "x", &default_config()));
        }
        results
    });
    for (aggregate, result) in COUNTING_OPS.iter().zip(result.iter()) {
        assert_eq!(
            *result,
            json!({"aggregate": aggregate, "field": "a", "type": "quantitative"})
        );
    }
    assert_eq!(logger.warns.len(), 4);
}

#[test]
fn init_channel_def_should_return_field_def_with_default_type() {
    let field_def = json!({"field": "a"});
    assert_eq!(
        init_channel_def(&field_def, "x", &default_config()),
        json!({"field": "a", "type": "nominal"})
    );
}

#[test]
fn init_channel_def_should_drop_invalid_aggregate_ops_and_throw_warning() {
    let (result, logger) = log::capture(|| {
        let field_def = json!({"aggregate": "boxplot", "field": "a", "type": "quantitative"});
        init_channel_def(&field_def, "x", &default_config())
    });
    assert_eq!(result, json!({"field": "a", "type": "quantitative"}));
    assert_eq!(
        logger.warns[0],
        json!(log::message::invalid_aggregate("boxplot"))
    );
}

#[test]
fn init_channel_def_should_drop_invalid_custom_format_type_and_throw_warning() {
    let (result, logger) = log::capture(|| {
        let field_def = json!({"field": "a", "type": "quantitative", "formatType": "foo", "format": ".2f"});
        init_channel_def(&field_def, "text", &default_config())
    });
    assert_eq!(result, json!({"field": "a", "type": "quantitative"}));
    assert_eq!(logger.warns[0], json!("Config.customFormatTypes is not true, thus custom format type and format for channel text are dropped."));
}

#[test]
fn init_channel_def_should_drop_invalid_custom_format_type_for_x_encoding_and_throw_warning() {
    let (result, logger) = log::capture(|| {
        let field_def = json!({"field": "a", "type": "quantitative", "axis": {"formatType": "foo", "format": ".2f"}});
        init_channel_def(&field_def, "x", &default_config())
    });
    assert_eq!(
        result,
        json!({"field": "a", "type": "quantitative", "axis": {}})
    );
    assert_eq!(logger.warns[0], json!("Config.customFormatTypes is not true, thus custom format type and format for channel x are dropped."));
}

#[test]
fn init_channel_def_should_drop_invalid_custom_format_type_for_row_encoding_and_throw_warning() {
    let (result, logger) = log::capture(|| {
        let field_def = json!({"field": "a", "type": "quantitative", "header": {"formatType": "foo", "format": ".2f"}});
        init_channel_def(&field_def, "row", &default_config())
    });
    assert_eq!(
        result,
        json!({"field": "a", "type": "quantitative", "header": {}})
    );
    assert_eq!(logger.warns[0], json!("Config.customFormatTypes is not true, thus custom format type and format for channel row are dropped."));
}

#[test]
fn init_channel_def_should_drop_invalid_custom_format_type_for_color_encoding_and_throw_warning() {
    let (result, logger) = log::capture(|| {
        let field_def = json!({"field": "a", "type": "quantitative", "legend": {"formatType": "foo", "format": ".2f"}});
        init_channel_def(&field_def, "color", &default_config())
    });
    assert_eq!(
        result,
        json!({"field": "a", "type": "quantitative", "legend": {}})
    );
    assert_eq!(logger.warns[0], json!("Config.customFormatTypes is not true, thus custom format type and format for channel color are dropped."));
}

// ---------------------------------------------------------------------------
// channelCompatability
// ---------------------------------------------------------------------------

#[test]
fn channel_compatibility_row_column_is_incompatible_with_continuous_field() {
    for channel in ["row", "column"] {
        assert!(!channel_compatibility(&json!({"field": "a", "type": "quantitative"}), channel).compatible);
    }
}

#[test]
fn channel_compatibility_row_column_is_compatible_with_discrete_field() {
    for channel in ["row", "column"] {
        assert!(channel_compatibility(&json!({"field": "a", "type": "nominal"}), channel).compatible);
    }
}

#[test]
fn channel_compatibility_xy_color_text_detail_is_compatible_with_continuous_field() {
    for channel in ["x", "y", "color", "text", "detail"] {
        assert!(channel_compatibility(&json!({"field": "a", "type": "quantitative"}), channel).compatible);
    }
}

#[test]
fn channel_compatibility_xy_color_text_detail_is_compatible_with_discrete_field() {
    for channel in ["x", "y", "color", "text", "detail"] {
        assert!(channel_compatibility(&json!({"field": "a", "type": "nominal"}), channel).compatible);
    }
}

#[test]
fn channel_compatibility_opacity_size_x2_y2_is_compatible_with_continuous_field() {
    for channel in ["opacity", "size", "x2", "y2"] {
        assert!(channel_compatibility(&json!({"field": "a", "type": "quantitative"}), channel).compatible);
    }
}

#[test]
fn channel_compatibility_opacity_size_x2_y2_is_compatible_with_binned_field() {
    for channel in ["opacity", "size", "x2", "y2"] {
        assert!(channel_compatibility(
            &json!({"bin": true, "field": "a", "type": "quantitative"}),
            channel
        )
        .compatible);
    }
}

#[test]
fn channel_compatibility_opacity_size_x2_y2_is_incompatible_with_nominal_field() {
    for channel in ["opacity", "size", "x2", "y2"] {
        assert!(!channel_compatibility(&json!({"field": "a", "type": "nominal"}), channel).compatible);
    }
}

#[test]
fn channel_compatibility_shape_is_compatible_with_nominal_field() {
    assert!(channel_compatibility(&json!({"field": "a", "type": "nominal"}), "shape").compatible);
}

#[test]
fn channel_compatibility_shape_is_compatible_with_ordinal_field() {
    assert!(channel_compatibility(&json!({"field": "a", "type": "ordinal"}), "shape").compatible);
}

#[test]
fn channel_compatibility_shape_is_incompatible_with_quantitative_field() {
    assert!(!channel_compatibility(&json!({"field": "a", "type": "quantitative"}), "shape").compatible);
}

#[test]
fn channel_compatibility_shape_is_the_only_channel_incompatible_with_geojson_field() {
    for channel in CHANNELS {
        assert_eq!(
            channel_compatibility(&json!({"field": "a", "type": "geojson"}), channel).compatible,
            *channel == "shape"
        );
    }
}

#[test]
fn channel_compatibility_order_is_incompatible_with_nominal_field() {
    assert!(!channel_compatibility(&json!({"field": "a", "type": "nominal"}), "order").compatible);
}

#[test]
fn channel_compatibility_order_is_compatible_with_ordinal_field() {
    assert!(channel_compatibility(&json!({"field": "a", "type": "ordinal"}), "order").compatible);
}

#[test]
fn channel_compatibility_order_is_compatible_with_quantitative_field() {
    assert!(channel_compatibility(&json!({"field": "a", "type": "quantitative"}), "order").compatible);
}

// ---------------------------------------------------------------------------
// functionalTitleFormatter
// ---------------------------------------------------------------------------

#[test]
fn functional_title_formatter_should_return_correct_title_for_argmin() {
    assert_eq!(
        functional_title_formatter(&json!({"field": "f", "aggregate": {"argmin": "a"}})),
        "f for argmin(a)"
    );
}

#[test]
fn functional_title_formatter_should_return_correct_title_for_aggregate() {
    assert_eq!(
        functional_title_formatter(&json!({"field": "f", "aggregate": {"argmax": "a"}})),
        "f for argmax(a)"
    );
}

#[test]
fn functional_title_formatter_should_return_correct_title_for_exponential() {
    assert_eq!(
        functional_title_formatter(&json!({"field": "f", "aggregate": {"exponential": 0.5}})),
        "EXPONENTIAL(f)"
    );
}

#[test]
fn functional_title_formatter_should_return_correct_title_for_exponentialb() {
    assert_eq!(
        functional_title_formatter(&json!({"field": "f", "aggregate": {"exponentialb": 0.5}})),
        "EXPONENTIALB(f)"
    );
}

// ---------------------------------------------------------------------------
// defaultTitle()
// ---------------------------------------------------------------------------

#[test]
fn default_title_should_return_correct_title_for_aggregate() {
    assert_eq!(default_title(&json!({"field": "f", "aggregate": "mean"}), &json!({})), "Mean of f");
}

#[test]
fn default_title_should_return_correct_title_for_argmin() {
    assert_eq!(
        default_title(&json!({"field": "f", "aggregate": {"argmin": "a"}}), &json!({})),
        "f for min a"
    );
}

#[test]
fn default_title_should_return_correct_title_for_argmax() {
    assert_eq!(
        default_title(&json!({"field": "f", "aggregate": {"argmax": "a"}}), &json!({})),
        "f for max a"
    );
}

#[test]
fn default_title_should_return_correct_title_for_exponential() {
    assert_eq!(
        default_title(&json!({"field": "f", "aggregate": {"exponential": 0.5}}), &json!({})),
        "Exponential of f"
    );
}

#[test]
fn default_title_should_return_correct_title_for_exponentialb() {
    assert_eq!(
        default_title(&json!({"field": "f", "aggregate": {"exponentialb": 0.5}}), &json!({})),
        "Exponentialb of f"
    );
}

#[test]
fn default_title_should_return_correct_title_for_count() {
    assert_eq!(
        default_title(&json!({"aggregate": "count"}), &json!({"countTitle": "baz!"})),
        "baz!"
    );
}

#[test]
fn default_title_should_return_correct_title_for_bin() {
    let field_def = json!({"field": "f", "type": "quantitative", "bin": true});
    assert_eq!(default_title(&field_def, &json!({})), "f (binned)");
}

#[test]
fn default_title_should_return_correct_functional_title_for_bin() {
    let field_def = json!({"field": "f", "type": "quantitative", "bin": true});
    assert_eq!(
        default_title(&field_def, &json!({"fieldTitle": "functional"})),
        "BIN(f)"
    );
}

#[test]
fn default_title_should_return_correct_title_for_time_unit_month() {
    let field_def = json!({"field": "f", "type": "temporal", "timeUnit": "month"});
    assert_eq!(default_title(&field_def, &json!({})), "f (month)");
}

#[test]
fn default_title_should_return_correct_title_for_time_unit_yearmonthdate() {
    let field_def = json!({"field": "f", "type": "temporal", "timeUnit": "yearmonthdate"});
    assert_eq!(default_title(&field_def, &json!({})), "f (year-month-date)");
}

#[test]
fn default_title_should_return_correct_title_for_time_unit_day() {
    let field_def = json!({"field": "f", "type": "temporal", "timeUnit": "day"});
    assert_eq!(default_title(&field_def, &json!({})), "f (day)");
}

#[test]
fn default_title_should_return_correct_title_for_time_unit_yearquarter() {
    let field_def = json!({"field": "f", "type": "temporal", "timeUnit": "yearquarter"});
    assert_eq!(default_title(&field_def, &json!({})), "f (year-quarter)");
}

#[test]
fn default_title_should_return_correct_title_for_raw_field() {
    let field_def = json!({"field": "f", "type": "temporal"});
    assert_eq!(default_title(&field_def, &json!({})), "f");
}

// ---------------------------------------------------------------------------
// isDiscrete()
// ---------------------------------------------------------------------------

#[test]
fn is_discrete_returns_true_for_nominal_field_def() {
    assert!(is_discrete(&json!({"field": "f", "type": "nominal"})));
}

#[test]
fn is_discrete_returns_true_for_ordinal_field_def() {
    assert!(is_discrete(&json!({"field": "f", "type": "ordinal"})));
}

#[test]
fn is_discrete_returns_true_for_geojson_field_def() {
    assert!(is_discrete(&json!({"field": "f", "type": "geojson"})));
}

#[test]
fn is_discrete_returns_false_for_temporal_field_def() {
    assert!(!is_discrete(&json!({"field": "f", "type": "temporal"})));
}

#[test]
fn is_discrete_returns_false_for_unbinned_quantitative_field_def() {
    assert!(!is_discrete(&json!({"field": "f", "type": "quantitative"})));
}

#[test]
fn is_discrete_returns_true_for_binned_quantitative_field_def_bin_true() {
    assert!(is_discrete(&json!({"field": "f", "type": "quantitative", "bin": true})));
}

#[test]
fn is_discrete_returns_true_for_pre_binned_quantitative_field_def() {
    assert!(is_discrete(&json!({"field": "f", "type": "quantitative", "bin": "binned"})));
}

#[test]
fn is_discrete_returns_true_for_quantitative_field_def_with_bin_params() {
    assert!(is_discrete(&json!({"field": "f", "type": "quantitative", "bin": {"maxbins": 10}})));
}

#[test]
fn is_discrete_returns_false_for_quantitative_datum_def() {
    assert!(!is_discrete(&json!({"datum": 1, "type": "quantitative"})));
}

#[test]
fn is_discrete_returns_true_for_nominal_datum_def() {
    assert!(is_discrete(&json!({"datum": "a", "type": "nominal"})));
}

#[test]
fn is_discrete_throws_for_a_def_without_explicit_type() {
    let result = std::panic::catch_unwind(|| is_discrete(&json!({"datum": 1})));
    assert!(result.is_err());
    let payload = result.unwrap_err();
    let message = payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_default();
    assert_eq!(message, "Invalid field type \"undefined\".");
}

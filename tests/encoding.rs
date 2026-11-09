//! Port of `test/encoding.test.ts`.

use serde_json::{json, Map, Value};
use vega_lite::channel::{
    COLOR, DETAIL, FILLOPACITY, OPACITY, SIZE, STROKEOPACITY, STROKEWIDTH, UNIT_CHANNELS, X2, Y2,
};
use vega_lite::channeldef::is_position_field_or_datum_def;
use vega_lite::channeldef_support::default_config;
use vega_lite::encoding::{
    channel_has_nested_offset_scale, dropping_color, extract_transforms_from_encoding, field_defs,
    init_encoding, mark_channel_compatible, offset_nested_inside_continuous_position_scale_dropped,
    path_grouping_fields, ExtractTransformsOutput, REPLACE_ANGLE_WITH_THETA,
};
use vega_lite::log;
use vega_lite::mark::{Mark, CIRCLE, POINT, SQUARE, TICK};
use vega_lite::util::internal_field;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn init(encoding: Value, mark: Mark, filled: Option<bool>) -> Value {
    init_encoding(&encoding, mark, filled, &default_config())
}

fn extract(encoding: Value) -> ExtractTransformsOutput {
    extract_transforms_from_encoding(&encoding, &default_config())
}

fn grouping(mark: &str, encoding: Value) -> Vec<String> {
    path_grouping_fields(mark, &encoding)
}

fn names(fields: &[&str]) -> Vec<String> {
    fields.iter().map(|field| field.to_string()).collect()
}

/// Renders an [`ExtractTransformsOutput`] the way the upstream object literal
/// does, so a whole result can be compared against `json!`.
fn extract_value(output: ExtractTransformsOutput) -> Value {
    json!({
        "bins": output.bins,
        "timeUnits": output.time_units,
        "aggregate": output.aggregate,
        "groupby": output.groupby,
        "encoding": output.encoding,
    })
}

// ---------------------------------------------------------------------------
// initEncoding
// ---------------------------------------------------------------------------

#[test]
fn should_drop_color_channel_if_fill_is_specified_and_filled_is_true() {
    let (encoding, logger) = log::capture(|| {
        init(
            json!({
                "color": {"field": "a", "type": "quantitative"},
                "fill": {"field": "b", "type": "quantitative"},
            }),
            Mark::Bar,
            Some(true),
        )
    });

    assert_eq!(
        encoding,
        json!({"fill": {"field": "b", "type": "quantitative"}})
    );
    assert_eq!(
        logger.warns[0],
        json!(dropping_color("encoding", true, false))
    );
}

#[test]
fn should_replace_angle_channel_for_arc_marks_with_theta() {
    let (encoding, logger) = log::capture(|| {
        init(
            json!({
                "color": {"field": "a", "type": "quantitative"},
                "angle": {"field": "b", "type": "quantitative"},
            }),
            Mark::Arc,
            None,
        )
    });

    assert_eq!(
        encoding,
        json!({
            "color": {"field": "a", "type": "quantitative"},
            "theta": {"field": "b", "type": "quantitative"},
        })
    );
    assert_eq!(logger.warns[0], json!(REPLACE_ANGLE_WITH_THETA));
}

#[test]
fn should_drop_color_channel_if_stroke_is_specified_and_filled_is_false() {
    let (encoding, logger) = log::capture(|| {
        init(
            json!({
                "color": {"field": "a", "type": "quantitative"},
                "stroke": {"field": "b", "type": "quantitative"},
            }),
            Mark::Point,
            Some(false),
        )
    });

    assert_eq!(
        encoding,
        json!({"stroke": {"field": "b", "type": "quantitative"}})
    );
    assert_eq!(
        logger.warns[0],
        json!(dropping_color("encoding", false, true))
    );
}

#[test]
fn drops_x_offset_if_x_is_continuous() {
    let (encoding, logger) = log::capture(|| {
        init(
            json!({
                "x": {"field": "a", "type": "quantitative"},
                "xOffset": {"field": "b", "type": "quantitative"},
            }),
            Mark::Point,
            Some(false),
        )
    });

    assert_eq!(
        encoding,
        json!({"x": {"field": "a", "type": "quantitative"}})
    );
    assert_eq!(
        logger.warns[0],
        json!(offset_nested_inside_continuous_position_scale_dropped("x"))
    );
}

#[test]
fn drops_x_offset_if_x_is_binned_quantitative() {
    let (encoding, logger) = log::capture(|| {
        init(
            json!({
                "x": {"field": "a", "type": "quantitative", "bin": true},
                "xOffset": {"field": "b", "type": "nominal"},
            }),
            Mark::Point,
            Some(false),
        )
    });

    assert_eq!(
        encoding,
        json!({"x": {"field": "a", "type": "quantitative", "bin": {"maxbins": 10}}})
    );
    assert_eq!(
        logger.warns[0],
        json!(offset_nested_inside_continuous_position_scale_dropped("x"))
    );
}

#[test]
fn does_not_drop_x_offset_if_x_is_time_with_time_unit() {
    let encoding = init(
        json!({
            "x": {"field": "a", "type": "temporal", "timeUnit": "year"},
            "xOffset": {"field": "b", "type": "nominal"},
        }),
        Mark::Point,
        Some(false),
    );

    assert_eq!(
        encoding,
        json!({
            "x": {"field": "a", "type": "temporal", "timeUnit": {"unit": "year"}},
            "xOffset": {"field": "b", "type": "nominal"},
        })
    );
}

// ---------------------------------------------------------------------------
// extractTransformsFromEncoding
// ---------------------------------------------------------------------------

#[test]
fn should_include_axis_in_extracted_encoding() {
    let encoding = extract(json!({
        "x": {"field": "dose", "type": "ordinal", "axis": {"labelAngle": 15}},
        "y": {"field": "response", "type": "quantitative"},
    }))
    .encoding;

    let x = &encoding["x"];
    assert!(!x.is_null());
    assert!(is_position_field_or_datum_def(x));
    let axis = x.get("axis");
    assert!(axis.is_some());
    assert_eq!(x["axis"]["labelAngle"], json!(15));
}

#[test]
fn should_extract_time_unit_from_encoding_field_definition_and_add_axis_format() {
    let output = extract(init(
        json!({
            "x": {"timeUnit": "yearmonthdatehoursminutes", "field": "a", "type": "temporal"},
            "y": {"field": "b", "type": "quantitative"},
        }),
        Mark::Line,
        Some(false),
    ));

    assert_eq!(
        extract_value(output),
        json!({
            "bins": [],
            "timeUnits": [{"timeUnit": {"unit": "yearmonthdatehoursminutes"}, "field": "a", "as": "yearmonthdatehoursminutes_a"}],
            "aggregate": [],
            "groupby": ["yearmonthdatehoursminutes_a", "b"],
            "encoding": {
                "x": {
                    "field": "yearmonthdatehoursminutes_a",
                    "type": "temporal",
                    "title": "a (year-month-date-hours-minutes)",
                },
                "y": {"field": "b", "type": "quantitative"},
            },
        })
    );
}

#[test]
fn should_produce_format_and_format_type_in_axis_when_there_is_time_unit() {
    let output = extract(init(
        json!({
            "x": {"field": "a", "type": "quantitative"},
            "y": {"timeUnit": "year", "field": "b", "type": "ordinal"},
        }),
        Mark::Line,
        Some(false),
    ));

    assert_eq!(
        output.encoding["y"],
        json!({
            "axis": {"formatType": "time"},
            "field": "year_b",
            "title": "b (year)",
            "type": "ordinal",
        })
    );
}

#[test]
fn should_not_produce_format_type_in_axis_when_there_is_time_unit_with_type_temporal() {
    let output = extract(init(
        json!({
            "x": {"field": "a", "type": "quantitative"},
            "y": {"timeUnit": "year", "field": "b", "type": "temporal"},
        }),
        Mark::Line,
        Some(false),
    ));

    assert_eq!(
        output.encoding["y"],
        json!({
            "field": "year_b",
            "title": "b (year)",
            "type": "temporal",
        })
    );
}

#[test]
fn should_produce_format_and_format_type_in_legend_when_there_is_time_unit() {
    let output = extract(init(
        json!({
            "x": {"field": "a", "type": "quantitative"},
            "y": {"field": "b", "type": "ordinal"},
            "detail": {"field": "c", "timeUnit": "month", "type": "nominal"},
        }),
        Mark::Line,
        Some(false),
    ));

    assert_eq!(
        output.encoding["detail"],
        json!({
            "legend": {"formatType": "time"},
            "field": "month_c",
            "title": "c (month)",
            "type": "nominal",
        })
    );
}

#[test]
fn should_not_produce_format_type_in_legend_when_there_is_time_unit_with_type_temporal() {
    let output = extract(init(
        json!({
            "x": {"field": "a", "type": "quantitative"},
            "y": {"field": "b", "type": "ordinal"},
            "detail": {"field": "c", "timeUnit": "month", "type": "temporal"},
        }),
        Mark::Line,
        Some(false),
    ));

    assert_eq!(
        output.encoding["detail"],
        json!({
            "field": "month_c",
            "title": "c (month)",
            "type": "temporal",
        })
    );
}

#[test]
fn should_produce_format_and_format_type_when_there_is_time_unit_in_tooltip_or_text_channel() {
    let output = extract(init(
        json!({
            "x": {"field": "a", "type": "quantitative"},
            "y": {"field": "b", "type": "ordinal"},
            "tooltip": {"field": "c", "timeUnit": "month", "type": "nominal"},
            "text": {"field": "c", "timeUnit": "month", "type": "nominal"},
        }),
        Mark::Text,
        Some(false),
    ));

    assert_eq!(
        output.encoding["tooltip"],
        json!({
            "formatType": "time",
            "field": "month_c",
            "title": "c (month)",
            "type": "nominal",
        })
    );
    assert_eq!(
        output.encoding["text"],
        json!({
            "formatType": "time",
            "field": "month_c",
            "title": "c (month)",
            "type": "nominal",
        })
    );
}

#[test]
fn should_extract_aggregates_from_encoding() {
    let output = extract(init(
        json!({
            "x": {"field": "a", "type": "quantitative"},
            "y": {"aggregate": "max", "field": "b", "type": "quantitative"},
        }),
        Mark::Line,
        Some(false),
    ));

    assert_eq!(
        extract_value(output),
        json!({
            "bins": [],
            "timeUnits": [],
            "aggregate": [{"op": "max", "field": "b", "as": "max_b"}],
            "groupby": ["a"],
            "encoding": {
                "x": {"field": "a", "type": "quantitative"},
                "y": {
                    "field": "max_b",
                    "type": "quantitative",
                    "title": "Max of b",
                },
            },
        })
    );
}

#[test]
fn should_extract_aggregates_with_exponential_operations_from_encoding() {
    let output = extract(init(
        json!({
            "x": {"field": "a", "type": "quantitative"},
            "y": {"aggregate": {"exponential": 0.3}, "field": "b", "type": "quantitative"},
        }),
        Mark::Line,
        Some(false),
    ));

    assert_eq!(
        extract_value(output),
        json!({
            "bins": [],
            "timeUnits": [],
            "aggregate": [{"op": {"exponential": 0.3}, "field": "b", "as": "exponential_b"}],
            "groupby": ["a"],
            "encoding": {
                "x": {"field": "a", "type": "quantitative"},
                "y": {
                    "field": "exponential_b",
                    "type": "quantitative",
                    "title": "Exponential of b",
                },
            },
        })
    );
}

#[test]
fn should_extract_aggregates_with_exponentialb_operations_from_encoding() {
    let output = extract(init(
        json!({
            "x": {"field": "a", "type": "quantitative"},
            "y": {"aggregate": {"exponentialb": 0.3}, "field": "b", "type": "quantitative"},
        }),
        Mark::Line,
        Some(false),
    ));

    assert_eq!(
        extract_value(output),
        json!({
            "bins": [],
            "timeUnits": [],
            "aggregate": [{"op": {"exponentialb": 0.3}, "field": "b", "as": "exponentialb_b"}],
            "groupby": ["a"],
            "encoding": {
                "x": {"field": "a", "type": "quantitative"},
                "y": {
                    "field": "exponentialb_b",
                    "type": "quantitative",
                    "title": "Exponentialb of b",
                },
            },
        })
    );
}

#[test]
fn should_extract_binning_from_encoding() {
    let output = extract(init(
        json!({
            "x": {"field": "a", "type": "ordinal", "bin": true},
            "y": {"type": "quantitative", "aggregate": "count"},
        }),
        Mark::Bar,
        Some(true),
    ));

    assert_eq!(
        extract_value(output),
        json!({
            "bins": [{"bin": {"maxbins": 10}, "field": "a", "as": "bin_maxbins_10_a"}],
            "timeUnits": [],
            "aggregate": [{"op": "count", "as": internal_field("count")}],
            "groupby": [
                "bin_maxbins_10_a",
                "bin_maxbins_10_a_end",
                "bin_maxbins_10_a_range",
            ],
            "encoding": {
                "x": {
                    "field": "bin_maxbins_10_a",
                    "type": "quantitative",
                    "title": "a (binned)",
                    "bin": "binned",
                },
                "x2": {"field": "bin_maxbins_10_a_end"},
                "y": {
                    "field": internal_field("count"),
                    "type": "quantitative",
                    "title": "Count of Records",
                },
            },
        })
    );
}

#[test]
fn should_preserve_auxiliary_properties_ie_axis_in_encoding() {
    let output = extract(init(
        json!({
            "x": {"field": "a", "type": "quantitative"},
            "y": {
                "aggregate": "mean",
                "field": "b",
                "type": "quantitative",
                "title": "foo",
                "axis": {"title": "foo", "format": ".2e"},
            },
        }),
        Mark::Line,
        Some(false),
    ));

    assert_eq!(
        extract_value(output),
        json!({
            "bins": [],
            "timeUnits": [],
            "aggregate": [{"op": "mean", "field": "b", "as": "mean_b"}],
            "groupby": ["a"],
            "encoding": {
                "x": {"field": "a", "type": "quantitative"},
                "y": {
                    "field": "mean_b",
                    "type": "quantitative",
                    "title": "foo",
                    "axis": {"title": "foo", "format": ".2e"},
                },
            },
        })
    );
}

// ---------------------------------------------------------------------------
// markChannelCompatible
// ---------------------------------------------------------------------------

#[test]
fn should_support_x2_for_circle_point_square_and_tick_mark_with_binned_data() {
    let encoding = json!({
        "x": {
            "field": "bin_start",
            "bin": "binned",
            "type": "quantitative",
            "axis": {"tickMinStep": 2},
        },
        "x2": {"field": "bin_end"},
        "y": {"field": "count", "type": "quantitative"},
    });
    assert!(mark_channel_compatible(&encoding, X2, &CIRCLE));
    assert!(mark_channel_compatible(&encoding, X2, &POINT));
    assert!(mark_channel_compatible(&encoding, X2, &SQUARE));
    assert!(mark_channel_compatible(&encoding, X2, &TICK));
}

#[test]
fn should_support_y2_for_circle_point_square_and_tick_mark_with_binned_data() {
    let encoding = json!({
        "y": {
            "field": "bin_start",
            "bin": "binned",
            "type": "quantitative",
            "axis": {"tickMinStep": 2},
        },
        "y2": {"field": "bin_end"},
        "x": {"field": "count", "type": "quantitative"},
    });
    assert!(mark_channel_compatible(&encoding, Y2, &CIRCLE));
    assert!(mark_channel_compatible(&encoding, Y2, &POINT));
    assert!(mark_channel_compatible(&encoding, Y2, &SQUARE));
    assert!(mark_channel_compatible(&encoding, Y2, &TICK));
}

#[test]
fn should_not_support_x2_for_circle_point_square_and_tick_mark_without_binned_data() {
    let encoding = json!({
        "x": {
            "field": "bin_start",
            "type": "quantitative",
            "axis": {"tickMinStep": 2},
        },
        "x2": {"field": "bin_end"},
        "y": {"field": "count", "type": "quantitative"},
    });
    assert!(!mark_channel_compatible(&encoding, X2, &CIRCLE));
    assert!(!mark_channel_compatible(&encoding, X2, &POINT));
    assert!(!mark_channel_compatible(&encoding, X2, &SQUARE));
    assert!(!mark_channel_compatible(&encoding, X2, &TICK));
}

#[test]
fn should_not_support_y2_for_circle_point_square_and_tick_mark_with_binned_data() {
    let encoding = json!({
        "y": {
            "field": "bin_start",
            "type": "quantitative",
            "axis": {"tickMinStep": 2},
        },
        "y2": {"field": "bin_end"},
        "x": {"field": "count", "type": "quantitative"},
    });
    assert!(!mark_channel_compatible(&encoding, Y2, &CIRCLE));
    assert!(!mark_channel_compatible(&encoding, Y2, &POINT));
    assert!(!mark_channel_compatible(&encoding, Y2, &SQUARE));
    assert!(!mark_channel_compatible(&encoding, Y2, &TICK));
}

// ---------------------------------------------------------------------------
// pathGroupingFields()
// ---------------------------------------------------------------------------

#[test]
fn should_return_fields_for_unaggregate_detail_color_size_opacity_field_defs() {
    for channel in [
        DETAIL,
        COLOR,
        SIZE,
        OPACITY,
        FILLOPACITY,
        STROKEOPACITY,
        STROKEWIDTH,
    ] {
        let mut encoding = Map::new();
        encoding.insert(
            channel.to_string(),
            json!({"field": "a", "type": "nominal"}),
        );
        assert_eq!(
            grouping("line", Value::Object(encoding)),
            names(&["a"]),
            "channel {channel}"
        );
    }
}

#[test]
fn should_not_return_a_field_for_size_of_a_trail_mark() {
    assert_eq!(
        grouping("trail", json!({"size": {"field": "a", "type": "nominal"}})),
        names(&[])
    );
}

#[test]
fn should_not_return_fields_for_aggregate_detail_color_size_opacity_field_defs() {
    for channel in [
        DETAIL,
        COLOR,
        SIZE,
        OPACITY,
        FILLOPACITY,
        STROKEOPACITY,
        STROKEWIDTH,
    ] {
        let mut encoding = Map::new();
        encoding.insert(
            channel.to_string(),
            json!({"aggregate": "mean", "field": "a", "type": "nominal"}),
        );
        assert_eq!(
            grouping("line", Value::Object(encoding)),
            names(&[]),
            "channel {channel}"
        );
    }
}

#[test]
fn should_return_condition_detail_fields_for_color_size_shape() {
    for channel in [
        COLOR,
        SIZE,
        OPACITY,
        FILLOPACITY,
        STROKEOPACITY,
        STROKEWIDTH,
    ] {
        let mut encoding = Map::new();
        encoding.insert(
            channel.to_string(),
            json!({"condition": {"param": "sel", "field": "a", "type": "nominal"}}),
        );
        assert_eq!(
            grouping("line", Value::Object(encoding)),
            names(&["a"]),
            "channel {channel}"
        );
    }
}

#[test]
fn should_not_return_errors_for_all_channels() {
    for channel in UNIT_CHANNELS {
        let mut encoding = Map::new();
        encoding.insert(
            channel.to_string(),
            json!({"field": "a", "type": "nominal"}),
        );
        // Must not panic.
        let _ = grouping("line", Value::Object(encoding));
    }
}

#[test]
fn should_not_include_fields_from_tooltip() {
    assert_eq!(
        grouping(
            "line",
            json!({"tooltip": {"field": "a", "type": "nominal"}})
        ),
        names(&[])
    );
}

#[test]
fn should_not_group_line_area_trail_by_the_main_channel_when_using_an_offset_field() {
    for mark in ["line", "area", "trail"] {
        assert_ne!(
            grouping(
                mark,
                json!({
                    "x": {"field": "a", "type": "nominal"},
                    "y": {"field": "c", "type": "nominal"},
                    "yOffset": {"field": "b", "type": "quantitative"},
                })
            ),
            names(&["c"])
        );
        assert_ne!(
            grouping(
                mark,
                json!({
                    "x": {"field": "a", "type": "nominal"},
                    "y": {"field": "c", "type": "nominal"},
                    "xOffset": {"field": "b", "type": "quantitative"},
                })
            ),
            names(&["a"])
        );
    }
}

#[test]
fn should_group_by_an_explicit_detail_field_when_using_an_offset_field() {
    for mark in ["line", "area", "trail"] {
        assert_eq!(
            grouping(
                mark,
                json!({
                    "x": {"field": "a", "type": "nominal"},
                    "y": {"field": "c", "type": "nominal"},
                    "yOffset": {"field": "b", "aggregate": "sum", "type": "quantitative"},
                    "detail": {"field": "c", "type": "nominal"},
                })
            ),
            names(&["c"])
        );
    }
}

// ---------------------------------------------------------------------------
// fieldDefs
// ---------------------------------------------------------------------------

#[test]
fn field_defs_should_return_field_defs() {
    assert_eq!(
        field_defs(&json!({
            "x": {"field": "foo", "type": "quantitative"},
            "color": {
                "condition": {
                    "test": "datum.val > 12",
                    "field": "bar",
                    "type": "quantitative",
                },
                "value": "red",
            },
        })),
        vec![
            json!({"field": "foo", "type": "quantitative"}),
            json!({"field": "bar", "test": "datum.val > 12", "type": "quantitative"}),
        ]
    );
}

// ---------------------------------------------------------------------------
// channelHasNestedOffsetScale
// ---------------------------------------------------------------------------

#[test]
fn returns_true_for_nominal_x_with_x_offset() {
    assert!(channel_has_nested_offset_scale(
        &json!({"x": {"field": "a", "type": "nominal"}, "xOffset": {"field": "b", "type": "nominal"}}),
        "x",
    ));
}

#[test]
fn returns_true_for_ordinal_x_with_x_offset() {
    assert!(channel_has_nested_offset_scale(
        &json!({"x": {"field": "a", "type": "ordinal"}, "xOffset": {"field": "b", "type": "nominal"}}),
        "x",
    ));
}

#[test]
fn returns_false_for_binned_quantitative_x_with_x_offset() {
    assert!(!channel_has_nested_offset_scale(
        &json!({"x": {"field": "a", "type": "quantitative", "bin": true}, "xOffset": {"field": "b", "type": "nominal"}}),
        "x",
    ));
}

#[test]
fn returns_false_for_unbinned_quantitative_x_with_x_offset() {
    assert!(!channel_has_nested_offset_scale(
        &json!({"x": {"field": "a", "type": "quantitative"}, "xOffset": {"field": "b", "type": "nominal"}}),
        "x",
    ));
}

#[test]
fn returns_false_when_offset_channel_is_missing() {
    assert!(!channel_has_nested_offset_scale(
        &json!({"x": {"field": "a", "type": "nominal"}}),
        "x",
    ));
}

#[test]
fn returns_true_for_temporal_x_with_time_unit_and_x_offset() {
    assert!(channel_has_nested_offset_scale(
        &json!({"x": {"field": "a", "type": "temporal", "timeUnit": "year"}, "xOffset": {"field": "b", "type": "nominal"}}),
        "x",
    ));
}

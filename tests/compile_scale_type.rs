//! Port of `test/compile/scale/type.test.ts`.

use serde_json::{json, Value};

use vega_lite::channel::{range_type, RangeType, RADIUS, SCALE_CHANNELS, THETA, X, Y};
use vega_lite::compile::scale::type_::scale_type;
use vega_lite::log;
use vega_lite::mark::PRIMITIVE_MARKS;
use vega_lite::scale::ScaleType;
use vega_lite::timeunit::{
    is_utc_time_unit, LOCAL_MULTI_TIMEUNIT_INDEX, LOCAL_SINGLE_TIMEUNIT_INDEX,
    UTC_MULTI_TIMEUNIT_INDEX, UTC_SINGLE_TIMEUNIT_INDEX,
};

fn st(
    specified_scale: Value,
    channel: &str,
    field_def: Value,
    mark: &str,
) -> Option<ScaleType> {
    scale_type(
        &specified_scale,
        channel,
        &field_def,
        &json!({"type": mark}),
        false,
    )
}

fn ts(t: ScaleType) -> Value {
    json!({"type": t.as_str()})
}

fn time_units() -> Vec<&'static str> {
    let mut units: Vec<&'static str> = Vec::new();
    for u in LOCAL_SINGLE_TIMEUNIT_INDEX
        .iter()
        .chain(UTC_SINGLE_TIMEUNIT_INDEX.iter())
        .chain(LOCAL_MULTI_TIMEUNIT_INDEX.iter())
        .chain(UTC_MULTI_TIMEUNIT_INDEX.iter())
    {
        if !units.contains(u) {
            units.push(u);
        }
    }
    units
}

#[test]
fn should_show_warning_if_users_try_to_override_the_scale_and_use_bin() {
    let (result, logger) = log::capture(|| {
        (
            st(json!({"type": "point"}), "color", json!({"type": "quantitative", "field": "x", "bin": true}), "point"),
            st(json!({}), "color", json!({"type": "nominal"}), "point"),
        )
    });
    assert_eq!(result.0, Some(ScaleType::BinOrdinal));
    assert_eq!(
        logger.warns[0].as_str().unwrap(),
        "FieldDef does not work with \"point\" scale. We are using \"bin-ordinal\" scale instead."
    );
}

#[test]
fn should_return_ordinal_scale_for_nominal_data_by_default() {
    assert_eq!(
        st(json!({}), "color", json!({"type": "nominal"}), "point"),
        Some(ScaleType::Ordinal)
    );
}

#[test]
fn should_return_ordinal_scale_for_ordinal_data() {
    // Note: upstream test uses nominal here as well.
    assert_eq!(
        st(json!({}), "color", json!({"type": "nominal"}), "point"),
        Some(ScaleType::Ordinal)
    );
}

#[test]
fn should_return_ordinal_for_nominal_field() {
    assert_eq!(
        st(json!({}), "shape", json!({"type": "nominal"}), "point"),
        Some(ScaleType::Ordinal)
    );
}

#[test]
fn should_return_ordinal_even_if_other_type_is_specified() {
    let bad_types = [ScaleType::Linear, ScaleType::Band, ScaleType::Point];
    let (results, logger) = log::capture(|| {
        bad_types
            .iter()
            .map(|bad| {
                st(ts(*bad), "shape", json!({"field": "x", "type": "nominal"}), "point")
            })
            .collect::<Vec<_>>()
    });
    for (i, bad) in bad_types.iter().enumerate() {
        assert_eq!(results[i], Some(ScaleType::Ordinal));
        assert_eq!(
            logger.warns[i].as_str().unwrap(),
            vega_lite::compile::support::scale_type_not_work_with_channel(
                "shape",
                bad.as_str(),
                "ordinal"
            )
        );
    }
    assert_eq!(logger.warns.len(), 3);
}

#[test]
fn should_return_ordinal_for_an_ordinal_field_and_throw_a_warning() {
    let (result, logger) = log::capture(|| {
        st(json!({}), "shape", json!({"type": "ordinal"}), "point")
    });
    assert_eq!(result, Some(ScaleType::Ordinal));
    assert_eq!(
        logger.warns[0].as_str().unwrap(),
        "Using discrete channel \"shape\" to encode \"ordinal\" field can be misleading as it does not encode order."
    );
}

#[test]
fn should_return_point_scale_for_ordinal_xy_for_other_marks() {
    for mark in PRIMITIVE_MARKS {
        let mark = mark.as_str();
        if ["bar", "rule", "rect", "image", "arc", "tick"].contains(&mark) {
            continue;
        }
        for t in ["ordinal", "nominal"] {
            for channel in [X, Y] {
                assert_eq!(
                    st(json!({}), channel, json!({"type": t}), mark),
                    Some(ScaleType::Point),
                    "mark={mark} channel={channel} type={t}"
                );
            }
        }
    }
}

#[test]
fn should_return_band_scale_for_ordinal_xy_when_mark_is_rect_rule_bar() {
    for t in ["ordinal", "nominal"] {
        for channel in [X, Y] {
            for mark in ["bar", "rule", "rect"] {
                assert_eq!(
                    st(json!({}), channel, json!({"type": t}), mark),
                    Some(ScaleType::Band)
                );
            }
        }
    }
}

#[test]
fn should_return_band_scale_for_ordinal_theta_radius_when_mark_is_arc() {
    for t in ["ordinal", "nominal"] {
        for channel in [RADIUS, THETA] {
            assert_eq!(
                st(json!({}), channel, json!({"type": t}), "arc"),
                Some(ScaleType::Band)
            );
        }
    }
}

#[test]
fn should_return_point_scale_for_xy_when_mark_is_point() {
    for t in ["ordinal", "nominal"] {
        for channel in [X, Y] {
            assert_eq!(
                st(json!({}), channel, json!({"type": t}), "point"),
                Some(ScaleType::Point)
            );
        }
    }
}

#[test]
fn should_return_point_scale_for_xy_when_mark_is_point_when_ordinal_scale_is_specified() {
    let cases: Vec<(&str, &str)> = ["ordinal", "nominal"]
        .iter()
        .flat_map(|t| [X, Y].iter().map(move |c| (*c, *t)))
        .collect();
    let (results, logger) = log::capture(|| {
        cases
            .iter()
            .map(|(channel, t)| {
                st(json!({"type": "ordinal"}), channel, json!({"type": t}), "point")
            })
            .collect::<Vec<_>>()
    });
    for (i, (channel, _)) in cases.iter().enumerate() {
        assert_eq!(results[i], Some(ScaleType::Point));
        assert_eq!(
            logger.warns[i].as_str().unwrap(),
            vega_lite::compile::support::scale_type_not_work_with_channel(
                channel, "ordinal", "point"
            )
        );
    }
    assert_eq!(logger.warns.len(), 4);
}

#[test]
fn should_return_point_scale_for_ordinal_nominal_on_other_continuous_channels() {
    let other_continuous: Vec<&str> = SCALE_CHANNELS
        .iter()
        .copied()
        .filter(|c| range_type(c) == Some(RangeType::Continuous) && !(*c == X || *c == Y))
        .collect();
    for mark in PRIMITIVE_MARKS {
        for t in ["ordinal", "nominal"] {
            for channel in &other_continuous {
                assert_eq!(
                    st(json!({}), channel, json!({"type": t}), mark.as_str()),
                    Some(ScaleType::Point)
                );
            }
        }
    }
}

#[test]
fn should_return_band_scale_for_nominal_ordinal_time_by_default() {
    assert_eq!(
        st(json!({}), "time", json!({"type": "nominal"}), "point"),
        Some(ScaleType::Band)
    );
    assert_eq!(
        st(json!({}), "time", json!({"type": "ordinal"}), "point"),
        Some(ScaleType::Band)
    );
}

#[test]
fn should_return_time_scale_for_temporal_color_field_by_default() {
    assert_eq!(
        st(json!({}), "color", json!({"type": "temporal"}), "point"),
        Some(ScaleType::Time)
    );
}

#[test]
fn should_return_ordinal_for_temporal_field_on_shape_and_warn() {
    let (result, logger) = log::capture(|| {
        st(
            json!({}),
            "shape",
            json!({"type": "temporal", "timeUnit": "yearmonth"}),
            "point",
        )
    });
    assert_eq!(result, Some(ScaleType::Ordinal));
    assert_eq!(
        logger.warns[0].as_str().unwrap(),
        "Using discrete channel \"shape\" to encode \"temporal\" field can be misleading as it does not encode magnitude."
    );
}

#[test]
fn should_return_time_for_all_non_utc_time_units() {
    for time_unit in time_units().into_iter().filter(|t| !is_utc_time_unit(t)) {
        assert_eq!(
            st(json!({}), Y, json!({"type": "temporal", "field": "x", "timeUnit": time_unit}), "point"),
            Some(ScaleType::Time),
            "timeUnit={time_unit}"
        );
    }
}

#[test]
fn should_return_utc_for_all_utc_time_units() {
    for time_unit in time_units().into_iter().filter(|t| is_utc_time_unit(t)) {
        assert_eq!(
            st(json!({}), Y, json!({"type": "temporal", "field": "x", "timeUnit": time_unit}), "point"),
            Some(ScaleType::Utc),
            "timeUnit={time_unit}"
        );
    }
}

#[test]
fn should_return_band_scale_for_temporal_data_on_time_channel() {
    assert_eq!(
        st(json!({}), "time", json!({"type": "temporal"}), "point"),
        Some(ScaleType::Band)
    );
}

#[test]
fn should_return_linear_scale_for_quantitative_color_field_by_default() {
    assert_eq!(
        st(json!({}), "color", json!({"type": "quantitative"}), "point"),
        Some(ScaleType::Linear)
    );
}

#[test]
fn should_return_linear_scale_for_piecewise_quantitative_color_field() {
    assert_eq!(
        st(
            json!({"domain": [1, 2, 3], "range": ["red", "green", "blue"]}),
            "color",
            json!({"type": "quantitative"}),
            "point"
        ),
        Some(ScaleType::Linear)
    );
}

#[test]
fn should_return_ordinal_bin_scale_for_quantitative_color_field_with_binning() {
    assert_eq!(
        st(
            json!({}),
            "color",
            json!({"type": "quantitative", "field": "x", "bin": true}),
            "point"
        ),
        Some(ScaleType::BinOrdinal)
    );
}

#[test]
fn should_return_ordinal_for_quantitative_on_discrete_channel_and_warn() {
    let (result, logger) = log::capture(|| {
        st(json!({}), "shape", json!({"type": "quantitative"}), "point")
    });
    assert_eq!(result, Some(ScaleType::Ordinal));
    assert_eq!(
        logger.warns[0].as_str().unwrap(),
        "Using discrete channel \"shape\" to encode \"quantitative\" field can be misleading as it does not encode magnitude."
    );
}

#[test]
fn should_return_linear_scale_for_quantitative_by_default() {
    assert_eq!(
        st(json!({}), "x", json!({"type": "quantitative", "field": "x"}), "point"),
        Some(ScaleType::Linear)
    );
}

#[test]
fn should_return_linear_scale_for_quantitative_even_if_binned() {
    assert_eq!(
        st(
            json!({}),
            "opacity",
            json!({"type": "quantitative", "field": "x", "bin": true}),
            "point"
        ),
        Some(ScaleType::Linear)
    );
}

#[test]
fn should_return_linear_scale_for_quantitative_x_and_y() {
    assert_eq!(
        st(
            json!({}),
            "x",
            json!({"type": "quantitative", "field": "x", "bin": true}),
            "point"
        ),
        Some(ScaleType::Linear)
    );
}

#[test]
fn should_return_band_scale_for_quantitative_data_on_time_channel() {
    assert_eq!(
        st(json!({}), "time", json!({"type": "quantitative"}), "point"),
        Some(ScaleType::Band)
    );
}

#[test]
fn should_return_specified_value_if_data_type_is_ordinal_or_nominal_and_scale_type_is_ordinal() {
    assert_eq!(
        st(ts(ScaleType::Ordinal), "shape", json!({"type": "nominal"}), "point"),
        Some(ScaleType::Ordinal)
    );
}

#[test]
fn should_return_default_scale_type_if_data_type_is_temporal_but_scale_type_is_not_time_or_utc() {
    let (_, logger) = log::capture(|| {
        assert_eq!(
            st(
                ts(ScaleType::Linear),
                "x",
                json!({"type": "temporal", "field": "x", "timeUnit": "year"}),
                "point"
            ),
            Some(ScaleType::Time)
        );
        assert_eq!(
            st(
                ts(ScaleType::Linear),
                "color",
                json!({"type": "temporal", "field": "x", "timeUnit": "year"}),
                "point"
            ),
            Some(ScaleType::Time)
        );
    });
    assert_eq!(
        logger.warns[0].as_str().unwrap(),
        "FieldDef does not work with \"linear\" scale. We are using \"time\" scale instead."
    );
}

#[test]
fn should_return_time_if_data_type_is_temporal_but_scale_type_is_discrete() {
    let (result, logger) = log::capture(|| {
        st(
            ts(ScaleType::Point),
            "x",
            json!({"type": "temporal", "field": "x", "timeUnit": "year"}),
            "point",
        )
    });
    assert_eq!(result, Some(ScaleType::Time));
    assert_eq!(
        logger.warns[0].as_str().unwrap(),
        "FieldDef does not work with \"point\" scale. We are using \"time\" scale instead."
    );
}

#[test]
fn should_return_utc_scale_type_if_data_type_is_temporal_and_scale_type_is_utc() {
    assert_eq!(
        st(
            ts(ScaleType::Utc),
            "x",
            json!({"type": "temporal", "field": "x", "timeUnit": "year"}),
            "point"
        ),
        Some(ScaleType::Utc)
    );
}

#[test]
fn should_return_default_scale_type_if_data_type_is_quantitative_and_scale_type_is_time() {
    let (result, logger) = log::capture(|| {
        st(ts(ScaleType::Time), "x", json!({"field": "x", "type": "quantitative"}), "point")
    });
    assert_eq!(result, Some(ScaleType::Linear));
    assert_eq!(
        logger.warns[0].as_str().unwrap(),
        "FieldDef does not work with \"time\" scale. We are using \"linear\" scale instead."
    );
}

#[test]
fn should_return_default_scale_type_if_data_type_is_temporal_and_scale_type_supports_quantitative() {
    assert_eq!(
        st(
            ts(ScaleType::Time),
            "x",
            json!({"field": "x", "type": "temporal"}),
            "point"
        ),
        Some(ScaleType::Time)
    );
}

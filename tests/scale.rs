//! Port of `test/scale.test.ts`.

use vega_lite::channel::{ScaleChannel, SCALE_CHANNELS};
use vega_lite::scale::*;
use vega_lite::util::some;

/// Port of `without` from `test/util.ts`.
fn without<T: PartialEq + Copy>(array: &[T], excluded: &[T]) -> Vec<T> {
    array
        .iter()
        .copied()
        .filter(|item| !excluded.contains(item))
        .collect()
}

#[test]
fn scale_type_support_property_should_have_at_least_one_supported_scale_type_for_all_properties() {
    for prop in SCALE_PROPERTIES {
        assert!(
            some(&SCALE_TYPES, |scale_type, _| {
                scale_type_support_property(*scale_type, prop)
            }),
            "No scale type supports scale property {prop}"
        );
    }
}

#[test]
fn scale_types_should_either_have_continuous_or_discrete_domain() {
    for scale_type in SCALE_TYPES {
        assert!(
            has_continuous_domain(scale_type) != has_discrete_domain(scale_type),
            "{scale_type} should have exactly one of continuous or discrete domain"
        );
    }
}

#[test]
fn channel_support_scale_type_should_have_at_least_one_supported_scale_type_for_all_channels() {
    for channel in SCALE_CHANNELS {
        assert!(
            some(&SCALE_TYPES, |scale_type, _| {
                channel_support_scale_type(channel, *scale_type, false)
            }),
            "No scale type is supported by channel {channel}"
        );
    }
}

#[test]
fn channel_support_scale_type_should_have_at_least_one_supported_channel_for_all_scale_types() {
    for scale_type in SCALE_TYPES {
        assert!(
            some(&SCALE_CHANNELS, |channel, _| {
                channel_support_scale_type(channel, scale_type, false)
            }),
            "No channel supports scale type {scale_type}"
        );
    }
}

#[test]
fn shape_should_support_only_ordinal_and_discretizing_scales() {
    let supported_scales = [vec![ScaleType::Ordinal], CONTINUOUS_TO_DISCRETE_SCALES.to_vec()].concat();
    for scale_type in &supported_scales {
        assert!(
            channel_support_scale_type("shape", *scale_type, false),
            "shape should support {scale_type}"
        );
    }
    let unsupported = without(&SCALE_TYPES, &supported_scales);
    for scale_type in unsupported {
        assert!(
            !channel_support_scale_type("shape", scale_type, false),
            "shape should not support {scale_type}"
        );
    }
}

#[test]
fn stroke_dash_should_support_only_ordinal_and_discretizing_scales() {
    let supported_scales = [vec![ScaleType::Ordinal], CONTINUOUS_TO_DISCRETE_SCALES.to_vec()].concat();
    for scale_type in &supported_scales {
        assert!(
            channel_support_scale_type("strokeDash", *scale_type, false),
            "strokeDash should support {scale_type}"
        );
    }
    let unsupported = without(&SCALE_TYPES, &supported_scales);
    for scale_type in unsupported {
        assert!(
            !channel_support_scale_type("strokeDash", scale_type, false),
            "strokeDash should not support {scale_type}"
        );
    }
}

#[test]
fn color_should_support_all_scale_types_except_band() {
    for scale_type in SCALE_TYPES {
        assert_eq!(
            channel_support_scale_type("color", scale_type, false),
            scale_type != ScaleType::Band,
            "unexpected support for {scale_type}"
        );
    }
}

#[test]
fn x_and_y_support_all_continuous_scale_types_as_well_as_band_and_point() {
    // x, y should use either band or point for ordinal input
    let scale_types = [
        CONTINUOUS_TO_CONTINUOUS_SCALES.to_vec(),
        vec![ScaleType::Band, ScaleType::Point],
    ]
    .concat();

    for channel in ["x", "y"] as [ScaleChannel; 2] {
        assert!(
            !channel_support_scale_type(channel, ScaleType::Ordinal, false),
            "{channel} should not support ordinal"
        );
        for scale_type in &scale_types {
            assert!(
                channel_support_scale_type(channel, *scale_type, false),
                "{channel} should support {scale_type}"
            );
        }
    }
}

#[test]
fn x_and_y_do_not_support_point_scale_if_there_is_a_nested_offset_scale() {
    for channel in ["x", "y"] as [ScaleChannel; 2] {
        assert!(
            !channel_support_scale_type(channel, ScaleType::Point, true),
            "{channel} should not support point with a nested offset scale"
        );
    }
}

#[test]
fn size_and_opacity_support_all_continuous_scale_types_as_well_as_band_and_point() {
    let scale_types = [
        CONTINUOUS_TO_CONTINUOUS_SCALES.to_vec(),
        vec![ScaleType::Band, ScaleType::Point],
    ]
    .concat();

    for channel in ["size", "opacity"] as [ScaleChannel; 2] {
        for scale_type in &scale_types {
            assert!(
                channel_support_scale_type(channel, *scale_type, false),
                "{channel} should support {scale_type}"
            );
        }
    }
}

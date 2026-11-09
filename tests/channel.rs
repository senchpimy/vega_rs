//! Port of `test/channel.test.ts`.

use vega_lite::channel::*;

/// Port of `without` from `test/util.ts`.
fn without(array: &[&'static str], excluded: &[&str]) -> Vec<&'static str> {
    array
        .iter()
        .copied()
        .filter(|item| !excluded.contains(item))
        .collect()
}

#[test]
fn unit_channels_should_be_channels_without_row_and_column() {
    assert_eq!(
        UNIT_CHANNELS.to_vec(),
        without(&CHANNELS, &[ROW, COLUMN, FACET])
    );
}

#[test]
fn single_def_channels_should_be_channels_without_detail_and_order() {
    assert_eq!(
        SINGLE_DEF_CHANNELS.to_vec(),
        without(&CHANNELS, &[DETAIL, ORDER, TOOLTIP])
    );
}

#[test]
fn scale_channels_should_be_unit_channels_without_ranges_and_non_scale_channels() {
    assert_eq!(
        SCALE_CHANNELS.to_vec(),
        without(
            &UNIT_CHANNELS,
            &[
                X2, Y2, "xError", "yError", "xError2", "yError2", THETA2, RADIUS2, LATITUDE,
                LONGITUDE, LATITUDE2, LONGITUDE2, ORDER, DETAIL, KEY, TEXT, TOOLTIP, HREF, URL,
                DESCRIPTION,
            ],
        )
    );
}

#[test]
fn nonposition_scale_channels_should_be_scale_channels_without_position_channels() {
    assert_eq!(
        NONPOSITION_SCALE_CHANNELS.to_vec(),
        without(
            &SCALE_CHANNELS,
            &[X, Y, XOFFSET, YOFFSET, THETA, RADIUS]
        )
    );
}

#[test]
fn is_scale_channel_should_return_true_for_all_scale_channels() {
    for channel in SCALE_CHANNELS {
        assert!(is_scale_channel(channel), "{channel} should be a scale channel");
    }
}

#[test]
fn range_type_should_be_defined_for_all_channels_without_throwing() {
    for c in CHANNELS {
        // Must not panic.
        let _ = range_type(c);
    }
}

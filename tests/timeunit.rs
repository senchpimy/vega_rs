//! Port of `test/timeunit.test.ts`.

use serde_json::json;
use vega_lite::timeunit::{
    contains_time_unit, field_expr, format_expression, is_binned_time_unit,
    BINNED_TIMEUNIT_INDEX, LOCAL_MULTI_TIMEUNIT_INDEX, TIMEUNIT_PARTS,
};

#[test]
fn should_return_true_for_time_unit_parts() {
    for part in TIMEUNIT_PARTS {
        assert!(contains_time_unit(part, part));
    }
}

#[test]
fn should_return_true_for_yearquarter_given_quarter() {
    assert!(contains_time_unit("yearquarter", "quarter"));
}

#[test]
fn should_return_true_for_seconds_and_milliseconds_given_secondsmilliseconds() {
    assert!(contains_time_unit("secondsmilliseconds", "seconds"));
}

#[test]
fn should_return_true_for_milliseconds_given_secondsmilliseconds() {
    assert!(contains_time_unit("secondsmilliseconds", "milliseconds"));
}

#[test]
fn should_return_false_for_quarter_given_year() {
    assert!(!contains_time_unit("year", "quarter"));
}

#[test]
fn should_return_false_for_seconds_given_milliseconds() {
    assert!(!contains_time_unit("milliseconds", "seconds"));
}

#[test]
fn should_return_false_for_day_given_dayofyear() {
    assert!(!contains_time_unit("dayofyear", "day"));
}

#[test]
fn should_return_false_for_year_given_dayofyear() {
    assert!(!contains_time_unit("dayofyear", "year"));
}

#[test]
fn should_return_correct_field_expression_for_yearmonthdatehoursminutesseconds() {
    assert_eq!(
        field_expr("yearmonthdatehoursminutesseconds", "x"),
        "datetime(year(datum[\"x\"]), month(datum[\"x\"]), date(datum[\"x\"]), hours(datum[\"x\"]), minutes(datum[\"x\"]), seconds(datum[\"x\"]), 0)"
    );
}

#[test]
fn should_return_correct_field_expression_for_quarter() {
    assert_eq!(
        field_expr("quarter", "x"),
        "datetime(2012, (quarter(datum[\"x\"])-1)*3, 1, 0, 0, 0, 0)"
    );
}

#[test]
fn should_return_correct_field_expression_for_day() {
    assert_eq!(
        field_expr("day", "x"),
        "datetime(2012, 0, day(datum[\"x\"])+1, 0, 0, 0, 0)"
    );
}

#[test]
fn should_return_correct_field_expression_for_milliseconds() {
    assert_eq!(
        field_expr("milliseconds", "x"),
        "datetime(2012, 0, 1, 0, 0, 0, milliseconds(datum[\"x\"]))"
    );
}

#[test]
fn should_return_correct_field_expression_for_monthdate() {
    assert_eq!(
        field_expr("monthdate", "x"),
        "datetime(2012, month(datum[\"x\"]), date(datum[\"x\"]), 0, 0, 0, 0)"
    );
}

#[test]
fn should_return_correct_field_expression_with_utc_for_milliseconds() {
    assert_eq!(
        field_expr("utcquarter", "x"),
        "datetime(2012, (utcquarter(datum[\"x\"])-1)*3, 1, 0, 0, 0, 0)"
    );
    assert_eq!(
        field_expr("utcmilliseconds", "x"),
        "datetime(2012, 0, 1, 0, 0, 0, utcmilliseconds(datum[\"x\"]))"
    );
}

#[test]
fn should_return_correct_template_for_yearmonthdatehoursminutesseconds() {
    assert_eq!(
        format_expression(Some("yearmonthdatehoursminutesseconds"), "datum.x", false).as_deref(),
        Some("timeFormat(datum.x, timeUnitSpecifier([\"year\",\"month\",\"date\",\"hours\",\"minutes\",\"seconds\"], {\"year-month\":\"%b %Y \",\"year-month-date\":\"%b %d, %Y \"}))")
    );
}

#[test]
fn should_return_correct_template_for_yearmonth_no_comma() {
    assert_eq!(
        format_expression(Some("yearmonth"), "datum.x", false).as_deref(),
        Some("timeFormat(datum.x, timeUnitSpecifier([\"year\",\"month\"], {\"year-month\":\"%b %Y \",\"year-month-date\":\"%b %d, %Y \"}))")
    );
}

#[test]
fn should_return_correct_template_for_day() {
    assert_eq!(
        format_expression(Some("day"), "datum.x", false).as_deref(),
        Some("timeFormat(datum.x, timeUnitSpecifier([\"day\"], {\"year-month\":\"%b %Y \",\"year-month-date\":\"%b %d, %Y \"}))")
    );
}

#[test]
fn should_return_correct_template_for_day_shortened() {
    assert_eq!(
        format_expression(Some("day"), "datum.x", false).as_deref(),
        Some("timeFormat(datum.x, timeUnitSpecifier([\"day\"], {\"year-month\":\"%b %Y \",\"year-month-date\":\"%b %d, %Y \"}))")
    );
}

#[test]
fn should_return_correct_template_for_quarter() {
    assert_eq!(
        format_expression(Some("quarter"), "datum.x", false).as_deref(),
        Some("timeFormat(datum.x, timeUnitSpecifier([\"quarter\"], {\"year-month\":\"%b %Y \",\"year-month-date\":\"%b %d, %Y \"}))")
    );
}

#[test]
fn should_return_correct_template_for_yearquarter() {
    assert_eq!(
        format_expression(Some("yearquarter"), "datum.x", false).as_deref(),
        Some("timeFormat(datum.x, timeUnitSpecifier([\"year\",\"quarter\"], {\"year-month\":\"%b %Y \",\"year-month-date\":\"%b %d, %Y \"}))")
    );
}

#[test]
fn should_return_correct_template_for_milliseconds() {
    assert_eq!(
        format_expression(Some("milliseconds"), "datum.x", false).as_deref(),
        Some("timeFormat(datum.x, timeUnitSpecifier([\"milliseconds\"], {\"year-month\":\"%b %Y \",\"year-month-date\":\"%b %d, %Y \"}))")
    );
}

#[test]
fn should_return_correct_template_for_no_time_unit() {
    assert_eq!(format_expression(None, "datum.x", false), None);
}

#[test]
fn should_return_correct_template_for_yearmonth_no_comma_with_utc_scale() {
    assert_eq!(
        format_expression(Some("yearmonth"), "datum.x", true).as_deref(),
        Some("utcFormat(datum.x, timeUnitSpecifier([\"year\",\"month\"], {\"year-month\":\"%b %Y \",\"year-month-date\":\"%b %d, %Y \"}))")
    );
}

#[test]
fn should_return_correct_template_for_utcyearmonth_no_comma() {
    assert_eq!(
        format_expression(Some("utcyearmonth"), "datum.x", true).as_deref(),
        Some("utcFormat(datum.x, timeUnitSpecifier([\"year\",\"month\"], {\"year-month\":\"%b %Y \",\"year-month-date\":\"%b %d, %Y \"}))")
    );
}

#[test]
fn should_return_true_for_binned_time_unit_strings() {
    for unit in BINNED_TIMEUNIT_INDEX {
        assert!(is_binned_time_unit(&json!(unit)), "expected {unit} to be binned");
    }
}

#[test]
fn should_return_true_for_binned_time_unit_params() {
    for unit in LOCAL_MULTI_TIMEUNIT_INDEX {
        assert!(
            is_binned_time_unit(&json!({"unit": unit, "binned": true})),
            "expected {unit} params to be binned"
        );
    }
}

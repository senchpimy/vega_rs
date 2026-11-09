//! Port of `test/datetime.test.ts`.

use serde_json::json;
use vega_lite::datetime::{date_time_to_expr, date_time_to_timestamp, is_date_time};
use vega_lite::log;

#[test]
fn correctly_classifies_unit_with_zero() {
    assert!(is_date_time(&json!({"hours": 0})));
}

#[test]
fn should_drop_day_if_day_is_combined_with_year_month_date() {
    let d = json!({
        "year": 2007,
        "day": "monday",
    });
    let (expr, logger) = log::capture(|| date_time_to_expr(d.clone()));
    assert_eq!(expr, "datetime(2007, 0, 1, 0, 0, 0, 0)");
    assert_eq!(logger.warns[0], json!(log::message::dropped_day(&d)));
}

#[test]
fn should_normalize_numeric_quarter_correctly() {
    let expr = date_time_to_expr(json!({"quarter": 2}));
    assert_eq!(expr, "datetime(2012, 3, 1, 0, 0, 0, 0)");
}

#[test]
fn should_log_warning_for_quarter_greater_than_four() {
    let (expr, logger) = log::capture(|| date_time_to_expr(json!({"quarter": 5})));
    assert_eq!(expr, "datetime(2012, 12, 1, 0, 0, 0, 0)");
    assert_eq!(
        logger.warns[0],
        json!(log::message::invalid_time_unit("quarter", &json!(5)))
    );
}

#[test]
#[should_panic]
fn should_throw_error_for_invalid_quarter() {
    date_time_to_expr(json!({"quarter": "Q"}));
}

#[test]
fn should_normalize_numeric_month_correctly() {
    let expr = date_time_to_expr(json!({"month": 1}));
    assert_eq!(expr, "datetime(2012, 0, 1, 0, 0, 0, 0)");
}

#[test]
fn should_normalize_month_name_correctly() {
    for month in ["January", "january", "Jan", "jan"] {
        assert_eq!(
            date_time_to_expr(json!({"month": month})),
            "datetime(2012, 0, 1, 0, 0, 0, 0)"
        );
    }
}

#[test]
#[should_panic]
fn should_throw_error_for_invalid_month() {
    date_time_to_expr(json!({"month": "J"}));
}

#[test]
fn should_normalize_numeric_day_of_week_correctly() {
    for day in [0, 7] {
        assert_eq!(
            date_time_to_expr(json!({"day": day})),
            "datetime(2012, 0, 1, 0, 0, 0, 0)"
        );
    }
}

#[test]
fn should_normalize_day_name_correctly_and_use_year_2006_to_ensure_correct() {
    for day in ["Sunday", "sunday", "Sun", "sun"] {
        assert_eq!(
            date_time_to_expr(json!({"day": day})),
            "datetime(2012, 0, 1, 0, 0, 0, 0)"
        );
    }
}

#[test]
#[should_panic]
fn should_throw_error_for_invalid_day() {
    date_time_to_expr(json!({"day": "S"}));
}

#[test]
fn should_use_utc_expression_if_utc_is_specified() {
    let expr = date_time_to_expr(json!({
        "year": 2007,
        "date": 1,
        "utc": true,
    }));
    assert_eq!(expr, "utc(2007, 0, 1, 0, 0, 0, 0)");
}

#[test]
fn should_return_date_as_timestamp_if_specified_with_month_number() {
    let d = json!({
        "year": 1970,
        "month": 1, // January
        "date": 1,
    });
    // Upstream compares with the local-time `new Date(1970, 0, 1, ...)`.
    // The example corpus uses TZ=America/Los_Angeles (UTC-8 in January).
    assert_eq!(date_time_to_timestamp(d), 28_800_000);
}

#[test]
fn should_return_date_as_timestamp_if_specified_with_month_name() {
    let d = json!({
        "year": 1970,
        "month": "January",
        "date": 1,
    });
    assert_eq!(date_time_to_timestamp(d), 28_800_000);
}

#[test]
fn should_use_utc_if_specified() {
    let d = json!({
        "year": 2007,
        "date": 1,
        "utc": true,
    });
    // +new Date(Date.UTC(2007, 0, 1, 0, 0, 0, 0))
    assert_eq!(date_time_to_timestamp(d), 1_167_609_600_000);
}

#[test]
fn should_support_accidental_use_of_strings() {
    let d = json!({
        "year": "2007",
        "month": "1",
        "date": "1",
    });
    // Upstream compares with local `new Date(2007, 0, 1, ...)` in
    // America/Los_Angeles (UTC-8 in January).
    assert_eq!(date_time_to_timestamp(d), 1_167_638_400_000);
}

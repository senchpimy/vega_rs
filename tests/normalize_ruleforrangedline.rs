//! Port of `test/normalize/ruleforrangedline.test.ts`.

use serde_json::json;
use vega_lite::log;
use vega_lite::normalize::normalize;

#[test]
fn correctly_normalizes_line_with_rule_where_there_is_x2_or_y2() {
    let spec = json!({
        "data": {"url": "data/stocks.csv", "format": {"type": "csv"}},
        "mark": "line",
        "encoding": {
            "x": {"field": "date", "type": "temporal"},
            "y": {"field": "price", "type": "quantitative"},
            "x2": {"field": "date2"}
        }
    });

    let (normalized, local_logger) = log::capture(|| normalize(&spec, None));
    let mut expected = spec.clone();
    expected["mark"] = json!("rule");
    assert_eq!(normalized.unwrap(), expected);
    assert_eq!(
        local_logger.warns[0],
        json!(
            "Line mark is for continuous lines and thus cannot be used with x2. We will use the rule mark (line segments) instead."
        )
    );
}

#[test]
fn does_not_normalize_line_when_there_is_x2_or_y2_but_primary_channel_is_binned() {
    let spec = json!({
        "data": {"url": "data/stocks.csv", "format": {"type": "csv"}},
        "mark": "line",
        "encoding": {
            "x": {"bin": "binned", "field": "x", "type": "quantitative"},
            "x2": {"field": "x2"},
            "y": {"field": "price", "type": "quantitative"}
        }
    });
    assert_eq!(normalize(&spec, None).unwrap(), spec);
}

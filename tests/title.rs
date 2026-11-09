//! Port of `test/title.test.ts`.

use serde_json::json;
use vega_lite::title::extract_title_config;

#[test]
fn extract_angle_and_limit_of_config_title_as_title_property() {
    let config = json!({"angle": 1, "limit": 5});
    let extracted = extract_title_config(config.as_object().unwrap());
    assert_eq!(
        serde_json::Value::Object(extracted.non_mark_title_properties),
        json!({"angle": 1, "limit": 5})
    );
}

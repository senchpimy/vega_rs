//! Port of `test/compile/axis/config.test.ts`.

use serde_json::json;

use vega_lite::compile::axis::config::get_axis_configs;
use vega_lite::scale::ScaleType;

#[test]
fn correctly_get_axis_types_for_orient_signal() {
    let config = json!({
        "axisTop": {"labelBaseline": "line-bottom", "labelPadding": 2},
        "axisBottom": {"labelBaseline": {"signal": "a"}},
    });
    let axis_configs = get_axis_configs("x", Some(ScaleType::Point), &json!({"signal": "a"}), &config);

    assert_eq!(
        axis_configs.vg_axis_config,
        json!({
            "labelBaseline": {"signal": "a === \"bottom\" ? a : line-bottom"},
            "labelPadding": {"signal": "a === \"bottom\" ? null : 2"},
        })
    );
}

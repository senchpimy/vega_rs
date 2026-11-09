//! Port of `test/axis.test.ts`.

use vega_lite::axis_component::axis_component_properties;

#[test]
fn should_have_disable_grid_scale_scale_and_orient_as_the_first_items() {
    let props = axis_component_properties();
    assert_eq!(props[0], "disable");
    assert_eq!(props[1], "gridScale");
    assert_eq!(props[2], "scale");
    assert_eq!(props[3], "orient");
}

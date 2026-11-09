//! Phase 3 renderer geometry: per-corner rounded rectangles, local-coordinate
//! symbols/arcs, and custom symbol path scaling.

use serde_json::json;
use vega_svg::scenegraph::Item;
use vega_svg::svg::render;

fn root() -> Item {
    let mut root = Item::new("group");
    root.class.push("mark-group".into());
    root.class.push("role-frame".into());
    root.class.push("root".into());
    root.set("width", json!(200));
    root.set("height", json!(200));
    root.set("_svg_width", json!(200));
    root.set("_svg_height", json!(200));
    root.set("_view_width", json!(200));
    root.set("_view_height", json!(200));
    root
}

#[test]
fn rounded_rect_uses_independent_corner_radii() {
    let mut root = root();
    let mut rect = Item::new("rect");
    rect.set("width", json!(100));
    rect.set("height", json!(50));
    rect.set("cornerRadiusTopLeft", json!(4));
    rect.set("cornerRadiusTopRight", json!(4));
    rect.set("cornerRadiusBottomLeft", json!(0));
    rect.set("cornerRadiusBottomRight", json!(0));
    rect.set("fill", json!("#4c78a8"));
    root.items.push(rect);

    let svg = render(&root, None);
    // Upstream `rectangle.js`: cubic corners with C = 0.448084975506, and the
    // bottom corners stay square because their radii are zero.
    assert!(
        svg.contains(
            "M4,0L96,0C98.20766,0,100,1.79234,100,4L100,50C100,50,100,50,100,50L0,50C0,50,0,50,0,50L0,4C0,1.79234,1.79234,0,4,0Z"
        ),
        "{svg}"
    );
}

#[test]
fn builtin_symbol_geometry_is_local_and_translated() {
    let mut root = root();
    let mut symbol = Item::new("symbol");
    symbol.set("x", json!(10));
    symbol.set("y", json!(20));
    symbol.set("size", json!(100));
    symbol.set("shape", json!("square"));
    symbol.set("fill", json!("#4c78a8"));
    root.items.push(symbol);

    let svg = render(&root, None);
    // r = sqrt(size) / 2 = 5, emitted at the origin.
    assert!(svg.contains("d=\"M-5,-5h10v10h-10Z\""), "{svg}");
    assert!(svg.contains("transform=\"translate(10,20)\""), "{svg}");
}

#[test]
fn custom_symbol_path_is_scaled_by_sqrt_size_over_two() {
    let mut root = root();
    let mut symbol = Item::new("symbol");
    symbol.set("x", json!(0));
    symbol.set("y", json!(0));
    symbol.set("size", json!(100));
    symbol.set("shape", json!("M0,0L1,1"));
    symbol.set("fill", json!("#4c78a8"));
    root.items.push(symbol);

    let svg = render(&root, None);
    assert!(svg.contains("d=\"M0,0L1,1\""), "{svg}");
    assert!(svg.contains("transform=\"translate(0,0) scale(5)\""), "{svg}");
}

#[test]
fn arc_path_is_local_and_positioned_with_a_transform() {
    let mut root = root();
    let mut arc = Item::new("arc");
    arc.set("x", json!(10));
    arc.set("y", json!(20));
    arc.set("startAngle", json!(0));
    arc.set("endAngle", json!(std::f64::consts::FRAC_PI_2));
    arc.set("outerRadius", json!(10));
    arc.set("fill", json!("#4c78a8"));
    root.items.push(arc);

    let svg = render(&root, None);
    assert!(svg.contains("transform=\"translate(10,20)\""), "{svg}");
    assert!(
        svg.contains("d=\"M0,-10A10,10,0,0,1,10,0L0,0Z\""),
        "{svg}"
    );
}

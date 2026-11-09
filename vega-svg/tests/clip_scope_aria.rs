use serde_json::json;
use vega_svg::scenegraph::Item;
use vega_svg::svg::render;

fn root(width: f64, height: f64) -> Item {
    let mut item = Item::new("group");
    item.class.push("mark-group".into());
    item.class.push("role-frame".into());
    item.class.push("root".into());
    item.set("width", json!(width));
    item.set("height", json!(height));
    item.set("_svg_width", json!(width));
    item.set("_svg_height", json!(height));
    item.set("_view_width", json!(width));
    item.set("_view_height", json!(height));
    item
}

fn group_mark(name: &str, x: f64) -> Item {
    let mut group = Item::new("group");
    group.class.push("mark-group".into());
    group.class.push("role-mark".into());
    group.set("_mark_name", json!(name));
    group.set("x", json!(x));
    group.set("y", json!(0));
    group.set("width", json!(10));
    group.set("height", json!(10));

    let mut child = Item::new("group");
    child.class.push("mark-rect".into());
    child.class.push("role-mark".into());
    let mut rect = Item::new("rect");
    rect.set("width", json!(5));
    rect.set("height", json!(5));
    rect.set("fill", json!("#4c78a8"));
    child.items.push(rect);
    group.items.push(child);
    group
}

#[test]
fn mark_clip_emits_clip_path_definition_once() {
    let mut root = root(100.0, 100.0);

    let mut marks = Item::new("group");
    marks.class.push("mark-rect".into());
    marks.class.push("role-mark".into());
    marks.set("_clip", json!([0, 0, 100, 100]));

    let mut rect = Item::new("rect");
    rect.set("width", json!(10));
    rect.set("height", json!(10));
    rect.set("fill", json!("#4c78a8"));
    marks.items.push(rect);

    root.items.push(marks);

    let svg = render(&root, None);
    assert!(svg.contains(
        "<clipPath id=\"clip1\"><rect x=\"0\" y=\"0\" width=\"100\" height=\"100\"/></clipPath>"
    ));
    assert!(svg.contains("class=\"mark-rect role-mark\""));
    assert!(svg.contains("clip-path=\"url(#clip1)\""));
    assert_eq!(svg.matches("<clipPath").count(), 1);
}

#[test]
fn group_items_of_one_mark_share_a_single_role_scope() {
    let mut root = root(100.0, 100.0);
    root.items.push(group_mark("cell", 0.0));
    root.items.push(group_mark("cell", 10.0));
    root.items.push(group_mark("cell", 20.0));

    let svg = render(&root, None);
    assert_eq!(svg.matches("class=\"mark-group role-scope cell\"").count(), 1);
    assert_eq!(svg.matches("transform=\"translate(10,0)\"").count(), 1);
    assert_eq!(svg.matches("transform=\"translate(20,0)\"").count(), 1);
}

#[test]
fn item_descriptions_use_aria_label_instead_of_title() {
    let mut root = root(100.0, 100.0);

    let mut text = Item::new("text");
    text.set("x", json!(10));
    text.set("y", json!(20));
    text.set("text", json!("Andy"));
    text.set("description", json!("a: 30; b: 28; label: Andy"));
    root.items.push(text);

    let svg = render(&root, None);
    assert!(!svg.contains("<title>"));
    assert!(svg.contains("aria-label=\"a: 30; b: 28; label: Andy\""));
    assert!(svg.contains("role=\"graphics-symbol\""));
    assert!(svg.contains("aria-roledescription=\"text mark\""));
}

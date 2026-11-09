//! Port of `test/compile/data/source.test.ts`.

use serde_json::json;

use vega_lite::compile::data::source::SourceNode;

#[test]
fn explicit_values_should_have_values() {
    let source = SourceNode::new(Some(&json!({
        "values": [
            {"a": 1, "b": 2, "c": 3},
            {"a": 4, "b": 5, "c": 6}
        ]
    })));
    assert_eq!(
        SourceNode::data(&source)["values"],
        json!([
            {"a": 1, "b": 2, "c": 3},
            {"a": 4, "b": 5, "c": 6}
        ])
    );
}

#[test]
fn explicit_values_should_have_no_format_type() {
    let source = SourceNode::new(Some(&json!({
        "values": [
            {"a": 1, "b": 2, "c": 3},
            {"a": 4, "b": 5, "c": 6}
        ]
    })));
    assert!(SourceNode::data(&source).get("format").is_none());
}

#[test]
fn explicit_values_as_csv_should_have_values() {
    let source = SourceNode::new(Some(&json!({
        "values": "a\n1\n2\n3",
        "format": {"type": "csv"}
    })));
    assert_eq!(SourceNode::data(&source)["values"], json!("a\n1\n2\n3"));
}

#[test]
fn explicit_values_as_csv_should_have_correct_type() {
    let source = SourceNode::new(Some(&json!({
        "values": "a\n1\n2\n3",
        "format": {"type": "csv"}
    })));
    assert_eq!(SourceNode::data(&source)["format"]["type"], json!("csv"));
}

#[test]
fn link_to_url_should_have_format_type_csv() {
    let source = SourceNode::new(Some(&json!({"url": "http://foo.bar/file.csv"})));
    assert_eq!(SourceNode::data(&source)["format"]["type"], json!("csv"));
}

#[test]
fn link_to_url_should_have_correct_url() {
    let source = SourceNode::new(Some(&json!({"url": "http://foo.bar/file.csv"})));
    assert_eq!(
        SourceNode::data(&source)["url"],
        json!("http://foo.bar/file.csv")
    );
}

#[test]
fn without_file_ending_should_have_format_type_json() {
    let source = SourceNode::new(Some(&json!({"url": "http://foo.bar/file.baz"})));
    assert_eq!(SourceNode::data(&source)["format"]["type"], json!("json"));
}

#[test]
fn no_data_should_provide_placeholder_source_data() {
    let source = SourceNode::new(None);
    assert_eq!(SourceNode::data_name(&source), Some("source".to_string()));
}

#[test]
fn named_data_source_should_provide_named_source_data() {
    let source = SourceNode::new(Some(&json!({"name": "foo"})));
    assert_eq!(SourceNode::data_name(&source), Some("foo".to_string()));
}

#[test]
fn json_format_should_include_property_if_specified() {
    let source = SourceNode::new(Some(&json!({
        "url": "http://foo.bar",
        "format": {"type": "json", "property": "baz"}
    })));
    assert_eq!(SourceNode::data(&source)["format"]["property"], json!("baz"));
}

#[test]
fn topojson_feature_should_have_format_type_topojson() {
    let source = SourceNode::new(Some(&json!({
        "url": "http://foo.bar",
        "format": {"type": "topojson", "feature": "baz"}
    })));
    assert_eq!(
        SourceNode::data(&source)["format"]["type"],
        json!("topojson")
    );
}

#[test]
fn topojson_feature_should_have_format_feature_baz() {
    let source = SourceNode::new(Some(&json!({
        "url": "http://foo.bar",
        "format": {"type": "topojson", "feature": "baz"}
    })));
    assert_eq!(
        SourceNode::data(&source)["format"]["feature"],
        json!("baz")
    );
}

#[test]
fn topojson_mesh_should_have_format_type_topojson() {
    let source = SourceNode::new(Some(&json!({
        "url": "http://foo.bar",
        "format": {"type": "topojson", "mesh": "baz"}
    })));
    assert_eq!(
        SourceNode::data(&source)["format"]["type"],
        json!("topojson")
    );
}

#[test]
fn topojson_mesh_should_have_format_mesh_baz() {
    let source = SourceNode::new(Some(&json!({
        "url": "http://foo.bar",
        "format": {"type": "topojson", "mesh": "baz"}
    })));
    assert_eq!(SourceNode::data(&source)["format"]["mesh"], json!("baz"));
}

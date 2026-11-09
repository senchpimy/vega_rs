//! Port of `test/compile/mark/text.test.ts`.

#[path = "mark_util.rs"]
mod mark_util;

use serde_json::json;

use mark_util::{assert_eq_json, parse_unit_model_with_scale_and_layout};
use vega_lite::compile_common::BIN_RANGE_DELIMITER;
use vega_lite::compile::mark::text::encode_entry as text;
use vega_lite::compile::model::Model;
use vega_lite::compile::support::init_config;

#[test]
fn stacked_x_uses_stack_end() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "text",
        "encoding": {
            "x": {"aggregate": "sum", "field": "a", "type": "quantitative", "stack": "zero"},
            "color": {"field": "b", "type": "ordinal"}
        },
        "data": {"url": "data/barley.json"}
    }));
    assert_eq_json(json!({"scale": "x", "field": "sum_a_end"}), text(&model)["x"].clone());
}

#[test]
fn stacked_theta_uses_stack_mid() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "text",
        "encoding": {
            "theta": {"field": "value", "type": "quantitative", "stack": true},
            "color": {"field": "b", "type": "ordinal"}
        },
        "data": {"url": "data/barley.json"}
    }));
    assert_eq_json(
        json!({"signal": "scale(\"theta\", 0.5 * datum[\"value_start\"] + 0.5 * datum[\"value_end\"])"}),
        text(&model)["theta"].clone(),
    );
}

#[test]
fn stacked_y_uses_stack_end() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "text",
        "encoding": {
            "y": {"aggregate": "sum", "field": "a", "type": "quantitative", "stack": "zero"},
            "color": {"field": "b", "type": "ordinal"}
        },
        "data": {"url": "data/barley.json"}
    }));
    assert_eq_json(json!({"scale": "y", "field": "sum_a_end"}), text(&model)["y"].clone());
}

#[test]
fn quantitative_and_format_uses_number_template() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "text",
        "encoding": {"text": {"field": "foo", "type": "quantitative", "format": "d"}}
    }));
    assert_eq_json(
        json!({"signal": "format(datum[\"foo\"], \"d\")"}),
        text(&model)["text"].clone(),
    );
}

#[test]
fn custom_format_type() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "text",
        "encoding": {"text": {"field": "foo", "type": "quantitative", "format": "d", "formatType": "numberFormat"}},
        "config": {"customFormatTypes": true}
    }));
    assert_eq_json(
        json!({"signal": "numberFormat(datum[\"foo\"], \"d\")"}),
        text(&model)["text"].clone(),
    );
}

#[test]
fn binned_quantitative_outputs_bin_range() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "text",
        "encoding": {"text": {"bin": true, "field": "foo", "type": "quantitative", "format": "d"}}
    }));
    assert_eq_json(
        json!({"signal": format!(
            "!isValid(datum[\"bin_maxbins_10_foo\"]) || !isFinite(+datum[\"bin_maxbins_10_foo\"]) ? \"null\" : format(datum[\"bin_maxbins_10_foo\"], \"d\") + \"{BIN_RANGE_DELIMITER}\" + format(datum[\"bin_maxbins_10_foo_end\"], \"d\")"
        )}),
        text(&model)["text"].clone(),
    );
}

#[test]
fn temporal_uses_date_template() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "text",
        "encoding": {"text": {"field": "foo", "type": "temporal"}}
    }));
    assert_eq_json(
        json!({"signal": "timeFormat(datum[\"foo\"], \"%b %d, %Y\")"}),
        text(&model)["text"].clone(),
    );
}

#[test]
fn x_y_text_ordinal_scale_on_x() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "text",
        "encoding": {
            "x": {"field": "Acceleration", "type": "ordinal"},
            "y": {"field": "Displacement", "type": "quantitative"},
            "text": {"field": "Origin", "type": "ordinal"}
        },
        "data": {"url": "data/cars.json"}
    }));
    assert_eq_json(json!({"scale": "x", "field": "Acceleration"}), text(&model)["x"].clone());
}

#[test]
fn x_y_text_ordinal_scale_on_y() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "text",
        "encoding": {
            "x": {"field": "Acceleration", "type": "ordinal"},
            "y": {"field": "Displacement", "type": "quantitative"},
            "text": {"field": "Origin", "type": "ordinal"}
        },
        "data": {"url": "data/cars.json"}
    }));
    assert_eq_json(json!({"scale": "y", "field": "Displacement"}), text(&model)["y"].clone());
}

#[test]
fn x_y_text_ordinal_is_centered() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "text",
        "encoding": {
            "x": {"field": "Acceleration", "type": "ordinal"},
            "y": {"field": "Displacement", "type": "quantitative"},
            "text": {"field": "Origin", "type": "ordinal"}
        },
        "data": {"url": "data/cars.json"}
    }));
    assert_eq_json(json!({"value": "center"}), text(&model)["align"].clone());
}

#[test]
fn x_y_text_ordinal_maps_text_without_template() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": "text",
        "encoding": {
            "x": {"field": "Acceleration", "type": "ordinal"},
            "y": {"field": "Displacement", "type": "quantitative"},
            "text": {"field": "Origin", "type": "ordinal"}
        },
        "data": {"url": "data/cars.json"}
    }));
    assert_eq_json(
        json!({"signal": "isValid(datum[\"Origin\"]) ? datum[\"Origin\"] : \"\"+datum[\"Origin\"]"}),
        text(&model)["text"].clone(),
    );
}

#[test]
fn size_in_mark_def_maps_to_font_size() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "text", "size": 5},
        "encoding": {"text": {"field": "Origin", "type": "ordinal"}},
        "data": {"url": "data/cars.json"}
    }));
    assert_eq_json(json!({"value": 5}), text(&model)["fontSize"].clone());
}

#[test]
fn config_text_size_maps_to_font_size() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "text"},
        "encoding": {"text": {"field": "Origin", "type": "ordinal"}},
        "data": {"url": "data/cars.json"},
        "config": {"text": {"size": 25}}
    }));
    assert_eq_json(json!({"value": 25}), text(&model)["fontSize"].clone());
}

#[test]
fn config_text_font_size_maps_to_font_size() {
    let model = parse_unit_model_with_scale_and_layout(&json!({
        "mark": {"type": "text"},
        "encoding": {"text": {"field": "Origin", "type": "ordinal"}},
        "data": {"url": "data/cars.json"},
        "config": {"text": {"fontSize": 25}}
    }));
    assert_eq_json(json!({"value": 25}), text(&model)["fontSize"].clone());
}

#[test]
fn row_column_text_color_size() {
    let spec = json!({
        "mark": "text",
        "encoding": {
            "text": {"field": "Acceleration", "type": "quantitative", "aggregate": "mean"},
            "color": {"field": "Acceleration", "type": "quantitative", "aggregate": "mean"},
            "size": {"field": "Acceleration", "type": "quantitative", "aggregate": "mean"}
        },
        "data": {"url": "data/cars.json"},
        "config": {"mark": {"invalid": "break-paths-show-domains"}}
    });
    let config = init_config(spec.get("config"));
    let mut model = Model::new_unit(&spec, None, "child", None, &config);
    model.parse_scale();
    model.parse_layout_size();
    let props = text(&model);
    assert_eq_json(json!({"signal": "child_width", "mult": 0.5}), props["x"].clone());
    assert_eq_json(json!({"mult": 0.5, "signal": "child_height"}), props["y"].clone());
    assert_eq_json(
        json!({"signal": "format(datum[\"mean_Acceleration\"], \"\")"}),
        props["text"].clone(),
    );
    assert_eq_json(
        json!({"scale": "child_color", "field": "mean_Acceleration"}),
        props["fill"].clone(),
    );
    assert_eq_json(
        json!({"scale": "child_size", "field": "mean_Acceleration"}),
        props["fontSize"].clone(),
    );
}

fn row_column_model() -> Model {
    let spec = json!({
        "mark": "text",
        "encoding": {
            "text": {"field": "Acceleration", "type": "quantitative", "aggregate": "mean"},
            "color": {"field": "Acceleration", "type": "quantitative", "aggregate": "mean"},
            "size": {"field": "Acceleration", "type": "quantitative", "aggregate": "mean"}
        },
        "data": {"url": "data/cars.json"},
        "config": {"mark": {"invalid": "break-paths-show-domains"}}
    });
    let config = init_config(spec.get("config"));
    let mut model = Model::new_unit(&spec, None, "child", None, &config);
    model.parse_scale();
    model.parse_layout_size();
    model
}

#[test]
fn row_column_text_x() {
    assert_eq_json(json!({"signal": "child_width", "mult": 0.5}), text(&row_column_model())["x"].clone());
}

#[test]
fn row_column_text_y() {
    assert_eq_json(json!({"mult": 0.5, "signal": "child_height"}), text(&row_column_model())["y"].clone());
}

#[test]
fn row_column_text_fill() {
    assert_eq_json(
        json!({"scale": "child_color", "field": "mean_Acceleration"}),
        text(&row_column_model())["fill"].clone(),
    );
}

#[test]
fn row_column_text_font_size() {
    assert_eq_json(
        json!({"scale": "child_size", "field": "mean_Acceleration"}),
        text(&row_column_model())["fontSize"].clone(),
    );
}

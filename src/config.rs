//! Port of `src/config.ts`.

use serde_json::{Map, Value};

use crate::compile_common::{signal_or_value_ref_with_condition, signal_ref_or_value};
use crate::config_support::{
    assemble_parameter_signals, default_legend_config, default_scale_config,
    default_selection_config, is_conditional_axis_value, is_empty_value, is_object, merge_config,
    truthy, write_config, AXIS_CONFIGS, HEADER_CONFIGS,
};
use crate::expr::replace_expr_ref;
use crate::guide::VL_ONLY_LEGEND_CONFIG;
use crate::mark::{
    default_bar_config, default_mark_config, default_rect_config, default_tick_config,
    vl_only_mark_specific_config_properties, Mark, COMPOSITE_MARKS, MARK_CONFIGS, PRIMITIVE_MARKS,
    VL_ONLY_MARK_CONFIG_PROPERTIES,
};
use crate::spec::{is_step, DEFAULT_SPACING};
use crate::title::extract_title_config;
use crate::util::{array, duplicate, has_property, omit};

pub const DEFAULT_STEP: f64 = 20.0;

/// `defaultViewConfig`.
pub fn default_view_config() -> Value {
    serde_json::json!({
        "continuousWidth": 300,
        "continuousHeight": 300,
        "step": DEFAULT_STEP as i64,
    })
}

/// `getViewConfigContinuousSize`.
pub fn get_view_config_continuous_size(view_config: &Value, channel: &str) -> Option<Value> {
    let object = view_config.as_object()?;
    let primary = object.get(channel).filter(|v| !v.is_null());
    primary
        .or_else(|| {
            object
                .get(if channel == "width" {
                    "continuousWidth"
                } else {
                    "continuousHeight"
                })
                .filter(|v| !v.is_null())
        })
        .cloned()
}

/// `getViewConfigDiscreteStep`.
pub fn get_view_config_discrete_step(view_config: &Value, channel: &str) -> Value {
    let size = get_view_config_discrete_size(view_config, channel);
    if is_step(&size) {
        size.get("step").cloned().unwrap_or(Value::Null)
    } else {
        serde_json::json!(DEFAULT_STEP as i64)
    }
}

/// `getViewConfigDiscreteSize`.
pub fn get_view_config_discrete_size(view_config: &Value, channel: &str) -> Value {
    let object = match view_config.as_object() {
        Some(object) => object,
        None => return fallback_step(None),
    };

    let size = object.get(channel).filter(|v| !v.is_null()).or_else(|| {
        object
            .get(if channel == "width" {
                "discreteWidth"
            } else {
                "discreteHeight"
            })
            .filter(|v| !v.is_null())
    });

    // `getFirstDefined(size, {step: viewConfig.step})`
    match size {
        Some(size) => size.clone(),
        None => fallback_step(object.get("step")),
    }
}

fn fallback_step(step: Option<&Value>) -> Value {
    match step {
        Some(step) => serde_json::json!({ "step": step }),
        None => serde_json::json!({}),
    }
}

/// `isVgScheme`.
pub fn is_vg_scheme(range_scheme: &Value) -> bool {
    has_property(range_scheme, "scheme")
}

/// `DEFAULT_FONT_SIZE`.
pub fn default_font_size() -> Value {
    serde_json::json!({
        "text": 11,
        "guideLabel": 10,
        "guideTitle": 11,
        "groupTitle": 13,
        "groupSubtitle": 12,
    })
}

const TAB10: [&str; 10] = [
    "#4c78a8", "#f58518", "#e45756", "#72b7b2", "#54a24b", "#eeca3b", "#b279a2", "#ff9da6",
    "#9d755d", "#bab0ac",
];

/// `DEFAULT_COLOR`.
pub fn default_color() -> Value {
    serde_json::json!({
        "blue": TAB10[0],
        "orange": TAB10[1],
        "red": TAB10[2],
        "teal": TAB10[3],
        "green": TAB10[4],
        "yellow": TAB10[5],
        "purple": TAB10[6],
        "pink": TAB10[7],
        "brown": TAB10[8],
        "gray0": "#000",
        "gray1": "#111",
        "gray2": "#222",
        "gray3": "#333",
        "gray4": "#444",
        "gray5": "#555",
        "gray6": "#666",
        "gray7": "#777",
        "gray8": "#888",
        "gray9": "#999",
        "gray10": "#aaa",
        "gray11": "#bbb",
        "gray12": "#ccc",
        "gray13": "#ddd",
        "gray14": "#eee",
        "gray15": "#fff",
    })
}

/// `defaultConfig`.
pub fn default_config() -> Value {
    static DEFAULT: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    DEFAULT.get_or_init(build_default_config).clone()
}

fn build_default_config() -> Value {
    serde_json::json!({
        "background": "white",
        "padding": 5,
        "timeFormat": "%b %d, %Y",
        "countTitle": "Count of Records",

        "view": default_view_config(),

        "mark": default_mark_config(),

        "arc": {},
        "area": {},
        "bar": default_bar_config(),
        "circle": {},
        "geoshape": {},
        "image": {},
        "line": {},
        "point": {},
        "rect": default_rect_config(),
        "rule": {"color": "black"},
        "square": {},
        "text": {"color": "black"},
        "tick": default_tick_config(),
        "trail": {},

        "boxplot": {
            "size": 14,
            "extent": 1.5,
            "box": {},
            "median": {"color": "white"},
            "outliers": {},
            "rule": {},
            "ticks": null,
        },

        "errorbar": {
            "center": "mean",
            "rule": true,
            "ticks": false,
        },

        "errorband": {
            "band": {"opacity": 0.3},
            "borders": false,
        },

        "scale": default_scale_config(),

        "projection": {},

        "legend": default_legend_config(),
        "header": {"titlePadding": 10, "labelPadding": 10},
        "headerColumn": {},
        "headerRow": {},
        "headerFacet": {},

        "selection": default_selection_config(),
        "style": {},

        "title": {},

        "facet": {"spacing": DEFAULT_SPACING as i64},
        "concat": {"spacing": DEFAULT_SPACING as i64},

        "normalizedNumberFormat": ".0%",
    })
}

/// `colorSignalConfig`.
pub fn color_signal_config(color: &Value) -> Value {
    let value = if color.is_object() {
        let mut merged = default_color();
        if let (Some(merged), Some(color)) = (merged.as_object_mut(), color.as_object()) {
            for (k, v) in color {
                merged.insert(k.clone(), v.clone());
            }
        }
        merged
    } else {
        default_color()
    };

    serde_json::json!({
        "signals": [{"name": "color", "value": value}],
        "mark": {"color": {"signal": "color.blue"}},
        "rule": {"color": {"signal": "color.gray0"}},
        "text": {"color": {"signal": "color.gray0"}},
        "style": {
            "guide-label": {"fill": {"signal": "color.gray0"}},
            "guide-title": {"fill": {"signal": "color.gray0"}},
            "group-title": {"fill": {"signal": "color.gray0"}},
            "group-subtitle": {"fill": {"signal": "color.gray0"}},
            "cell": {"stroke": {"signal": "color.gray8"}},
        },
        "axis": {
            "domainColor": {"signal": "color.gray13"},
            "gridColor": {"signal": "color.gray8"},
            "tickColor": {"signal": "color.gray13"},
        },
        "range": {
            "category": [
                {"signal": "color.blue"},
                {"signal": "color.orange"},
                {"signal": "color.red"},
                {"signal": "color.teal"},
                {"signal": "color.green"},
                {"signal": "color.yellow"},
                {"signal": "color.purple"},
                {"signal": "color.pink"},
                {"signal": "color.brown"},
                {"signal": "color.grey8"},
            ],
        },
    })
}

/// `fontSizeSignalConfig`.
pub fn font_size_signal_config(font_size: &Value) -> Value {
    let value = if font_size.is_object() {
        let mut merged = default_font_size();
        if let (Some(merged), Some(font_size)) = (merged.as_object_mut(), font_size.as_object()) {
            for (k, v) in font_size {
                merged.insert(k.clone(), v.clone());
            }
        }
        merged
    } else {
        default_font_size()
    };

    serde_json::json!({
        "signals": [{"name": "fontSize", "value": value}],
        "text": {"fontSize": {"signal": "fontSize.text"}},
        "style": {
            "guide-label": {"fontSize": {"signal": "fontSize.guideLabel"}},
            "guide-title": {"fontSize": {"signal": "fontSize.guideTitle"}},
            "group-title": {"fontSize": {"signal": "fontSize.groupTitle"}},
            "group-subtitle": {"fontSize": {"signal": "fontSize.groupSubtitle"}},
        },
    })
}

/// `fontConfig`.
pub fn font_config(font: &str) -> Value {
    serde_json::json!({
        "text": {"font": font},
        "style": {
            "guide-label": {"font": font},
            "guide-title": {"font": font},
            "group-title": {"font": font},
            "group-subtitle": {"font": font},
        },
    })
}

fn get_axis_config_internal(axis_config: &Value) -> Value {
    let mut out = Map::new();
    if let Value::Object(map) = axis_config {
        for (prop, val) in map {
            out.insert(
                prop.clone(),
                if is_conditional_axis_value(val) {
                    signal_or_value_ref_with_condition(val)
                } else {
                    signal_ref_or_value(val)
                },
            );
        }
    }
    Value::Object(out)
}

fn get_style_config_internal(style_config: &Value) -> Value {
    let mut out = Map::new();
    if let Value::Object(map) = style_config {
        for (prop, val) in map {
            out.insert(prop.clone(), get_axis_config_internal(val));
        }
    }
    Value::Object(out)
}

/// `configPropsWithExpr` from `initConfig`.
fn config_props_with_expr() -> Vec<&'static str> {
    let mut props: Vec<&'static str> = Vec::new();
    props.extend(MARK_CONFIGS);
    props.extend(AXIS_CONFIGS);
    props.extend(HEADER_CONFIGS);
    props.extend([
        "background",
        "padding",
        "legend",
        "lineBreak",
        "scale",
        "style",
        "title",
        "view",
    ]);
    props
}

/// `initConfig`.
pub fn init_config(specified_config: &Value) -> Value {
    // The overwhelmingly common case is a spec without `config` (or with an
    // empty one). Building and post-processing the large default config is
    // deterministic, so do it once per process and hand out clones.
    if specified_config
        .as_object()
        .map(|m| m.is_empty())
        .unwrap_or(true)
    {
        static DEFAULT: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
        return DEFAULT
            .get_or_init(|| init_config_uncached(&serde_json::json!({})))
            .clone();
    }
    init_config_uncached(specified_config)
}

fn init_config_uncached(specified_config: &Value) -> Value {
    let specified = specified_config.as_object().cloned().unwrap_or_default();

    let color = specified.get("color").cloned();
    let font = specified.get("font").cloned();
    let font_size = specified.get("fontSize").cloned();
    let selection = specified.get("selection").cloned();

    let mut rest_config = specified.clone();
    for prop in ["color", "font", "fontSize", "selection"] {
        rest_config.remove(prop);
    }

    let mut configs = vec![serde_json::json!({}), duplicate(&default_config())];

    if let Some(font) = font.filter(truthy) {
        if let Some(font) = font.as_str() {
            configs.push(font_config(font));
        }
    }
    if let Some(color) = color.filter(truthy) {
        configs.push(color_signal_config(&color));
    }
    if let Some(font_size) = font_size.filter(truthy) {
        configs.push(font_size_signal_config(&font_size));
    }
    configs.push(Value::Object(rest_config));

    let mut merged = match merge_config(&configs) {
        Value::Object(map) => map,
        _ => Map::new(),
    };

    // mergeConfig doesn't recurse and overrides object values.
    if let Some(selection) = selection.filter(truthy) {
        write_config(
            &mut merged,
            "selection",
            &selection,
            crate::config_support::RecurseStrategy::All,
        );
    }

    let mut output = omit(&merged, &config_props_with_expr());

    for prop in ["background", "lineBreak", "padding"] {
        if let Some(value) = merged.get(prop).filter(|v| truthy(v)) {
            output.insert(prop.to_string(), signal_ref_or_value(value));
        }
    }

    for mark_config_type in MARK_CONFIGS {
        if let Some(value) = merged.get(mark_config_type).filter(|v| truthy(v)) {
            output.insert(
                mark_config_type.to_string(),
                replace_expr_ref_value(value, 0),
            );
        }
    }

    for axis_config_type in AXIS_CONFIGS {
        if let Some(value) = merged.get(axis_config_type).filter(|v| truthy(v)) {
            output.insert(
                axis_config_type.to_string(),
                get_axis_config_internal(value),
            );
        }
    }

    for header_config_type in HEADER_CONFIGS {
        if let Some(value) = merged.get(header_config_type).filter(|v| truthy(v)) {
            output.insert(
                header_config_type.to_string(),
                replace_expr_ref_value(value, 0),
            );
        }
    }

    if let Some(legend) = merged.get("legend").filter(|v| truthy(v)) {
        output.insert("legend".to_string(), replace_expr_ref_value(legend, 0));
    }

    if let Some(scale) = merged.get("scale").filter(|v| truthy(v)) {
        let mut scale_obj = scale.as_object().cloned().unwrap_or_default();
        let invalid = scale_obj.remove("invalid");
        let new_scale_invalid = match invalid.as_ref() {
            Some(Value::Object(map)) => Value::Object(replace_expr_ref(map, 1)),
            _ => Value::Object(Map::new()),
        };

        let mut new_scale = match replace_expr_ref_value(&Value::Object(scale_obj), 0) {
            Value::Object(map) => map,
            _ => Map::new(),
        };
        if let Value::Object(invalid) = &new_scale_invalid {
            if !invalid.is_empty() {
                new_scale.insert("invalid".to_string(), new_scale_invalid.clone());
            }
        }
        output.insert("scale".to_string(), Value::Object(new_scale));
    }

    if let Some(style) = merged.get("style").filter(|v| truthy(v)) {
        output.insert("style".to_string(), get_style_config_internal(style));
    }

    if let Some(title) = merged.get("title").filter(|v| truthy(v)) {
        output.insert("title".to_string(), replace_expr_ref_value(title, 0));
    }

    if let Some(view) = merged.get("view").filter(|v| truthy(v)) {
        output.insert("view".to_string(), replace_expr_ref_value(view, 0));
    }

    Value::Object(output)
}

fn replace_expr_ref_value(value: &Value, level: i64) -> Value {
    match value {
        Value::Object(map) => Value::Object(replace_expr_ref(map, level)),
        _ => value.clone(),
    }
}

const VL_ONLY_CONFIG_PROPERTIES: [&str; 30] = [
    "color",
    "fontSize",
    "background", // We apply background to the spec directly.
    "padding",
    "facet",
    "concat",
    "numberFormat",
    "numberFormatType",
    "normalizedNumberFormat",
    "normalizedNumberFormatType",
    "timeFormat",
    "countTitle",
    "header",
    "axisQuantitative",
    "axisTemporal",
    "axisDiscrete",
    "axisPoint",
    "axisXBand",
    "axisXPoint",
    "axisXDiscrete",
    "axisXQuantitative",
    "axisXTemporal",
    "axisYBand",
    "axisYPoint",
    "axisYDiscrete",
    "axisYQuantitative",
    "axisYTemporal",
    "scale",
    "selection",
    "overlay",
];

const VIEW_VL_ONLY_CONFIGS: [&str; 5] = [
    "continuousWidth",
    "continuousHeight",
    "discreteWidth",
    "discreteHeight",
    "step",
];

/// `stripAndRedirectConfig`.
pub fn strip_and_redirect_config(config: &Value) -> Option<Value> {
    let mut config = match config {
        Value::Object(map) => duplicate(map),
        _ => return None,
    };

    for prop in VL_ONLY_CONFIG_PROPERTIES {
        config.remove(prop);
    }

    if let Some(Value::Object(axis)) = config.get_mut("axis") {
        let conditional: Vec<String> = axis
            .iter()
            .filter(|(_, value)| is_conditional_axis_value(value))
            .map(|(prop, _)| prop.clone())
            .collect();
        for prop in conditional {
            axis.remove(&prop);
        }
    }

    if let Some(Value::Object(legend)) = config.get_mut("legend") {
        for prop in VL_ONLY_LEGEND_CONFIG {
            legend.remove(prop);
        }
    }

    // Remove Vega-Lite only generic mark config.
    if let Some(Value::Object(mark)) = config.get_mut("mark") {
        for prop in VL_ONLY_MARK_CONFIG_PROPERTIES {
            mark.remove(prop);
        }
        if mark.get("tooltip").map(is_object).unwrap_or(false) {
            mark.remove("tooltip");
        }
    }

    if let Some(params) = config.get("params").cloned() {
        let params = array(&params);
        let mut signals = config.get("signals").map(array).unwrap_or_default();
        signals.extend(assemble_parameter_signals(&params));
        config.insert("signals".to_string(), Value::Array(signals));
        config.remove("params");
    }

    let mut mark_styles: Vec<&str> = vec!["view"];
    mark_styles.extend(PRIMITIVE_MARKS.iter().map(Mark::as_str));

    for mark_type in mark_styles {
        // Remove Vega-Lite-only mark config.
        if let Some(Value::Object(mark_config)) = config.get_mut(mark_type) {
            for prop in VL_ONLY_MARK_CONFIG_PROPERTIES {
                mark_config.remove(prop);
            }
        }

        // Remove Vega-Lite only mark-specific config.
        let vl_only_mark_specific_configs: Vec<&'static str> = if mark_type == "view" {
            VIEW_VL_ONLY_CONFIGS.to_vec()
        } else {
            vl_only_mark_specific_config_properties(mark_type)
        };

        if let Some(Value::Object(mark_config)) = config.get_mut(mark_type) {
            for prop in vl_only_mark_specific_configs {
                mark_config.remove(prop);
            }
        }

        // Redirect mark config to config.style so that mark config only affects
        // its own mark type.
        redirect_config_to_style_config(&mut config, mark_type, None, None);
    }

    // Clean up the composite mark config.
    for m in COMPOSITE_MARKS {
        config.remove(m);
    }

    redirect_title_config(&mut config);

    // Remove empty config objects.
    let empty: Vec<String> = config
        .iter()
        .filter(|(_, value)| is_object(value) && is_empty_value(value))
        .map(|(prop, _)| prop.clone())
        .collect();
    for prop in empty {
        config.remove(&prop);
    }

    if config.is_empty() {
        None
    } else {
        Some(Value::Object(config))
    }
}

/// Redirect config.title so that title config does not affect header labels.
fn redirect_title_config(config: &mut Map<String, Value>) {
    let title = config
        .get("title")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let extracted = extract_title_config(&title);

    if !extracted.title_mark_config.is_empty() {
        let style = ensure_object(config, "style");
        let mut group_title = style
            .get("group-title")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for (k, v) in extracted.title_mark_config {
            group_title.insert(k, v);
        }
        style.insert("group-title".to_string(), Value::Object(group_title));
    }
    if !extracted.subtitle_mark_config.is_empty() {
        let style = ensure_object(config, "style");
        let mut group_subtitle = style
            .get("group-subtitle")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for (k, v) in extracted.subtitle_mark_config {
            group_subtitle.insert(k, v);
        }
        style.insert("group-subtitle".to_string(), Value::Object(group_subtitle));
    }

    // Subtitle part can stay in config.title since header titles do not use
    // subtitle.
    if !extracted.subtitle.is_empty() {
        config.insert("title".to_string(), Value::Object(extracted.subtitle));
    } else {
        config.remove("title");
    }
}

fn ensure_object<'a>(config: &'a mut Map<String, Value>, key: &str) -> &'a mut Map<String, Value> {
    if !config.get(key).map(Value::is_object).unwrap_or(false) {
        config.insert(key.to_string(), Value::Object(Map::new()));
    }
    config.get_mut(key).unwrap().as_object_mut().unwrap()
}

fn redirect_config_to_style_config(
    config: &mut Map<String, Value>,
    prop: &str,
    to_prop: Option<&str>,
    composite_mark_part: Option<&str>,
) {
    let prop_config: Map<String, Value> = match composite_mark_part {
        Some(part) => config
            .get(prop)
            .and_then(|v| v.get(part))
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default(),
        None => config
            .get(prop)
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default(),
    };

    let to_prop = if prop == "view" {
        Some("cell")
    } else {
        to_prop.or(Some(prop))
    };

    let existing_style = to_prop
        .and_then(|to| config.get("style").and_then(|style| style.get(to)))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let mut style = prop_config;
    for (k, v) in existing_style {
        style.insert(k, v);
    }

    if !style.is_empty() {
        let to_prop = to_prop.unwrap_or(prop);
        let style_config = ensure_object(config, "style");
        style_config.insert(to_prop.to_string(), Value::Object(style));
    }

    if composite_mark_part.is_none() {
        config.remove(prop);
    }
}

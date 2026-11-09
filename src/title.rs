//! Port of `src/title.ts`.

use serde_json::{Map, Value};

use crate::util::pick;

#[derive(Debug, Clone, Default)]
pub struct ExtractedTitleConfig {
    pub title_mark_config: Map<String, Value>,
    pub subtitle_mark_config: Map<String, Value>,
    /// Non-mark title config that need to be hardcoded in the title directive.
    pub non_mark_title_properties: Map<String, Value>,
    pub subtitle: Map<String, Value>,
}

/// Port of `extractTitleConfig`.
pub fn extract_title_config(title_config: &Map<String, Value>) -> ExtractedTitleConfig {
    let mut rest = title_config.clone();

    let take = |rest: &mut Map<String, Value>, key: &str| -> Option<Value> {
        rest.remove(key).filter(|v| !v.is_null())
    };

    // These are non-mark title config that need to be hardcoded.
    let anchor = take(&mut rest, "anchor");
    let frame = take(&mut rest, "frame");
    let offset = take(&mut rest, "offset");
    let orient = take(&mut rest, "orient");
    let angle = take(&mut rest, "angle");
    let limit = take(&mut rest, "limit");

    // color needs to be redirected to fill
    let color = take(&mut rest, "color");

    // subtitle properties
    let subtitle_color = take(&mut rest, "subtitleColor");
    let subtitle_font = take(&mut rest, "subtitleFont");
    let subtitle_font_size = take(&mut rest, "subtitleFontSize");
    let subtitle_font_style = take(&mut rest, "subtitleFontStyle");
    let subtitle_font_weight = take(&mut rest, "subtitleFontWeight");
    let subtitle_line_height = take(&mut rest, "subtitleLineHeight");
    let subtitle_padding = take(&mut rest, "subtitlePadding");

    let mut title_mark_config = rest;
    if let Some(color) = color {
        title_mark_config.insert("fill".to_string(), color);
    }

    let mut non_mark_title_properties = Map::new();
    for (key, value) in [
        ("anchor", anchor),
        ("frame", frame),
        ("offset", offset),
        ("orient", orient),
        ("angle", angle),
        ("limit", limit),
    ] {
        if let Some(value) = value {
            non_mark_title_properties.insert(key.to_string(), value);
        }
    }

    let mut subtitle = Map::new();
    for (key, value) in [
        ("subtitleColor", subtitle_color),
        ("subtitleFont", subtitle_font),
        ("subtitleFontSize", subtitle_font_size),
        ("subtitleFontStyle", subtitle_font_style),
        ("subtitleFontWeight", subtitle_font_weight),
        ("subtitleLineHeight", subtitle_line_height),
        ("subtitlePadding", subtitle_padding),
    ] {
        if let Some(value) = value {
            subtitle.insert(key.to_string(), value);
        }
    }

    let subtitle_mark_config = pick(title_config, &["align", "baseline", "dx", "dy", "limit"]);

    ExtractedTitleConfig {
        title_mark_config,
        subtitle_mark_config,
        non_mark_title_properties,
        subtitle,
    }
}

/// `isText(v)`
pub fn is_text(v: &Value) -> bool {
    match v {
        Value::String(_) => true,
        Value::Array(arr) => arr.first().map(|x| x.is_string()).unwrap_or(false),
        _ => false,
    }
}

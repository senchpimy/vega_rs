//! Port of `src/compile/mark/base.ts`.

use serde_json::Value;

use crate::compile::model::Model;

use super::arc::encode_entry as arc_entry;
use super::area::encode_entry as area_entry;
use super::bar::encode_entry as bar_entry;
use super::geoshape::{encode_entry as geoshape_entry, post_encoding_transform as geoshape_post};
use super::image::encode_entry as image_entry;
use super::line::encode_entry as line_entry;
use super::line::trail_entry;
use super::point::{circle_entry, point_entry, square_entry};
use super::rect::encode_entry as rect_entry;
use super::rule::encode_entry as rule_entry;
use super::text::encode_entry as text_entry;
use super::tick::encode_entry as tick_entry;

#[derive(Clone, Copy)]
pub struct MarkCompiler {
    pub vg_mark: &'static str,
    pub encode_entry: fn(&Model) -> Value,
    pub post_encoding_transform: Option<fn(&Model) -> Value>,
}

pub fn mark_compiler(mark: &str) -> Option<MarkCompiler> {
    let compiler = match mark {
        "arc" => MarkCompiler {
            vg_mark: "arc",
            encode_entry: arc_entry,
            post_encoding_transform: None,
        },
        "area" => MarkCompiler {
            vg_mark: "area",
            encode_entry: area_entry,
            post_encoding_transform: None,
        },
        "bar" => MarkCompiler {
            vg_mark: "rect",
            encode_entry: bar_entry,
            post_encoding_transform: None,
        },
        "circle" => MarkCompiler {
            vg_mark: "symbol",
            encode_entry: circle_entry,
            post_encoding_transform: None,
        },
        "geoshape" => MarkCompiler {
            vg_mark: "shape",
            encode_entry: geoshape_entry,
            post_encoding_transform: Some(geoshape_post),
        },
        "image" => MarkCompiler {
            vg_mark: "image",
            encode_entry: image_entry,
            post_encoding_transform: None,
        },
        "line" => MarkCompiler {
            vg_mark: "line",
            encode_entry: line_entry,
            post_encoding_transform: None,
        },
        "point" => MarkCompiler {
            vg_mark: "symbol",
            encode_entry: point_entry,
            post_encoding_transform: None,
        },
        "rect" => MarkCompiler {
            vg_mark: "rect",
            encode_entry: rect_entry,
            post_encoding_transform: None,
        },
        "rule" => MarkCompiler {
            vg_mark: "rule",
            encode_entry: rule_entry,
            post_encoding_transform: None,
        },
        "square" => MarkCompiler {
            vg_mark: "symbol",
            encode_entry: square_entry,
            post_encoding_transform: None,
        },
        "text" => MarkCompiler {
            vg_mark: "text",
            encode_entry: text_entry,
            post_encoding_transform: None,
        },
        "tick" => MarkCompiler {
            vg_mark: "rect",
            encode_entry: tick_entry,
            post_encoding_transform: None,
        },
        "trail" => MarkCompiler {
            vg_mark: "trail",
            encode_entry: trail_entry,
            post_encoding_transform: None,
        },
        _ => return None,
    };
    Some(compiler)
}

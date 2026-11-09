//! Port of `src/compile/layoutsize/component.ts`.

use serde_json::Value;

use crate::compile::split::Split;

/// Port of `LayoutSize` (`number | 'container' | 'step' | 'merged'`).
pub type LayoutSize = Value;

/// Port of `LayoutSizeIndex`.
pub type LayoutSizeIndex = Split;

/// Port of `LayoutSizeComponent`.
pub type LayoutSizeComponent = Split;

/// Port of `getSizeTypeFromLayoutSizeType`.
pub fn get_size_type_from_layout_size_type(layout_size_type: &str) -> &'static str {
    match layout_size_type {
        "childWidth" => "width",
        "childHeight" => "height",
        "width" => "width",
        "height" => "height",
        _ => "width",
    }
}

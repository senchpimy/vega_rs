//! Render [Vega](https://github.com/vega/vega) specifications to SVG.
//!
//! The crate follows the modular structure of the upstream Vega packages:
//!
//! | module | upstream |
//! | --- | --- |
//! | `expression` | `vega-expression` |
//! | `data` | `vega-transforms` |
//! | `scale` | `vega-scale` |
//! | `scenegraph` | `vega-scenegraph` |
//! | `runtime` | `vega-runtime`, `vega-parser`, `vega-view` |
//! | `svg` | `vega-scenegraph` SVG renderer |
//!
//! Two entry points are provided:
//!
//! ```ignore
//! # use serde_json::json;
//! let vega = json!({"width": 100, "height": 100, "marks": []});
//! let svg = vega_svg::vega_to_svg(&vega).unwrap();
//! # let _ = svg;
//! ```
//!
//! and `vega_lite_to_svg`, which accepts a Vega-Lite specification directly.

pub mod bounds;
pub mod color;
pub mod data;
pub mod error;
pub mod expression;
pub mod format;
pub mod geo;
pub mod interpolate;
pub mod projection;
pub mod runtime;
pub mod scale;
pub mod scenegraph;
pub mod svg;
pub mod value;

pub use error::{Error, ErrorKind};
pub use runtime::{
    compiled_to_svg, vega_lite_to_svg, vega_lite_to_svg_with_options, vega_to_svg,
    vega_to_svg_with_options, RenderOptions,
};

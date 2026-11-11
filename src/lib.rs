//! A Rust port of [Vega-Lite](https://github.com/vega/vega-lite).
//!
//! The port keeps the structure of the original TypeScript sources so that the
//! upstream test-suite can be translated module by module.
//!
//! # Example
//!
//! ```
//! use serde_json::json;
//!
//! let spec = json!({
//!     "data": {"values": [{"a": "A", "b": 1}, {"a": "B", "b": 2}]},
//!     "mark": "bar",
//!     "encoding": {
//!         "x": {"field": "a", "type": "nominal"},
//!         "y": {"field": "b", "type": "quantitative"}
//!     }
//! });
//!
//! let compiled = vega_lite::compile(&spec).expect("valid spec");
//! assert!(compiled.spec.get("marks").is_some());
//! ```
//!
//! The compiled Vega specification can be rendered to SVG with the companion
//! [`vega-svg`](https://docs.rs/vega-svg) crate in this workspace.

pub mod aggregate;
pub mod axis;
pub mod axis_component;
pub mod bin;
pub mod channel;
pub mod channeldef;
pub mod channeldef_support;
pub mod compile;
pub mod compositemark;
pub mod compile_common;
pub mod config;
pub mod config_support;
pub mod compile_predicate;
pub mod datetime;
pub mod encoding;
pub mod expr;
pub mod guide;
pub mod log;
pub mod logical;
pub mod mark;
pub mod normalize;
pub mod parameter;
pub mod normalize_support;
pub mod predicate;
pub mod scale;
pub mod selection;
pub mod spec;
pub mod stack;
pub mod timeunit;
pub mod title;
pub mod type_;
pub mod util;

/// Compile a Vega-Lite specification into a Vega specification.
///
/// Convenience re-export of [`compile::compile::compile`], matching the
/// top-level `compile()` export of the upstream TypeScript package.
pub use compile::compile::{compile, CompileResult};

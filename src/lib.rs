//! A Rust port of [Vega-Lite](https://github.com/vega/vega-lite).
//!
//! The port keeps the structure of the original TypeScript sources so that the
//! upstream test-suite can be translated module by module.

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

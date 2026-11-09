//! Port of `src/compile/mark/`.

pub mod arc;
pub mod area;
pub mod bar;
pub mod base;
pub mod encode;
pub mod geoshape;
pub mod image;
pub mod init;
pub mod line;
// Mirrors the upstream module layout (`src/compile/mark/mark.ts`).
#[allow(clippy::module_inception)]
pub mod mark;
pub mod point;
pub mod rect;
pub mod rule;
pub mod text;
pub mod tick;

//! Port of `src/compile/` (data transforms, model tree and scales).

pub mod axis;
pub mod buildmodel;
// Mirrors the upstream module layout (`src/compile/compile.ts`).
#[allow(clippy::module_inception)]
pub mod compile;
pub mod concat;
pub mod data;
pub mod facet;
pub mod layer;
pub mod header;
pub mod layoutsize;
pub mod legend;
pub mod mark;
pub mod model;
pub mod projection;
pub mod scale;
pub mod selection;
pub mod split;
pub mod support;
pub mod unit;

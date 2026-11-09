//! Port of `src/compile/selection/`.

pub mod assemble;
pub mod clear;
pub mod component;
pub mod inputs;
pub mod interval;
pub mod legends;
pub mod message;
pub mod nearest;
pub mod parse;
pub mod point;
pub mod project;
pub mod scales;
pub mod selector;
pub mod toggle;
pub mod translate;
pub mod zoom;

pub use component::{
    is_timer_selection, selection_compilers, unit_name, ProjectionSignals, SelectionCompiler,
    SelectionComponent, SelectionProjection, SelectionProjectionComponent, MODIFY,
    SELECTION_DOMAIN, STORE, TUPLE, VL_SELECTION_RESOLVE,
};
pub use interval::{BRUSH, GEO_INIT_TICK, SCALE_TRIGGER};
pub use project::TUPLE_FIELDS;
pub use toggle::TOGGLE;

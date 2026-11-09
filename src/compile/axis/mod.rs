//! Port of `src/compile/axis/` (axis component, config resolution, rules,
//! encoding, parsing and assembly).

pub mod assemble;
pub mod component;
pub mod config;
pub mod encode;
pub mod format;
pub mod parse;
pub mod properties;

pub use assemble::{assemble_axes, assemble_axes_from_value, assemble_axis, assemble_axis_signals};
pub use component::{
    axis_component_properties_list, axis_index_from_value, axis_index_to_value, AxisComponent,
    AxisComponentIndex,
};
pub use config::{get_axis_config, get_axis_config_style, get_axis_configs, AxisConfigs};
pub use parse::{parse_axes_and_headers, parse_layer_axes, parse_unit_axes};
pub use properties::{
    axis_rule, default_band_position, default_grid, default_label_align, default_label_baseline,
    default_label_flush, default_label_overlap, default_orient, default_tick_count,
    default_tick_min_step, default_zindex, get_field_def_title, get_label_angle, grid_scale,
    is_axis_rule, values, AxisRuleParams,
};

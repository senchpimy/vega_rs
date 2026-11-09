//! Deep-compares our compiled output against the upstream `examples/compiled/*.vg.json`.
//!
//! Set `VEGA_LITE_REPO` to point at a Vega-Lite checkout (default `/tmp/opencode/vega-lite`).
//!
//! The supported list contains examples our compiler reproduces exactly.  The
//! ignored ones document known gaps (facet headers, composite marks, concat
//! layout) and are expected to differ.

use std::env;
use std::fs;
use std::path::PathBuf;

use serde_json::Value;

fn repo() -> PathBuf {
    if let Ok(path) = env::var("VEGA_LITE_REPO") {
        let path = PathBuf::from(path);
        if path.join("examples/specs").is_dir() {
            return path;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn compile_example(name: &str) -> Value {
    let repo = repo();
    let spec_path = repo.join("examples/specs").join(format!("{name}.vl.json"));
    let vg_path = repo.join("examples/compiled").join(format!("{name}.vg.json"));
    let spec: Value = serde_json::from_str(
        &fs::read_to_string(&spec_path).unwrap_or_else(|e| panic!("read {spec_path:?}: {e}")),
    )
    .expect("parse vl spec");
    let expected: Value = serde_json::from_str(
        &fs::read_to_string(&vg_path).unwrap_or_else(|e| panic!("read {vg_path:?}: {e}")),
    )
    .expect("parse vg spec");

    let result = vega_lite::compile::compile::compile(&spec)
        .unwrap_or_else(|e| panic!("compile {name} failed: {e}"));
    if result.spec != expected {
        let actual = serde_json::to_string_pretty(&result.spec).unwrap();
        let expected_pretty = serde_json::to_string_pretty(&expected).unwrap();
        let diff = first_diff(&result.spec, &expected, "$".to_string());
        panic!(
            "example {name} does not match\nfirst difference: {diff}\n\n--- actual ---\n{actual}\n\n--- expected ---\n{expected_pretty}"
        );
    }
    result.spec
}

fn first_diff(actual: &Value, expected: &Value, path: String) -> String {
    match (actual, expected) {
        (Value::Object(a), Value::Object(e)) => {
            for key in a.keys() {
                if !e.contains_key(key) {
                    return format!("{path}.{key}: unexpected key in actual");
                }
                let diff = first_diff(&a[key], &e[key], format!("{path}.{key}"));
                if !diff.is_empty() {
                    return diff;
                }
            }
            for key in e.keys() {
                if !a.contains_key(key) {
                    return format!("{path}.{key}: missing in actual (expected {:?})", e[key]);
                }
            }
            String::new()
        }
        (Value::Array(a), Value::Array(e)) => {
            if a.len() != e.len() {
                return format!("{path}: array length {} != {}", a.len(), e.len());
            }
            for (i, (av, ev)) in a.iter().zip(e.iter()).enumerate() {
                let diff = first_diff(av, ev, format!("{path}[{i}]"));
                if !diff.is_empty() {
                    return diff;
                }
            }
            String::new()
        }
        (a, e) => {
            if a == e {
                String::new()
            } else {
                format!("{path}: {a} != {e}")
            }
        }
    }
}

macro_rules! example_test {
    ($fn:ident, $name:expr) => {
        #[test]
        fn $fn() {
            compile_example($name);
        }
    };
    ($fn:ident, $name:expr, ignore) => {
        #[test]
        #[ignore = "known gap in the Rust port"]
        fn $fn() {
            compile_example($name);
        }
    };
}

// Supported examples (all match upstream exactly).
example_test!(bar, "bar");
example_test!(bar_1d, "bar_1d");
example_test!(bar_1d_default_size, "bar_1d_default_size");
example_test!(bar_aggregate, "bar_aggregate");
example_test!(bar_aggregate_count, "bar_aggregate_count");
example_test!(bar_argmax, "bar_argmax");
example_test!(bar_grouped, "bar_grouped");
example_test!(bar_tooltip, "bar_tooltip");
example_test!(point_1d, "point_1d");
example_test!(point_2d_aggregate, "point_2d_aggregate");
example_test!(point_href, "point_href");
example_test!(line, "line");
example_test!(line_color, "line_color");
example_test!(line_step, "line_step");
example_test!(line_month, "line_month");
example_test!(line_calculate, "line_calculate");
example_test!(area, "area");
example_test!(area_vertical, "area_vertical");
example_test!(area_cumulative_freq, "area_cumulative_freq");
example_test!(stacked_bar_1d, "stacked_bar_1d");
example_test!(tick_dot, "tick_dot");
example_test!(tick_dot_thickness, "tick_dot_thickness");
example_test!(rect_heatmap, "rect_heatmap");
example_test!(text_scatterplot_colored, "text_scatterplot_colored");
example_test!(circle, "circle");
example_test!(square, "square");
example_test!(histogram_ordinal, "histogram_ordinal");
example_test!(window_rank, "window_rank");
example_test!(sequence_line, "sequence_line");
example_test!(geo_circle, "geo_circle");
example_test!(arc_radial, "arc_radial");

// Scale ordering, bin signal names, layered size signals, and arc ranged
// positions are all part of the exact-output regressions covered here.
example_test!(arc_ordinal_theta, "arc_ordinal_theta");
example_test!(bar_1d_binned, "bar_1d_binned");
example_test!(area_gradient, "area_gradient");
example_test!(area_horizon, "area_horizon");
example_test!(area_overlay, "area_overlay");
example_test!(area_overlay_with_y2, "area_overlay_with_y2");

// The arc radial histogram exercises per-layer data output nodes.
example_test!(arc_radial_histogram, "arc_radial_histogram");

// Facet examples (header marks, header data and layout offsets).
example_test!(area_density_facet, "area_density_facet");
example_test!(facet_cross_independent_scale, "facet_cross_independent_scale");
example_test!(trellis_anscombe, "trellis_anscombe");
example_test!(trellis_scatter_binned_row, "trellis_scatter_binned_row");
example_test!(trellis_scatter_small, "trellis_scatter_small");

// Selection predicates inside conditional encodings.
example_test!(selection_composition_and, "selection_composition_and");
example_test!(selection_composition_or, "selection_composition_or");
example_test!(
    selection_type_interval_invert,
    "selection_type_interval_invert"
);

// Known gaps.
#[test]
#[ignore = "facet header marks/layout are not ported"]
fn facet_boxplot() {
    compile_example("boxplot_2D_vertical");
}
#[test]
#[ignore = "composite mark normalization (boxplot) is not ported"]
fn boxplot() {
    compile_example("boxplot_2D_vertical_single_per_group");
}
#[test]
fn errorband() {
    compile_example("errorband_2d_vertical_borders");
}
#[test]
#[ignore = "facet headers are not ported"]
fn faceted_concat() {
    compile_example("concat_bar_scales_discretize");
}
#[test]
#[ignore = "repeat composition with projections is not fully ported"]
fn repeat_line() {
    compile_example("repeat_line_weather");
}
/// Stack order, stack groupby, corner radius groups, bin size ranges, time-unit
/// sort indexes and binned rect band positions are all covered by these.
#[test]
fn stacked_bar_and_area_examples() {
    for name in [
        "stacked_area",
        "stacked_area_binned",
        "stacked_area_normalize",
        "stacked_area_ordinal",
        "stacked_area_overlay",
        "stacked_area_stream",
        "stacked_bar_count",
        "stacked_bar_count_corner_radius_config",
        "stacked_bar_count_corner_radius_mark",
        "stacked_bar_count_corner_radius_mark_x",
        "stacked_bar_count_corner_radius_stroke",
        "stacked_bar_h",
        "stacked_bar_h_custom_color_domain",
        "stacked_bar_h_normalized_labeled",
        "stacked_bar_h_order",
        "stacked_bar_h_order_custom",
        "stacked_bar_normalize",
        "stacked_bar_population",
        "stacked_bar_size",
        "stacked_bar_sum_opacity",
        "stacked_bar_v",
        "stacked_bar_v_ascending",
        "stacked_bar_weather",
        "bar_argmax_transform",
        "bar_binned_data",
        "bar_binned_yearmonth_grouped_center_band",
        "bar_binned_yearmonth_label_band_center",
        "bar_corner_radius_end",
        "bar_custom_time_domain",
        "bar_default_tooltip_title_null",
        "bar_diverging_stack_population_pyramid",
        "bar_grouped_custom_color_domain",
        "bar_grouped_errorbar",
        "bar_grouped_stacked",
        "bar_grouped_timeunit_yearweek",
        "bar_group_timeunit",
        "bar_layered_transparent",
        "bar_params",
        "bar_params_bound",
        "bar_sort_by_count",
        "bar_temporal_color_config_time_unit_band_position",
        "circle_binned",
        "circle_binned_maxbins_2",
        "circle_binned_maxbins_20",
        "circle_binned_maxbins_5",
        "circle_custom_tick_labels",
        "circle_github_punchcard",
        "circle_labelangle_orient_signal",
        "circle_wilkinson_dotplot_stacked",
        "histogram_bin_spacing",
        "histogram_bin_spacing_reverse",
        "histogram_nonlinear",
        "histogram_no_spacing",
        "histogram_ordinal_sort",
        "histogram_rel_freq",
        "argmin_spaces",
        "layer_bar_labels_grey",
        "layer_cumulative_histogram",
        "parallel_coordinate",
        "point_aggregate_detail",
        "point_binned_size",
        "point_bubble",
        "point_colorramp_size",
        "point_shape_custom",
        "rect_binned_heatmap",
        "rect_heatmap_weather_temporal_center_band",
        "rect_heatmap_weather_temporal_center_band_config",
        "tick_histogram",
        "tick_timeunit",
        "wheat_wages",
    ] {
        compile_example(name);
    }
}
// Selection, interactive and concat-layout regressions fixed by the
// domainRaw / concat-layout / legend-encode work.
example_test!(concat_bar_scales_discretize, "concat_bar_scales_discretize");
example_test!(
    concat_bar_scales_discretize_2_cols,
    "concat_bar_scales_discretize_2_cols"
);
example_test!(concat_hover, "concat_hover");
example_test!(concat_weather, "concat_weather");
example_test!(hconcat_weather, "hconcat_weather");
example_test!(vconcat_weather, "vconcat_weather");
example_test!(nested_concat_align, "nested_concat_align");
example_test!(interactive_bin_extent, "interactive_bin_extent");
example_test!(
    interactive_bin_extent_bottom,
    "interactive_bin_extent_bottom"
);
example_test!(
    interactive_dashboard_europe_pop,
    "interactive_dashboard_europe_pop"
);
example_test!(interactive_multi_line_label, "interactive_multi_line_label");
example_test!(interactive_overview_detail, "interactive_overview_detail");
example_test!(interactive_panzoom_splom, "interactive_panzoom_splom");
example_test!(
    interactive_point_domain_raw_binding,
    "interactive_point_domainRaw_binding"
);
example_test!(interactive_seattle_weather, "interactive_seattle_weather");
example_test!(interactive_splom, "interactive_splom");
example_test!(selection_brush_timeunit, "selection_brush_timeunit");
example_test!(selection_concat, "selection_concat");
example_test!(selection_filter, "selection_filter");
example_test!(selection_filter_composition, "selection_filter_composition");
example_test!(selection_filter_false, "selection_filter_false");
example_test!(selection_filter_true, "selection_filter_true");
example_test!(
    selection_project_multi_cylinders_origin,
    "selection_project_multi_cylinders_origin"
);
example_test!(selection_project_multi_origin, "selection_project_multi_origin");
example_test!(
    selection_project_single_cylinders_origin,
    "selection_project_single_cylinders_origin"
);
example_test!(
    selection_project_single_origin,
    "selection_project_single_origin"
);
example_test!(selection_resolution_global, "selection_resolution_global");
example_test!(
    selection_resolution_intersect,
    "selection_resolution_intersect"
);
example_test!(selection_resolution_union, "selection_resolution_union");
example_test!(selection_translate_brush_drag, "selection_translate_brush_drag");
example_test!(
    selection_translate_brush_shift_drag,
    "selection_translate_brush_shift-drag"
);
example_test!(
    selection_translate_scatterplot_drag,
    "selection_translate_scatterplot_drag"
);
example_test!(
    selection_translate_scatterplot_shift_drag,
    "selection_translate_scatterplot_shift-drag"
);
example_test!(
    selection_zoom_brush_shift_wheel,
    "selection_zoom_brush_shift-wheel"
);
example_test!(selection_zoom_brush_wheel, "selection_zoom_brush_wheel");
example_test!(
    selection_zoom_scatterplot_shift_wheel,
    "selection_zoom_scatterplot_shift-wheel"
);
example_test!(
    selection_zoom_scatterplot_wheel,
    "selection_zoom_scatterplot_wheel"
);
example_test!(concat_bar_layer_circle, "concat_bar_layer_circle");
example_test!(
    layer_circle_independent_color,
    "layer_circle_independent_color"
);
example_test!(
    layer_text_heatmap_joinaggregate,
    "layer_text_heatmap_joinaggregate"
);


// Regressions fixed while aligning layer, facet, repeat and concat output.
example_test!(group_boxplot_1d_horizontal, "boxplot_1D_horizontal");
example_test!(group_boxplot_1d_horizontal_custom_mark, "boxplot_1D_horizontal_custom_mark");
example_test!(group_boxplot_1d_horizontal_explicit, "boxplot_1D_horizontal_explicit");
example_test!(group_boxplot_1d_invalid, "boxplot_1D_invalid");
example_test!(group_boxplot_1d_vertical, "boxplot_1D_vertical");
example_test!(group_boxplot_2d_horizontal, "boxplot_2D_horizontal");
example_test!(group_boxplot_2d_horizontal_color_size, "boxplot_2D_horizontal_color_size");
example_test!(group_boxplot_2d_vertical_single_per_group_color, "boxplot_2D_vertical_single_per_group_color");
example_test!(group_boxplot_groupped, "boxplot_groupped");
example_test!(group_boxplot_tooltip_aggregate, "boxplot_tooltip_aggregate");
example_test!(group_boxplot_tooltip_not_aggregate, "boxplot_tooltip_not_aggregate");
example_test!(group_concat_bar_layer_circle, "concat_bar_layer_circle");
example_test!(group_concat_bar_scales_discretize_2_cols, "concat_bar_scales_discretize_2_cols");
example_test!(group_concat_hover, "concat_hover");
example_test!(group_concat_hover_filter, "concat_hover_filter");
example_test!(group_concat_layer_voyager_result, "concat_layer_voyager_result");
example_test!(group_concat_population_pyramid, "concat_population_pyramid");
example_test!(group_concat_weather, "concat_weather");
example_test!(group_facet_bullet, "facet_bullet");
example_test!(group_facet_column_facet_column_point_future, "facet_column_facet_column_point_future");
example_test!(group_facet_column_facet_row_point_future, "facet_column_facet_row_point_future");
example_test!(group_facet_custom, "facet_custom");
example_test!(group_facet_custom_header, "facet_custom_header");
example_test!(group_facet_independent_scale, "facet_independent_scale");
example_test!(group_facet_independent_scale_layer_broken, "facet_independent_scale_layer_broken");
example_test!(group_facet_row_facet_row_point_future, "facet_row_facet_row_point_future");
example_test!(group_layer_bar_annotations, "layer_bar_annotations");
example_test!(group_layer_bar_fruit, "layer_bar_fruit");
example_test!(group_layer_bar_labels_grey, "layer_bar_labels_grey");
example_test!(group_layer_bar_tick_datum_grouped, "layer_bar_tick_datum_grouped");
example_test!(group_layer_boxplot_circle, "layer_boxplot_circle");
example_test!(group_layer_circle_independent_color, "layer_circle_independent_color");
example_test!(group_layer_cumulative_histogram, "layer_cumulative_histogram");
example_test!(group_layer_falkensee, "layer_falkensee");
example_test!(group_layer_likert, "layer_likert");
example_test!(group_layer_line_co2_concentration, "layer_line_co2_concentration");
example_test!(group_layer_line_errorband_2d_horizontal_borders_strokedash, "layer_line_errorband_2d_horizontal_borders_strokedash");
example_test!(group_layer_line_errorband_ci, "layer_line_errorband_ci");
example_test!(group_layer_line_errorband_pre_aggregated, "layer_line_errorband_pre_aggregated");
example_test!(group_layer_line_exponential, "layer_line_exponential");
example_test!(group_layer_overlay, "layer_overlay");
example_test!(group_layer_point_errorbar_2d_horizontal_color_encoding, "layer_point_errorbar_2d_horizontal_color_encoding");
example_test!(group_layer_point_errorbar_2d_horizontal_custom_ticks, "layer_point_errorbar_2d_horizontal_custom_ticks");
example_test!(group_layer_text_heatmap_joinaggregate, "layer_text_heatmap_joinaggregate");
example_test!(group_repeat_child_layer, "repeat_child_layer");
example_test!(group_repeat_histogram, "repeat_histogram");
example_test!(group_repeat_histogram_autosize, "repeat_histogram_autosize");
example_test!(group_repeat_histogram_flights, "repeat_histogram_flights");
example_test!(group_repeat_independent_colors, "repeat_independent_colors");
example_test!(group_repeat_splom, "repeat_splom");
example_test!(group_repeat_splom_cars, "repeat_splom_cars");
example_test!(group_trellis_area_seattle, "trellis_area_seattle");
example_test!(group_trellis_bar, "trellis_bar");
example_test!(group_trellis_bar_no_header, "trellis_bar_no_header");
example_test!(group_trellis_barley, "trellis_barley");
example_test!(group_trellis_barley_independent, "trellis_barley_independent");
example_test!(group_trellis_barley_layer_median, "trellis_barley_layer_median");
example_test!(group_trellis_column_year, "trellis_column_year");
example_test!(group_trellis_cross_sort, "trellis_cross_sort");
example_test!(group_trellis_cross_sort_array, "trellis_cross_sort_array");
example_test!(group_trellis_line_quarter, "trellis_line_quarter");
example_test!(group_trellis_scatter, "trellis_scatter");
example_test!(group_trellis_stacked_bar, "trellis_stacked_bar");

// Projection merging: renamed projection names must be applied to references
// inside data flow transforms, mark transforms and selection signals, and
// ExprRefs in projection properties must be resolved to signal refs.
example_test!(geo_constant_value, "geo_constant_value");
example_test!(geo_custom_projection, "geo_custom_projection");
example_test!(geo_layer, "geo_layer");
example_test!(geo_layer_line_london, "geo_layer_line_london");
example_test!(geo_sphere, "geo_sphere");
example_test!(geo_text, "geo_text");
example_test!(geo_params_projections, "geo_params_projections");
example_test!(
    interactive_1d_geo_brush,
    "interactive_1d_geo_brush"
);
example_test!(
    interactive_geo_earthquakes,
    "interactive_geo_earthquakes"
);
// Exact-output regressions for mark/encode and facet/concat assembly details:
// group-relative arc radius, custom tooltip number formats, pivot-derived
// tooltips, scale-bound clip propagation, facet header offsets, encoded
// position offsets and facet selection unit names.
example_test!(arc_facet, "arc_facet");
example_test!(
    config_number_format_type_test,
    "config_numberFormatType_test"
);
example_test!(
    interactive_multi_line_pivot_tooltip,
    "interactive_multi_line_pivot_tooltip"
);
example_test!(
    interactive_panzoom_vconcat_shared,
    "interactive_panzoom_vconcat_shared"
);
example_test!(line_concat_facet, "line_concat_facet");
example_test!(text_params, "text_params");
example_test!(trellis_selections, "trellis_selections");

// Lookup transforms must keep their secondary source and output nodes in the
// optimized dataflow.
example_test!(geo_choropleth, "geo_choropleth");
example_test!(lookup, "lookup");

// Selection materialization and identifier nodes in split data branches.
example_test!(
    interactive_global_development,
    "interactive_global_development"
);
example_test!(
    interactive_histogram_full_height_hover,
    "interactive_histogram_full_height_hover"
);

// Invalid-data handling feeds scale domains from the pre/post-filter outputs,
// and local datetimes follow the example-corpus timezone (America/Los_Angeles).
example_test!(
    test_invalid_break_paths_filter_domains,
    "test_invalid_break_paths_filter_domains"
);
example_test!(
    test_invalid_break_paths_show_domains,
    "test_invalid_break_paths_show_domains"
);
example_test!(interactive_index_chart, "interactive_index_chart");

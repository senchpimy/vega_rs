//! Port of `test/stack.test.ts`.

use serde_json::{json, Value};
use vega_lite::log;
use vega_lite::mark::{Mark, ARC, AREA, BAR, PRIMITIVE_MARKS, RECT};
use vega_lite::stack::{stack, StackOffset, STACKABLE_MARKS, STACK_BY_DEFAULT_MARKS};

const DETAIL: &str = "detail";
const X: &str = "x";
const Y: &str = "y";
const YOFFSET: &str = "yOffset";

fn stackable_non_polar_marks() -> Vec<Mark> {
    STACKABLE_MARKS
        .iter()
        .copied()
        .filter(|m| *m != ARC)
        .collect()
}

fn stack_by_default_non_polar_marks() -> Vec<Mark> {
    STACK_BY_DEFAULT_MARKS
        .iter()
        .copied()
        .filter(|m| *m != ARC)
        .collect()
}

/// `stack(spec.mark, spec.encoding)` on a JSON spec.
fn stack_spec(spec: &Value) -> Option<vega_lite::stack::StackProperties> {
    stack(&spec["mark"], &spec["encoding"])
}

/// Insert an explicit `stack` value into the encoding's `x` channel. `None`
/// leaves the property absent (i.e. JavaScript `undefined`).
fn set_stack(encoding: &mut Value, stack_value: Option<Value>) {
    if let Some(value) = stack_value {
        encoding["x"]["stack"] = value;
    }
}

#[test]
fn should_be_disabled_for_non_stackable_marks_with_at_least_one_of_the_stack_channel() {
    let mark = RECT;
    {
        let spec = json!({
            "data": {"url": "data/barley.json"},
            "mark": mark.as_str(),
            "encoding": {
                "x": {"aggregate": "sum", "field": "yield", "type": "quantitative"},
                "y": {"field": "variety", "type": "nominal"},
                "color": {"field": "site", "type": "nominal"},
            },
        });
        assert_eq!(stack_spec(&spec), None);
    }
}

#[test]
fn should_be_allowed_for_raw_plot() {
    for mark in stackable_non_polar_marks() {
        let spec = json!({
            "data": {"url": "data/barley.json"},
            "mark": mark.as_str(),
            "encoding": {
                "x": {"field": "yield", "type": "quantitative", "stack": "zero"},
                "y": {"field": "variety", "type": "nominal"},
                "color": {"field": "site", "type": "nominal"},
            },
        });
        let stack_props = stack_spec(&spec).expect("expected stack properties");
        assert_eq!(stack_props.field_channel, X);
    }
}

#[test]
fn doesnt_stacked_binned_field() {
    for mark in stackable_non_polar_marks() {
        let spec = json!({
            "data": {"url": "data/barley.json"},
            "mark": mark.as_str(),
            "encoding": {
                "x": {"field": "yield", "type": "quantitative", "bin": true},
            },
        });
        assert_eq!(stack_spec(&spec), None);
    }
}

#[test]
fn doesnt_stacked_the_dimension_field_on_a_1d_vertical_bar_with_dimension_only() {
    let spec = json!({
        "data": {"url": "data/barley.json"},
        "mark": {"type": "bar", "orient": "vertical"},
        "encoding": {
            "x": {"field": "yield", "type": "quantitative"},
        },
    });
    assert_eq!(stack_spec(&spec), None);
}

#[test]
fn doesnt_stacked_the_dimension_field_on_a_1d_horizontal_bar_with_dimension_only() {
    let spec = json!({
        "data": {"url": "data/barley.json"},
        "mark": {"type": "bar", "orient": "horizontal"},
        "encoding": {
            "y": {"field": "yield", "type": "quantitative"},
        },
    });
    assert_eq!(stack_spec(&spec), None);
}

#[test]
fn should_be_disabled_when_stack_is_false() {
    for mark in stackable_non_polar_marks() {
        let spec = json!({
            "data": {"url": "data/barley.json"},
            "mark": mark.as_str(),
            "encoding": {
                "x": {"aggregate": "mean", "field": "yield", "type": "quantitative", "stack": false},
                "y": {"field": "variety", "type": "nominal"},
                "color": {"field": "site", "type": "nominal"},
            },
        });
        assert_eq!(stack_spec(&spec), None);
    }
}

#[test]
fn should_stack_default_stack_marks() {
    for mark in STACK_BY_DEFAULT_MARKS {
        let spec = json!({
            "data": {"url": "data/barley.json"},
            "mark": mark.as_str(),
            "encoding": {
                "x": {"field": "yield", "type": "quantitative"},
                "y": {"field": "variety", "type": "nominal"},
            },
        });
        let stack_props = stack_spec(&spec).expect("expected stack properties");
        assert_eq!(stack_props.field_channel, X);
    }
}

#[test]
fn should_prioritize_axis_with_stack() {
    for mark in stackable_non_polar_marks() {
        let spec = json!({
            "data": {"url": "data/barley.json"},
            "mark": mark.as_str(),
            "encoding": {
                "x": {"field": "yield", "type": "quantitative", "stack": "zero"},
                "y": {"field": "variety", "type": "quantitative"},
                "color": {"field": "site", "type": "nominal"},
            },
        });
        let stack_props = stack_spec(&spec).expect("expected stack properties");
        assert_eq!(stack_props.field_channel, X);
    }
}

#[test]
fn should_disable_default_x_y_stacking_when_only_offset_is_present_on_the_orthogonal_axis() {
    for mark in [BAR, AREA] {
        assert_eq!(
            stack(
                &json!(mark.as_str()),
                &json!({
                    "x": {"field": "value", "type": "quantitative"},
                    "yOffset": {"field": "density", "type": "quantitative"},
                }),
            ),
            None
        );

        assert_eq!(
            stack(
                &json!(mark.as_str()),
                &json!({
                    "y": {"field": "value", "type": "quantitative"},
                    "xOffset": {"field": "density", "type": "quantitative"},
                }),
            ),
            None
        );
    }
}

#[test]
fn should_still_allow_explicit_stacking_on_x_y_when_offset_is_present() {
    for mark in [BAR, AREA] {
        let stack_props = stack(
            &json!(mark.as_str()),
            &json!({
                "x": {"field": "value", "type": "quantitative", "stack": "zero"},
                "yOffset": {"field": "density", "type": "quantitative"},
                "color": {"field": "site", "type": "nominal"},
            }),
        )
        .expect("expected stack properties");
        assert_eq!(stack_props.field_channel, X);
    }
}

#[test]
fn should_always_be_disabled_if_the_stackby_channel_is_aggregated() {
    let offsets: Vec<Option<Value>> = vec![
        None,
        Some(json!("center")),
        Some(json!("zero")),
        Some(json!("normalize")),
        Some(json!(null)),
        Some(json!("none")),
    ];
    for stack_value in offsets {
        for mark in PRIMITIVE_MARKS {
            let mut encoding = json!({
                "x": {"aggregate": "sum", "field": "yield", "type": "quantitative"},
                "y": {"field": "variety", "type": "nominal"},
                "color": {"aggregate": "count", "type": "quantitative"},
            });
            set_stack(&mut encoding, stack_value.clone());
            let _stack = stack(&json!(mark.as_str()), &encoding);
            assert_eq!(_stack, None);
        }
    }
}

#[test]
fn can_be_enabled_if_one_of_the_stackby_channels_is_not_aggregated() {
    let offsets: Vec<Option<Value>> = vec![
        None,
        Some(json!("center")),
        Some(json!("zero")),
        Some(json!("normalize")),
    ];
    for stack_value in offsets {
        let marks = if stack_value.is_none() {
            stack_by_default_non_polar_marks()
        } else {
            stackable_non_polar_marks()
        };
        for mark in marks {
            let mut encoding = json!({
                "x": {"aggregate": "sum", "field": "yield", "type": "quantitative"},
                "y": {"field": "variety", "type": "nominal"},
                "color": {"aggregate": "count", "type": "quantitative"},
                "detail": {"field": "site", "type": "nominal"},
            });
            set_stack(&mut encoding, stack_value.clone());

            let _stack =
                stack(&json!(mark.as_str()), &encoding).expect("expected stack properties");
            assert_eq!(_stack.stack_by[0].channel, DETAIL);
        }
    }
}

#[test]
fn should_not_include_tooltip_in_stack_by() {
    let spec = json!({
        "data": {"url": "data/barley.json"},
        "mark": "bar",
        "encoding": {
            "x": {"aggregate": "sum", "field": "yield", "type": "quantitative"},
            "y": {"field": "variety", "type": "nominal"},
            "detail": {"field": "site", "type": "nominal"},
            "tooltip": {"field": "total_yield", "type": "nominal"},
        },
    });

    let _stack = stack_spec(&spec).expect("expected stack properties");
    for stack_by in &_stack.stack_by {
        assert_ne!(stack_by.channel, "tooltip");
    }
}

#[test]
fn should_always_be_disabled_if_both_x_and_y_are_aggregate() {
    for mark in PRIMITIVE_MARKS {
        let spec = json!({
            "data": {"url": "data/barley.json"},
            "mark": mark.as_str(),
            "encoding": {
                "x": {"aggregate": "sum", "field": "yield", "type": "quantitative"},
                "y": {"aggregate": "count", "type": "quantitative"},
                "color": {"field": "site", "type": "nominal"},
            },
        });
        assert_eq!(stack_spec(&spec), None);
    }
}

#[test]
fn should_always_be_disabled_if_neither_x_nor_y_is_aggregate_or_stack() {
    for mark in PRIMITIVE_MARKS {
        let spec = json!({
            "data": {"url": "data/barley.json"},
            "mark": mark.as_str(),
            "encoding": {
                "x": {"field": "variety", "type": "nominal"},
                "y": {"field": "variety", "type": "nominal"},
                "color": {"field": "site", "type": "nominal"},
            },
        });
        assert_eq!(stack_spec(&spec), None);
    }
}

#[test]
fn should_always_be_disabled_if_there_is_both_x_and_x2() {
    let offsets: Vec<Option<Value>> = vec![
        None,
        Some(json!("center")),
        Some(json!("zero")),
        Some(json!("normalize")),
    ];
    for stack_value in offsets {
        let marks = if stack_value.is_none() {
            stack_by_default_non_polar_marks()
        } else {
            stackable_non_polar_marks()
        };
        for mark in marks {
            let mut encoding = json!({
                "x": {"field": "a", "type": "quantitative", "aggregate": "sum"},
                "x2": {"field": "a", "aggregate": "sum"},
                "y": {"field": "variety", "type": "nominal"},
                "color": {"field": "site", "type": "nominal"},
            });
            set_stack(&mut encoding, stack_value.clone());
            assert_eq!(stack(&json!(mark.as_str()), &encoding), None);
        }
    }
}

#[test]
fn should_always_be_disabled_if_there_is_both_y_and_y2() {
    let offsets: Vec<Option<Value>> = vec![
        None,
        Some(json!("center")),
        Some(json!("zero")),
        Some(json!("normalize")),
    ];
    for stack_value in offsets {
        let marks = if stack_value.is_none() {
            stack_by_default_non_polar_marks()
        } else {
            stackable_non_polar_marks()
        };
        for mark in marks {
            let mut encoding = json!({
                "y": {"field": "a", "type": "quantitative", "aggregate": "sum"},
                "y2": {"field": "a", "aggregate": "sum"},
                "x": {"field": "variety", "type": "nominal"},
                "color": {"field": "site", "type": "nominal"},
            });
            // `set_stack` targets x; for this test the stack belongs on y.
            if let Some(value) = &stack_value {
                encoding["y"]["stack"] = value.clone();
            }
            assert_eq!(stack(&json!(mark.as_str()), &encoding), None);
        }
    }
}

#[test]
fn should_always_return_stack_and_only_warn_for_non_undefined_stack_if_the_aggregated_axis_has_non_linear_scale(
) {
    let offsets: Vec<Option<&str>> = vec![None, Some("center"), Some("zero"), Some("normalize")];
    for stack_value in offsets {
        for scale_type in ["log", "pow", "sqrt"] {
            let marks = if stack_value.is_none() {
                stack_by_default_non_polar_marks()
            } else {
                stackable_non_polar_marks()
            };
            for mark in marks {
                let mut encoding = json!({
                    "x": {
                        "field": "a",
                        "type": "quantitative",
                        "aggregate": "sum",
                        "scale": {"type": scale_type},
                    },
                    "y": {"field": "variety", "type": "nominal"},
                    "color": {"field": "site", "type": "nominal"},
                });
                if let Some(stack_value) = stack_value {
                    encoding["x"]["stack"] = json!(stack_value);
                }

                let (result, logger) = log::capture(|| stack(&json!(mark.as_str()), &encoding));
                assert!(result.is_some(), "expected stack properties");
                let warns = logger.warns;

                if stack_value.is_some() {
                    assert_eq!(
                        warns.last(),
                        Some(&json!(log::message::stack_non_linear_scale(scale_type)))
                    );
                } else {
                    assert_eq!(warns.len(), 0);
                }
            }
        }
    }
}

#[test]
fn should_throw_warning_if_the_aggregated_axis_has_a_non_summative_aggregate() {
    let offsets: Vec<Option<&str>> = vec![None, Some("center"), Some("zero"), Some("normalize")];
    for stack_value in offsets {
        for aggregate in ["average", "variance", "q3"] {
            let marks = if stack_value.is_none() {
                stack_by_default_non_polar_marks()
            } else {
                stackable_non_polar_marks()
            };
            for mark in marks {
                let mut encoding = json!({
                    "x": {
                        "aggregate": aggregate,
                        "field": "a",
                        "type": "quantitative",
                    },
                    "y": {"field": "variety", "type": "nominal"},
                    "color": {"field": "site", "type": "nominal"},
                });
                if let Some(stack_value) = stack_value {
                    encoding["x"]["stack"] = json!(stack_value);
                }

                let (_result, logger) = log::capture(|| stack(&json!(mark.as_str()), &encoding));
                let warns = logger.warns;
                assert_eq!(
                    warns.last(),
                    Some(&json!(log::message::stack_non_summative_aggregate(
                        aggregate
                    )))
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// stack().groupbyChannels, .fieldChannels
// ---------------------------------------------------------------------------

#[test]
fn should_be_correct_for_horizontal() {
    for stackable_mark in [BAR, AREA] {
        let spec = json!({
            "data": {"url": "data/barley.json"},
            "mark": stackable_mark.as_str(),
            "encoding": {
                "x": {"aggregate": "sum", "field": "yield", "type": "quantitative"},
                "y": {"field": "variety", "type": "nominal"},
                "color": {"field": "site", "type": "nominal"},
            },
        });
        let _stack = stack_spec(&spec).expect("expected stack properties");
        assert_eq!(_stack.field_channel, X);
        assert_eq!(_stack.groupby_channels, vec![Y]);
    }
}

#[test]
fn should_be_correct_for_grouped_bar() {
    for stackable_mark in [BAR, AREA] {
        let spec = json!({
            "data": {"url": "data/barley.json"},
            "mark": stackable_mark.as_str(),
            "encoding": {
                "x": {"field": "yield", "type": "quantitative"},
                "y": {"field": "variety", "type": "nominal"},
                "yOffset": {"field": "site", "type": "nominal"},
                "color": {"field": "site", "type": "nominal"},
            },
        });
        let _stack = stack_spec(&spec).expect("expected stack properties");
        assert_eq!(_stack.field_channel, X);
        assert_eq!(_stack.groupby_channels, vec![Y, YOFFSET]);
    }
}

#[test]
fn should_be_correct_for_grouped_bar_without_nesting() {
    for stackable_mark in [BAR, AREA] {
        let spec = json!({
            "data": {"url": "data/barley.json"},
            "mark": stackable_mark.as_str(),
            "encoding": {
                "x": {"field": "yield", "type": "quantitative"},
                "yOffset": {"field": "site", "type": "nominal"},
                "color": {"field": "site", "type": "nominal"},
            },
        });
        let _stack = stack_spec(&spec).expect("expected stack properties");
        assert_eq!(_stack.field_channel, X);
        assert_eq!(_stack.groupby_channels, vec![YOFFSET]);
    }
}

#[test]
fn should_stack_even_for_two_plain_quantitatives_with_the_orient_on_the_axes() {
    for mark in [BAR, AREA] {
        let spec = json!({
            "data": {"url": "data/movies.json"},
            "mark": {"type": mark.as_str(), "orient": "vertical"},
            "encoding": {
                "x": {"field": "IMDB Rating", "type": "quantitative"},
                "y": {"field": "Rotten Tomatoes Rating", "type": "quantitative"},
            },
        });
        let stack_props = stack_spec(&spec).expect("expected stack properties");
        assert_eq!(stack_props.field_channel, Y);
    }
}

#[test]
fn should_be_correct_for_horizontal_single() {
    for stackable_mark in [BAR, AREA] {
        let spec = json!({
            "data": {"url": "data/barley.json"},
            "mark": stackable_mark.as_str(),
            "encoding": {
                "x": {"aggregate": "sum", "field": "yield", "type": "quantitative"},
                "color": {"field": "site", "type": "nominal"},
            },
        });
        let _stack = stack_spec(&spec).expect("expected stack properties");
        assert_eq!(_stack.field_channel, X);
        assert!(_stack.groupby_channels.is_empty());
    }
}

#[test]
fn should_be_correct_for_vertical() {
    for stackable_mark in [BAR, AREA] {
        let spec = json!({
            "data": {"url": "data/barley.json"},
            "mark": stackable_mark.as_str(),
            "encoding": {
                "y": {"aggregate": "sum", "field": "yield", "type": "quantitative"},
                "x": {"field": "variety", "type": "nominal"},
                "color": {"field": "site", "type": "nominal"},
            },
        });
        let _stack = stack_spec(&spec).expect("expected stack properties");
        assert_eq!(_stack.field_channel, Y);
        assert_eq!(_stack.groupby_channels, vec![X]);
    }
}

#[test]
fn should_be_correct_for_vertical_single() {
    for stackable_mark in [BAR, AREA] {
        let spec = json!({
            "data": {"url": "data/barley.json"},
            "mark": stackable_mark.as_str(),
            "encoding": {
                "y": {"aggregate": "sum", "field": "yield", "type": "quantitative"},
                "color": {"field": "site", "type": "nominal"},
            },
        });
        let _stack = stack_spec(&spec).expect("expected stack properties");
        assert_eq!(_stack.field_channel, Y);
        assert!(_stack.groupby_channels.is_empty());
    }
}

#[test]
fn should_be_correct_for_pie() {
    let spec = json!({
        "data": {"url": "data/barley.json"},
        "mark": "arc",
        "encoding": {
            "theta": {"field": "field", "type": "quantitative"},
            "color": {"field": "id", "type": "nominal"},
        },
    });
    let _stack = stack_spec(&spec).expect("expected stack properties");
    assert_eq!(_stack.field_channel, "theta");
    assert_eq!(_stack.stack_by[0].channel, "color");
}

#[test]
fn should_be_correct_for_radial_chart() {
    let spec = json!({
        "data": {"url": "data/barley.json"},
        "mark": "arc",
        "encoding": {
            "theta": {"field": "field", "type": "quantitative", "stack": true},
            "radius": {
                "field": "field",
                "type": "quantitative",
                "scale": {"type": "sqrt", "zero": true, "range": [20, 100]},
            },
        },
    });
    let _stack = stack_spec(&spec).expect("expected stack properties");
    assert_eq!(_stack.field_channel, "theta");
}

// ---------------------------------------------------------------------------
// stack().offset
// ---------------------------------------------------------------------------

#[test]
fn should_be_zero_for_stackable_marks_with_at_least_of_of_the_stack_channel_if_stacked_is_unspecified_or_true(
) {
    let offsets: Vec<Option<Value>> = vec![None, Some(json!(true))];
    for stack_value in offsets {
        for stackable_mark in [BAR, AREA] {
            let mut encoding = json!({
                "x": {"aggregate": "sum", "field": "yield", "type": "quantitative"},
                "y": {"field": "variety", "type": "nominal"},
                "color": {"field": "site", "type": "nominal"},
            });
            set_stack(&mut encoding, stack_value.clone());
            let stack_props =
                stack(&json!(stackable_mark.as_str()), &encoding).expect("expected stack");
            assert_eq!(stack_props.offset, StackOffset::Zero);
        }
    }
}

#[test]
fn should_be_the_specified_stacked_for_stackable_marks_with_at_least_one_of_the_stack_channel() {
    for stack_value in ["center", "zero", "normalize"] {
        for stackable_mark in [BAR, AREA] {
            let mut encoding = json!({
                "x": {"aggregate": "sum", "field": "yield", "type": "quantitative"},
                "y": {"field": "variety", "type": "nominal"},
                "color": {"field": "site", "type": "nominal"},
            });
            encoding["x"]["stack"] = json!(stack_value);
            let stack_props =
                stack(&json!(stackable_mark.as_str()), &encoding).expect("expected stack");
            assert_eq!(
                stack_props.offset,
                stack_value.parse::<StackOffset>().unwrap()
            );
        }
    }
}

#[test]
fn should_impute_area() {
    let spec = json!({
        "data": {"url": "data/barley.json"},
        "mark": "area",
        "encoding": {
            "x": {"aggregate": "sum", "field": "yield", "type": "quantitative", "stack": "zero"},
            "y": {"field": "variety", "type": "nominal"},
            "color": {"field": "site", "type": "nominal"},
        },
    });
    assert!(stack_spec(&spec).expect("expected stack").impute);
}

#[test]
fn should_allow_disabling_of_imputation() {
    let spec = json!({
        "data": {"url": "data/barley.json"},
        "mark": "area",
        "encoding": {
            "x": {"aggregate": "sum", "field": "yield", "type": "quantitative", "stack": "zero", "impute": null},
            "y": {"field": "variety", "type": "nominal"},
            "color": {"field": "site", "type": "nominal"},
        },
    });
    assert!(!stack_spec(&spec).expect("expected stack").impute);
}

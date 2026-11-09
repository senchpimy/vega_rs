//! Sanity tests for the `mark` and `aggregate` ports.

use serde_json::json;
use vega_lite::aggregate::*;
use vega_lite::mark::*;

#[test]
fn mark_constants_and_predicates() {
    assert_eq!(Mark::ALL.len(), 14);
    assert_eq!(PRIMITIVE_MARKS.len(), 14);
    assert_eq!(MARK_INDEX.len(), 14);

    for mark in Mark::ALL {
        assert!(is_mark(mark.as_str()));
        assert!(is_primitive_mark(&mark));
        assert!(is_primitive_mark(mark.as_str()));
        assert!(!is_composite_mark(mark.as_str()));
    }

    assert_eq!(ARC, Mark::Arc);
    assert_eq!(GEOSHAPE.as_str(), "geoshape");
    assert_eq!("trail".parse::<Mark>(), Ok(Mark::Trail));
    assert_eq!("bogus".parse::<Mark>(), Err(()));
    assert!(!is_mark("boxplot"));
}

#[test]
fn composite_and_mark_def_predicates() {
    assert_eq!(COMPOSITE_MARKS.len(), 3);
    for m in COMPOSITE_MARKS {
        assert!(is_composite_mark(m));
        assert!(!is_primitive_mark(m));
    }
    assert!(is_composite_mark("boxplot"));
    assert!(!is_composite_mark("bar"));

    let def = json!({"type": "bar", "color": "red"});
    assert!(is_mark_def(&def));
    assert!(!is_mark_def(&json!("bar")));
    assert_eq!(get_mark_type(&def), Some("bar"));
    assert_eq!(get_mark_type(&json!("line")), Some("line"));
    assert_eq!(get_mark_type(&json!({"type": "area"})), Some("area"));
    assert!(is_primitive_mark(&def));

    let mark_def = MarkDef {
        mark_type: "point".to_string(),
        props: Default::default(),
    };
    assert!(is_primitive_mark(&mark_def));
    assert_eq!(get_mark_type(&mark_def), Some("point"));
}

#[test]
fn mark_sets() {
    for m in ["line", "area", "trail"] {
        assert!(is_path_mark(m));
        assert!(is_continuous_to_continuous_mark(m));
    }
    assert!(!is_path_mark("bar"));

    for m in ["rect", "bar", "image", "arc", "tick"] {
        assert!(is_rect_based_mark(m));
    }
    assert!(!is_rect_based_mark("line"));

    assert!(is_bar_or_area("bar"));
    assert!(is_bar_or_area("area"));
    assert!(!is_bar_or_area("point"));

    assert_eq!(STACKABLE_MARKS.len(), 10);
    assert_eq!(STACK_BY_DEFAULT_MARKS.len(), 3);
    assert!(is_stackable_mark("text"));
    assert!(!is_stackable_mark("geoshape"));
    assert!(is_stack_by_default_mark("arc"));
    assert!(!is_stack_by_default_mark("line"));
}

#[test]
fn mark_config_helpers() {
    assert_eq!(
        VL_ONLY_MARK_CONFIG_PROPERTIES,
        [
            "color",
            "filled",
            "invalid",
            "order",
            "radius2",
            "theta2",
            "timeUnitBandSize",
            "timeUnitBandPosition"
        ]
    );
    assert_eq!(
        FILL_STROKE_CONFIG.len(),
        STROKE_CONFIG.len() + FILL_CONFIG.len()
    );
    assert_eq!(MARK_CONFIGS.len(), 15);

    assert_eq!(
        vl_only_mark_specific_config_properties("area"),
        vec!["line", "point"]
    );
    assert_eq!(
        vl_only_mark_specific_config_properties("tick"),
        vec![
            "bandSize",
            "thickness",
            "binSpacing",
            "continuousBandSize",
            "discreteBandSize",
            "minBandSize"
        ]
    );

    assert_eq!(default_mark_config()["color"], "#4c78a8");
    assert_eq!(default_bar_config()["binSpacing"], 1);
    assert_eq!(default_rect_config()["binSpacing"], 0);
    assert_eq!(default_tick_config()["thickness"], 1);

    assert!(is_relative_band_size(&json!({"band": 0.5})));
    assert!(!is_relative_band_size(&json!(0.5)));
}

#[test]
fn mark_style_and_config_lookup() {
    let mark = json!({"type": "bar", "color": "red", "style": "foo"});
    assert_eq!(get_styles(&mark), vec!["bar", "foo"]);

    let config = json!({
        "mark": {"opacity": 0.5, "strokeWidth": 9},
        "bar": {"size": 10, "strokeWidth": 7},
        "style": {"foo": {"strokeWidth": 2}}
    });

    assert_eq!(
        get_mark_style_config("strokeWidth", &mark, &config["style"]),
        Some(json!(2))
    );
    assert_eq!(
        get_mark_config("size", &mark, &config, None),
        Some(json!(10))
    );
    // style config has higher precedence
    assert_eq!(
        get_mark_config("strokeWidth", &mark, &config, None),
        Some(json!(2))
    );
    assert_eq!(
        get_mark_config("opacity", &mark, &config, None),
        Some(json!(0.5))
    );
    assert_eq!(get_mark_config("unknown", &mark, &config, None), None);

    assert_eq!(
        get_mark_prop_or_config("color", &mark, &config, None, false),
        Some(json!("red"))
    );
    assert_eq!(
        get_mark_prop_or_config("size", &mark, &config, None, false),
        Some(json!(10))
    );
    // ignoreVgConfig suppresses config lookups when no vgChannel differs
    assert_eq!(
        get_mark_prop_or_config("size", &mark, &config, None, true),
        None
    );
}

#[test]
fn aggregate_op_constants() {
    assert_eq!(AGGREGATE_OP_INDEX.len(), 25);
    assert_eq!(AGGREGATE_OPS.len(), 25);
    for (name, op) in AGGREGATE_OP_INDEX.iter().zip(AGGREGATE_OPS) {
        assert_eq!(op.as_str(), *name);
        assert!(is_aggregate_op(name));
    }
    assert!(is_aggregate_op("exponentialb"));
    assert!(is_aggregate_op("argmin"));
    assert!(!is_aggregate_op("nope"));
    assert_eq!("q1".parse::<AggregateOp>(), Ok(AggregateOp::Q1));
    assert_eq!("nope".parse::<AggregateOp>(), Err(()));
}

#[test]
fn aggregate_defs() {
    let exp = Aggregate::Exponential(0.5);
    let expb = Aggregate::ExponentialB(2.0);
    let argmin = Aggregate::Argmin("a".into());
    let argmax = Aggregate::Argmax("b".into());
    let sum = Aggregate::Op(AggregateOp::Sum);

    assert!(is_exponential_def(&exp));
    assert!(is_exponentialb_def(&expb));
    assert!(is_parameterized_aggregate_def(&exp));
    assert!(is_parameterized_aggregate_def(&expb));
    assert!(!is_parameterized_aggregate_def(&sum));

    assert_eq!(get_aggregate_op(&exp), "exponential");
    assert_eq!(get_aggregate_op(&expb), "exponentialb");
    assert_eq!(get_aggregate_op(&sum), "sum");
    assert_eq!(get_aggregate_param(&exp), Some(0.5));
    assert_eq!(get_aggregate_param(&expb), Some(2.0));
    assert_eq!(get_aggregate_param(&sum), None);

    assert!(is_argmin_def(&argmin));
    assert!(is_argmax_def(&argmax));
    assert!(!is_argmin_def(&argmax));
    assert_eq!(get_field(&argmin), Some("a".to_string()));
    assert_eq!(get_field(&argmax), Some("b".to_string()));
    assert_eq!(get_field(&sum), None);
}

#[test]
fn aggregate_sets_and_predicates() {
    let count = Aggregate::Op(AggregateOp::Count);
    let sum = Aggregate::Op(AggregateOp::Sum);
    let min = Aggregate::Op(AggregateOp::Min);

    assert!(is_counting_aggregate_op(Some(&count)));
    assert!(is_counting_aggregate_op(Some(&Aggregate::Op(
        AggregateOp::Distinct
    ))));
    assert!(!is_counting_aggregate_op(Some(&sum)));
    assert!(!is_counting_aggregate_op(None));
    assert!(is_counting_op_name("valid"));

    assert!(is_min_max_op(Some(&min)));
    assert!(is_min_max_op(Some(&Aggregate::Op(AggregateOp::Max))));
    assert!(!is_min_max_op(Some(&sum)));
    assert!(is_min_max_op_name("max"));

    assert_eq!(COUNTING_OPS.len(), 4);
    assert_eq!(SUM_OPS.len(), 5);
    assert_eq!(SHARED_DOMAIN_OPS.len(), 7);
    assert_eq!(SHARED_DOMAIN_OP_INDEX, SHARED_DOMAIN_OPS);
    assert_eq!(MULTIDOMAIN_SORT_OP_INDEX, ["count", "min", "max"]);
    assert!(SHARED_DOMAIN_OPS.contains(&"q1"));
    assert!(!SHARED_DOMAIN_OPS.contains(&"sum"));
}

#[test]
fn aggregate_json_predicates() {
    assert!(is_aggregate(&json!("mean")));
    assert!(is_aggregate(&json!({"argmin": "a"})));
    assert!(is_aggregate(&json!({"exponential": 0.5})));
    assert!(!is_aggregate(&json!("bogus")));
    assert!(!is_aggregate(&json!(1)));

    assert_eq!(
        get_field_from_value(&json!({"argmax": "x"})),
        Some("x".to_string())
    );
    assert_eq!(
        get_field_from_value(&json!({"argmax": {"field": "x"}})),
        Some("x".to_string())
    );
    assert_eq!(get_field_from_value(&json!({"mean": {}})), None);

    assert!(is_composite_aggregate(&json!("boxplot")));
    assert!(is_composite_aggregate(&json!({"type": "errorbar"})));
    assert!(!is_composite_aggregate(&json!("mean")));
}

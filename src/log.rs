//! Port of `src/log/index.ts` and the subset of `src/log/message.ts` used so far.

use std::cell::RefCell;

use serde_json::Value;

pub mod message {
    use super::*;
    use crate::util::stringify;

    // TIMEUNIT
    pub fn invalid_time_unit(unit_name: &str, value: &Value) -> String {
        format!("Invalid {unit_name}: {}.", stringify(value))
    }

    pub fn dropped_day(d: &Value) -> String {
        format!(
            "Dropping day from datetime {} as day cannot be combined with other units.",
            stringify(d)
        )
    }

    // STACK
    pub fn cannot_stack_ranged_mark(channel: &str) -> String {
        format!("Cannot stack \"{channel}\" if there is already \"{channel}2\".")
    }

    pub fn stack_non_linear_scale(scale_type: &str) -> String {
        format!("Stack is applied to a non-linear scale ({scale_type}).")
    }

    pub fn stack_non_summative_aggregate(aggregate: &str) -> String {
        format!("Stacking is applied even though the aggregate function is non-summative (\"{aggregate}\").")
    }

    // ENCODING
    pub fn invalid_field_type(t: &str) -> String {
        format!("Invalid field type \"{t}\".")
    }

    pub fn invalid_field_type_for_count_aggregate(t: &str, aggregate: &str) -> String {
        format!("Invalid field type \"{t}\" for aggregate: \"{aggregate}\", using \"quantitative\" instead.")
    }

    pub fn invalid_aggregate(aggregate: &str) -> String {
        format!("Invalid aggregation operator \"{aggregate}\".")
    }

    pub fn missing_field_type(channel: &str, new_type: &str) -> String {
        format!("Missing type for channel \"{channel}\", using \"{new_type}\" instead.")
    }

    pub fn empty_field_def(field_def: &Value, channel: &str) -> String {
        format!(
            "Dropping {} from channel \"{channel}\" since it does not contain any data field, datum, value, or signal.",
            stringify(field_def)
        )
    }

    pub fn incompatible_channel(channel: &str, mark_or_facet: &str, when: Option<&str>) -> String {
        format!(
            "{channel} dropped as it is incompatible with \"{mark_or_facet}\"{}.",
            when.map(|w| format!(" when {w}")).unwrap_or_default()
        )
    }

    pub fn invalid_encoding_channel(channel: &str) -> String {
        format!("{channel}-encoding is dropped as {channel} is not a valid encoding channel.")
    }

    pub fn channel_should_be_discrete(channel: &str) -> String {
        format!("{channel} encoding should be discrete (ordinal / nominal / binned).")
    }

    // SCALE
    pub fn invalid_scale_property(prop: &str, scale_type: &str) -> String {
        format!("Invalid scale property {prop} for scale type {scale_type}.")
    }

    pub fn invalid_spec(spec: &Value) -> String {
        format!(
            "Invalid specification {}. Make sure the specification includes at least one of the following properties: \"mark\", \"layer\", \"facet\", \"hconcat\", \"vconcat\", \"concat\", or \"repeat\".",
            stringify(spec)
        )
    }

    pub const FIT_NON_SINGLE: &str = "Autosize \"fit\" only works for single views and layered views.";

    pub fn container_size_non_single(name: &str) -> String {
        let u_name = if name == "width" { "Width" } else { "Height" };
        format!("{u_name} \"container\" only works for single views and layered views.")
    }

    pub fn container_size_not_compatible_with_autosize(name: &str) -> String {
        let u_name = if name == "width" { "Width" } else { "Height" };
        let fit_direction = if name == "width" { "x" } else { "y" };
        format!("{u_name} \"container\" only works well with autosize \"fit\" or \"fit-{fit_direction}\".")
    }

    pub fn offset_nested_inside_continuous_position_scale_dropped(main_channel: &str) -> String {
        format!("{main_channel}Offset dropped because {main_channel} is continuous")
    }

    pub fn dropping_fit(channel: Option<&str>) -> String {
        match channel {
            Some(channel) => format!(
                "Dropping \"fit-{channel}\" because spec has discrete {}.",
                if channel == "x" { "width" } else { "height" }
            ),
            None => "Dropping \"fit\" because spec has discrete size.".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LocalLogger {
    pub warns: Vec<Value>,
    pub infos: Vec<Value>,
    pub debugs: Vec<Value>,
    pub level: u8,
}

impl Default for LocalLogger {
    fn default() -> Self {
        Self {
            warns: Vec::new(),
            infos: Vec::new(),
            debugs: Vec::new(),
            level: WARN,
        }
    }
}

pub const NONE: u8 = 0;
pub const ERROR: u8 = 1;
pub const WARN: u8 = 2;
pub const INFO: u8 = 3;
pub const DEBUG: u8 = 4;

thread_local! {
    static LOGGERS: RefCell<Vec<LocalLogger>> = const { RefCell::new(Vec::new()) };
}

/// Runs `f` capturing all warnings/infos/debugs it emits. Returns the result and
/// the captured logger.
pub fn capture<T>(f: impl FnOnce() -> T) -> (T, LocalLogger) {
    LOGGERS.with(|l| l.borrow_mut().push(LocalLogger::default()));
    let result = f();
    let logger = LOGGERS.with(|l| l.borrow_mut().pop().unwrap());
    (result, logger)
}

pub fn warn(m: impl Into<Value>) {
    LOGGERS.with(|l| {
        if let Some(logger) = l.borrow_mut().last_mut() {
            if logger.level >= WARN {
                logger.warns.push(m.into());
            }
        }
    });
}

pub fn info(m: impl Into<Value>) {
    LOGGERS.with(|l| {
        if let Some(logger) = l.borrow_mut().last_mut() {
            if logger.level >= INFO {
                logger.infos.push(m.into());
            }
        }
    });
}

pub fn debug(m: impl Into<Value>) {
    LOGGERS.with(|l| {
        if let Some(logger) = l.borrow_mut().last_mut() {
            if logger.level >= DEBUG {
                logger.debugs.push(m.into());
            }
        }
    });
}

pub fn error(m: impl Into<Value>) {
    let m = m.into();
    LOGGERS.with(|l| {
        if let Some(logger) = l.borrow_mut().last_mut() {
            if logger.level >= ERROR {
                panic!("{}", m.as_str().unwrap_or_default());
            }
        }
    });
    panic!("{}", m.as_str().unwrap_or_default());
}

//! Port of `src/aggregate.ts`.
//!
//! `FieldName` (from the out-of-scope `channeldef.ts`) is represented as a
//! `String` here. Aggregate definitions are modelled by [`Aggregate`] instead
//! of a TypeScript union.

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mark::is_composite_mark;
use crate::util::has_property;

/// All Vega aggregate operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AggregateOp {
    Argmax,
    Argmin,
    Average,
    Count,
    Distinct,
    Exponential,
    Exponentialb,
    Product,
    Max,
    Mean,
    Median,
    Min,
    Missing,
    Q1,
    Q3,
    Ci0,
    Ci1,
    Stderr,
    Stdev,
    Stdevp,
    Sum,
    Valid,
    Values,
    Variance,
    Variancep,
}

impl AggregateOp {
    pub fn as_str(&self) -> &'static str {
        match self {
            AggregateOp::Argmax => "argmax",
            AggregateOp::Argmin => "argmin",
            AggregateOp::Average => "average",
            AggregateOp::Count => "count",
            AggregateOp::Distinct => "distinct",
            AggregateOp::Exponential => "exponential",
            AggregateOp::Exponentialb => "exponentialb",
            AggregateOp::Product => "product",
            AggregateOp::Max => "max",
            AggregateOp::Mean => "mean",
            AggregateOp::Median => "median",
            AggregateOp::Min => "min",
            AggregateOp::Missing => "missing",
            AggregateOp::Q1 => "q1",
            AggregateOp::Q3 => "q3",
            AggregateOp::Ci0 => "ci0",
            AggregateOp::Ci1 => "ci1",
            AggregateOp::Stderr => "stderr",
            AggregateOp::Stdev => "stdev",
            AggregateOp::Stdevp => "stdevp",
            AggregateOp::Sum => "sum",
            AggregateOp::Valid => "valid",
            AggregateOp::Values => "values",
            AggregateOp::Variance => "variance",
            AggregateOp::Variancep => "variancep",
        }
    }
}

impl std::fmt::Display for AggregateOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for AggregateOp {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "argmax" => Ok(AggregateOp::Argmax),
            "argmin" => Ok(AggregateOp::Argmin),
            "average" => Ok(AggregateOp::Average),
            "count" => Ok(AggregateOp::Count),
            "distinct" => Ok(AggregateOp::Distinct),
            "exponential" => Ok(AggregateOp::Exponential),
            "exponentialb" => Ok(AggregateOp::Exponentialb),
            "product" => Ok(AggregateOp::Product),
            "max" => Ok(AggregateOp::Max),
            "mean" => Ok(AggregateOp::Mean),
            "median" => Ok(AggregateOp::Median),
            "min" => Ok(AggregateOp::Min),
            "missing" => Ok(AggregateOp::Missing),
            "q1" => Ok(AggregateOp::Q1),
            "q3" => Ok(AggregateOp::Q3),
            "ci0" => Ok(AggregateOp::Ci0),
            "ci1" => Ok(AggregateOp::Ci1),
            "stderr" => Ok(AggregateOp::Stderr),
            "stdev" => Ok(AggregateOp::Stdev),
            "stdevp" => Ok(AggregateOp::Stdevp),
            "sum" => Ok(AggregateOp::Sum),
            "valid" => Ok(AggregateOp::Valid),
            "values" => Ok(AggregateOp::Values),
            "variance" => Ok(AggregateOp::Variance),
            "variancep" => Ok(AggregateOp::Variancep),
            _ => Err(()),
        }
    }
}

/// Membership index for all aggregate operators (`keys(AGGREGATE_OP_INDEX)`).
pub const AGGREGATE_OP_INDEX: [&str; 25] = [
    "argmax",
    "argmin",
    "average",
    "count",
    "distinct",
    "exponential",
    "exponentialb",
    "product",
    "max",
    "mean",
    "median",
    "min",
    "missing",
    "q1",
    "q3",
    "ci0",
    "ci1",
    "stderr",
    "stdev",
    "stdevp",
    "sum",
    "valid",
    "values",
    "variance",
    "variancep",
];

/// All aggregate operators, in the same order as `AGGREGATE_OP_INDEX`.
pub const AGGREGATE_OPS: [AggregateOp; 25] = [
    AggregateOp::Argmax,
    AggregateOp::Argmin,
    AggregateOp::Average,
    AggregateOp::Count,
    AggregateOp::Distinct,
    AggregateOp::Exponential,
    AggregateOp::Exponentialb,
    AggregateOp::Product,
    AggregateOp::Max,
    AggregateOp::Mean,
    AggregateOp::Median,
    AggregateOp::Min,
    AggregateOp::Missing,
    AggregateOp::Q1,
    AggregateOp::Q3,
    AggregateOp::Ci0,
    AggregateOp::Ci1,
    AggregateOp::Stderr,
    AggregateOp::Stdev,
    AggregateOp::Stdevp,
    AggregateOp::Sum,
    AggregateOp::Valid,
    AggregateOp::Values,
    AggregateOp::Variance,
    AggregateOp::Variancep,
];

pub const MULTIDOMAIN_SORT_OP_INDEX: [&str; 3] = ["count", "min", "max"];

/// An aggregate that takes a field name, e.g. `{"argmin": "field"}`.
pub type FieldName = String;

/// An aggregate operation that takes an aggregate parameter, expressed as an
/// object such as `{"exponential": 0.5}`.
#[derive(Debug, Clone, PartialEq)]
pub enum Aggregate {
    /// A plain (non-parameterized) aggregate operator.
    Op(AggregateOp),
    /// `{"argmin": field}`
    Argmin(FieldName),
    /// `{"argmax": field}`
    Argmax(FieldName),
    /// `{"exponential": number}`
    Exponential(f64),
    /// `{"exponentialb": number}`
    ExponentialB(f64),
}

impl Aggregate {
    pub fn as_op(&self) -> Option<AggregateOp> {
        match self {
            Aggregate::Op(op) => Some(*op),
            _ => None,
        }
    }

    /// The operation name, unwrapping parameterized aggregate defs to their op.
    pub fn op_name(&self) -> &'static str {
        match self {
            Aggregate::Op(op) => op.as_str(),
            Aggregate::Argmin(_) => "argmin",
            Aggregate::Argmax(_) => "argmax",
            Aggregate::Exponential(_) => "exponential",
            Aggregate::ExponentialB(_) => "exponentialb",
        }
    }

    /// The parameter of a parameterized aggregate def, if any.
    pub fn param(&self) -> Option<f64> {
        match self {
            Aggregate::Exponential(v) | Aggregate::ExponentialB(v) => Some(*v),
            _ => None,
        }
    }
}

impl From<AggregateOp> for Aggregate {
    fn from(op: AggregateOp) -> Self {
        Aggregate::Op(op)
    }
}

impl FromStr for Aggregate {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<AggregateOp>().map(Aggregate::Op)
    }
}

/// Port of `isArgminDef`.
pub fn is_argmin_def(a: &Aggregate) -> bool {
    matches!(a, Aggregate::Argmin(_))
}

/// Port of `isArgmaxDef`.
pub fn is_argmax_def(a: &Aggregate) -> bool {
    matches!(a, Aggregate::Argmax(_))
}

/// Port of `isExponentialDef`.
pub fn is_exponential_def(a: &Aggregate) -> bool {
    matches!(a, Aggregate::Exponential(_))
}

/// Port of `isExponentialBDef`.
pub fn is_exponentialb_def(a: &Aggregate) -> bool {
    matches!(a, Aggregate::ExponentialB(_))
}

/// An aggregate operation that takes an aggregate parameter, expressed as an
/// object such as `{"exponential": 0.5}`.
pub type ParameterizedAggregateDef = Aggregate;

/// Port of `isParameterizedAggregateDef`.
pub fn is_parameterized_aggregate_def(a: &Aggregate) -> bool {
    is_exponential_def(a) || is_exponentialb_def(a)
}

/// Port of `getAggregateOp`: the operation name for an aggregate, unwrapping
/// parameterized aggregate defs to their op.
pub fn get_aggregate_op(a: &Aggregate) -> &'static str {
    a.op_name()
}

/// Port of `getAggregateParam`: the parameter of a parameterized aggregate
/// def, or `None` for other aggregates.
pub fn get_aggregate_param(a: &Aggregate) -> Option<f64> {
    a.param()
}

/// Port of `isAggregateOp` (only string operators are aggregate ops).
pub fn is_aggregate_op(a: &str) -> bool {
    AGGREGATE_OP_INDEX.contains(&a)
}

pub const COUNTING_OPS: [&str; 4] = ["count", "valid", "missing", "distinct"];

/// Port of `isCountingAggregateOp`.
pub fn is_counting_aggregate_op(aggregate: Option<&Aggregate>) -> bool {
    aggregate
        .map(|a| COUNTING_OPS.contains(&a.op_name()))
        .unwrap_or(false)
}

/// Convenience overload of [`is_counting_aggregate_op`] for a plain operation
/// name.
pub fn is_counting_op_name(aggregate: &str) -> bool {
    COUNTING_OPS.contains(&aggregate)
}

/// Port of `isMinMaxOp`.
pub fn is_min_max_op(aggregate: Option<&Aggregate>) -> bool {
    aggregate
        .map(|a| matches!(a.op_name(), "min" | "max"))
        .unwrap_or(false)
}

/// Convenience overload of [`is_min_max_op`] for a plain operation name.
pub fn is_min_max_op_name(aggregate: &str) -> bool {
    matches!(aggregate, "min" | "max")
}

/// Additive-based aggregation operations. These can be applied to stack.
pub const SUM_OPS: [&str; 5] = ["count", "sum", "distinct", "valid", "missing"];

/// Aggregation operators that always produce values within the range
/// `[domainMin, domainMax]`.
pub const SHARED_DOMAIN_OPS: [&str; 7] = ["mean", "average", "median", "q1", "q3", "min", "max"];

/// Membership index of [`SHARED_DOMAIN_OPS`] (present in older upstream
/// versions of `aggregate.ts`).
pub const SHARED_DOMAIN_OP_INDEX: [&str; 7] = SHARED_DOMAIN_OPS;

/// Returns whether a JSON value is an aggregate definition: an aggregate
/// operator name or one of the `argmin`/`argmax`/`exponential`/`exponentialb`
/// object forms.
pub fn is_aggregate(a: &Value) -> bool {
    match a {
        Value::String(s) => is_aggregate_op(s),
        Value::Object(_) => {
            has_property(a, "argmin")
                || has_property(a, "argmax")
                || has_property(a, "exponential")
                || has_property(a, "exponentialb")
        }
        _ => false,
    }
}

/// Returns the field referenced by an `argmin`/`argmax` aggregate definition.
pub fn get_field(a: &Aggregate) -> Option<FieldName> {
    match a {
        Aggregate::Argmin(f) | Aggregate::Argmax(f) => Some(f.clone()),
        _ => None,
    }
}

/// Returns the field referenced by an `argmin`/`argmax` JSON aggregate
/// definition.
pub fn get_field_from_value(a: &Value) -> Option<FieldName> {
    if let Value::Object(map) = a {
        if let Some(field) = map.get("argmin").or_else(|| map.get("argmax")) {
            match field {
                Value::String(s) => return Some(s.clone()),
                Value::Object(o) => {
                    if let Some(s) = o.get("field").and_then(Value::as_str) {
                        return Some(s.to_string());
                    }
                }
                _ => {}
            }
        }
    }
    None
}

/// Port of the composite-aggregate predicate requested for this module.
/// Composite aggregates are the composite mark names (`boxplot`, `errorbar`,
/// `errorband`), represented either directly or as `{type: "..."}`.
pub fn is_composite_aggregate(a: &Value) -> bool {
    match a {
        Value::String(s) => crate::mark::is_composite_mark_name(s),
        _ => is_composite_mark(a),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_ops_roundtrip() {
        for op in AGGREGATE_OPS {
            assert_eq!(op.as_str().parse::<AggregateOp>(), Ok(op));
            assert!(is_aggregate_op(op.as_str()));
        }
    }
}

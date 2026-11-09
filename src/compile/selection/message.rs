//! Port of the selection-related messages of `src/log/message.ts`.

use crate::util::string_value;
use serde_json::Value;

pub fn cannot_project_on_channel_without_field(channel: &str) -> String {
    format!("Cannot project a selection on encoding channel \"{channel}\", which has no field.")
}

pub fn cannot_project_aggregate(channel: &str, aggregate: &str) -> String {
    format!(
        "Cannot project a selection on encoding channel \"{channel}\" as it uses an aggregate function (\"{aggregate}\")."
    )
}

pub fn nearest_not_support_for_continuous(mark: &str) -> String {
    format!("The \"nearest\" transform is not supported for {mark} marks.")
}

pub const SCALE_BINDINGS_CONTINUOUS: &str =
    "Scale bindings are currently only supported for scales with unbinned, continuous domains.";

pub const SEQUENTIAL_SCALE_DEPRECATED: &str =
    "Sequntial scales are deprecated. The available quantitative scale type values are linear, log, pow, sqrt, symlog, time and utc";

pub const LEGEND_BINDINGS_MUST_HAVE_PROJECTION: &str =
    "Legend bindings are only supported for selections over an individual field or encoding channel.";

pub const INTERVAL_INITIALIZED_WITH_POS: &str =
    "Interval selections should be initialized using \"x\", \"y\", \"longitude\", or \"latitude\" keys.";

pub const MULTIPLE_TIMER_ANIMATION_SELECTION: &str =
    "Multiple timer selections in one unit spec are not supported. Ignoring all but the first.";

pub const MULTI_VIEW_ANIMATION_UNSUPPORTED: &str =
    "Animation involving facet, layer, or concat is currently unsupported.";

pub fn selection_as_scale_domain_without_field(field: &str) -> String {
    format!(
        "A \"field\" or \"encoding\" must be specified when using a selection as a scale domain. Using \"field\": {}.",
        string_value(&Value::String(field.to_string()))
    )
}

pub fn selection_as_scale_domain_wrong_encodings(
    encodings: &[super::component::SelectionProjection],
    encoding: &str,
    extent: &Value,
    field: &str,
) -> String {
    let prefix = if encodings.is_empty() {
        "No "
    } else {
        "Multiple "
    };
    let param = extent.get("param").cloned().unwrap_or(Value::Null);
    format!(
        "{prefix}matching {} encoding found for selection {}. Using \"field\": {}.",
        string_value(&Value::String(encoding.to_string())),
        string_value(&param),
        string_value(&Value::String(field.to_string()))
    )
}

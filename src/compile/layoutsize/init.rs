//! Port of `src/compile/layoutsize/init.ts`.

use serde_json::Value;

use crate::channel::{get_size_channel, POSITION_SCALE_CHANNELS};
use crate::channeldef::is_continuous_field_or_datum_def;
use crate::log;

use super::super::support;

/// Port of `initLayoutSize`.
pub fn init_layout_size(encoding: &Value, size: &Value) -> Value {
    let mut size = size.clone();
    if !size.is_object() {
        size = Value::Object(Default::default());
    }
    for channel in POSITION_SCALE_CHANNELS {
        let size_type = get_size_channel(channel).unwrap_or("width");
        if support::is_step(&size[size_type])
            && is_continuous_field_or_datum_def(&encoding[channel]) {
                size.as_object_mut().unwrap().remove(size_type);
                log::warn(support::step_dropped(size_type));
            }
    }
    size
}

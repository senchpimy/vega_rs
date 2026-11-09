//! Port of `src/compile/mark/encode/nonposition.ts`.

use serde_json::Value;

use crate::channeldef::is_conditional_def;
use crate::compile::model::Model;
use crate::compile_common::signal_or_value_ref;
use crate::mark::get_mark_prop_or_config;

use super::conditional::wrap_condition;
use super::invalid::get_conditional_value_ref_for_including_invalid_value;
use super::valueref::{mid_point, MidPointParams};

pub fn non_position(
    channel: &str,
    model: &Model,
    opt: NonPositionOpt,
) -> Value {
    let mark_def = &model.unit.mark_def;
    let encoding = &model.unit.encoding;
    let config = &model.config;
    let vg_channel = opt.vg_channel.as_deref();

    let mut default_ref = opt.default_ref;
    let mut default_value = opt.default_value;

    let channel_def = encoding.get(channel);

    if default_ref.is_none() {
        if default_value.is_none() {
            default_value = get_mark_prop_or_config(
                channel,
                mark_def,
                config,
                Some(vg_channel.unwrap_or(channel)),
                !is_conditional_def(channel_def.unwrap_or(&Value::Null)),
            );
        }
        if let Some(default_value) = &default_value {
            default_ref = Some(signal_or_value_ref(default_value));
        }
    }

    let scale_name = model.scale_name(channel, false).unwrap_or_default();
    let scale = model.get_scale_component_ref(channel);

    let invalid_value_ref = get_conditional_value_ref_for_including_invalid_value(
        channel,
        channel_def,
        scale,
        &scale_name,
        mark_def,
        config,
    );

    let main_ref_fn = |c_def: &Value| -> Option<Value> {
        let reference = mid_point(&MidPointParams {
            channel,
            channel_def: Some(c_def),
            channel2_def: None,
            mark_def,
            config,
            scale_name: &scale_name,
            scale,
            stack: None,
            offset: None,
            default_ref: default_ref.clone(),
            band_position: None,
        });
        if reference.is_null() {
            None
        } else {
            Some(reference)
        }
    };

    wrap_condition(
        model,
        channel_def,
        vg_channel.unwrap_or(channel),
        invalid_value_ref,
        main_ref_fn,
    )
}

#[derive(Default)]
pub struct NonPositionOpt {
    pub default_value: Option<Value>,
    pub vg_channel: Option<String>,
    pub default_ref: Option<Value>,
}

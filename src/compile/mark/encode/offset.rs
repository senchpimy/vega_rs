//! Port of `src/compile/mark/encode/offset.ts`.

use serde_json::Value;

use crate::channeldef_support::truthy;
use crate::compile::model::Model;
use crate::compile_common::signal_or_value_ref;

use super::valueref::{mid_point, MidPointParams};

#[derive(Debug, Clone, Default)]
pub struct Offset {
    pub offset_type: Option<&'static str>,
    pub offset: Option<Value>,
}

pub fn position_offset(
    channel: &str,
    mark_def: &Value,
    encoding: &Value,
    model: Option<&Model>,
    band_position: Option<&Value>,
) -> Offset {
    let offset_channel = crate::channel::get_offset_channel(channel)
        .map(str::to_string)
        .unwrap_or_else(|| format!("{channel}Offset"));

    let default_value = mark_def.get(&offset_channel).cloned();
    let channel_def = encoding.get(&offset_channel).cloned();

    if matches!(offset_channel.as_str(), "xOffset" | "yOffset")
        && channel_def.as_ref().map(truthy).unwrap_or(false)
    {
        if let Some(model) = model {
            let channel_def = channel_def.unwrap();
            let scale_name = model
                .scale_name(&offset_channel, false)
                .unwrap_or_default();
            let scale = model.get_scale_component(&offset_channel);
            let reference = mid_point(&MidPointParams {
                channel: &offset_channel,
                channel_def: Some(&channel_def),
                channel2_def: None,
                mark_def,
                config: &model.config,
                scale_name: &scale_name,
                scale: scale.as_ref(),
                stack: None,
                offset: None,
                default_ref: default_value.map(|v| signal_or_value_ref(&v)),
                band_position: band_position.cloned(),
            });
            return Offset {
                offset_type: Some("encoding"),
                offset: Some(reference),
            };
        }
    }

    if let Some(mark_def_offset_value) = mark_def.get(&offset_channel) {
        if truthy(mark_def_offset_value) {
            return Offset {
                offset_type: Some("visual"),
                offset: Some(mark_def_offset_value.clone()),
            };
        }
    }

    Offset::default()
}

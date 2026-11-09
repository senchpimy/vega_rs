//! Port of `src/compile/selection/point.ts`.

use serde_json::{json, Value};

use crate::channel::TIME;
use crate::compile::model::Model;
use crate::selection::SELECTION_ID;
use crate::util::{flat_access_with_datum, string_value};

use super::component::{
    is_timer_selection, unit_name, SelectionCompiler, SelectionComponent, TUPLE,
};
use super::interval::BRUSH;
use super::project::TUPLE_FIELDS;

pub const CURR: &str = "_curr";
pub const ANIM_VALUE: &str = "anim_value";
pub const ANIM_CLOCK: &str = "anim_clock";
pub const EASED_ANIM_CLOCK: &str = "eased_anim_clock";
pub const MIN_EXTENT: &str = "min_extent";
pub const MAX_RANGE_EXTENT: &str = "max_range_extent";
pub const LAST_TICK: &str = "last_tick_at";
pub const IS_PLAYING: &str = "is_playing";
pub const THROTTLE: f64 = (1.0 / 60.0) * 1000.0; // 60 FPS

pub fn compiler() -> SelectionCompiler {
    SelectionCompiler {
        defined,
        parse: None,
        signals: Some(signals),
        top_level_signals: Some(top_level_signals),
        modify_expr: None,
        marks: None,
    }
}

pub fn defined(sel_cmpt: &SelectionComponent) -> bool {
    sel_cmpt.type_ == "point"
}

fn animation_signals(selection_name: &str, scale_name: &str) -> Vec<Value> {
    vec![
        json!({"name": EASED_ANIM_CLOCK, "update": ANIM_CLOCK}),
        json!({"name": format!("{selection_name}_domain"), "init": format!("domain('{scale_name}')")}),
        json!({"name": MIN_EXTENT, "init": format!("extent({selection_name}_domain)[0]")}),
        json!({"name": MAX_RANGE_EXTENT, "init": format!("extent(range('{scale_name}'))[1]")}),
        json!({"name": ANIM_VALUE, "update": format!("invert('{scale_name}', {EASED_ANIM_CLOCK})")}),
    ]
}

pub fn top_level_signals(
    _model: &Model,
    sel_cmpt: &SelectionComponent,
    mut signals: Vec<Value>,
) -> Vec<Value> {
    if is_timer_selection(sel_cmpt) {
        signals.push(json!({
            "name": ANIM_CLOCK,
            "init": "0",
            "on": [{
                "events": {"type": "timer", "throttle": THROTTLE},
                "update": format!(
                    "{IS_PLAYING} ? ({ANIM_CLOCK} + (now() - {LAST_TICK}) > {MAX_RANGE_EXTENT} ? 0 : {ANIM_CLOCK} + (now() - {LAST_TICK})) : {ANIM_CLOCK}"
                ),
            }],
        }));
        signals.push(json!({
            "name": LAST_TICK,
            "init": "now()",
            "on": [{"events": [{"signal": ANIM_CLOCK}, {"signal": IS_PLAYING}], "update": "now()"}],
        }));
        signals.push(json!({"name": IS_PLAYING, "init": "true"}));
    }

    signals
}

pub fn signals(model: &Model, sel_cmpt: &SelectionComponent, signals: Vec<Value>) -> Vec<Value> {
    let name = sel_cmpt.name.clone();
    let fields_sg = format!("{name}{TUPLE_FIELDS}");
    let project = &sel_cmpt.project;
    let datum = "(item().isVoronoi ? datum.datum : datum)";

    // Only add a discrete selection to the store if a datum is present _and_
    // the interaction isn't occurring on a group mark.
    let mut brushes: Vec<String> = Vec::new();
    for cmpt in super::component::ordered_components(model).iter() {
        if cmpt.type_ == "interval" {
            brushes.push(format!(
                "indexof(item().mark.name, '{}{}') < 0",
                cmpt.name, BRUSH
            ));
        }
    }

    let mut test = "datum && item().mark.marktype !== 'group' && indexof(item().mark.role, 'legend') < 0".to_string();
    if !brushes.is_empty() {
        test.push_str(&format!(" && {}", brushes.join(" && ")));
    }

    let mut update = format!("unit: {}, ", unit_name(model, true));

    if project.has_selection_id {
        update.push_str(&format!(
            "{SELECTION_ID}: {}",
            flat_access_with_datum(SELECTION_ID, datum)
        ));
    } else if is_timer_selection(sel_cmpt) {
        update.push_str(&format!(
            "fields: {fields_sg}, values: [{ANIM_VALUE} ? {ANIM_VALUE} : {MIN_EXTENT}]"
        ));
    } else {
        let values: Vec<String> = project
            .items
            .iter()
            .map(|p| {
                let field_def = p.channel.as_ref().and_then(|c| model.field_def(c));
                let bin = field_def
                    .as_ref()
                    .and_then(|f| f.get("bin"))
                    .map(super::component::truthy_value)
                    .unwrap_or(false);
                if bin {
                    let channel = p.channel.clone().unwrap_or_default();
                    let end_opt = crate::channeldef::FieldRefOption {
                        bin_suffix: Some("end".to_string()),
                        ..Default::default()
                    };
                    format!(
                        "[{}, {}]",
                        flat_access_with_datum(&model.vg_field(&channel), datum),
                        flat_access_with_datum(&model.vg_field_with(&channel, &end_opt), datum)
                    )
                } else {
                    flat_access_with_datum(&p.field, datum)
                }
            })
            .collect();

        update.push_str(&format!(
            "fields: {fields_sg}, values: [{}]",
            values.join(", ")
        ));
    }

    if is_timer_selection(sel_cmpt) {
        let scale_name = model.scale_name(TIME, false).unwrap_or_default();
        let mut out = signals;
        out.extend(animation_signals(&sel_cmpt.name, &scale_name));
        out.push(json!({"name": format!("{name}{TUPLE}"), "update": format!("{{{update}}}")}));
        out
    } else {
        let mut out = signals;
        let on = match &sel_cmpt.events {
            Some(events) => json!([{
                "events": events,
                "update": format!("{test} ? {{{update}}} : null"),
                "force": true,
            }]),
            None => json!([]),
        };
        out.push(json!({"name": format!("{name}{TUPLE}"), "on": on}));
        out
    }
}

#[allow(dead_code)]
fn _unused(x: &str) -> String {
    string_value(&json!(x))
}

//! Port of `src/channel.ts`.
//!
//! Constants and utilities for encoding channels (visual variables) such as
//! `x`, `y`, `color`.
//!
//! Channels are represented as string constants, mirroring the way the
//! TypeScript source treats them. The `RangeType` enum normally lives in
//! `src/compile/scale/type.ts`; since that module is out of scope it is defined
//! here.

use serde::{Deserialize, Serialize};

// Facet
pub const ROW: &str = "row";
pub const COLUMN: &str = "column";

pub const FACET: &str = "facet";

// Position
pub const X: &str = "x";
pub const Y: &str = "y";
pub const X2: &str = "x2";
pub const Y2: &str = "y2";

// Position Offset
pub const XOFFSET: &str = "xOffset";
pub const YOFFSET: &str = "yOffset";

// Arc-Position
pub const RADIUS: &str = "radius";
pub const RADIUS2: &str = "radius2";
pub const THETA: &str = "theta";
pub const THETA2: &str = "theta2";

// Geo Position
pub const LATITUDE: &str = "latitude";
pub const LONGITUDE: &str = "longitude";
pub const LATITUDE2: &str = "latitude2";
pub const LONGITUDE2: &str = "longitude2";

// Time
pub const TIME: &str = "time";

// Mark property with scale
pub const COLOR: &str = "color";

pub const FILL: &str = "fill";

pub const STROKE: &str = "stroke";

pub const SHAPE: &str = "shape";
pub const SIZE: &str = "size";

pub const ANGLE: &str = "angle";

pub const OPACITY: &str = "opacity";
pub const FILLOPACITY: &str = "fillOpacity";

pub const STROKEOPACITY: &str = "strokeOpacity";

pub const STROKEWIDTH: &str = "strokeWidth";
pub const STROKEDASH: &str = "strokeDash";

// Non-scale channel
pub const TEXT: &str = "text";
pub const ORDER: &str = "order";
pub const DETAIL: &str = "detail";
pub const KEY: &str = "key";

pub const TOOLTIP: &str = "tooltip";
pub const HREF: &str = "href";

pub const URL: &str = "url";
pub const DESCRIPTION: &str = "description";

/// A channel name. Channels are plain strings in this port.
pub type Channel = &'static str;
/// A channel name or a facet channel.
pub type ExtendedChannel = &'static str;

/// The type of range a channel expects.
///
/// Mirrors `RangeType` from `src/compile/scale/type.ts`:
/// `'continuous' | 'discrete' | 'flexible' | undefined`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RangeType {
    Continuous,
    Discrete,
    Flexible,
}

// ---------------------------------------------------------------------------
// Position channels
// ---------------------------------------------------------------------------

pub const POSITION_CHANNEL_INDEX: [&str; 4] = [X, Y, X2, Y2];
pub const POSITION_CHANNELS: [&str; 4] = POSITION_CHANNEL_INDEX;

pub const POLAR_POSITION_CHANNEL_INDEX: [&str; 4] = [THETA, THETA2, RADIUS, RADIUS2];
pub const POLAR_POSITION_CHANNELS: [&str; 4] = POLAR_POSITION_CHANNEL_INDEX;

pub fn is_polar_position_channel(c: &str) -> bool {
    contains(&POLAR_POSITION_CHANNEL_INDEX, c)
}

pub const GEO_POSITION_CHANNEL_INDEX: [&str; 4] = [LONGITUDE, LONGITUDE2, LATITUDE, LATITUDE2];
pub const GEOPOSITION_CHANNELS: [&str; 4] = GEO_POSITION_CHANNEL_INDEX;

pub fn get_position_channel_from_lat_long(channel: &str) -> Option<&'static str> {
    match channel {
        LATITUDE => Some(Y),
        LATITUDE2 => Some(Y2),
        LONGITUDE => Some(X),
        LONGITUDE2 => Some(X2),
        _ => None,
    }
}

pub fn is_geo_position_channel(c: &str) -> bool {
    contains(&GEO_POSITION_CHANNEL_INDEX, c)
}

// ---------------------------------------------------------------------------
// Unit channels
// ---------------------------------------------------------------------------

/// `UNIT_CHANNEL_INDEX`, in the insertion order of the TypeScript object.
pub const UNIT_CHANNEL_INDEX: [&str; 34] = [
    // ...POSITION_CHANNEL_INDEX
    X,
    Y,
    X2,
    Y2,
    // ...POLAR_POSITION_CHANNEL_INDEX
    THETA,
    THETA2,
    RADIUS,
    RADIUS2,
    // ...GEO_POSITION_CHANNEL_INDEX
    LONGITUDE,
    LONGITUDE2,
    LATITUDE,
    LATITUDE2,
    XOFFSET,
    YOFFSET,
    // color
    COLOR,
    FILL,
    STROKE,
    // time
    TIME,
    // other non-position with scale
    OPACITY,
    FILLOPACITY,
    STROKEOPACITY,
    STROKEWIDTH,
    STROKEDASH,
    SIZE,
    ANGLE,
    SHAPE,
    // channels without scales
    ORDER,
    TEXT,
    DETAIL,
    KEY,
    TOOLTIP,
    HREF,
    URL,
    DESCRIPTION,
];

pub fn is_color_channel(channel: &str) -> bool {
    channel == COLOR || channel == FILL || channel == STROKE
}

pub const FACET_CHANNEL_INDEX: [&str; 3] = [ROW, COLUMN, FACET];
pub const FACET_CHANNELS: [&str; 3] = FACET_CHANNEL_INDEX;

/// `CHANNEL_INDEX`, in the insertion order of the TypeScript object.
pub const CHANNEL_INDEX: [&str; 37] = [
    // ...UNIT_CHANNEL_INDEX
    X,
    Y,
    X2,
    Y2,
    THETA,
    THETA2,
    RADIUS,
    RADIUS2,
    LONGITUDE,
    LONGITUDE2,
    LATITUDE,
    LATITUDE2,
    XOFFSET,
    YOFFSET,
    COLOR,
    FILL,
    STROKE,
    TIME,
    OPACITY,
    FILLOPACITY,
    STROKEOPACITY,
    STROKEWIDTH,
    STROKEDASH,
    SIZE,
    ANGLE,
    SHAPE,
    ORDER,
    TEXT,
    DETAIL,
    KEY,
    TOOLTIP,
    HREF,
    URL,
    DESCRIPTION,
    // ...FACET_CHANNEL_INDEX
    ROW,
    COLUMN,
    FACET,
];

pub const CHANNELS: [&str; 37] = CHANNEL_INDEX;

/// Channels that cannot have an array of channel definitions.
pub const SINGLE_DEF_CHANNEL_INDEX: [&str; 34] = [
    X, Y, X2, Y2, THETA, THETA2, RADIUS, RADIUS2, LONGITUDE, LONGITUDE2, LATITUDE, LATITUDE2,
    XOFFSET, YOFFSET, COLOR, FILL, STROKE, TIME, OPACITY, FILLOPACITY, STROKEOPACITY,
    STROKEWIDTH, STROKEDASH, SIZE, ANGLE, SHAPE, TEXT, KEY, HREF, URL, DESCRIPTION, ROW, COLUMN,
    FACET,
];

pub const SINGLE_DEF_CHANNELS: [&str; 34] = SINGLE_DEF_CHANNEL_INDEX;

pub const SINGLE_DEF_UNIT_CHANNEL_INDEX: [&str; 31] = [
    X, Y, X2, Y2, THETA, THETA2, RADIUS, RADIUS2, LONGITUDE, LONGITUDE2, LATITUDE, LATITUDE2,
    XOFFSET, YOFFSET, COLOR, FILL, STROKE, TIME, OPACITY, FILLOPACITY, STROKEOPACITY,
    STROKEWIDTH, STROKEDASH, SIZE, ANGLE, SHAPE, TEXT, KEY, HREF, URL, DESCRIPTION,
];

pub const SINGLE_DEF_UNIT_CHANNELS: [&str; 31] = SINGLE_DEF_UNIT_CHANNEL_INDEX;

pub fn is_single_def_unit_channel(s: &str) -> bool {
    contains(&SINGLE_DEF_UNIT_CHANNEL_INDEX, s)
}

pub fn is_channel(s: &str) -> bool {
    contains(&CHANNEL_INDEX, s)
}

// ---------------------------------------------------------------------------
// Secondary range channels
// ---------------------------------------------------------------------------

pub const SECONDARY_RANGE_CHANNEL: [&str; 6] = [X2, Y2, LATITUDE2, LONGITUDE2, THETA2, RADIUS2];

pub fn is_secondary_range_channel(c: &str) -> bool {
    let main = get_main_range_channel(c);
    main != c
}

/// Get the main channel for a range channel. E.g. `x` for `x2`.
pub fn get_main_range_channel(channel: &str) -> &str {
    match channel {
        X2 => X,
        Y2 => Y,
        LATITUDE2 => LATITUDE,
        LONGITUDE2 => LONGITUDE,
        THETA2 => THETA,
        RADIUS2 => RADIUS,
        _ => channel,
    }
}

/// Get the secondary channel for a range channel. E.g. `x2` for `x`.
pub fn get_secondary_range_channel(channel: &str) -> Option<&'static str> {
    match channel {
        X => Some(X2),
        Y => Some(Y2),
        LATITUDE => Some(LATITUDE2),
        LONGITUDE => Some(LONGITUDE2),
        THETA => Some(THETA2),
        RADIUS => Some(RADIUS2),
        _ => None,
    }
}

pub fn get_vg_position_channel(channel: &str) -> &str {
    if is_polar_position_channel(channel) {
        return match channel {
            THETA => "startAngle",
            THETA2 => "endAngle",
            RADIUS => "outerRadius",
            RADIUS2 => "innerRadius",
            _ => channel,
        };
    }
    channel
}

pub fn get_size_channel(channel: &str) -> Option<&'static str> {
    match channel {
        X | X2 => Some("width"),
        Y | Y2 => Some("height"),
        _ => None,
    }
}

/// Get the offset channel for a channel. E.g. `xOffset` for `x`.
pub fn get_offset_channel(channel: &str) -> Option<&'static str> {
    match channel {
        X => Some("xOffset"),
        Y => Some("yOffset"),
        X2 => Some("x2Offset"),
        Y2 => Some("y2Offset"),
        THETA => Some("thetaOffset"),
        RADIUS => Some("radiusOffset"),
        THETA2 => Some("theta2Offset"),
        RADIUS2 => Some("radius2Offset"),
        _ => None,
    }
}

pub fn get_offset_scale_channel(channel: &str) -> Option<&'static str> {
    match channel {
        X => Some(XOFFSET),
        Y => Some(YOFFSET),
        _ => None,
    }
}

pub fn get_main_channel_from_offset_channel(channel: &str) -> Option<&'static str> {
    match channel {
        XOFFSET => Some(X),
        YOFFSET => Some(Y),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Unit channels without facet channels
// ---------------------------------------------------------------------------

pub const UNIT_CHANNELS: [&str; 34] = UNIT_CHANNEL_INDEX;

/// `NONPOSITION_CHANNEL_INDEX`, in insertion order.
pub const NONPOSITION_CHANNEL_INDEX: [&str; 20] = [
    COLOR,
    FILL,
    STROKE,
    TIME,
    OPACITY,
    FILLOPACITY,
    STROKEOPACITY,
    STROKEWIDTH,
    STROKEDASH,
    SIZE,
    ANGLE,
    SHAPE,
    ORDER,
    TEXT,
    DETAIL,
    KEY,
    TOOLTIP,
    HREF,
    URL,
    DESCRIPTION,
];

pub const NONPOSITION_CHANNELS: [&str; 20] = NONPOSITION_CHANNEL_INDEX;

pub const POSITION_SCALE_CHANNEL_INDEX: [&str; 2] = [X, Y];
pub const POSITION_SCALE_CHANNELS: [&str; 2] = POSITION_SCALE_CHANNEL_INDEX;

pub fn is_x_or_y(channel: &str) -> bool {
    contains(&POSITION_SCALE_CHANNEL_INDEX, channel)
}

pub const POLAR_POSITION_SCALE_CHANNEL_INDEX: [&str; 2] = [THETA, RADIUS];
pub const POLAR_POSITION_SCALE_CHANNELS: [&str; 2] = POLAR_POSITION_SCALE_CHANNEL_INDEX;

pub fn is_polar_position_scale_channel(channel: &str) -> bool {
    contains(&POLAR_POSITION_SCALE_CHANNEL_INDEX, channel)
}

pub fn get_position_scale_channel(size_type: &str) -> &'static str {
    if size_type == "width" {
        X
    } else {
        Y
    }
}

pub const OFFSET_SCALE_CHANNEL_INDEX: [&str; 2] = [XOFFSET, YOFFSET];

pub const OFFSET_SCALE_CHANNELS: [&str; 2] = OFFSET_SCALE_CHANNEL_INDEX;

pub fn is_x_or_y_offset(channel: &str) -> bool {
    contains(&OFFSET_SCALE_CHANNEL_INDEX, channel)
}

pub const TIME_SCALE_CHANNEL_INDEX: [&str; 1] = [TIME];
pub const TIME_SCALE_CHANNELS: [&str; 1] = TIME_SCALE_CHANNEL_INDEX;

pub fn is_time(channel: &str) -> bool {
    contains(&TIME_SCALE_CHANNEL_INDEX, channel)
}

/// `NONPOSITION_SCALE_CHANNEL_INDEX`, in insertion order.
pub const NONPOSITION_SCALE_CHANNEL_INDEX: [&str; 12] = [
    COLOR,
    FILL,
    STROKE,
    TIME,
    OPACITY,
    FILLOPACITY,
    STROKEOPACITY,
    STROKEWIDTH,
    STROKEDASH,
    SIZE,
    ANGLE,
    SHAPE,
];

pub const NONPOSITION_SCALE_CHANNELS: [&str; 12] = NONPOSITION_SCALE_CHANNEL_INDEX;

pub fn is_non_position_scale_channel(channel: &str) -> bool {
    contains(&NONPOSITION_CHANNEL_INDEX, channel)
}

// ---------------------------------------------------------------------------
// Marks
// ---------------------------------------------------------------------------

/// Mark types, mirrored locally because `src/mark.rs` is out of scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mark {
    Arc,
    Area,
    Bar,
    Circle,
    Geoshape,
    Image,
    Line,
    Rule,
    Point,
    Rect,
    Square,
    Trail,
    Text,
    Tick,
}

impl Mark {
    pub const ALL: [Mark; 14] = [
        Mark::Arc,
        Mark::Area,
        Mark::Bar,
        Mark::Circle,
        Mark::Geoshape,
        Mark::Image,
        Mark::Line,
        Mark::Rule,
        Mark::Point,
        Mark::Rect,
        Mark::Square,
        Mark::Trail,
        Mark::Text,
        Mark::Tick,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Mark::Arc => "arc",
            Mark::Area => "area",
            Mark::Bar => "bar",
            Mark::Circle => "circle",
            Mark::Geoshape => "geoshape",
            Mark::Image => "image",
            Mark::Line => "line",
            Mark::Rule => "rule",
            Mark::Point => "point",
            Mark::Rect => "rect",
            Mark::Square => "square",
            Mark::Trail => "trail",
            Mark::Text => "text",
            Mark::Tick => "tick",
        }
    }
}

impl std::fmt::Display for Mark {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Mark {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Mark::ALL
            .iter()
            .copied()
            .find(|m| m.as_str() == s)
            .ok_or(())
    }
}

/// Whether a mark supports a channel `always` or only when `binned`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarkSupport {
    Always,
    Binned,
}

/// Return whether a channel supports a particular mark type.
pub fn support_mark(channel: &str, mark: Mark) -> Option<MarkSupport> {
    get_supported_mark(channel)
        .into_iter()
        .find(|(m, _)| *m == mark)
        .map(|(_, support)| support)
}

/// Return a dictionary showing whether a channel supports mark type.
fn get_supported_mark(channel: &str) -> Vec<(Mark, MarkSupport)> {
    let all = |m: Mark| (m, MarkSupport::Always);
    let all_marks = || Mark::ALL.iter().copied().map(all).collect::<Vec<_>>();
    let all_except_geoshape = || {
        Mark::ALL
            .iter()
            .copied()
            .filter(|m| *m != Mark::Geoshape)
            .map(all)
            .collect::<Vec<_>>()
    };

    match channel {
        COLOR | FILL | STROKE | DESCRIPTION | DETAIL | KEY | TOOLTIP | HREF | ORDER | OPACITY
        | FILLOPACITY | STROKEOPACITY | STROKEWIDTH | FACET | ROW | COLUMN => all_marks(),
        X | Y | XOFFSET | YOFFSET | LATITUDE | LONGITUDE | TIME => all_except_geoshape(),
        X2 | Y2 | LATITUDE2 | LONGITUDE2 => vec![
            (Mark::Area, MarkSupport::Always),
            (Mark::Bar, MarkSupport::Always),
            (Mark::Image, MarkSupport::Always),
            (Mark::Rect, MarkSupport::Always),
            (Mark::Rule, MarkSupport::Always),
            (Mark::Circle, MarkSupport::Binned),
            (Mark::Point, MarkSupport::Binned),
            (Mark::Square, MarkSupport::Binned),
            (Mark::Tick, MarkSupport::Binned),
            (Mark::Line, MarkSupport::Binned),
            (Mark::Trail, MarkSupport::Binned),
        ],
        SIZE => vec![
            (Mark::Point, MarkSupport::Always),
            (Mark::Tick, MarkSupport::Always),
            (Mark::Rule, MarkSupport::Always),
            (Mark::Circle, MarkSupport::Always),
            (Mark::Square, MarkSupport::Always),
            (Mark::Bar, MarkSupport::Always),
            (Mark::Text, MarkSupport::Always),
            (Mark::Line, MarkSupport::Always),
            (Mark::Trail, MarkSupport::Always),
        ],
        STROKEDASH => vec![
            (Mark::Line, MarkSupport::Always),
            (Mark::Point, MarkSupport::Always),
            (Mark::Tick, MarkSupport::Always),
            (Mark::Rule, MarkSupport::Always),
            (Mark::Circle, MarkSupport::Always),
            (Mark::Square, MarkSupport::Always),
            (Mark::Bar, MarkSupport::Always),
            (Mark::Geoshape, MarkSupport::Always),
        ],
        SHAPE => vec![
            (Mark::Point, MarkSupport::Always),
            (Mark::Geoshape, MarkSupport::Always),
        ],
        TEXT => vec![(Mark::Text, MarkSupport::Always)],
        ANGLE => vec![
            (Mark::Point, MarkSupport::Always),
            (Mark::Square, MarkSupport::Always),
            (Mark::Text, MarkSupport::Always),
        ],
        URL => vec![(Mark::Image, MarkSupport::Always)],
        THETA => vec![
            (Mark::Text, MarkSupport::Always),
            (Mark::Arc, MarkSupport::Always),
        ],
        RADIUS => vec![
            (Mark::Text, MarkSupport::Always),
            (Mark::Arc, MarkSupport::Always),
        ],
        THETA2 | RADIUS2 => vec![(Mark::Arc, MarkSupport::Always)],
        _ => Vec::new(),
    }
}

/// Whether Vega supports legends for a particular channel.
pub fn support_legend(channel: &str) -> bool {
    matches!(
        channel,
        COLOR | FILL | STROKE | SIZE | SHAPE | OPACITY | STROKEWIDTH | STROKEDASH
    )
}

// ---------------------------------------------------------------------------
// Scale channels
// ---------------------------------------------------------------------------

/// `SCALE_CHANNEL_INDEX`, in insertion order.
pub const SCALE_CHANNEL_INDEX: [&str; 18] = [
    // ...POSITION_SCALE_CHANNEL_INDEX
    X,
    Y,
    // ...POLAR_POSITION_SCALE_CHANNEL_INDEX
    THETA,
    RADIUS,
    // ...OFFSET_SCALE_CHANNEL_INDEX
    XOFFSET,
    YOFFSET,
    // ...NONPOSITION_SCALE_CHANNEL_INDEX
    COLOR,
    FILL,
    STROKE,
    TIME,
    OPACITY,
    FILLOPACITY,
    STROKEOPACITY,
    STROKEWIDTH,
    STROKEDASH,
    SIZE,
    ANGLE,
    SHAPE,
];

/// List of channels with scales.
pub const SCALE_CHANNELS: [&str; 18] = SCALE_CHANNEL_INDEX;

/// A channel that has a scale.
pub type ScaleChannel = &'static str;

pub fn is_scale_channel(channel: &str) -> bool {
    contains(&SCALE_CHANNEL_INDEX, channel)
}

/// Return the type of range a channel expects.
pub fn range_type(channel: &str) -> Option<RangeType> {
    match channel {
        X | Y | THETA | RADIUS | XOFFSET | YOFFSET | SIZE | ANGLE | STROKEWIDTH | OPACITY
        | FILLOPACITY | STROKEOPACITY | TIME | X2 | Y2 | THETA2 | RADIUS2 => None,

        FACET | ROW | COLUMN | SHAPE | STROKEDASH | TEXT | TOOLTIP | HREF | URL | DESCRIPTION => {
            Some(RangeType::Discrete)
        }

        // Color can be either continuous or discrete, depending on scale type.
        COLOR | FILL | STROKE => Some(RangeType::Flexible),

        // No scale, no range type.
        LATITUDE | LONGITUDE | LATITUDE2 | LONGITUDE2 | DETAIL | KEY | ORDER => None,

        _ => None,
    }
}

fn contains(array: &[&str], item: &str) -> bool {
    array.contains(&item)
}

//! Animations the agent designs as data: timed text, boxes, workspace images and clips with
//! entrance and exit motion. Arrumo validates the spec and renders it with FFmpeg at any frame size, so the same
//! animation serves low-resolution previews, final renders and library previews without running
//! agent-written code.

use std::{
    error::Error,
    fmt::Write as _,
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    MotionKind,
    brand_fonts::{FontRole, FontSet, FontWeight},
};

pub(crate) const MAX_LAYERS: usize = 12;
/// The most camera moves one clip layer has.
const MAX_ZOOM_KEYS: usize = 8;
const MAX_ZOOM: f64 = 3.0;
/// The most spotlights one clip layer has.
const MAX_SPOTLIGHTS: usize = 6;
/// How long a spotlight takes to dim the rest of the clip, and how dark it gets.
const SPOTLIGHT_EASE: f64 = 0.3;
const SPOTLIGHT_DIM: f64 = 0.62;
/// How long a layer's sound-effect file may play before it fades out.
const SOUND_ASSET_SECONDS: f64 = 4.0;
pub(crate) const MIN_SECONDS: f64 = 1.0;
pub(crate) const MAX_SECONDS: f64 = 20.0;
const MAX_STORED_SECONDS: f64 = 60.0;
const MAX_TEXT: usize = 240;
const FPS: u32 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LayerType {
    Text,
    Box,
    Image,
    Video,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Motion {
    None,
    Fade,
    Rise,
    Drop,
    SlideLeft,
    SlideRight,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Layer {
    #[serde(rename = "type")]
    pub kind: LayerType,
    pub text: Option<String>,
    pub asset_id: Option<i64>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub size: f64,
    pub color: String,
    pub bold: bool,
    pub opacity: f64,
    pub start: f64,
    pub end: f64,
    pub enter: Motion,
    pub exit: Motion,
    /// A sound effect played as the layer arrives.
    #[serde(default)]
    pub sound: Sound,
    /// A workspace sound-effect file played as the layer arrives, in place of `sound`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound_asset_id: Option<i64>,
    /// How text lines sit inside the layer's width.
    #[serde(default, skip_serializing_if = "Align::is_center")]
    pub align: Align,
    /// Corner rounding of boxes and images, as a fraction of their shorter side (0 to 0.5).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub radius: f64,
    /// A second colour a box fades into towards its bottom-right corner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradient_to: Option<String>,
    /// A soft shadow under a box or image.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub shadow: bool,
    /// Which brand font text is set in.
    #[serde(default, skip_serializing_if = "is_body")]
    pub font: FontRole,
    /// Where a clip layer starts playing its clip, in seconds into the clip.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub clip_start: f64,
    /// Camera moves over a clip layer: the view eases from one key to the next.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub zoom: Vec<ZoomKey>,
    /// Moments when a clip layer dims all but one area of the clip.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spotlight: Vec<Spotlight>,
}

/// From `from` to `to` seconds into the animation, everything in the clip outside a rounded area
/// darkens; the area is centred on (`x`, `y`) and `width` by `height`, all fractions of the clip.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub(crate) struct Spotlight {
    pub from: f64,
    pub to: f64,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A point the view over a clip layer passes through: at `at` seconds into the animation it shows
/// the clip `scale` times larger, centred on (`x`, `y`) as fractions of the clip.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub(crate) struct ZoomKey {
    pub at: f64,
    pub x: f64,
    pub y: f64,
    pub scale: f64,
}

fn is_body(role: &FontRole) -> bool {
    *role == FontRole::Body
}

fn is_zero(value: &f64) -> bool {
    *value == 0.0
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Align {
    Left,
    #[default]
    Center,
    Right,
}

impl Align {
    fn is_center(&self) -> bool {
        *self == Self::Center
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Sound {
    #[default]
    None,
    Whoosh,
    Pop,
    Click,
    Chime,
    Rise,
}

impl Sound {
    /// An FFmpeg source graph producing the effect as 48 kHz stereo, or `None` for silence.
    fn source(self) -> Option<&'static str> {
        Some(match self {
            Self::None => return None,
            Self::Whoosh => {
                "anoisesrc=d=0.5:c=pink:a=0.6,highpass=f=300,lowpass=f=3500,afade=t=in:d=0.2,afade=t=out:st=0.2:d=0.3,volume=0.5"
            }
            Self::Pop => "sine=f=520:d=0.12,afade=t=out:st=0.02:d=0.1,volume=0.6",
            Self::Click => "sine=f=1800:d=0.04,afade=t=out:st=0.005:d=0.035,volume=0.5",
            Self::Chime => {
                "aevalsrc=0.3*sin(2*PI*880*t)*exp(-4*t)+0.2*sin(2*PI*1320*t)*exp(-5*t):d=0.9"
            }
            Self::Rise => {
                "aevalsrc=0.35*sin(2*PI*(260+520*t)*t)*min(1\\,t*8):d=0.5,afade=t=out:st=0.35:d=0.15"
            }
        })
    }
}

/// A validated animation, stored with results and library items so it can be rendered again.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MotionSpec {
    pub title: String,
    pub kind: MotionKind,
    pub usage: String,
    pub duration_seconds: f64,
    pub background: String,
    /// A second background colour the frame fades into towards its bottom-right corner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_to: Option<String>,
    pub layers: Vec<Layer>,
    /// How a storyboard scene hands over to the next one.
    #[serde(
        default,
        skip_serializing_if = "crate::scene_transitions::Transition::is_none"
    )]
    pub transition: crate::scene_transitions::Transition,
}

/// The JSON schema agents answer with for one animation. `assets` are the image IDs it may use.
pub(crate) fn spec_schema(assets: &[i64]) -> Value {
    spec_schema_with_clips(assets, &[])
}

/// Like `spec_schema`, also offering clip layers that play the workspace videos in `clips`.
pub(crate) fn spec_schema_with_clips(images: &[i64], clips: &[i64]) -> Value {
    spec_schema_with_media(images, clips, &[])
}

/// Like `spec_schema_with_clips`, also letting layers play the workspace sound effects in `sounds`.
pub(crate) fn spec_schema_with_media(images: &[i64], clips: &[i64], sounds: &[i64]) -> Value {
    let assets: Vec<i64> = images.iter().chain(clips).copied().collect();
    let motion = json!({"enum": ["none", "fade", "rise", "drop", "slide-left", "slide-right"]});
    let unit = json!({"type": "number", "minimum": 0, "maximum": 1});
    let optional_color =
        json!({"anyOf": [{"type": "null"}, {"type": "string", "pattern": "^#[0-9A-Fa-f]{6}$"}]});
    let asset = if assets.is_empty() {
        json!({"type": "null"})
    } else {
        json!({"anyOf": [{"type": "null"}, {"enum": assets}]})
    };
    let mut layer = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "text", "assetId", "x", "y", "width", "height", "size",
            "color", "bold", "opacity", "start", "end", "enter", "exit", "sound",
            "align", "radius", "gradientTo", "shadow", "font"],
        "properties": {
            "type": {"enum": ["text", "box", "image"]},
            "text": {"type": ["string", "null"], "maxLength": MAX_TEXT},
            "assetId": asset,
            "x": unit, "y": unit, "width": unit, "height": unit,
            "size": {"type": "number", "minimum": 0.02, "maximum": 0.25},
            "color": {"type": "string", "pattern": "^#[0-9A-Fa-f]{6}$"},
            "bold": {"type": "boolean"},
            "opacity": unit,
            "start": {"type": "number", "minimum": 0},
            "end": {"type": "number", "minimum": 0},
            "enter": motion,
            "exit": motion,
            "sound": {"enum": ["none", "whoosh", "pop", "click", "chime", "rise"]},
            "align": {"enum": ["left", "center", "right"]},
            "radius": {"type": "number", "minimum": 0, "maximum": 0.5},
            "gradientTo": optional_color,
            "shadow": {"type": "boolean"},
            "font": {"enum": ["heading", "body"]}
        }
    });
    if !clips.is_empty() {
        layer["properties"]["type"] = json!({"enum": ["text", "box", "image", "video"]});
        layer["properties"]["clipStart"] = json!({"type": "number", "minimum": 0});
        layer["properties"]["zoom"] = json!({
            "type": "array",
            "maxItems": MAX_ZOOM_KEYS,
            "items": {
                "type": "object",
                "additionalProperties": false,
                "required": ["at", "x", "y", "scale"],
                "properties": {
                    "at": {"type": "number", "minimum": 0},
                    "x": unit, "y": unit,
                    "scale": {"type": "number", "minimum": 1, "maximum": MAX_ZOOM}
                }
            }
        });
        layer["properties"]["spotlight"] = json!({
            "type": "array",
            "maxItems": MAX_SPOTLIGHTS,
            "items": {
                "type": "object",
                "additionalProperties": false,
                "required": ["from", "to", "x", "y", "width", "height"],
                "properties": {
                    "from": {"type": "number", "minimum": 0},
                    "to": {"type": "number", "minimum": 0},
                    "x": unit, "y": unit, "width": unit, "height": unit
                }
            }
        });
        if let Some(required) = layer["required"].as_array_mut() {
            required.extend([json!("clipStart"), json!("zoom"), json!("spotlight")]);
        }
    }
    if !sounds.is_empty() {
        layer["properties"]["soundAssetId"] =
            json!({"anyOf": [{"type": "null"}, {"enum": sounds}]});
        if let Some(required) = layer["required"].as_array_mut() {
            required.push(json!("soundAssetId"));
        }
    }
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["title", "kind", "usage", "durationSeconds", "background", "backgroundTo", "layers"],
        "properties": {
            "title": {"type": "string", "maxLength": 80},
            "kind": {"enum": ["animation", "transition"]},
            "usage": {"type": "string", "maxLength": 600},
            "durationSeconds": {"type": "number", "minimum": MIN_SECONDS, "maximum": MAX_SECONDS},
            "background": {"type": "string", "pattern": "^#[0-9A-Fa-f]{6}$"},
            "backgroundTo": optional_color,
            "layers": {
                "type": "array",
                "minItems": 1,
                "maxItems": MAX_LAYERS,
                "items": layer
            }
        }
    })
}

/// How a spec is described to the agent, next to the schema.
pub(crate) const SPEC_GUIDE: &str = "An animation is a timed composition on a background colour, or a \
diagonal gradient from background to backgroundTo (null for a solid one). \
Positions are fractions of the frame: x and y place the layer's centre, width and height its size \
(for text, width is the widest a line may run before wrapping and height is ignored). Text size is a \
fraction of the frame height; 0.06 to 0.1 suits titles and 0.035 to 0.05 body text. Boxes are solid \
rectangles with an opacity, useful for bars, cards, underlines and lower thirds. Images show a \
workspace image by assetId, fitted inside their box. Layers paint in order, so put backgrounds \
first. start and end are seconds within the animation; enter and exit choose how a layer arrives \
and leaves. Use the brand colours, keep strong contrast between text and what is behind it, keep \
text short and inside the frame with a safe margin of 0.06, and stagger entrances so the motion \
reads clearly. A layer can play a short sound as it arrives (a whoosh for slides, a pop or click for \
items appearing, a chime for a reveal, a rise for a build-up); use them sparingly, on the beats that \
matter, and none when the animation will sit under speech. Text is set in the brand's heading or body \
font (font; a plain sans-serif when the brand has none), with no emoji, pictographs or arrows, so \
write only letters, digits and common punctuation; use the heading font for titles and big numbers, and draw shapes, grids, bars and diagrams with box \
layers. Leave text null for boxes and images and assetId null for text and boxes. \
For a polished look: align a column of text left (align sets where lines sit within width, so a \
left-aligned layer starts at x minus width/2), round cards and images slightly (radius 0.08 to 0.2 \
of their shorter side; 0.5 makes pills and circles), give a box a subtle gradient with gradientTo, and \
lift cards and images off the background with shadow. Keep generous margins, one clear focal point per \
moment, a few sizes of text with a strong hierarchy, and consistent spacing between items.";

/// How clip layers are described to the agent, added to `SPEC_GUIDE` when it may use them.
pub(crate) const CLIP_GUIDE: &str = "Video layers play a workspace clip (assetId) fitted inside their box, \
starting clipStart seconds into the clip and playing for the layer's span; if the clip runs out, its last \
frame holds. Use them for screen recordings and product footage: float the clip on the background with a \
radius of about 0.03 and a shadow, and keep it large (width 0.7 to 0.9) so the interface stays readable. \
When the same clip plays in consecutive scenes, start each where the previous one stopped, so no moment \
repeats. Most clips need no camera move at all. To point at a control, a panel or text the narration \
talks about, use a spotlight: from and to are seconds of the animation, and x, y, width and height place \
a rounded area as fractions of the clip (x and y its centre) that stays bright while the rest of the clip \
dims. Make the area a little larger than its subject, start it a moment before the narration names it \
and keep it at least a second and a half. A spotlight marks a fixed place on the screen, so end it before \
the clip scrolls or changes page and start the next one after the movement, at least 0.6 s later: \
spotlights closer than that are joined into one handover. zoom moves the camera over the clip, cropping it inside its \
box: each key says at which second of the animation the view is scale times larger (1 to 3) and centred \
on x and y as fractions of the clip; the view eases from key to key. Keep zoom for text too small to read \
at full size, at most once or twice in a video, and ease back out; an empty list keeps the whole clip in \
view. Each clip's notes say what happens at which second and where, so pick clipStart, spotlights and \
zoom keys from them. Set text null, and color to the background's colour; it only shows under the clip's \
rounded corners.";

fn hex_color(value: &str) -> Option<String> {
    let digits = value.strip_prefix('#')?;
    (digits.len() == 6 && digits.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| format!("0x{}", digits.to_ascii_uppercase()))
}

impl MotionSpec {
    /// Lengthens the spec to at least `seconds`, keeping layers that lasted to the end on screen.
    pub(crate) fn extend_to(&mut self, seconds: f64) {
        let seconds = seconds.min(MAX_STORED_SECONDS);
        if seconds <= self.duration_seconds {
            return;
        }
        let previous = self.duration_seconds;
        for layer in &mut self.layers {
            if layer.end >= previous - 0.05 {
                layer.end = seconds;
            }
        }
        self.duration_seconds = seconds;
    }
}

/// Pictographs, arrows and other symbols the text font cannot draw.
fn undrawable(c: char) -> bool {
    matches!(u32::from(c), 0x2190..=0x21FF | 0x2300..=0x23FF | 0x2460..=0x27BF | 0x2900..=0x2BFF
        | 0xFE00..=0xFE0F | 0x1F000..)
        || c == '\u{200D}'
}

fn clean_text(value: &str, limit: usize) -> String {
    let kept: String = value
        .chars()
        .filter(|c| (!c.is_control() || *c == '\n') && !undrawable(*c))
        .take(limit)
        .collect();
    kept.lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

impl MotionSpec {
    /// Reads an agent's answer, clamping numbers into range and rejecting what cannot be drawn.
    /// `image_allowed` says whether an asset ID is a usable workspace image.
    pub(crate) fn from_answer(
        answer: &Value,
        image_allowed: impl Fn(i64) -> bool,
    ) -> Result<Self, &'static str> {
        Self::from_answer_with_clips(answer, image_allowed, |_| false)
    }

    /// Like `from_answer_with_clips`, also keeping layer sound effects that `sound_allowed` approves.
    pub(crate) fn from_answer_with_media(
        answer: &Value,
        image_allowed: impl Fn(i64) -> bool,
        clip_allowed: impl Fn(i64) -> bool,
        sound_allowed: impl Fn(i64) -> bool,
    ) -> Result<Self, &'static str> {
        Self::read_answer(answer, image_allowed, clip_allowed, sound_allowed)
    }

    /// Like `from_answer`, also accepting clip layers whose asset ID `clip_allowed` approves.
    pub(crate) fn from_answer_with_clips(
        answer: &Value,
        image_allowed: impl Fn(i64) -> bool,
        clip_allowed: impl Fn(i64) -> bool,
    ) -> Result<Self, &'static str> {
        Self::read_answer(answer, image_allowed, clip_allowed, |_| false)
    }

    fn read_answer(
        answer: &Value,
        image_allowed: impl Fn(i64) -> bool,
        clip_allowed: impl Fn(i64) -> bool,
        sound_allowed: impl Fn(i64) -> bool,
    ) -> Result<Self, &'static str> {
        let invalid = "The agent's animation did not follow the expected structure";
        let mut spec: MotionSpec = serde_json::from_value(answer.clone()).map_err(|_| invalid)?;
        spec.title = clean_text(&spec.title, 80);
        spec.usage = clean_text(&spec.usage, 600);
        if spec.title.is_empty() {
            spec.title = "Animation".into();
        }
        if !spec.duration_seconds.is_finite() {
            return Err(invalid);
        }
        // Agents design up to MAX_SECONDS; narration can lengthen a stored scene further.
        spec.duration_seconds = spec.duration_seconds.clamp(MIN_SECONDS, MAX_STORED_SECONDS);
        hex_color(&spec.background).ok_or(invalid)?;
        spec.background_to = spec
            .background_to
            .filter(|color| hex_color(color).is_some());
        let duration = spec.duration_seconds;
        spec.layers.truncate(MAX_LAYERS);
        spec.layers.retain_mut(|layer| {
            layer.sound_asset_id = layer.sound_asset_id.filter(|id| sound_allowed(*id));
            if layer.sound_asset_id.is_some() {
                layer.sound = Sound::None;
            }
            let unit = |value: f64| {
                if value.is_finite() {
                    value.clamp(0.0, 1.0)
                } else {
                    0.5
                }
            };
            layer.x = unit(layer.x);
            layer.y = unit(layer.y);
            // Text needs room to wrap; boxes and images may be hairlines.
            let least = if layer.kind == LayerType::Text {
                0.02
            } else {
                0.001
            };
            layer.width = unit(layer.width).max(least);
            layer.height = unit(layer.height).max(least.min(0.005));
            layer.opacity = unit(layer.opacity);
            layer.size = if layer.size.is_finite() {
                layer.size.clamp(0.02, 0.25)
            } else {
                0.06
            };
            layer.start = if layer.start.is_finite() {
                layer.start.clamp(0.0, duration)
            } else {
                0.0
            };
            layer.end = if layer.end.is_finite() {
                layer.end.clamp(0.0, duration)
            } else {
                duration
            };
            layer.radius = if layer.radius.is_finite() {
                layer.radius.clamp(0.0, 0.5)
            } else {
                0.0
            };
            layer.gradient_to = layer
                .gradient_to
                .take()
                .filter(|color| layer.kind == LayerType::Box && hex_color(color).is_some());
            if layer.kind == LayerType::Text {
                layer.radius = 0.0;
                layer.shadow = false;
            }
            if layer.kind == LayerType::Video {
                layer.clip_start = if layer.clip_start.is_finite() {
                    layer.clip_start.clamp(0.0, 3600.0)
                } else {
                    0.0
                };
                layer.zoom.retain(|key| {
                    [key.at, key.x, key.y, key.scale]
                        .iter()
                        .all(|value| value.is_finite())
                });
                layer.zoom.truncate(MAX_ZOOM_KEYS);
                for key in &mut layer.zoom {
                    key.at = key.at.clamp(0.0, duration);
                    key.x = key.x.clamp(0.0, 1.0);
                    key.y = key.y.clamp(0.0, 1.0);
                    key.scale = key.scale.clamp(1.0, MAX_ZOOM);
                }
                layer.zoom.sort_by(|a, b| a.at.total_cmp(&b.at));
                layer.spotlight.retain_mut(|light| {
                    let values = [
                        light.from,
                        light.to,
                        light.x,
                        light.y,
                        light.width,
                        light.height,
                    ];
                    if !values.iter().all(|value| value.is_finite()) {
                        return false;
                    }
                    light.from = light.from.clamp(0.0, duration);
                    light.to = light.to.clamp(0.0, duration);
                    light.x = light.x.clamp(0.0, 1.0);
                    light.y = light.y.clamp(0.0, 1.0);
                    light.width = light.width.clamp(0.02, 1.0);
                    light.height = light.height.clamp(0.02, 1.0);
                    light.to - light.from >= 2.0 * SPOTLIGHT_EASE
                });
                layer.spotlight.truncate(MAX_SPOTLIGHTS);
                layer.spotlight.sort_by(|a, b| a.from.total_cmp(&b.from));
                // A spotlight that follows soon after another takes over from it without the
                // clip brightening in between.
                for next in 1..layer.spotlight.len() {
                    let from = layer.spotlight[next].from;
                    let previous = &mut layer.spotlight[next - 1];
                    if from - previous.to < 2.0 * SPOTLIGHT_EASE {
                        previous.to = previous.to.max(from + SPOTLIGHT_EASE).min(duration);
                    }
                }
            } else {
                layer.clip_start = 0.0;
                layer.zoom.clear();
                layer.spotlight.clear();
            }
            if layer.end - layer.start < 0.1 || hex_color(&layer.color).is_none() {
                return false;
            }
            match layer.kind {
                LayerType::Text => {
                    let text = layer.text.as_deref().map(|text| clean_text(text, MAX_TEXT));
                    layer.text = text.filter(|text| !text.is_empty());
                    layer.asset_id = None;
                    layer.text.is_some()
                }
                LayerType::Box => {
                    layer.text = None;
                    layer.asset_id = None;
                    layer.opacity > 0.0
                }
                LayerType::Image => {
                    layer.text = None;
                    layer.asset_id.is_some_and(&image_allowed)
                }
                LayerType::Video => {
                    layer.text = None;
                    layer.asset_id.is_some_and(&clip_allowed)
                }
            }
        });
        if spec.layers.is_empty() {
            return Err("The agent's animation has nothing to show");
        }
        Ok(spec)
    }
}

/// Wraps text into lines that fit `columns` characters, breaking long words.
fn wrap(text: &str, columns: usize) -> String {
    let columns = columns.max(4);
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let mut word = word.to_owned();
            while word.chars().count() > columns {
                if !line.is_empty() {
                    lines.push(std::mem::take(&mut line));
                }
                let head: String = word.chars().take(columns).collect();
                word = word.chars().skip(columns).collect();
                lines.push(head);
            }
            if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > columns {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(&word);
        }
        lines.push(line);
    }
    lines.join("\n")
}

/// FFmpeg expressions for a layer's entrance and exit: (alpha, x offset, y offset) in pixels.
fn motion_expressions(layer: &Layer, width: u32, height: u32) -> (String, String, String) {
    let span = (layer.end - layer.start).max(0.1);
    let ease = (span / 3.0).min(0.6);
    let (start, end) = (layer.start, layer.end);
    let enter = format!("(1-pow(1-clip((t-{start:.3})/{ease:.3},0,1),3))");
    let exit = format!("pow(clip((t-{:.3})/{ease:.3},0,1),3)", end - ease);
    let mut alpha = String::from("1");
    if layer.enter != Motion::None {
        alpha = enter.clone();
    }
    if layer.exit != Motion::None {
        alpha = format!("{alpha}*(1-{exit})");
    }
    let (dx, dy) = (f64::from(width) * 0.08, f64::from(height) * 0.06);
    let offset = |motion: Motion, progress: &str, arriving: bool| -> (String, String) {
        let amount = if arriving {
            format!("(1-{progress})")
        } else {
            progress.to_owned()
        };
        // Arrivals move towards the resting place; exits continue in the same direction.
        let sign = if arriving { 1.0 } else { -1.0 };
        match motion {
            Motion::Rise => ("0".into(), format!("{:.2}*{amount}", sign * dy)),
            Motion::Drop => ("0".into(), format!("{:.2}*{amount}", -sign * dy)),
            Motion::SlideLeft => (format!("{:.2}*{amount}", sign * dx), "0".into()),
            Motion::SlideRight => (format!("{:.2}*{amount}", -sign * dx), "0".into()),
            Motion::None | Motion::Fade => ("0".into(), "0".into()),
        }
    };
    let (enter_x, enter_y) = offset(layer.enter, &enter, true);
    let (exit_x, exit_y) = offset(layer.exit, &exit, false);
    (
        alpha,
        format!("({enter_x})+({exit_x})"),
        format!("({enter_y})+({exit_y})"),
    )
}

/// Copies the fonts drawtext uses into the job directory as `font-<role>-<weight>`, so filter
/// arguments need no path escaping: the brand's files where there are some, else the system font.
pub(crate) fn ensure_fonts(directory: &Path, fonts: &FontSet) -> io::Result<()> {
    for role in [FontRole::Heading, FontRole::Body] {
        for weight in [FontWeight::Regular, FontWeight::Bold] {
            let source = match fonts.file(role, weight) {
                Some(path) => path.to_path_buf(),
                None => system_font(weight.as_str()).ok_or_else(|| {
                    io::Error::other("No system font was found for animation text")
                })?,
            };
            fs::copy(source, directory.join(font_name(role, weight)))?;
        }
    }
    Ok(())
}

fn font_name(role: FontRole, weight: FontWeight) -> String {
    format!("font-{}-{}", role.as_str(), weight.as_str())
}

fn system_font(weight: &str) -> Option<PathBuf> {
    if let Ok(output) = Command::new("fc-match")
        .args(["-f", "%{file}", &format!("sans-serif:weight={weight}")])
        .output()
        && output.status.success()
    {
        let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
        if path.is_file() {
            return Some(path);
        }
    }
    let candidates: &[&str] = if weight == "bold" {
        &[
            "C:\\Windows\\Fonts\\segoeuib.ttf",
            "C:\\Windows\\Fonts\\arialbd.ttf",
            "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        ]
    } else {
        &[
            "C:\\Windows\\Fonts\\segoeui.ttf",
            "C:\\Windows\\Fonts\\arial.ttf",
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ]
    };
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
}

/// Where layers are drawn: a frame size, the job directory for text files, a label prefix that
/// keeps several animations in one graph apart, and when the animation starts on the base video.
struct LayerTarget<'a> {
    width: u32,
    height: u32,
    directory: &'a Path,
    prefix: &'a str,
    shift: f64,
    /// How long image and box sources last, in seconds of the base video.
    length: f64,
    fonts: &'a FontSet,
}

impl LayerTarget<'_> {
    /// Appends the filters that draw `spec`'s layers over the `base` stream to `graph`, adding any
    /// image inputs to `args` from input index `first_input`, and returns the final stream label.
    fn draw(
        &self,
        spec: &MotionSpec,
        base: &str,
        first_input: usize,
        graph: &mut String,
        args: &mut Vec<String>,
        images: &dyn Fn(i64) -> Option<PathBuf>,
    ) -> io::Result<String> {
        let (width, height, directory, prefix, duration) = (
            self.width,
            self.height,
            self.directory,
            self.prefix,
            self.length,
        );
        let mut current = base.to_owned();
        let mut next_input = first_input;
        let (frame_w, frame_h) = (f64::from(width), f64::from(height));
        for (index, layer) in spec.layers.iter().enumerate() {
            let mut layer = layer.clone();
            layer.start += self.shift;
            layer.end += self.shift;
            let layer = &layer;
            let (alpha, dx, dy) = motion_expressions(layer, width, height);
            let enable = format!("between(t,{:.3},{:.3})", layer.start, layer.end);
            let color = hex_color(&layer.color).ok_or_else(|| io::Error::other("Bad colour"))?;
            let label = format!("{prefix}v{index}");
            match layer.kind {
                LayerType::Text => {
                    let font_px = (layer.size * frame_h).round().max(8.0);
                    let weight = if layer.bold {
                        FontWeight::Bold
                    } else {
                        FontWeight::Regular
                    };
                    let advance = self.fonts.advance(layer.font, weight);
                    let columns = (layer.width * frame_w / (font_px * advance)).floor() as usize;
                    let wrapped = wrap(layer.text.as_deref().unwrap_or_default(), columns);
                    let font = font_name(layer.font, weight);
                    // FFmpeg 8's drawtext draws a missing-glyph box at each line break, so
                    // every line is its own centred drawtext.
                    let lines: Vec<&str> = wrapped.lines().collect();
                    let line_height = font_px * 1.25;
                    let top = layer.y * frame_h - line_height * lines.len() as f64 / 2.0;
                    let anchor = match layer.align {
                        Align::Left => format!("{:.2}", (layer.x - layer.width / 2.0) * frame_w),
                        Align::Center => format!("{:.2}-text_w/2", layer.x * frame_w),
                        Align::Right => {
                            format!("{:.2}-text_w", (layer.x + layer.width / 2.0) * frame_w)
                        }
                    };
                    let mut line_input = current.clone();
                    for (number, line) in lines.iter().enumerate() {
                        let file = format!("{prefix}text-{index}-{number}.txt");
                        fs::write(directory.join(&file), line)?;
                        let output = if number + 1 == lines.len() {
                            label.clone()
                        } else {
                            format!("{label}l{number}")
                        };
                        write!(
                            graph,
                            "[{line_input}]drawtext=fontfile={font}:textfile={file}:expansion=none:\
                         fontsize={font_px:.0}:fontcolor={color}:\
                         alpha='{alpha}':x='{anchor}+{dx}':y='{:.2}+{dy}':\
                         enable='{enable}'[{output}];",
                            top + line_height * number as f64 + font_px * 0.1,
                        )
                        .ok();
                        line_input = output;
                    }
                    if lines.is_empty() {
                        write!(graph, "[{current}]null[{label}];").ok();
                    }
                }
                LayerType::Box | LayerType::Image => {
                    let box_w = ((layer.width * frame_w).round() as u32).max(2);
                    let box_h = ((layer.height * frame_h).round() as u32).max(2);
                    let source = format!("{prefix}s{index}");
                    let shaped = layer.kind == LayerType::Box || layer.radius > 0.0 || layer.shadow;
                    let path = if layer.kind == LayerType::Image {
                        Some(
                            layer
                                .asset_id
                                .and_then(images)
                                .ok_or_else(|| io::Error::other("An animation image is missing"))?,
                        )
                    } else {
                        None
                    };
                    let input = if shaped {
                        let file = format!("{prefix}shape-{index}.png");
                        let fill = match &path {
                            Some(path) => Fill::Image(path),
                            None => Fill::Color {
                                from: &layer.color,
                                to: layer.gradient_to.as_deref(),
                            },
                        };
                        let shape = Shape {
                            width: box_w,
                            height: box_h,
                            radius: layer.radius,
                            opacity: layer.opacity,
                            shadow: layer.shadow.then_some(frame_h.min(frame_w)),
                        };
                        shape_png(directory, &file, &fill, &shape)?;
                        file
                    } else {
                        path.unwrap_or_default().to_string_lossy().into_owned()
                    };
                    args.extend(["-loop", "1", "-framerate", "30", "-t"].map(String::from));
                    args.push(format!("{duration:.3}"));
                    args.push("-i".into());
                    args.push(input);
                    if shaped {
                        write!(graph, "[{next_input}:v]format=rgba[{source}];").ok();
                    } else {
                        write!(
                            graph,
                            "[{next_input}:v]scale={box_w}:{box_h}:force_original_aspect_ratio=decrease,\
                             format=rgba,colorchannelmixer=aa={:.3}[{source}];",
                            layer.opacity
                        )
                        .ok();
                    }
                    next_input += 1;
                    // Overlay alpha cannot follow an expression, so fade the layer itself.
                    let faded = format!("{prefix}f{index}");
                    let span = (layer.end - layer.start).max(0.1);
                    let ease = (span / 3.0).min(0.6);
                    let mut fades = Vec::new();
                    if layer.enter != Motion::None {
                        fades.push(format!(
                            "fade=t=in:st={:.3}:d={ease:.3}:alpha=1",
                            layer.start
                        ));
                    }
                    if layer.exit != Motion::None {
                        fades.push(format!(
                            "fade=t=out:st={:.3}:d={ease:.3}:alpha=1",
                            layer.end - ease
                        ));
                    }
                    if fades.is_empty() {
                        fades.push("null".into());
                    }
                    write!(graph, "[{source}]{}[{faded}];", fades.join(",")).ok();
                    write!(
                    graph,
                    "[{current}][{faded}]overlay=x='{:.2}-overlay_w/2+{dx}':y='{:.2}-overlay_h/2+{dy}':\
                     enable='{enable}':eof_action=pass[{label}];",
                    layer.x * frame_w,
                    layer.y * frame_h,
                )
                .ok();
                }
                LayerType::Video => {
                    let path = layer
                        .asset_id
                        .and_then(images)
                        .ok_or_else(|| io::Error::other("An animation clip is missing"))?;
                    let (clip_w, clip_h) = clip_size(&path)?;
                    let fit = (layer.width * frame_w / clip_w).min(layer.height * frame_h / clip_h);
                    let even = |value: f64| ((value / 2.0).round() as u32).max(1) * 2;
                    let (view_w, view_h) = (even(clip_w * fit), even(clip_h * fit));
                    let span = (layer.end - layer.start).max(0.1);
                    let ease = (span / 3.0).min(0.6);
                    let mut fades = Vec::new();
                    if layer.enter != Motion::None {
                        fades.push(format!(
                            "fade=t=in:st={:.3}:d={ease:.3}:alpha=1",
                            layer.start
                        ));
                    }
                    if layer.exit != Motion::None {
                        fades.push(format!(
                            "fade=t=out:st={:.3}:d={ease:.3}:alpha=1",
                            layer.end - ease
                        ));
                    }
                    if layer.opacity < 1.0 {
                        fades.push(format!("colorchannelmixer=aa={:.3}", layer.opacity));
                    }
                    if fades.is_empty() {
                        fades.push("null".into());
                    }
                    let fades = fades.join(",");
                    let place = format!(
                        "x='{:.2}-overlay_w/2+{dx}':y='{:.2}-overlay_h/2+{dy}':enable='{enable}':eof_action=pass",
                        layer.x * frame_w,
                        layer.y * frame_h,
                    );
                    let mut still = |file: &str| -> usize {
                        args.extend(["-loop", "1", "-framerate", "30", "-t"].map(String::from));
                        args.push(format!("{duration:.3}"));
                        args.extend(["-i".to_owned(), file.to_owned()]);
                        next_input += 1;
                        next_input - 1
                    };
                    // A card in the background colour casts the shadow; the clip covers it.
                    if layer.shadow {
                        let file = format!("{prefix}card-{index}.png");
                        let shape = Shape {
                            width: view_w,
                            height: view_h,
                            radius: layer.radius,
                            opacity: 1.0,
                            shadow: Some(frame_h.min(frame_w)),
                        };
                        let fill = Fill::Color {
                            from: &layer.color,
                            to: None,
                        };
                        shape_png(directory, &file, &fill, &shape)?;
                        let input = still(&file);
                        write!(
                            graph,
                            "[{input}:v]format=rgba,{fades}[{prefix}c{index}];\
                             [{current}][{prefix}c{index}]overlay={place}[{label}c];"
                        )
                        .ok();
                        current = format!("{label}c");
                    }
                    let mask = if layer.radius > 0.0 {
                        let file = format!("{prefix}mask-{index}.png");
                        let shape = Shape {
                            width: view_w,
                            height: view_h,
                            radius: layer.radius,
                            opacity: 1.0,
                            shadow: None,
                        };
                        let fill = Fill::Color {
                            from: "#FFFFFF",
                            to: None,
                        };
                        shape_png(directory, &file, &fill, &shape)?;
                        Some(still(&file))
                    } else {
                        None
                    };
                    let lights: Vec<(usize, Spotlight)> = layer
                        .spotlight
                        .iter()
                        .enumerate()
                        .map(|(number, light)| {
                            let file = format!("{prefix}light-{index}-{number}.png");
                            spotlight_png(directory, &file, (clip_w, clip_h), light)?;
                            let shifted = Spotlight {
                                from: light.from + self.shift,
                                to: light.to + self.shift,
                                ..*light
                            };
                            Ok((still(&file), shifted))
                        })
                        .collect::<io::Result<_>>()?;
                    args.extend([
                        "-ss".to_owned(),
                        format!("{:.3}", layer.clip_start),
                        "-t".to_owned(),
                        format!("{:.3}", span + 1.0),
                        "-i".to_owned(),
                        path.to_string_lossy().into_owned(),
                    ]);
                    let clip = next_input;
                    next_input += 1;
                    let keys: Vec<ZoomKey> = layer
                        .zoom
                        .iter()
                        .map(|key| ZoomKey {
                            at: key.at + self.shift,
                            ..*key
                        })
                        .collect();
                    let camera = if keys.is_empty() {
                        format!("scale={view_w}:{view_h}")
                    } else {
                        // Zooming from twice the size keeps the camera's pixel steps at half a pixel.
                        let scale = keyed(&keys, |key| key.scale);
                        let x = keyed(&keys, |key| key.x);
                        let y = keyed(&keys, |key| key.y);
                        format!(
                            "scale={}:{},zoompan=z='{scale}':\
                             x='max(0,min(iw-iw/zoom,({x})*iw-iw/zoom/2))':\
                             y='max(0,min(ih-ih/zoom,({y})*ih-ih/zoom/2))':\
                             d=1:s={view_w}x{view_h}:fps={FPS}",
                            view_w * 2,
                            view_h * 2,
                        )
                    };
                    write!(
                        graph,
                        "[{clip}:v]fps={FPS},setpts=PTS-STARTPTS,\
                         tpad=stop_mode=clone:stop_duration={span:.3},trim=duration={span:.3},\
                         setpts=PTS+{:.3}/TB,format=rgba[{prefix}lit{index}x];",
                        layer.start,
                    )
                    .ok();
                    // Spotlights go on the clip before the camera, so they follow any zoom.
                    let mut lit = format!("{prefix}lit{index}x");
                    for (number, (input, light)) in lights.iter().enumerate() {
                        write!(
                            graph,
                            "[{input}:v]format=rgba,\
                             fade=t=in:st={from:.3}:d={SPOTLIGHT_EASE}:alpha=1,\
                             fade=t=out:st={out:.3}:d={SPOTLIGHT_EASE}:alpha=1[{prefix}dim{index}x{number}];\
                             [{lit}][{prefix}dim{index}x{number}]overlay=0:0:\
                             enable='between(t,{from:.3},{to:.3})':eof_action=pass[{prefix}lit{index}x{number}];",
                            from = light.from,
                            to = light.to,
                            out = light.to - SPOTLIGHT_EASE,
                        )
                        .ok();
                        lit = format!("{prefix}lit{index}x{number}");
                    }
                    write!(
                        graph,
                        "[{lit}]{camera},setsar=1,format=rgba[{prefix}p{index}];"
                    )
                    .ok();
                    let mut picture = format!("{prefix}p{index}");
                    if let Some(mask) = mask {
                        write!(
                            graph,
                            "[{mask}:v]format=rgba,alphaextract[{prefix}m{index}];\
                             [{picture}][{prefix}m{index}]alphamerge[{prefix}r{index}];"
                        )
                        .ok();
                        picture = format!("{prefix}r{index}");
                    }
                    write!(
                        graph,
                        "[{picture}]{fades}[{prefix}f{index}];\
                         [{current}][{prefix}f{index}]overlay={place}[{label}];"
                    )
                    .ok();
                }
            }
            current = label;
        }
        Ok(current)
    }
}

/// Keeps a clip moving forward when consecutive scenes play it: a scene that would start the clip
/// before the previous scene stopped it starts where the previous one stopped, so no moment repeats.
pub(crate) fn continue_clips(scenes: &mut [MotionSpec]) {
    let mut reached: Vec<(i64, f64)> = Vec::new();
    for scene in scenes {
        let mut next = Vec::new();
        for layer in &mut scene.layers {
            let (LayerType::Video, Some(id)) = (layer.kind, layer.asset_id) else {
                continue;
            };
            if let Some(&(_, end)) = reached.iter().find(|(clip, _)| *clip == id) {
                let shift = end - layer.clip_start;
                if shift > 0.0 {
                    layer.clip_start = end;
                    follow_clip(layer, shift);
                }
            }
            next.push((id, layer.clip_start + (layer.end - layer.start)));
        }
        reached = next;
    }
}

/// Keeps a layer's spotlights and zoom keys on the clip moments they were placed for after its
/// clip starts `shift` seconds later: they move earlier, and those left too short are dropped.
fn follow_clip(layer: &mut Layer, shift: f64) {
    for light in &mut layer.spotlight {
        light.from = (light.from - shift).max(layer.start);
        light.to -= shift;
    }
    layer
        .spotlight
        .retain(|light| light.to - light.from >= 2.0 * SPOTLIGHT_EASE);
    if !layer.zoom.is_empty() {
        for key in &mut layer.zoom {
            key.at -= shift;
        }
        let before = layer.zoom.iter().rposition(|key| key.at <= layer.start);
        if let Some(first) = before {
            layer.zoom.drain(..first);
        }
    }
}

/// An FFmpeg expression of the stream time `it` that eases a value through `keys`: it holds the
/// first key's value before it, eases between neighbouring keys and holds the last value after.
fn keyed(keys: &[ZoomKey], value: impl Fn(&ZoomKey) -> f64) -> String {
    let Some(last) = keys.last() else {
        return "1".into();
    };
    let mut expression = format!("{:.4}", value(last));
    for pair in keys.windows(2).rev() {
        let (from, to) = (&pair[0], &pair[1]);
        let (a, b) = (value(from), value(to));
        let gap = to.at - from.at;
        let between = if gap < 0.01 {
            format!("{b:.4}")
        } else {
            format!(
                "{a:.4}+({:.4})*(1-cos(PI*(it-{:.3})/{gap:.3}))/2",
                b - a,
                from.at
            )
        };
        expression = format!("if(lt(it,{:.3}),{between},{expression})", to.at);
    }
    format!(
        "if(lt(it,{:.3}),{:.4},{expression})",
        keys[0].at,
        value(&keys[0])
    )
}

/// The width and height of a clip's first video stream.
fn clip_size(path: &Path) -> io::Result<(f64, f64)> {
    let output = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0", "-show_entries"])
        .args(["stream=width,height", "-of", "csv=p=0:s=x"])
        .arg(path)
        .stdin(Stdio::null())
        .output()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let (width, height) = text
        .trim()
        .split_once('x')
        .and_then(|(w, h)| Some((w.parse::<f64>().ok()?, h.parse::<f64>().ok()?)))
        .filter(|(w, h)| *w >= 2.0 && *h >= 2.0)
        .ok_or_else(|| io::Error::other("An animation clip has no picture"))?;
    Ok((width, height))
}

/// What fills a pre-rendered shape: a colour, optionally fading diagonally into another, or an
/// image fitted inside the shape.
enum Fill<'a> {
    Color { from: &'a str, to: Option<&'a str> },
    Image(&'a Path),
}

struct Shape {
    width: u32,
    height: u32,
    radius: f64,
    opacity: f64,
    /// The frame's shorter side when the shape casts a shadow, which sets the shadow's size.
    shadow: Option<f64>,
}

fn channels(color: &str) -> io::Result<[u8; 3]> {
    let digits = hex_color(color).ok_or_else(|| io::Error::other("Bad colour"))?;
    let value = u32::from_str_radix(&digits[2..], 16).map_err(io::Error::other)?;
    Ok([(value >> 16) as u8, (value >> 8) as u8, value as u8])
}

/// Renders one still of a box or image with rounded corners, a gradient and a soft shadow into
/// `file` inside `directory`. The shadow pads the still evenly, so its centre stays the shape's.
/// A clip-sized dark veil with a clear, rounded, softly edged window where the spotlight falls.
fn spotlight_png(
    directory: &Path,
    file: &str,
    (clip_w, clip_h): (f64, f64),
    light: &Spotlight,
) -> io::Result<()> {
    let (width, height) = (
        clip_w.round().max(2.0) as u32,
        clip_h.round().max(2.0) as u32,
    );
    let (half_w, half_h) = (light.width * clip_w / 2.0, light.height * clip_h / 2.0);
    let radius = (half_w.min(half_h) * 0.5).min(clip_w.min(clip_h) * 0.02);
    let feather = (clip_w.min(clip_h) * 0.008).max(2.0);
    let outside = format!(
        "(hypot(max(abs(X-{cx:.1})-{inner_w:.1},0),max(abs(Y-{cy:.1})-{inner_h:.1},0))-{radius:.1})",
        cx = light.x * clip_w,
        cy = light.y * clip_h,
        inner_w = half_w - radius,
        inner_h = half_h - radius,
    );
    let status = Command::new("ffmpeg")
        .args(["-nostdin", "-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg(format!("color=c=black:s={width}x{height}:d=0.1"))
        .arg("-vf")
        .arg(format!(
            "format=rgba,geq=r=0:g=0:b=0:a='{:.1}*clip({outside}/{feather:.1}+0.5,0,1)'",
            255.0 * SPOTLIGHT_DIM
        ))
        .args(["-frames:v", "1"])
        .arg(directory.join(file))
        .stdin(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other("Could not draw a spotlight"))
    }
}

fn shape_png(directory: &Path, file: &str, fill: &Fill, shape: &Shape) -> io::Result<()> {
    let (width, height) = (shape.width.max(2), shape.height.max(2));
    let radius = format!("({:.4}*min(W,H))", shape.radius);
    let mask = if shape.radius > 0.0 {
        format!(
            "*clip({radius}+0.5-hypot(max(max({radius}-X,X-(W-1-{radius})),0),\
             max(max({radius}-Y,Y-(H-1-{radius})),0)),0,1)"
        )
    } else {
        String::new()
    };
    let opacity = shape.opacity.clamp(0.0, 1.0);
    let mut args: Vec<String> = ["-nostdin", "-v", "error"].map(String::from).to_vec();
    let mut graph = match fill {
        Fill::Color { from, to } => {
            let start = channels(from)?;
            let end = to.map(channels).transpose()?.unwrap_or(start);
            let channel = |index: usize| {
                let (a, b) = (f64::from(start[index]), f64::from(end[index]));
                format!("{a:.1}+({:.1})*(X/W+Y/H)/2", b - a)
            };
            args.extend(["-f", "lavfi", "-i"].map(String::from));
            args.push(format!("color=c=black:s={width}x{height}:d=0.1"));
            format!(
                "[0:v]format=rgba,geq=r='{}':g='{}':b='{}':a='{:.1}{mask}'[shape]",
                channel(0),
                channel(1),
                channel(2),
                255.0 * opacity,
            )
        }
        Fill::Image(path) => {
            args.push("-i".into());
            args.push(path.to_string_lossy().into_owned());
            format!(
                "[0:v]scale={width}:{height}:force_original_aspect_ratio=decrease,format=rgba,\
                 geq=r='r(X,Y)':g='g(X,Y)':b='b(X,Y)':a='alpha(X,Y)*{opacity:.3}{mask}'[shape]"
            )
        }
    };
    let output = match shape.shadow {
        Some(frame) => {
            let pad = (frame * 0.04).round().max(4.0) as u32;
            let drop = pad / 3;
            write!(
                graph,
                ";[shape]split[top][under];\
                 [under]geq=r=0:g=0:b=0:a='alpha(X,Y)*0.4',pad=iw+{double}:ih+{double}:{pad}:{low}:color=black@0,\
                 gblur=sigma={sigma:.1}[soft];\
                 [soft][top]overlay={pad}:{pad}:format=auto[out]",
                double = pad * 2,
                low = pad + drop,
                sigma = f64::from(pad) / 2.5,
            )
            .ok();
            "[out]"
        }
        None => "[shape]",
    };
    args.extend([
        "-filter_complex".into(),
        graph,
        "-map".into(),
        output.into(),
    ]);
    args.extend(["-frames:v", "1", "-update", "1", "-y"].map(String::from));
    args.push(file.to_owned());
    let status = Command::new("ffmpeg")
        .args(&args)
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() && directory.join(file).is_file() {
        Ok(())
    } else {
        Err(io::Error::other("Could not draw an animation shape"))
    }
}

/// Draws `spec` over a video stream in a larger filter graph, starting `at` seconds into it.
/// Returns the label of the drawn stream.
#[allow(clippy::too_many_arguments)]
pub(crate) fn overlay_graph(
    spec: &MotionSpec,
    (width, height): (u32, u32),
    directory: &Path,
    prefix: &str,
    at: f64,
    base: &str,
    first_input: usize,
    graph: &mut String,
    args: &mut Vec<String>,
    images: &dyn Fn(i64) -> Option<PathBuf>,
    fonts: &FontSet,
) -> io::Result<String> {
    ensure_fonts(directory, fonts)?;
    LayerTarget {
        width,
        height,
        directory,
        prefix,
        shift: at,
        length: at + spec.duration_seconds,
        fonts,
    }
    .draw(spec, base, first_input, graph, args, images)
}

/// Builds the FFmpeg arguments that render `spec` at `width`×`height` into `output` (a name inside
/// `directory`). Text files are written next to it. `images` resolves asset IDs to files.
#[allow(clippy::too_many_arguments)]
fn render_arguments(
    spec: &MotionSpec,
    width: u32,
    height: u32,
    directory: &Path,
    output: &str,
    images: &dyn Fn(i64) -> Option<PathBuf>,
    fonts: &FontSet,
    audio: Option<&Path>,
) -> io::Result<Vec<String>> {
    let duration = spec.duration_seconds;
    let background = hex_color(&spec.background).ok_or_else(|| io::Error::other("Bad colour"))?;
    let mut args: Vec<String> = [
        "-nostdin",
        "-v",
        "error",
        "-filter_threads",
        "1",
        "-f",
        "lavfi",
        "-i",
    ]
    .map(String::from)
    .to_vec();
    match &spec.background_to {
        Some(to) => {
            let fill = Fill::Color {
                from: &spec.background,
                to: Some(to),
            };
            let shape = Shape {
                width,
                height,
                radius: 0.0,
                opacity: 1.0,
                shadow: None,
            };
            shape_png(directory, "background.png", &fill, &shape)?;
            args.truncate(args.len() - 3);
            args.extend(["-loop", "1", "-framerate", "30", "-t"].map(String::from));
            args.push(format!("{duration:.3}"));
            args.extend(["-i".into(), "background.png".into()]);
        }
        None => args.push(format!(
            "color=c={background}:s={width}x{height}:r={FPS}:d={duration:.3}"
        )),
    }
    match audio {
        Some(audio) => args.extend(["-i".to_owned(), audio.to_string_lossy().into_owned()]),
        None => args.extend(["-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo"].map(String::from)),
    }
    let mut graph = String::new();
    let layers = LayerTarget {
        width,
        height,
        directory,
        prefix: "",
        shift: 0.0,
        length: duration,
        fonts,
    };
    let current = layers.draw(spec, "0:v", 2, &mut graph, &mut args, images)?;
    let pixel_format = if width.is_multiple_of(2) && height.is_multiple_of(2) {
        "yuv420p"
    } else {
        "yuv444p"
    };
    write!(graph, "[{current}]format={pixel_format},setsar=1[out]").ok();
    // Sound effects mix over the soundtrack (narration or silence) at each layer's entrance.
    // The soundtrack is padded to exactly the animation's length: an endless pad in the same
    // graph as the picture occasionally deadlocked FFmpeg 8.
    let mut mix = format!(
        ";[1:a:0]aformat=sample_rates=48000:channel_layouts=stereo,apad=whole_dur={duration:.3},atrim=end={duration:.3}[bed]"
    );
    let mut inputs = vec!["[bed]".to_owned()];
    for (index, layer) in spec.layers.iter().enumerate() {
        if let Some(path) = layer.sound_asset_id.and_then(images) {
            let input = args.iter().filter(|arg| *arg == "-i").count();
            args.extend(["-i".to_owned(), path.to_string_lossy().into_owned()]);
            let delay = (layer.start * 1000.0).round() as u64;
            write!(
                mix,
                ";[{input}:a:0]aformat=sample_rates=48000:channel_layouts=stereo,\
                 afade=t=out:st={SOUND_ASSET_SECONDS}:d=0.3,atrim=end={:.3},adelay={delay}:all=1[fx{index}]",
                SOUND_ASSET_SECONDS + 0.3
            )
            .ok();
            inputs.push(format!("[fx{index}]"));
        } else if let Some(source) = layer.sound.source() {
            let delay = (layer.start * 1000.0).round() as u64;
            write!(
                mix,
                ";{source},aformat=sample_rates=48000:channel_layouts=stereo,adelay={delay}:all=1[fx{index}]"
            )
            .ok();
            inputs.push(format!("[fx{index}]"));
        }
    }
    if inputs.len() == 1 {
        mix.push_str(";[bed]anull[audio]");
    } else {
        write!(
            mix,
            ";{}amix=inputs={}:normalize=0:duration=first[audio]",
            inputs.join(""),
            inputs.len()
        )
        .ok();
    }
    graph.push_str(&mix);
    args.extend(["-filter_complex".into(), graph]);
    args.extend(["-map", "[out]", "-map", "[audio]", "-t"].map(String::from));
    args.push(format!("{duration:.3}"));
    args.extend(
        [
            "-r",
            "30",
            "-c:v",
            "libx264",
            "-threads",
            "2",
            "-c:a",
            "aac",
            "-ar",
            "48000",
            "-ac",
            "2",
            "-movflags",
            "+faststart",
            "-y",
        ]
        .map(String::from),
    );
    args.push(output.to_owned());
    Ok(args)
}

/// Runs FFmpeg in `directory`, calling `check` while it works so callers can keep a lease or stop.
/// How long FFmpeg may go without its output growing before it is considered stuck.
const STALL: Duration = Duration::from_secs(30);
const ATTEMPTS: usize = 3;

pub(crate) fn run_ffmpeg(
    directory: &Path,
    args: &[String],
    check: &mut dyn FnMut() -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    for attempt in 1..=ATTEMPTS {
        match run_ffmpeg_once(directory, args, check)? {
            Some(()) => return Ok(()),
            None if attempt < ATTEMPTS => {
                eprintln!("FFmpeg stalled; retrying ({attempt}/{ATTEMPTS})")
            }
            None => {}
        }
    }
    Err(io::Error::other("FFmpeg stopped making progress").into())
}

/// Runs FFmpeg once. `Ok(None)` means it stalled and was stopped.
fn run_ffmpeg_once(
    directory: &Path,
    args: &[String],
    check: &mut dyn FnMut() -> Result<(), Box<dyn Error>>,
) -> Result<Option<()>, Box<dyn Error>> {
    let output = args.last().map(|name| directory.join(name));
    let size = || {
        output
            .as_ref()
            .and_then(|path| fs::metadata(path).ok())
            .map_or(0, |meta| meta.len())
    };
    let mut grown = (size(), Instant::now());
    let log = directory.join("ffmpeg-errors.txt");
    let mut process = Command::new("ffmpeg")
        .args(args)
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(fs::File::create(&log)?)
        .spawn()?;
    let mut last = Instant::now();
    loop {
        if let Some(status) = process.try_wait()? {
            if status.success() {
                return Ok(Some(()));
            }
            let errors = fs::read_to_string(&log).unwrap_or_default();
            let tail: String = errors.lines().rev().take(6).collect::<Vec<_>>().join(" | ");
            return Err(
                io::Error::other(format!("FFmpeg could not render the animation: {tail}")).into(),
            );
        }
        if last.elapsed() >= Duration::from_secs(2) {
            last = Instant::now();
            if let Err(error) = check() {
                let _ = process.kill();
                let _ = process.wait();
                return Err(error);
            }
            let now = size();
            if now != grown.0 {
                grown = (now, Instant::now());
            } else if grown.1.elapsed() >= STALL {
                let _ = process.kill();
                let _ = process.wait();
                return Ok(None);
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Renders `spec` to `output` (inside `directory`) at the given size.
pub(crate) fn render_spec(
    spec: &MotionSpec,
    size: (u32, u32),
    directory: &Path,
    output: &str,
    images: &dyn Fn(i64) -> Option<PathBuf>,
    fonts: &FontSet,
    check: &mut dyn FnMut() -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    render_spec_with_audio(spec, size, directory, output, images, fonts, None, check)
}

/// Renders `spec` with `audio` as its soundtrack, padded with silence to the spec's duration.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_spec_with_audio(
    spec: &MotionSpec,
    (width, height): (u32, u32),
    directory: &Path,
    output: &str,
    images: &dyn Fn(i64) -> Option<PathBuf>,
    fonts: &FontSet,
    audio: Option<&Path>,
    check: &mut dyn FnMut() -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    ensure_fonts(directory, fonts)?;
    let args = render_arguments(spec, width, height, directory, output, images, fonts, audio)?;
    run_ffmpeg(directory, &args, check)
}

/// Frames in an animation's thumbnail, sampled evenly across it.
const SUMMARY_FRAMES: u32 = 6;

/// Saves a thumbnail that summarises a rendered animation: six moments across its length in a
/// three-by-two grid, so the motion reads at a glance.
pub(crate) fn render_thumbnail(
    directory: &Path,
    video: &str,
    seconds: f64,
    output: &str,
) -> Result<(), Box<dyn Error>> {
    // Sample the middle of each sixth so the first and last frames (often empty) are skipped.
    let step = seconds.max(0.1) / f64::from(SUMMARY_FRAMES);
    for frame in 0..SUMMARY_FRAMES {
        let at = (step * (f64::from(frame) + 0.5)).min((seconds - 0.05).max(0.0));
        let name = format!("summary-{frame}.png");
        let args: Vec<String> = [
            "-nostdin",
            "-v",
            "error",
            "-ss",
            &format!("{at:.3}"),
            "-i",
            video,
            "-frames:v",
            "1",
            "-vf",
            "scale=480:-2",
            "-y",
            &name,
        ]
        .map(String::from)
        .to_vec();
        run_ffmpeg(directory, &args, &mut || Ok(()))?;
    }
    let args: Vec<String> = [
        "-nostdin",
        "-v",
        "error",
        "-framerate",
        "1",
        "-start_number",
        "0",
        "-i",
        "summary-%d.png",
        "-vf",
        "tile=3x2:padding=6:margin=6:color=0x101418",
        "-frames:v",
        "1",
        "-y",
        output,
    ]
    .map(String::from)
    .to_vec();
    let made = run_ffmpeg(directory, &args, &mut || Ok(()));
    for frame in 0..SUMMARY_FRAMES {
        let _ = fs::remove_file(directory.join(format!("summary-{frame}.png")));
    }
    made
}

/// SubRip captions timed to when each scene plays: its spoken words when it is narrated,
/// otherwise its on-screen text.
pub(crate) fn storyboard_subtitles(scenes: &[MotionSpec], spoken: &[Option<&str>]) -> String {
    let stamp = |seconds: f64| {
        let total = (seconds * 1000.0).round() as u64;
        format!(
            "{:02}:{:02}:{:02},{:03}",
            total / 3_600_000,
            total / 60_000 % 60,
            total / 1000 % 60,
            total % 1000
        )
    };
    let mut srt = String::new();
    let mut offset = 0.0;
    let mut cue = 1;
    for (index, scene) in scenes.iter().enumerate() {
        let mut lines: Vec<&str> = Vec::new();
        if let Some(words) = spoken
            .get(index)
            .copied()
            .flatten()
            .filter(|words| !words.trim().is_empty())
        {
            lines.push(words);
        }
        let narrated = !lines.is_empty();
        for layer in scene.layers.iter().filter(|_| !narrated) {
            if let Some(text) = layer.text.as_deref()
                && !lines.contains(&text)
            {
                lines.push(text);
            }
        }
        if !lines.is_empty() {
            let end = offset + scene.duration_seconds - 0.05;
            let _ = write!(
                srt,
                "{cue}\n{} --> {}\n{}\n\n",
                stamp(offset),
                stamp(end),
                lines.join("\n")
            );
            cue += 1;
        }
        offset += scene.duration_seconds;
    }
    srt
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer() -> Value {
        json!({
            "title": "Subscribe",
            "kind": "animation",
            "usage": "End card",
            "durationSeconds": 3,
            "background": "#102030",
            "layers": [
                {"type": "box", "text": null, "assetId": null, "x": 0.5, "y": 0.8, "width": 0.6,
                 "height": 0.12, "size": 0.05, "color": "#E03030", "bold": false, "opacity": 1,
                 "start": 0, "end": 3, "enter": "rise", "exit": "fade", "sound": "whoosh"},
                {"type": "text", "text": "Subscribe for more", "assetId": null, "x": 0.5, "y": 0.8,
                 "width": 0.6, "height": 0.1, "size": 0.06, "color": "#FFFFFF", "bold": true,
                 "opacity": 1, "start": 0.3, "end": 3, "enter": "slide-left", "exit": "none"},
                {"type": "image", "text": null, "assetId": 99, "x": 0.5, "y": 0.3, "width": 0.3,
                 "height": 0.3, "size": 0.05, "color": "#FFFFFF", "bold": false, "opacity": 1,
                 "start": 0, "end": 3, "enter": "fade", "exit": "fade"}
            ]
        })
    }

    #[test]
    fn answers_are_clamped_and_unknown_images_dropped() {
        let mut value = answer();
        value["durationSeconds"] = json!(90);
        value["layers"][1]["x"] = json!(4);
        let spec = MotionSpec::from_answer(&value, |id| id == 1).unwrap();
        assert_eq!(spec.duration_seconds, MAX_STORED_SECONDS);
        assert_eq!(spec.layers.len(), 2, "the unknown image is dropped");
        assert_eq!(spec.layers[1].x, 1.0);
        value["background"] = json!("red; drop");
        assert!(MotionSpec::from_answer(&value, |_| true).is_err());
    }

    #[test]
    fn storyboards_caption_their_text_per_scene() {
        let spec = MotionSpec::from_answer(&answer(), |_| true).unwrap();
        let srt = storyboard_subtitles(&[spec.clone(), spec.clone()], &[]);
        assert_eq!(
            srt,
            "1\n00:00:00,000 --> 00:00:02,950\nSubscribe for more\n\n2\n00:00:03,000 --> 00:00:05,950\nSubscribe for more\n\n"
        );
        let spoken = storyboard_subtitles(&[spec.clone(), spec], &[None, Some("Hit subscribe.")]);
        assert!(spoken.ends_with("2\n00:00:03,000 --> 00:00:05,950\nHit subscribe.\n\n"));
    }

    #[test]
    fn text_wraps_to_the_layer_width() {
        assert_eq!(wrap("one two three four", 9), "one two\nthree\nfour");
        assert_eq!(wrap("abcdefghij", 4), "abcd\nefgh\nij");
        assert_eq!(clean_text("Top ↻ 90° ⬜🟥 ok", 50), "Top 90° ok");
    }

    fn clip_layer() -> Value {
        json!({"type": "video", "text": null, "assetId": 7, "x": 0.5, "y": 0.5, "width": 0.8,
               "height": 0.8, "size": 0.05, "color": "#2337B4", "bold": false, "opacity": 1,
               "start": 0.5, "end": 3, "enter": "fade", "exit": "none", "sound": "none",
               "align": "center", "radius": 0.05, "gradientTo": null, "shadow": true,
               "font": "body", "clipStart": 1,
               "zoom": [{"at": 2.2, "x": 0.9, "y": 0.1, "scale": 9},
                        {"at": 1.5, "x": 0.5, "y": 0.5, "scale": 1}]})
    }

    #[test]
    fn clip_layers_are_offered_only_with_clips() {
        let plain = spec_schema(&[1]);
        let layer = &plain["properties"]["layers"]["items"];
        assert!(layer["properties"].get("zoom").is_none());
        assert!(!layer["properties"]["type"].to_string().contains("video"));
        let offered = spec_schema_with_clips(&[1], &[7]);
        let layer = &offered["properties"]["layers"]["items"];
        assert!(layer["properties"]["type"].to_string().contains("video"));
        assert!(layer["properties"]["assetId"].to_string().contains('7'));
        let required = layer["required"].to_string();
        assert!(required.contains("clipStart") && required.contains("zoom"));
    }

    #[test]
    fn clip_layers_need_an_approved_clip_and_keep_tidy_camera_keys() {
        let mut value = answer();
        value["layers"] = json!([clip_layer()]);
        assert!(
            MotionSpec::from_answer(&value, |_| true).is_err(),
            "plain animations never play clips"
        );
        let spec = MotionSpec::from_answer_with_clips(&value, |_| false, |id| id == 7).unwrap();
        let layer = &spec.layers[0];
        assert_eq!(layer.kind, LayerType::Video);
        assert_eq!(layer.clip_start, 1.0);
        assert_eq!(layer.zoom.len(), 2);
        assert_eq!(layer.zoom[0].at, 1.5, "keys are sorted by time");
        assert_eq!(layer.zoom[1].scale, MAX_ZOOM, "zoom is capped");
        assert!(MotionSpec::from_answer_with_clips(&value, |_| true, |id| id == 8).is_err());
        let mut boxed = answer();
        boxed["layers"][0]["zoom"] = json!([{"at": 0, "x": 0, "y": 0, "scale": 2}]);
        boxed["layers"][0]["clipStart"] = json!(4);
        let spec = MotionSpec::from_answer_with_clips(&boxed, |_| true, |_| true).unwrap();
        assert!(spec.layers[0].zoom.is_empty() && spec.layers[0].clip_start == 0.0);
    }

    #[test]
    fn camera_keys_ease_between_values() {
        let keys = [
            ZoomKey {
                at: 1.0,
                x: 0.5,
                y: 0.5,
                scale: 1.0,
            },
            ZoomKey {
                at: 2.0,
                x: 0.5,
                y: 0.5,
                scale: 2.0,
            },
        ];
        assert_eq!(
            keyed(&keys, |key| key.scale),
            "if(lt(it,1.000),1.0000,if(lt(it,2.000),1.0000+(1.0000)*(1-cos(PI*(it-1.000)/1.000))/2,2.0000))"
        );
        assert_eq!(
            keyed(&keys[..1], |key| key.x),
            "if(lt(it,1.000),0.5000,0.5000)"
        );
    }

    #[test]
    fn consecutive_scenes_never_replay_a_clip() {
        let scene = |clip_start: f64, seconds: f64| {
            MotionSpec::from_answer_with_clips(
                &json!({"title": "Clip", "kind": "animation", "usage": "", "durationSeconds": seconds,
                    "background": "#FFFFFF", "backgroundTo": null, "layers": [clip_layer()]}),
                |_| false,
                |id| id == 7,
            )
            .map(|mut spec| {
                spec.layers[0].clip_start = clip_start;
                spec.layers[0].start = 0.0;
                spec.layers[0].end = seconds;
                spec
            })
            .unwrap()
        };
        let mut scenes = vec![
            scene(10.0, 7.5),
            scene(15.5, 9.0),
            scene(30.0, 2.0),
            scene(0.0, 2.0),
        ];
        continue_clips(&mut scenes);
        let starts: Vec<f64> = scenes
            .iter()
            .map(|scene| scene.layers[0].clip_start)
            .collect();
        assert_eq!(starts, [10.0, 17.5, 30.0, 32.0]);
    }

    #[test]
    fn continued_clips_keep_spotlights_on_their_moments() {
        let spec = |clip_start: f64, lights: Value| {
            let mut layer = clip_layer();
            layer["start"] = json!(0.0);
            layer["end"] = json!(8.0);
            layer["spotlight"] = lights;
            let mut spec = MotionSpec::from_answer_with_clips(
                &json!({"title": "Clip", "kind": "animation", "usage": "", "durationSeconds": 8.0,
                    "background": "#FFFFFF", "backgroundTo": null, "layers": [layer]}),
                |_| false,
                |id| id == 7,
            )
            .unwrap();
            spec.layers[0].clip_start = clip_start;
            spec.layers[0].start = 0.0;
            spec.layers[0].end = 8.0;
            spec
        };
        let light = |from: f64, to: f64| json!({"from": from, "to": to, "x": 0.5, "y": 0.5, "width": 0.4, "height": 0.3});
        let mut scenes = vec![
            spec(0.0, json!([])),
            spec(5.0, json!([light(1.0, 2.5), light(4.0, 7.0)])),
        ];
        continue_clips(&mut scenes);
        let layer = &scenes[1].layers[0];
        assert_eq!(layer.clip_start, 8.0);
        let lights: Vec<(f64, f64)> = layer.spotlight.iter().map(|l| (l.from, l.to)).collect();
        assert_eq!(lights, [(1.0, 4.0)]);
    }

    #[test]
    fn clip_layers_render_rounded_and_zoomed() {
        if Command::new("ffmpeg").arg("-version").output().is_err() {
            return;
        }
        let directory = tempfile::tempdir().unwrap();
        // A clip whose top-right quarter is red and the rest green, so zooming in shows red.
        let clip = directory.path().join("clip.mp4");
        let made = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=green:s=320x200:r=30:d=4",
                "-vf",
            ])
            .arg("drawbox=x=160:y=0:w=160:h=100:color=red:t=fill")
            .args(["-pix_fmt", "yuv420p", "-y"])
            .arg(&clip)
            .status()
            .unwrap();
        assert!(made.success());
        let mut value = answer();
        value["durationSeconds"] = json!(3);
        value["layers"] = json!([clip_layer()]);
        let spec = MotionSpec::from_answer_with_clips(&value, |_| false, |_| true).unwrap();
        let resolve = |id: i64| (id == 7).then(|| clip.clone());
        render_spec(
            &spec,
            (640, 360),
            directory.path(),
            "out.mp4",
            &resolve,
            &FontSet::default(),
            &mut || Ok(()),
        )
        .unwrap();
        let frame_at = |at: &str| {
            Command::new("ffmpeg")
                .args(["-v", "error", "-ss", at, "-i"])
                .arg(directory.path().join("out.mp4"))
                .args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
                .output()
                .unwrap()
                .stdout
        };
        let pixel = |frame: &[u8], x: usize, y: usize| {
            let at = (y * 640 + x) * 3;
            [frame[at], frame[at + 1], frame[at + 2]]
        };
        let before = frame_at("0.2");
        let empty = pixel(&before, 320, 180);
        assert!(
            empty[0] < 60 && empty[1] < 60,
            "the clip has not entered yet: {empty:?}"
        );
        let fitted = frame_at("1.2");
        // Fitted inside 512x288, the 16:10 clip is 460x288: green bottom-left, red top-right.
        let green = pixel(&fitted, 250, 250);
        assert!(green[1] > 90 && green[0] < 80, "{green:?}");
        let red = pixel(&fitted, 450, 100);
        assert!(red[0] > 150 && red[1] < 80, "{red:?}");
        // The rounded corner shows the background, not the clip.
        let corner = pixel(&fitted, 91, 37);
        assert!(corner[0] < 60 && corner[1] < 60, "{corner:?}");
        let zoomed = frame_at("2.5");
        let centre = pixel(&zoomed, 320, 180);
        assert!(
            centre[0] > 150 && centre[1] < 80,
            "zoomed into the red corner: {centre:?}"
        );
    }

    #[test]
    fn spotlights_dim_all_but_their_area() {
        if Command::new("ffmpeg").arg("-version").output().is_err() {
            return;
        }
        let directory = tempfile::tempdir().unwrap();
        let clip = directory.path().join("clip.mp4");
        let made = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=0x40C040:s=320x200:r=30:d=4",
            ])
            .args(["-pix_fmt", "yuv420p", "-y"])
            .arg(&clip)
            .status()
            .unwrap();
        assert!(made.success());
        let mut layer = clip_layer();
        layer["zoom"] = json!([]);
        layer["spotlight"] =
            json!([{"from": 0.8, "to": 2.8, "x": 0.25, "y": 0.5, "width": 0.4, "height": 0.8}]);
        let mut value = answer();
        value["durationSeconds"] = json!(3);
        value["layers"] = json!([layer]);
        let spec = MotionSpec::from_answer_with_clips(&value, |_| false, |_| true).unwrap();
        assert_eq!(spec.layers[0].spotlight.len(), 1);
        let mut handover = clip_layer();
        handover["spotlight"] = json!([
            {"from": 2.0, "to": 2.9, "x": 0.8, "y": 0.5, "width": 0.2, "height": 0.2},
            {"from": 0.5, "to": 1.9, "x": 0.2, "y": 0.5, "width": 0.2, "height": 0.2}]);
        let mut handed = answer();
        handed["durationSeconds"] = json!(3);
        handed["layers"] = json!([handover]);
        let lights = MotionSpec::from_answer_with_clips(&handed, |_| false, |_| true)
            .unwrap()
            .layers[0]
            .spotlight
            .clone();
        assert_eq!(
            (lights[0].from, lights[0].to),
            (0.5, 2.0 + SPOTLIGHT_EASE),
            "fades overlap"
        );
        assert_eq!((lights[1].from, lights[1].to), (2.0, 2.9));
        let resolve = |id: i64| (id == 7).then(|| clip.clone());
        render_spec(
            &spec,
            (640, 360),
            directory.path(),
            "out.mp4",
            &resolve,
            &FontSet::default(),
            &mut || Ok(()),
        )
        .unwrap();
        let frame = Command::new("ffmpeg")
            .args(["-v", "error", "-ss", "2.0", "-i"])
            .arg(directory.path().join("out.mp4"))
            .args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
            .output()
            .unwrap()
            .stdout;
        let green = |x: usize, y: usize| frame[(y * 640 + x) * 3 + 1];
        // The 460 x 288 clip starts at x 90; the spotlight covers x 113 to 297.
        assert!(green(205, 180) > 150, "lit: {}", green(205, 180));
        assert!(green(450, 180) < 100, "dimmed: {}", green(450, 180));
    }

    #[test]
    fn rounded_gradient_boxes_and_backgrounds_render() {
        if Command::new("ffmpeg").arg("-version").output().is_err() || system_font("bold").is_none()
        {
            return;
        }
        let directory = tempfile::tempdir().unwrap();
        let spec = MotionSpec::from_answer(
            &json!({
                "title": "Card", "kind": "animation", "usage": "", "durationSeconds": 2,
                "background": "#000000", "backgroundTo": "#FFFFFF",
                "layers": [
                    {"type": "box", "text": null, "assetId": null, "x": 0.5, "y": 0.5, "width": 0.5,
                     "height": 0.4, "size": 0.05, "color": "#FF0000", "bold": false, "opacity": 1,
                     "start": 0, "end": 2, "enter": "none", "exit": "none", "sound": "none",
                     "align": "center", "radius": 0.3, "gradientTo": "#0000FF", "shadow": true},
                    {"type": "text", "text": "Left", "assetId": null, "x": 0.5, "y": 0.15,
                     "width": 0.8, "height": 0.1, "size": 0.08, "color": "#00FF00", "bold": true,
                     "opacity": 1, "start": 0, "end": 2, "enter": "none", "exit": "none",
                     "sound": "none", "align": "left", "radius": 0.2, "gradientTo": "#123456",
                     "shadow": true}
                ]
            }),
            |_| true,
        )
        .unwrap();
        assert_eq!(spec.layers[1].radius, 0.0);
        assert!(spec.layers[1].gradient_to.is_none() && !spec.layers[1].shadow);
        render_spec(
            &spec,
            (640, 360),
            directory.path(),
            "out.mp4",
            &|_| None,
            &FontSet::default(),
            &mut || Ok(()),
        )
        .unwrap();
        let frame = Command::new("ffmpeg")
            .args(["-v", "error", "-ss", "1", "-i"])
            .arg(directory.path().join("out.mp4"))
            .args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
            .output()
            .unwrap()
            .stdout;
        let pixel = |x: usize, y: usize| {
            let at = (y * 640 + x) * 3;
            [frame[at], frame[at + 1], frame[at + 2]]
        };
        let near = |value: [u8; 3], expected: [u8; 3]| {
            value.iter().zip(expected).all(|(a, b)| a.abs_diff(b) < 48)
        };
        assert!(near(pixel(3, 357), [10, 10, 10]) || pixel(3, 357)[0] < 140);
        assert!(pixel(636, 356)[0] > 200, "{:?}", pixel(636, 356));
        // The box's rounded corner shows the background; its body shows the gradient.
        let corner = pixel(163, 111);
        assert!(corner[0] < 100 && corner[2] < 100, "{corner:?}");
        let body = pixel(320, 180);
        assert!(body[0] > 80 && body[2] > 80 && body[1] < 60, "{body:?}");
        // Left-aligned text starts near the layer's left edge.
        let green = (60..200).any(|x| {
            (40..80).any(|y| {
                let [r, g, b] = pixel(x, y);
                g > 180 && r < 100 && b < 100
            })
        });
        assert!(green);
    }

    #[test]
    fn shaped_overlays_take_their_own_inputs_over_footage() {
        if Command::new("ffmpeg").arg("-version").output().is_err() || system_font("bold").is_none()
        {
            return;
        }
        let directory = tempfile::tempdir().unwrap();
        let spec = MotionSpec::from_answer(
            &json!({
                "title": "Lower third", "kind": "animation", "usage": "", "durationSeconds": 2,
                "background": "#000000", "backgroundTo": null,
                "layers": [{"type": "box", "text": null, "assetId": null, "x": 0.5, "y": 0.8,
                    "width": 0.6, "height": 0.2, "size": 0.05, "color": "#FF00FF", "bold": false,
                    "opacity": 1, "start": 0, "end": 2, "enter": "none", "exit": "none",
                    "sound": "none", "align": "center", "radius": 0.2, "gradientTo": null,
                    "shadow": true}]
            }),
            |_| true,
        )
        .unwrap();
        let (mut graph, mut inputs) = (String::new(), Vec::new());
        let mut drawn = "0:v".to_owned();
        for (number, at) in [0.0, 0.5].into_iter().enumerate() {
            let first = 1 + inputs.iter().filter(|arg| *arg == "-i").count();
            drawn = overlay_graph(
                &spec,
                (320, 180),
                directory.path(),
                &format!("o{number}"),
                at,
                &drawn,
                first,
                &mut graph,
                &mut inputs,
                &|_| None,
                &FontSet::default(),
            )
            .unwrap();
        }
        assert_eq!(inputs.iter().filter(|arg| *arg == "-i").count(), 2);
        graph.push_str(&format!("[{drawn}]null[video]"));
        let frame = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=0x404040:s=320x180:d=2:r=30",
            ])
            .args(&inputs)
            .args([
                "-filter_complex",
                &graph,
                "-map",
                "[video]",
                "-ss",
                "1",
                "-frames:v",
                "1",
            ])
            .args(["-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
            .current_dir(directory.path())
            .output()
            .unwrap();
        assert!(
            frame.status.success(),
            "{}",
            String::from_utf8_lossy(&frame.stderr)
        );
        let at = (144 * 320 + 160) * 3;
        let [r, g, b] = [frame.stdout[at], frame.stdout[at + 1], frame.stdout[at + 2]];
        assert!(r > 200 && g < 60 && b > 200, "{r} {g} {b}");
    }

    #[test]
    fn renders_a_spec_with_ffmpeg() {
        if Command::new("ffmpeg").arg("-version").output().is_err() || system_font("bold").is_none()
        {
            return;
        }
        let directory = tempfile::tempdir().unwrap();
        let image = directory.path().join("logo.png");
        let made = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=yellow:s=64x64",
                "-frames:v",
                "1",
            ])
            .arg(&image)
            .status()
            .unwrap();
        assert!(made.success());
        let spec = MotionSpec::from_answer(&answer(), |_| true).unwrap();
        let resolve = |_| Some(image.clone());
        render_spec(
            &spec,
            (320, 180),
            directory.path(),
            "out.mp4",
            &resolve,
            &FontSet::default(),
            &mut || Ok(()),
        )
        .unwrap();
        render_thumbnail(
            directory.path(),
            "out.mp4",
            spec.duration_seconds,
            "thumb.png",
        )
        .unwrap();
        let duration =
            crate::source_transcripts::media_duration(&directory.path().join("out.mp4")).unwrap();
        assert!((duration - 3.0).abs() < 0.2, "{duration}");
        assert!(directory.path().join("thumb.png").is_file());

        // Every effect renders and is audible.
        for (number, sound) in ["pop", "click", "chime", "rise", "whoosh"]
            .iter()
            .enumerate()
        {
            let mut value = answer();
            value["layers"][0]["sound"] = json!(sound);
            let spec = MotionSpec::from_answer(&value, |_| true).unwrap();
            let name = format!("sound-{number}.mp4");
            render_spec(
                &spec,
                (160, 90),
                directory.path(),
                &name,
                &resolve,
                &FontSet::default(),
                &mut || Ok(()),
            )
            .unwrap();
            let report = Command::new("ffmpeg")
                .args(["-v", "info", "-i"])
                .arg(directory.path().join(&name))
                .args(["-af", "volumedetect", "-vn", "-f", "null", "-"])
                .output()
                .unwrap();
            let report = String::from_utf8_lossy(&report.stderr);
            let peak: f64 = report
                .lines()
                .find_map(|line| line.split("max_volume: ").nth(1))
                .and_then(|value| value.trim_end_matches(" dB").parse().ok())
                .unwrap();
            assert!(peak > -30.0, "{sound} peaked at {peak} dB");
        }

        // A layer's sound-effect file plays in place of the built-in sound, cut after a few seconds.
        let effect = directory.path().join("effect.wav");
        let made = Command::new("ffmpeg")
            .args(["-v", "error", "-f", "lavfi", "-i", "sine=f=300:d=6"])
            .arg(&effect)
            .status()
            .unwrap();
        assert!(made.success());
        let mut value = answer();
        value["layers"][0]["sound"] = json!("none");
        value["layers"][0]["soundAssetId"] = json!(9);
        value["layers"][0]["start"] = json!(0);
        value["durationSeconds"] = json!(6);
        let spec =
            MotionSpec::from_answer_with_media(&value, |_| true, |_| false, |id| id == 9).unwrap();
        assert_eq!(spec.layers[0].sound_asset_id, Some(9));
        let resolve = |id| {
            (id == 9)
                .then(|| effect.clone())
                .or_else(|| Some(image.clone()))
        };
        render_spec(
            &spec,
            (160, 90),
            directory.path(),
            "effect.mp4",
            &resolve,
            &FontSet::default(),
            &mut || Ok(()),
        )
        .unwrap();
        let level = |from: f64| {
            let report = Command::new("ffmpeg")
                .args(["-v", "info", "-ss", &from.to_string(), "-t", "0.5", "-i"])
                .arg(directory.path().join("effect.mp4"))
                .args(["-af", "volumedetect", "-vn", "-f", "null", "-"])
                .output()
                .unwrap();
            String::from_utf8_lossy(&report.stderr)
                .lines()
                .find_map(|line| line.split("max_volume: ").nth(1))
                .and_then(|value| value.trim_end_matches(" dB").parse::<f64>().ok())
                .unwrap_or(-91.0)
        };
        assert!(level(1.0) > -30.0, "the effect plays");
        assert!(level(5.0) < -60.0, "the effect stops after its limit");
    }

    #[test]
    fn boxes_may_be_hairlines_but_text_keeps_room_to_wrap() {
        let mut value = answer();
        value["layers"][0]["type"] = json!("box");
        value["layers"][0]["width"] = json!(0.001);
        value["layers"][0]["height"] = json!(0.0005);
        let spec = MotionSpec::from_answer(&value, |_| true).unwrap();
        assert_eq!(spec.layers[0].width, 0.001);
        assert_eq!(spec.layers[0].height, 0.001);
        value["layers"][0]["type"] = json!("text");
        let spec = MotionSpec::from_answer(&value, |_| true).unwrap();
        assert!(spec.layers[0].width >= 0.02);
        assert!(spec.layers[0].height >= 0.005);
    }

    #[test]
    fn layers_keep_only_offered_sound_effects() {
        let mut value = answer();
        value["layers"][0]["sound"] = json!("chime");
        value["layers"][0]["soundAssetId"] = json!(4);
        let refused =
            MotionSpec::from_answer_with_media(&value, |_| true, |_| false, |_| false).unwrap();
        assert_eq!(refused.layers[0].sound_asset_id, None);
        assert_eq!(refused.layers[0].sound, Sound::Chime);
        let kept =
            MotionSpec::from_answer_with_media(&value, |_| true, |_| false, |id| id == 4).unwrap();
        assert_eq!(kept.layers[0].sound_asset_id, Some(4));
        assert_eq!(kept.layers[0].sound, Sound::None);
        // Plain animations never carry a sound-effect file.
        let plain = MotionSpec::from_answer(&value, |_| true).unwrap();
        assert_eq!(plain.layers[0].sound_asset_id, None);
    }
}

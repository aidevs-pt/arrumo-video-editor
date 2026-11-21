//! Animations the agent designs as data: timed text, boxes and workspace images with entrance and
//! exit motion. Arrumo validates the spec and renders it with FFmpeg at any frame size, so the same
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

use crate::MotionKind;

pub(crate) const MAX_LAYERS: usize = 12;
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
    pub layers: Vec<Layer>,
}

/// The JSON schema agents answer with for one animation. `assets` are the image IDs it may use.
pub(crate) fn spec_schema(assets: &[i64]) -> Value {
    let motion = json!({"enum": ["none", "fade", "rise", "drop", "slide-left", "slide-right"]});
    let unit = json!({"type": "number", "minimum": 0, "maximum": 1});
    let asset = if assets.is_empty() {
        json!({"type": "null"})
    } else {
        json!({"anyOf": [{"type": "null"}, {"enum": assets}]})
    };
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["title", "kind", "usage", "durationSeconds", "background", "layers"],
        "properties": {
            "title": {"type": "string", "maxLength": 80},
            "kind": {"enum": ["animation", "transition"]},
            "usage": {"type": "string", "maxLength": 600},
            "durationSeconds": {"type": "number", "minimum": MIN_SECONDS, "maximum": MAX_SECONDS},
            "background": {"type": "string", "pattern": "^#[0-9A-Fa-f]{6}$"},
            "layers": {
                "type": "array",
                "minItems": 1,
                "maxItems": MAX_LAYERS,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["type", "text", "assetId", "x", "y", "width", "height", "size",
                        "color", "bold", "opacity", "start", "end", "enter", "exit", "sound"],
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
                        "sound": {"enum": ["none", "whoosh", "pop", "click", "chime", "rise"]}
                    }
                }
            }
        }
    })
}

/// How a spec is described to the agent, next to the schema.
pub(crate) const SPEC_GUIDE: &str = "An animation is a timed composition on a solid background. \
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
matter, and none when the animation will sit under speech. Text is set in a plain sans-serif font that has no emoji, pictographs or arrows, so \
write only letters, digits and common punctuation, and draw shapes, grids, bars and diagrams with box \
layers. Leave text null for boxes and images and assetId null for text and boxes.";

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
        let duration = spec.duration_seconds;
        spec.layers.truncate(MAX_LAYERS);
        spec.layers.retain_mut(|layer| {
            let unit = |value: f64| {
                if value.is_finite() {
                    value.clamp(0.0, 1.0)
                } else {
                    0.5
                }
            };
            layer.x = unit(layer.x);
            layer.y = unit(layer.y);
            layer.width = unit(layer.width).max(0.02);
            layer.height = unit(layer.height).max(0.005);
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

pub(crate) fn ensure_fonts(directory: &Path) -> io::Result<()> {
    if directory.join("font-bold").is_file() {
        return Ok(());
    }
    install_fonts(directory)
}

/// Fonts drawtext uses, copied into the job directory so filter arguments need no path escaping.
fn install_fonts(directory: &Path) -> io::Result<()> {
    for (name, weight) in [("font-regular", "regular"), ("font-bold", "bold")] {
        let source = system_font(weight)
            .ok_or_else(|| io::Error::other("No system font was found for animation text"))?;
        fs::copy(source, directory.join(name))?;
    }
    Ok(())
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
                    let columns = (layer.width * frame_w / (font_px * 0.52)).floor() as usize;
                    let wrapped = wrap(layer.text.as_deref().unwrap_or_default(), columns);
                    let font = if layer.bold {
                        "font-bold"
                    } else {
                        "font-regular"
                    };
                    // FFmpeg 8's drawtext draws a missing-glyph box at each line break, so
                    // every line is its own centred drawtext.
                    let lines: Vec<&str> = wrapped.lines().collect();
                    let line_height = font_px * 1.25;
                    let top = layer.y * frame_h - line_height * lines.len() as f64 / 2.0;
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
                         alpha='{alpha}':x='{:.2}-text_w/2+{dx}':y='{:.2}+{dy}':\
                         enable='{enable}'[{output}];",
                            layer.x * frame_w,
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
                    if layer.kind == LayerType::Box {
                        write!(
                        graph,
                        "color=c={color}@{:.3}:s={box_w}x{box_h}:r={FPS}:d={duration:.3},format=rgba[{source}];",
                        layer.opacity
                    )
                    .ok();
                    } else {
                        let path = layer
                            .asset_id
                            .and_then(images)
                            .ok_or_else(|| io::Error::other("An animation image is missing"))?;
                        args.extend(["-loop", "1", "-framerate", "30", "-t"].map(String::from));
                        args.push(format!("{duration:.3}"));
                        args.push("-i".into());
                        args.push(path.to_string_lossy().into_owned());
                        write!(
                        graph,
                        "[{next_input}:v]scale={box_w}:{box_h}:force_original_aspect_ratio=decrease,\
                         format=rgba,colorchannelmixer=aa={:.3}[{source}];",
                        layer.opacity
                    )
                    .ok();
                        next_input += 1;
                    }
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
            }
            current = label;
        }
        Ok(current)
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
) -> io::Result<String> {
    ensure_fonts(directory)?;
    LayerTarget {
        width,
        height,
        directory,
        prefix,
        shift: at,
        length: at + spec.duration_seconds,
    }
    .draw(spec, base, first_input, graph, args, images)
}

/// Builds the FFmpeg arguments that render `spec` at `width`×`height` into `output` (a name inside
/// `directory`). Text files are written next to it. `images` resolves asset IDs to files.
fn render_arguments(
    spec: &MotionSpec,
    width: u32,
    height: u32,
    directory: &Path,
    output: &str,
    images: &dyn Fn(i64) -> Option<PathBuf>,
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
    args.push(format!(
        "color=c={background}:s={width}x{height}:r={FPS}:d={duration:.3}"
    ));
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
    };
    let current = layers.draw(spec, "0:v", 2, &mut graph, &mut args, images)?;
    let pixel_format = if width.is_multiple_of(2) && height.is_multiple_of(2) {
        "yuv420p"
    } else {
        "yuv444p"
    };
    write!(graph, "[{current}]format={pixel_format},setsar=1[out]").ok();
    // Sound effects mix over the soundtrack (narration or silence) at each layer's entrance.
    let mut mix =
        String::from(";[1:a:0]aformat=sample_rates=48000:channel_layouts=stereo,apad[bed]");
    let mut inputs = vec!["[bed]".to_owned()];
    for (index, layer) in spec.layers.iter().enumerate() {
        if let Some(source) = layer.sound.source() {
            let delay = (layer.start * 1000.0).round() as u64;
            write!(
                mix,
                ";{source},aformat=sample_rates=48000:channel_layouts=stereo,adelay={delay}:all=1[fx{index}]"
            )
            .ok();
            inputs.push(format!("[fx{index}]"));
        }
    }
    write!(
        mix,
        ";{}amix=inputs={}:normalize=0:duration=first[audio]",
        inputs.join(""),
        inputs.len()
    )
    .ok();
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
pub(crate) fn run_ffmpeg(
    directory: &Path,
    args: &[String],
    check: &mut dyn FnMut() -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
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
                return Ok(());
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
    check: &mut dyn FnMut() -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    render_spec_with_audio(spec, size, directory, output, images, None, check)
}

/// Renders `spec` with `audio` as its soundtrack, padded with silence to the spec's duration.
pub(crate) fn render_spec_with_audio(
    spec: &MotionSpec,
    (width, height): (u32, u32),
    directory: &Path,
    output: &str,
    images: &dyn Fn(i64) -> Option<PathBuf>,
    audio: Option<&Path>,
    check: &mut dyn FnMut() -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    ensure_fonts(directory)?;
    let args = render_arguments(spec, width, height, directory, output, images, audio)?;
    run_ffmpeg(directory, &args, check)
}

/// Saves a still from the middle of a rendered animation.
pub(crate) fn render_thumbnail(
    directory: &Path,
    video: &str,
    seconds: f64,
    output: &str,
) -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = [
        "-nostdin",
        "-v",
        "error",
        "-ss",
        &format!("{:.3}", seconds * 0.6),
        "-i",
        video,
        "-frames:v",
        "1",
        "-y",
        output,
    ]
    .map(String::from)
    .to_vec();
    run_ffmpeg(directory, &args, &mut || Ok(()))
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
    }
}

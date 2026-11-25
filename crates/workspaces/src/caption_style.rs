//! How burned-in subtitles look: one style per output format, overridden field by field by a
//! profile and then by a single video, and drawn by libass for both the export and its preview.

use crate::Launch;
use std::io;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptionFont {
    #[default]
    Sans,
    Serif,
    Mono,
}

impl CaptionFont {
    fn family(self) -> &'static str {
        match self {
            Self::Sans => "Sans",
            Self::Serif => "Serif",
            Self::Mono => "Monospace",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptionPosition {
    #[default]
    Bottom,
    Middle,
    Top,
}

/// A complete caption style. Sizes are relative so one style fits any frame size.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct CaptionStyle {
    pub font: CaptionFont,
    pub bold: bool,
    /// Text height in tenths of a percent of the frame's shorter side (20 to 120).
    pub size: u16,
    /// Text colour as `#RRGGBB`.
    pub color: String,
    /// Outline, or box, colour as `#RRGGBB`.
    pub outline_color: String,
    /// Outline width as a percent of the text height (0 to 30).
    pub outline: u8,
    /// Draw a filled box behind the text in the outline colour instead of an outline.
    pub background: bool,
    pub position: CaptionPosition,
    /// Distance from the top or bottom edge as a percent of the frame height (0 to 40).
    /// `None` keeps the default: higher on portrait frames, above short-video app controls.
    pub margin: Option<u8>,
}

impl Default for CaptionStyle {
    fn default() -> Self {
        Self {
            font: CaptionFont::Sans,
            bold: true,
            size: 55,
            color: "#FFFFFF".into(),
            outline_color: "#000000".into(),
            outline: 10,
            background: false,
            position: CaptionPosition::Bottom,
            margin: None,
        }
    }
}

/// Changes to a [`CaptionStyle`]: each field set here replaces the inherited one.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct CaptionStyleOverrides {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<CaptionFont>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<CaptionPosition>,
    /// `Some(None)` returns to the automatic margin.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    pub margin: Option<Option<u8>>,
}

mod double_option {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(
        value: &Option<Option<u8>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.flatten().serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Option<u8>>, D::Error> {
        Option::<u8>::deserialize(deserializer).map(Some)
    }
}

impl CaptionStyleOverrides {
    pub fn apply(&self, base: &CaptionStyle) -> CaptionStyle {
        CaptionStyle {
            font: self.font.unwrap_or(base.font),
            bold: self.bold.unwrap_or(base.bold),
            size: self.size.unwrap_or(base.size),
            color: self.color.clone().unwrap_or_else(|| base.color.clone()),
            outline_color: self
                .outline_color
                .clone()
                .unwrap_or_else(|| base.outline_color.clone()),
            outline: self.outline.unwrap_or(base.outline),
            background: self.background.unwrap_or(base.background),
            position: self.position.unwrap_or(base.position),
            margin: self.margin.unwrap_or(base.margin),
        }
    }

    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

fn hex_color(value: &str) -> Option<(u8, u8, u8)> {
    let digits = value.strip_prefix('#')?;
    if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).ok();
    Some((channel(0)?, channel(2)?, channel(4)?))
}

impl CaptionStyle {
    pub fn valid(&self) -> bool {
        (20..=120).contains(&self.size)
            && self.outline <= 30
            && self.margin.is_none_or(|margin| margin <= 40)
            && hex_color(&self.color).is_some()
            && hex_color(&self.outline_color).is_some()
    }

    pub(crate) fn check(&self) -> io::Result<()> {
        if self.valid() {
            Ok(())
        } else {
            Err(io::Error::other(
                "Subtitle style needs a size of 2 to 12%, an outline up to 30%, a margin up to 40% and #RRGGBB colours",
            ))
        }
    }

    /// The libass `force_style` for a frame of `width` × `height`. libass lays SRT out on a
    /// 288-line canvas scaled to the frame height.
    pub(crate) fn force_style(&self, width: u32, height: u32) -> String {
        let canvas = |share: f64| share * 288.0;
        let size = canvas(f64::from(self.size) / 1000.0) * f64::from(width.min(height))
            / f64::from(height);
        let margin = match self.margin {
            Some(margin) => canvas(f64::from(margin) / 100.0),
            None => canvas(if height > width { 0.2 } else { 0.07 }),
        };
        let ass = |value: &str| {
            let (red, green, blue) = hex_color(value).unwrap_or((255, 255, 255));
            format!("&H00{blue:02X}{green:02X}{red:02X}")
        };
        // force_style on SRT uses the legacy SSA numbering: 2 bottom, 6 top, 10 middle (centred).
        let alignment = match self.position {
            CaptionPosition::Bottom => 2,
            CaptionPosition::Middle => 10,
            CaptionPosition::Top => 6,
        };
        let outline = size * f64::from(self.outline) / 100.0;
        format!(
            "FontName={},Bold={},FontSize={size:.1},PrimaryColour={},OutlineColour={},BackColour={},BorderStyle={},Outline={:.1},Shadow=0,Alignment={alignment},MarginV={margin:.0}",
            self.font.family(),
            i32::from(self.bold),
            ass(&self.color),
            ass(&self.outline_color),
            ass(&self.outline_color),
            if self.background { 3 } else { 1 },
            if self.background {
                outline.max(size * 0.15)
            } else {
                outline
            },
        )
    }
}

/// A PNG of `text` drawn with `style` on a neutral gradient frame of the format's shape, at
/// most 640 pixels on its longer side, by the same libass renderer the export uses.
pub fn caption_preview(
    style: &CaptionStyle,
    width: u32,
    height: u32,
    text: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    style.check()?;
    if width == 0 || height == 0 || text.trim().is_empty() || text.chars().count() > 200 {
        return Err(io::Error::other("Write up to 200 characters of sample text").into());
    }
    let scale = 640.0 / f64::from(width.max(height));
    let even = |value: u32| ((f64::from(value) * scale / 2.0).round() as u32).max(8) * 2;
    let (preview_width, preview_height) = (even(width), even(height));
    let work = tempfile::tempdir()?;
    let cue: String = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(3)
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(
        work.path().join("captions.srt"),
        format!("1\n00:00:00,000 --> 00:00:05,000\n{cue}\n"),
    )?;
    let status = crate::quiet_command(crate::media_tools::ffmpeg())
        .args(["-nostdin", "-v", "error", "-f", "lavfi", "-i"])
        .arg(format!(
            "gradients=s={preview_width}x{preview_height}:c0=0x35506e:c1=0xb08a62:x0=0:y0=0:x1={preview_width}:y1={preview_height}:d=1:speed=0:seed=7"
        ))
        .arg("-vf")
        .arg(format!(
            "subtitles=filename=captions.srt:charenc=UTF-8:force_style='{}'",
            style.force_style(preview_width, preview_height)
        ))
        .args(["-frames:v", "1", "-y", "preview.png"])
        .current_dir(work.path())
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .launch_status()?;
    if !status.success() {
        return Err(io::Error::other("Could not draw the subtitle preview").into());
    }
    Ok(std::fs::read(work.path().join("preview.png"))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_matches_the_earlier_fixed_style() {
        let landscape = CaptionStyle::default().force_style(1920, 1080);
        assert!(landscape.contains("FontSize=15.8,") && landscape.contains("MarginV=20"));
        assert!(
            landscape.contains("PrimaryColour=&H00FFFFFF") && landscape.contains("Alignment=2")
        );
        let portrait = CaptionStyle::default().force_style(1080, 1920);
        assert!(portrait.contains("FontSize=8.9,") && portrait.contains("MarginV=58"));
    }

    #[test]
    fn colours_positions_and_boxes_follow_the_style() {
        let style = CaptionStyle {
            color: "#FFCC00".into(),
            outline_color: "#102030".into(),
            position: CaptionPosition::Top,
            background: true,
            margin: Some(10),
            font: CaptionFont::Serif,
            ..Default::default()
        };
        let forced = style.force_style(1920, 1080);
        assert!(forced.contains("FontName=Serif"));
        assert!(forced.contains("PrimaryColour=&H0000CCFF"));
        assert!(forced.contains("OutlineColour=&H00302010"));
        assert!(forced.contains("BorderStyle=3") && forced.contains("Alignment=6"));
        assert!(forced.contains("MarginV=29"));
    }

    #[test]
    fn overrides_replace_only_their_fields() {
        let base = CaptionStyle {
            margin: Some(12),
            ..Default::default()
        };
        let overrides: CaptionStyleOverrides =
            serde_json::from_str(r##"{"color":"#00FF00","margin":null}"##).unwrap();
        let styled = overrides.apply(&base);
        assert_eq!(styled.color, "#00FF00");
        assert_eq!(styled.margin, None);
        assert_eq!(styled.size, base.size);
        let untouched: CaptionStyleOverrides = serde_json::from_str("{}").unwrap();
        assert_eq!(untouched.apply(&base), base);
        assert!(untouched.is_empty());
    }

    #[test]
    fn previews_are_drawn_at_the_format_shape() {
        if crate::quiet_command(crate::media_tools::ffmpeg())
            .arg("-version")
            .launch_output()
            .is_err()
        {
            return;
        }
        let png =
            caption_preview(&CaptionStyle::default(), 1080, 1920, "Two lines\nof text").unwrap();
        assert!(png.starts_with(b"\x89PNG"));
        let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
        assert_eq!((width, height), (360, 640));
    }

    #[test]
    fn rejects_out_of_range_styles() {
        for style in [
            CaptionStyle {
                size: 5,
                ..Default::default()
            },
            CaptionStyle {
                outline: 50,
                ..Default::default()
            },
            CaptionStyle {
                color: "red".into(),
                ..Default::default()
            },
            CaptionStyle {
                margin: Some(60),
                ..Default::default()
            },
        ] {
            assert!(!style.valid(), "{style:?}");
        }
        assert!(CaptionStyle::default().valid());
    }
}

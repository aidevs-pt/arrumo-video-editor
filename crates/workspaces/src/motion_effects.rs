//! Light, depth and type effects for animations: the atmosphere behind the layers, soft light and
//! spark layers, cards seen in perspective, emphasised words and numbers that count up. Each effect
//! is drawn from the spec's own colours, so it serves any brand rather than one look.

use crate::Launch;
use std::{io, path::Path, process::Stdio};

use serde::{Deserialize, Serialize};

/// Light behind an animation's layers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Atmosphere {
    #[default]
    None,
    /// Soft beams falling from a source above the frame, fading before the lower third.
    Rays,
    /// Large soft pools of colour drifting slowly.
    Aura,
}

impl Atmosphere {
    pub(crate) fn is_none(&self) -> bool {
        *self == Self::None
    }
}

/// How a box or image card sits in depth.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Tilt {
    #[default]
    None,
    /// The left side recedes.
    Left,
    /// The right side recedes.
    Right,
    /// The top leans away, like a screen lying back.
    Back,
}

impl Tilt {
    pub(crate) fn is_none(&self) -> bool {
        *self == Self::None
    }

    /// The `perspective` filter that turns a still into this depth, or nothing when flat.
    pub(crate) fn filter(self) -> Option<String> {
        let (a, s) = (0.09, 0.05);
        let corners = match self {
            Tilt::None => return None,
            Tilt::Left => [(s, a), (1.0, 0.0), (s, 1.0 - a), (1.0, 1.0)],
            Tilt::Right => [(0.0, 0.0), (1.0 - s, a), (0.0, 1.0), (1.0 - s, 1.0 - a)],
            Tilt::Back => [(a, a * 1.4), (1.0 - a, a * 1.4), (0.0, 1.0), (1.0, 1.0)],
        };
        let points: Vec<String> = corners
            .iter()
            .enumerate()
            .map(|(index, (x, y))| format!("x{index}=W*{x:.3}:y{index}=H*{y:.3}"))
            .collect();
        Some(format!(
            "perspective={}:sense=destination:interpolation=cubic",
            points.join(":")
        ))
    }
}

/// Text with `*marked*` words: the text without the marks, and whether each of its words (split
/// on whitespace) is emphasised. Unpaired marks are kept as written.
pub(crate) fn parse_emphasis(text: &str) -> (String, Vec<bool>) {
    let marks = text.matches('*').count();
    if marks < 2 || !marks.is_multiple_of(2) {
        let words = text.split_whitespace().count();
        return (text.to_owned(), vec![false; words]);
    }
    let mut plain = String::with_capacity(text.len());
    let mut flags = Vec::new();
    let (mut inside, mut in_word, mut word_marked) = (false, false, false);
    for ch in text.chars() {
        if ch == '*' {
            inside = !inside;
            if in_word {
                word_marked |= inside;
            }
            continue;
        }
        if ch.is_whitespace() {
            if in_word {
                flags.push(word_marked);
                in_word = false;
            }
        } else if !in_word {
            in_word = true;
            word_marked = inside;
        } else {
            word_marked |= inside;
        }
        plain.push(ch);
    }
    if in_word {
        flags.push(word_marked);
    }
    (plain, flags)
}

/// The first number in a line, so it can count up to its value as the line arrives.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CountUp {
    prefix: String,
    suffix: String,
    value: f64,
    decimals: usize,
    group: Option<char>,
    point: char,
}

impl CountUp {
    pub(crate) fn parse(line: &str) -> Option<Self> {
        let pattern = regex::Regex::new(r"\d(?:[\d.,]*\d)?").ok()?;
        let found = pattern.find(line)?;
        let number = found.as_str();
        let separators: Vec<(usize, char)> = number
            .char_indices()
            .filter(|(_, ch)| *ch == '.' || *ch == ',')
            .collect();
        let (mut group, mut point, mut decimals) = (None, '.', 0);
        if let Some(&(at, last)) = separators.last() {
            let after = number.len() - at - 1;
            let repeated = separators.iter().filter(|(_, ch)| *ch == last).count();
            if after == 3 && (repeated > 1 || separators.len() == 1) {
                group = Some(last);
                point = if last == '.' { ',' } else { '.' };
            } else {
                point = last;
                decimals = after;
                group = separators.iter().map(|(_, ch)| *ch).find(|ch| *ch != last);
            }
        }
        let digits: String = number
            .chars()
            .filter(|ch| Some(*ch) != group)
            .map(|ch| if ch == point { '.' } else { ch })
            .collect();
        let value: f64 = digits.parse().ok()?;
        (value > 0.0 && value.is_finite()).then(|| Self {
            prefix: line[..found.start()].to_owned(),
            suffix: line[found.end()..].to_owned(),
            value,
            decimals,
            group,
            point,
        })
    }

    /// The line showing `share` (0 to 1) of the way to the number, written as the number was.
    pub(crate) fn at(&self, share: f64) -> String {
        let scale = 10f64.powi(self.decimals as i32);
        let value = (self.value * share.clamp(0.0, 1.0) * scale).round() / scale;
        let whole = value.trunc() as u64;
        let mut integer = whole.to_string();
        if let Some(group) = self.group {
            let digits: Vec<char> = integer.chars().collect();
            integer = digits
                .iter()
                .enumerate()
                .flat_map(|(index, ch)| {
                    let from_end = digits.len() - index;
                    let mark = (index > 0 && from_end.is_multiple_of(3)).then_some(group);
                    mark.into_iter().chain(std::iter::once(*ch))
                })
                .collect();
        }
        let mut number = integer;
        if self.decimals > 0 {
            let fraction = ((value - whole as f64) * scale).round() as u64;
            number.push(self.point);
            number.push_str(&format!("{fraction:0width$}", width = self.decimals));
        }
        format!("{}{number}{}", self.prefix, self.suffix)
    }
}

fn channels(color: &str) -> io::Result<[f64; 3]> {
    let digits = color
        .strip_prefix('#')
        .filter(|digits| digits.len() == 6)
        .ok_or_else(|| io::Error::other("Bad colour"))?;
    let value = u32::from_str_radix(digits, 16).map_err(io::Error::other)?;
    Ok([
        f64::from((value >> 16) as u8),
        f64::from((value >> 8) as u8),
        f64::from(value as u8),
    ])
}

/// How light a colour looks, from 0 (black) to 1 (white).
pub(crate) fn lightness(color: &str) -> f64 {
    channels(color).map_or(0.0, |[r, g, b]| {
        (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.0
    })
}

/// Draws one RGBA still of `width` by `height` from per-pixel expressions into `directory/file`,
/// at `detail` of its size and scaled up, since soft light needs few pixels.
fn draw_still(
    directory: &Path,
    file: &str,
    (width, height): (u32, u32),
    detail: f64,
    [r, g, b, a]: [String; 4],
    soften: f64,
) -> io::Result<()> {
    let (low_w, low_h) = (
        ((f64::from(width) * detail).round() as u32).max(8),
        ((f64::from(height) * detail).round() as u32).max(8),
    );
    let blur = if soften > 0.0 {
        format!(",gblur=sigma={soften:.2}")
    } else {
        String::new()
    };
    let status = crate::quiet_command(crate::media_tools::ffmpeg())
        .args(["-nostdin", "-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg(format!("color=c=black:s={low_w}x{low_h}:d=0.1"))
        .arg("-vf")
        .arg(format!(
            "format=rgba,geq=r='{r}':g='{g}':b='{b}':a='{a}'{blur},scale={width}:{height}:flags=bicubic"
        ))
        .args(["-frames:v", "1", "-update", "1"])
        .arg(file)
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .launch_status()?;
    if status.success() && directory.join(file).is_file() {
        Ok(())
    } else {
        Err(io::Error::other("Could not draw an animation light"))
    }
}

/// Mixes two colours.
fn mix(a: [f64; 3], b: [f64; 3], share: f64) -> [f64; 3] {
    [
        a[0] + (b[0] - a[0]) * share,
        a[1] + (b[1] - a[1]) * share,
        a[2] + (b[2] - a[2]) * share,
    ]
}

/// Colour expressions that blend `main` (weight `first`) and `second` (weight `other`), with the
/// alpha their weights add up to.
fn blend(main: [f64; 3], first: &str, second: [f64; 3], other: &str, strength: f64) -> [String; 4] {
    let total = format!("({first}+{other}+0.0001)");
    let channel = |index: usize| {
        format!(
            "({:.1}*{first}+{:.1}*{other})/{total}",
            main[index], second[index]
        )
    };
    [
        channel(0),
        channel(1),
        channel(2),
        format!("255*clip({strength:.3}*({first}+{other}),0,1)"),
    ]
}

/// The atmosphere still for a `width` by `height` frame: a little larger than the frame, so it can
/// drift without showing its edges. Light on a dark background glows; on a light one it tints.
pub(crate) fn atmosphere_png(
    directory: &Path,
    file: &str,
    style: Atmosphere,
    (width, height): (u32, u32),
    color: &str,
    second: Option<&str>,
    background: &str,
) -> io::Result<(u32, u32)> {
    let size = (
        (f64::from(width) * OVERSCAN).round() as u32,
        (f64::from(height) * OVERSCAN).round() as u32,
    );
    let light = channels(color)?;
    let dark = lightness(background) < 0.45;
    let detail = 1.0 / 3.0;
    let (w, h) = (f64::from(size.0) * detail, f64::from(size.1) * detail);
    let expressions = match style {
        Atmosphere::None => return Ok((0, 0)),
        Atmosphere::Rays => {
            let (sx, sy) = (0.6 * w, -0.1 * h);
            let angle = format!("atan2(X-{sx:.1},Y-{sy:.1})");
            let distance = format!("hypot(X-{sx:.1},Y-{sy:.1})");
            let beams = format!(
                "(0.55*pow(0.5+0.5*cos({angle}*11+0.7),5)+0.45*pow(0.5+0.5*cos({angle}*7-1.3),7)\
                 +0.3*pow(0.5+0.5*cos({angle}*17+2.1),9))"
            );
            // The beams fade with distance and before the lower third, so text there stays calm.
            let calm = format!("(1-0.9*clip((Y-{:.1})/{:.1},0,1))", h * 0.55, h * 0.3);
            let reach = format!("exp(-pow({distance}/{:.1},2))", h * 0.8);
            let core = format!("exp(-pow({distance}/{:.1},2))", h * 0.4);
            let hot = mix(light, [255.0, 248.0, 236.0], 0.6);
            blend(
                light,
                &format!("({beams}*{reach}*{calm})"),
                hot,
                &format!("(0.6*{core}*{calm})"),
                if dark { 0.85 } else { 0.4 },
            )
        }
        Atmosphere::Aura => {
            let other = match second {
                Some(color) => channels(color)?,
                None => mix(light, [255.0, 255.0, 255.0], 0.45),
            };
            let pool = |x: f64, y: f64, r: f64| {
                format!(
                    "exp(-pow(hypot(X-{:.1},Y-{:.1})/{:.1},2))",
                    x * w,
                    y * h,
                    r * w
                )
            };
            blend(
                light,
                &format!("({}+0.8*{})", pool(0.3, 0.3, 0.55), pool(0.45, 0.88, 0.5)),
                other,
                &pool(0.82, 0.55, 0.5),
                if dark { 0.55 } else { 0.7 },
            )
        }
    };
    draw_still(directory, file, size, detail, expressions, 2.0)?;
    Ok(size)
}

/// How much larger than the frame the atmosphere is drawn.
pub(crate) const OVERSCAN: f64 = 1.08;

/// The overlay placement that lets the atmosphere drift slowly around the frame.
pub(crate) fn atmosphere_drift((width, height): (u32, u32)) -> String {
    let (w, h) = (f64::from(width), f64::from(height));
    let (spare_x, spare_y) = (w * (OVERSCAN - 1.0) / 2.0, h * (OVERSCAN - 1.0) / 2.0);
    format!(
        "x='-{spare_x:.1}+{:.1}*sin(2*PI*t/11)':y='-{spare_y:.1}+{:.1}*sin(2*PI*t/17+1)'",
        spare_x * 0.8,
        spare_y * 0.6
    )
}

/// A soft pool of light filling a `width` by `height` ellipse.
pub(crate) fn light_png(
    directory: &Path,
    file: &str,
    (width, height): (u32, u32),
    color: &str,
    opacity: f64,
) -> io::Result<()> {
    let light = channels(color)?;
    let falloff = "exp(-3.2*(pow((X-W/2)/(W/2),2)+pow((Y-H/2)/(H/2),2)))";
    draw_still(
        directory,
        file,
        (width.max(8), height.max(8)),
        0.25,
        [
            format!("{:.1}", light[0]),
            format!("{:.1}", light[1]),
            format!("{:.1}", light[2]),
            format!("255*{opacity:.3}*{falloff}"),
        ],
        0.0,
    )
}

/// A four-point spark: a white-hot cross that fades along its arms, in a halo of `color`.
pub(crate) fn spark_png(directory: &Path, file: &str, size: u32, color: &str) -> io::Result<()> {
    let light = channels(color)?;
    let core = mix(light, [255.0, 250.0, 242.0], 0.8);
    let arms = "(exp(-abs(X-W/2)/(W*0.016))*exp(-pow((Y-H/2)/(H*0.45),2))\
                +exp(-abs(Y-H/2)/(H*0.016))*exp(-pow((X-W/2)/(W*0.45),2)))";
    let halo = "exp(-pow(hypot(X-W/2,Y-H/2)/(W*0.2),2))";
    draw_still(
        directory,
        file,
        (size.max(16), size.max(16)),
        1.0,
        blend(
            core,
            &format!("1.3*{arms}"),
            light,
            &format!("0.7*{halo}"),
            1.0,
        ),
        0.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marked_words_are_emphasised_and_the_marks_removed() {
        let (plain, flags) = parse_emphasis("Small habits *compound* fast");
        assert_eq!(plain, "Small habits compound fast");
        assert_eq!(flags, [false, false, true, false]);
        let (plain, flags) = parse_emphasis("Let *time work*.");
        assert_eq!(plain, "Let time work.");
        assert_eq!(flags, [false, true, true]);
        let (plain, flags) = parse_emphasis("2 * 3 is six");
        assert_eq!(plain, "2 * 3 is six");
        assert_eq!(flags, [false; 5]);
    }

    #[test]
    fn numbers_count_up_in_the_way_they_were_written() {
        let money = CountUp::parse("$1,250 saved").unwrap();
        assert_eq!(money.at(1.0), "$1,250 saved");
        assert_eq!(money.at(0.5), "$625 saved");
        assert_eq!(money.at(0.0), "$0 saved");
        let euros = CountUp::parse("12.500,50 €").unwrap();
        assert_eq!(euros.at(1.0), "12.500,50 €");
        assert_eq!(euros.at(0.1), "1.250,05 €");
        let share = CountUp::parse("Up 3.5%").unwrap();
        assert_eq!(share.at(1.0), "Up 3.5%");
        assert_eq!(share.at(0.5), "Up 1.8%");
        assert_eq!(CountUp::parse("37x").unwrap().at(0.5), "19x");
        assert!(CountUp::parse("No number").is_none());
        assert!(CountUp::parse("0 left").is_none());
    }

    #[test]
    fn tilts_warp_cards_into_perspective() {
        assert!(Tilt::None.filter().is_none());
        for tilt in [Tilt::Left, Tilt::Right, Tilt::Back] {
            assert!(tilt.filter().unwrap().starts_with("perspective="));
        }
    }

    #[test]
    fn light_follows_the_background() {
        assert!(lightness("#0A0605") < 0.1);
        assert!(lightness("#F4F1EA") > 0.9);
    }
}

//! Transitions between storyboard scenes.
//!
//! A transition never changes the video's length or when a scene starts: the outgoing scene holds
//! its last frame and the incoming scene its first for half the transition each, and the two
//! padded scenes overlap by the transition's length. Narration, subtitles and render checks keep
//! the timings of the plain join.

use serde::{Deserialize, Serialize};
use std::fmt::Write;

/// How one scene hands over to the next.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Transition {
    /// A straight cut.
    #[default]
    None,
    /// A cross-dissolve.
    Fade,
    /// The next scene pushes the current one off to the left.
    Push,
    /// The current scene zooms in and gives way to the next.
    Zoom,
    /// A fast horizontal blur between the scenes.
    Blur,
    /// Both scenes are faces of a cube that turns to the left.
    Cube,
    /// The current scene turns away like a card and the next turns in.
    Flip,
}

pub(crate) const TRANSITIONS: [&str; 7] = ["none", "fade", "push", "zoom", "blur", "cube", "flip"];

/// Frames per second scenes are rendered at.
const FPS: f64 = 30.0;
/// How far the camera sits from the screen in 3D transitions, in frame widths.
const CAMERA: f64 = 2.2;
/// The colour behind scenes as they turn in 3D.
const BACKDROP: &str = "0x0A0A0A";

impl Transition {
    pub(crate) fn is_none(&self) -> bool {
        *self == Self::None
    }

    fn seconds(self) -> f64 {
        match self {
            Self::None => 0.0,
            Self::Fade | Self::Push | Self::Blur => 0.5,
            Self::Zoom => 0.6,
            Self::Flip => 0.7,
            Self::Cube => 0.8,
        }
    }

    fn xfade(self) -> Option<&'static str> {
        match self {
            Self::Fade => Some("fade"),
            Self::Push => Some("slideleft"),
            Self::Zoom => Some("zoomin"),
            Self::Blur => Some("hblur"),
            Self::None | Self::Cube | Self::Flip => None,
        }
    }
}

/// The length of each join's transition in seconds: whole even frames, at most half of either
/// scene so a transition never eats a short scene.
pub(crate) fn join_seconds(lengths: &[f64], transitions: &[Transition]) -> Vec<f64> {
    lengths
        .windows(2)
        .zip(transitions)
        .map(|(pair, transition)| {
            let longest = transition.seconds().min(pair[0] / 2.0).min(pair[1] / 2.0);
            let frames = ((longest * FPS / 2.0).floor() * 2.0).max(0.0);
            frames / FPS
        })
        .collect()
}

/// An FFmpeg filter graph that joins video inputs `0..lengths.len()` (each `lengths[i]` seconds
/// long, `width` by `height`) into `[out]`, with `transitions[i]` between scene `i` and the next.
pub(crate) fn join_graph(
    lengths: &[f64],
    transitions: &[Transition],
    width: u32,
    height: u32,
) -> String {
    let joins = join_seconds(lengths, transitions);
    let mut graph = String::new();
    for (index, length) in lengths.iter().enumerate() {
        let before = if index == 0 {
            0.0
        } else {
            joins[index - 1] / 2.0
        };
        let after = joins.get(index).map_or(0.0, |seconds| seconds / 2.0);
        write!(
            graph,
            "[{index}:v]settb=1/{FPS},fps={FPS},setpts=PTS-STARTPTS,format=yuv420p,setsar=1,\
             trim=duration={length:.4},setpts=PTS-STARTPTS,\
             tpad=start_mode=clone:start_duration={before:.4}:stop_mode=clone:stop_duration={after:.4}[p{index}];"
        )
        .ok();
    }
    let mut current = "p0".to_owned();
    let mut elapsed =
        lengths.first().copied().unwrap_or(0.0) + joins.first().map_or(0.0, |s| s / 2.0);
    for (index, &seconds) in joins.iter().enumerate() {
        let next = format!("p{}", index + 1);
        let label = format!("j{index}");
        let offset = elapsed - seconds;
        let transition = transitions[index];
        match transition.xfade() {
            _ if seconds <= 0.0 => {
                write!(
                    graph,
                    "[{current}][{next}]concat=n=2:v=1:a=0,settb=1/{FPS}[{label}];"
                )
                .ok();
            }
            Some(kind) => {
                write!(
                    graph,
                    "[{current}][{next}]xfade=transition={kind}:duration={seconds:.4}:offset={offset:.4}[{label}];"
                )
                .ok();
            }
            None => turn(
                &mut graph, &current, &next, &label, transition, offset, seconds, width, height,
            ),
        }
        let following = lengths[index + 1]
            + joins.get(index + 1).map_or(0.0, |s| s / 2.0)
            + if seconds > 0.0 { seconds / 2.0 } else { 0.0 };
        elapsed += following - if seconds > 0.0 { seconds } else { 0.0 };
        current = label;
    }
    write!(graph, "[{current}]format=yuv420p[out]").ok();
    graph
}

/// Appends a 3D turn from `[current]` into `[next]` at `offset` seconds of `[current]`.
#[allow(clippy::too_many_arguments)]
fn turn(
    graph: &mut String,
    current: &str,
    next: &str,
    label: &str,
    transition: Transition,
    offset: f64,
    seconds: f64,
    width: u32,
    height: u32,
) {
    let frames = (seconds * FPS).round().max(2.0);
    let progress = format!("(0.5-0.5*cos(PI*min(in/{:.0}\\,1)))", frames - 1.0);
    let (outgoing, incoming) = match transition {
        Transition::Flip => (
            flip_corners(&format!("min(2*{progress}\\,1)"), false),
            flip_corners(&format!("max(2*{progress}-1\\,0)"), true),
        ),
        _ => (
            cube_corners(&progress, false),
            cube_corners(&progress, true),
        ),
    };
    let face = |corners: &str| {
        format!(
            "format=rgba,pad=iw+2:ih+2:1:1:color=black@0,\
             perspective={corners}:sense=destination:eval=frame,crop=iw-2:ih-2:1:1"
        )
    };
    write!(
        graph,
        "[{current}]split[{label}a][{label}b];\
         [{label}a]trim=duration={offset:.4},setpts=PTS-STARTPTS[{label}h];\
         [{label}b]trim=start={offset:.4}:duration={seconds:.4},setpts=PTS-STARTPTS,{out}[{label}o];\
         [{next}]split[{label}c][{label}d];\
         [{label}c]trim=duration={seconds:.4},setpts=PTS-STARTPTS,{inc}[{label}i];\
         [{label}d]trim=start={seconds:.4},setpts=PTS-STARTPTS[{label}r];\
         color=c={BACKDROP}:s={width}x{height}:r={FPS}:d={seconds:.4},format=rgba[{label}k];\
         [{label}k][{label}o]overlay=shortest=1[{label}m];\
         [{label}m][{label}i]overlay=shortest=1,format=yuv420p,setsar=1[{label}t];\
         [{label}h][{label}t][{label}r]concat=n=3:v=1:a=0,settb=1/{FPS}[{label}];",
        out = face(&outgoing),
        inc = face(&incoming),
    )
    .ok();
}

/// `perspective` corner options for one face of a cube turning left by `progress` (0 to 1) of a
/// quarter turn: the front face, or the right-hand face that turns to the front.
fn cube_corners(progress: &str, incoming: bool) -> String {
    let (cos, sin) = (
        format!("cos(PI/2*{progress})"),
        format!("sin(PI/2*{progress})"),
    );
    corners(|u| {
        if incoming {
            (
                format!("(W/2*{cos}+{u}*{sin})"),
                format!("(-W/2*{sin}+{u}*{cos})"),
            )
        } else {
            (
                format!("({u}*{cos}-W/2*{sin})"),
                format!("(-{u}*{sin}-W/2*{cos})"),
            )
        }
    })
}

/// `perspective` corner options for a card turning about its vertical centre line: the outgoing
/// card turns away by `progress` of a quarter turn, the incoming one turns in from the other side.
fn flip_corners(progress: &str, incoming: bool) -> String {
    let angle = if incoming {
        format!("(PI/2*(1-{progress}))")
    } else {
        format!("(-PI/2*{progress})")
    };
    let (cos, sin) = (format!("cos({angle})"), format!("sin({angle})"));
    corners(|u| (format!("({u}*{cos})"), format!("({u}*{sin})")))
}

/// Projects a face whose points at horizontal position `u` (from -W/2 to W/2) sit at `(x, z)`
/// relative to the screen centre, z pointing away from the viewer.
fn corners(place: impl Fn(&str) -> (String, String)) -> String {
    let mut options = Vec::with_capacity(4);
    for (index, (u, v)) in [(-1, -1), (1, -1), (-1, 1), (1, 1)].into_iter().enumerate() {
        let (x, z) = place(&format!("({u}*W/2)"));
        let scale = format!("({CAMERA}*W/({CAMERA}*W+{z}))");
        options.push(format!(
            "x{index}='W/2+{x}*{scale}':y{index}='H/2+({v}*H/2)*{scale}'"
        ));
    }
    options.join(":")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn transitions_fit_inside_short_scenes_in_whole_frames() {
        let joins = join_seconds(
            &[4.0, 1.0, 4.0, 4.0],
            &[Transition::Cube, Transition::Fade, Transition::None],
        );
        assert!((joins[0] - 0.4667).abs() < 0.001, "{joins:?}");
        assert!((joins[1] - 0.4667).abs() < 0.001, "{joins:?}");
        assert_eq!(joins[2], 0.0);
    }

    #[test]
    fn joins_with_transitions_keep_the_total_length() {
        if Command::new(crate::media_tools::ffmpeg())
            .arg("-version")
            .output()
            .is_err()
        {
            return;
        }
        let directory = tempfile::tempdir().unwrap();
        let lengths = [1.5, 2.0, 1.2, 1.4, 1.0];
        let mut args: Vec<String> = ["-nostdin", "-v", "error"].map(String::from).to_vec();
        for (index, (length, colour)) in lengths
            .iter()
            .zip(["red", "blue", "green", "white", "yellow"])
            .enumerate()
        {
            let name = format!("{index}.mp4");
            let made = Command::new(crate::media_tools::ffmpeg())
                .args(["-nostdin", "-v", "error", "-f", "lavfi", "-i"])
                .arg(format!("color=c={colour}:s=160x90:r=30:d={length}"))
                .args(["-pix_fmt", "yuv420p", "-y"])
                .arg(directory.path().join(&name))
                .status()
                .unwrap();
            assert!(made.success());
            args.extend([
                "-i".to_owned(),
                directory.path().join(name).to_string_lossy().into(),
            ]);
        }
        let transitions = [
            Transition::Cube,
            Transition::Flip,
            Transition::None,
            Transition::Push,
        ];
        let graph = join_graph(&lengths, &transitions, 160, 90);
        let output = directory.path().join("joined.mp4");
        args.extend([
            "-filter_complex".into(),
            graph,
            "-map".into(),
            "[out]".into(),
        ]);
        args.extend(["-c:v", "libx264", "-y"].map(String::from));
        args.push(output.to_string_lossy().into());
        let status = Command::new(crate::media_tools::ffmpeg())
            .args(&args)
            .status()
            .unwrap();
        assert!(status.success());
        let seconds = crate::source_transcripts::media_duration(&output).unwrap();
        let total: f64 = lengths.iter().sum();
        assert!((seconds - total).abs() < 0.07, "{seconds} vs {total}");
    }
}

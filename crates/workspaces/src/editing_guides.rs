//! Arrumo's editing craft notes. Each production stage's agent reads its guide inside the prompt,
//! kept apart from the creator's brief, and outside agents read them through the shared tools.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct EditingGuide {
    pub id: &'static str,
    pub title: &'static str,
    /// The production step whose agent reads it.
    pub stage: &'static str,
    #[serde(skip)]
    pub body: &'static str,
}

macro_rules! guide {
    ($id:literal, $title:literal, $stage:literal) => {
        EditingGuide {
            id: $id,
            title: $title,
            stage: $stage,
            body: include_str!(concat!("../guides/", $id, ".md")),
        }
    };
}

pub const EDITING_GUIDES: [EditingGuide; 18] = [
    guide!("script", "Writing the script", "script"),
    guide!("visuals", "Planning supporting visuals", "assets"),
    guide!("animation", "Designing animations", "assets"),
    guide!("direction", "Directing the look and motion", "assets"),
    guide!("cutting", "Cutting recorded footage", "preview"),
    guide!(
        "storyboard",
        "Storyboarding a video without footage",
        "preview"
    ),
    guide!("sound", "Sound, music and voice", "preview"),
    // Platform facts, searched 2026-11-25. Official: three-minute Shorts (YouTube Help,
    // "Understand three-minute YouTube Shorts"); end screens in the last 5 to 20 seconds of
    // videos of 25 seconds or more (YouTube Help, "Add end screens to videos"). Reported, so
    // the guide words them as tendencies: Reels over three minutes not recommended to
    // non-followers (Buffer, reviewed 2026-11); vertical feed overlays at the bottom and right
    // edge (TikTok Ads safe-zone help, creator templates); feeds autoplaying muted (vendor guides).
    guide!("platforms", "Platforms and formats", "preview"),
    guide!("portrait", "Choosing portrait cuts", "delivery"),
    guide!(
        "publishing",
        "Writing publishing text and thumbnails",
        "delivery"
    ),
    guide!("review", "Reviewing an edit before showing it", "review"),
    guide!("explainer", "Explainers and tutorials", "kind"),
    guide!("data-story", "Data stories and recaps", "kind"),
    guide!("promo", "Product, launch and ad videos", "kind"),
    guide!("short-vertical", "Short vertical videos", "kind"),
    guide!(
        "talking-head",
        "Talking heads, interviews and podcasts",
        "kind"
    ),
    guide!("titles", "Titles, logos and type in motion", "kind"),
    guide!(
        "animated-short",
        "Animated shorts, stories and moods",
        "kind"
    ),
];

/// The kinds of video the script stage chooses from, each with the shape it usually takes. Each
/// has the guide of the same ID, which the later stages read.
pub(crate) const VIDEO_KINDS: [(&str, &str); 7] = [
    (
        "explainer",
        "the viewer's problem, what it costs, the answer in one line, how it works in one to three steps, the result and one next step",
    ),
    (
        "data-story",
        "the question, the context on screen, the data building one series at a time, the moment that matters, the takeaway held still",
    ),
    (
        "promo",
        "a hook within two seconds, the problem or wish, the product as the answer, one proof, one call to action held at the end",
    ),
    (
        "short-vertical",
        "the payoff promised in the first second, beats that raise the stakes, the payoff, an ending that loops or points onward",
    ),
    (
        "talking-head",
        "the speaker opens, graphics carry figures and steps between their personal lines, the speaker closes",
    ),
    (
        "titles",
        "one idea set in type or a mark, revealed in reading order, landing on a held final frame",
    ),
    (
        "animated-short",
        "a title beat, a middle where something changes because of what came before, an ending that lands and holds",
    ),
];

/// The kinds a script result names, without unknown values or repeats. Results written before
/// kinds existed name none.
pub(crate) fn video_kinds(script: Option<&serde_json::Value>) -> Vec<&'static str> {
    let mut kinds = Vec::new();
    for value in script
        .and_then(|result| result.get("kinds"))
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some((id, _)) = VIDEO_KINDS
            .iter()
            .find(|(id, _)| Some(*id) == value.as_str())
            && !kinds.contains(id)
        {
            kinds.push(*id);
        }
    }
    kinds
}

/// The guides of a stage followed by those of the video's kinds.
pub(crate) fn guide_section_for(ids: &[&str], kinds: &[&'static str]) -> String {
    let all: Vec<&str> = ids.iter().copied().chain(kinds.iter().copied()).collect();
    guide_section(&all)
}

pub fn editing_guide(id: &str) -> Option<&'static EditingGuide> {
    EDITING_GUIDES.iter().find(|guide| guide.id == id)
}

/// Where the video goes, for a prompt's brief: the platform the creator named and the size of
/// each format, so the platforms guide applies to them.
pub(crate) fn delivery_brief(snapshot: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "platform": snapshot["options"]["platform"],
        "formats": snapshot["formats"].as_array().map(|formats| formats.iter().map(|entry| serde_json::json!({
            "name": entry["format"]["name"],
            "width": entry["format"]["width"],
            "height": entry["format"]["height"],
        })).collect::<Vec<_>>()),
    })
}

/// The guides for a prompt, as one section after the stage's instructions.
pub(crate) fn guide_section(ids: &[&str]) -> String {
    let bodies: Vec<&str> = ids
        .iter()
        .filter_map(|id| editing_guide(id))
        .map(|guide| guide.body.trim())
        .collect();
    format!(
        "EDITING GUIDE (Arrumo's craft notes; the creator's brief and feedback take precedence):\n{}\n\n",
        bodies.join("\n\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_guide_has_a_body_and_a_unique_id() {
        for guide in EDITING_GUIDES {
            assert!(guide.body.starts_with("# "), "{}", guide.id);
            assert_eq!(
                EDITING_GUIDES
                    .iter()
                    .filter(|other| other.id == guide.id)
                    .count(),
                1
            );
        }
        assert!(guide_section(&["cutting"]).contains("Cut in pauses"));
        assert!(editing_guide("missing").is_none());
    }

    #[test]
    fn every_kind_has_a_guide_and_scripts_name_only_known_kinds() {
        for (id, shape) in VIDEO_KINDS {
            assert_eq!(editing_guide(id).map(|guide| guide.stage), Some("kind"));
            assert!(!shape.is_empty());
        }
        let script =
            serde_json::json!({"kinds": ["promo", "general", "promo", "unknown", "titles"]});
        assert_eq!(video_kinds(Some(&script)), ["promo", "titles"]);
        assert!(video_kinds(Some(&serde_json::json!({}))).is_empty());
        assert!(video_kinds(None).is_empty());
        let section = guide_section_for(&["animation"], &["promo"]);
        assert!(
            section.contains("# Designing animations") && section.contains("# Product, launch")
        );
    }
}

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

pub const EDITING_GUIDES: [EditingGuide; 10] = [
    guide!("script", "Writing the script", "script"),
    guide!("visuals", "Planning supporting visuals", "assets"),
    guide!("animation", "Designing animations", "assets"),
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
];

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
}

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

pub const EDITING_GUIDES: [EditingGuide; 8] = [
    guide!("script", "Writing the script", "script"),
    guide!("visuals", "Planning supporting visuals", "assets"),
    guide!("animation", "Designing animations", "assets"),
    guide!("cutting", "Cutting recorded footage", "preview"),
    guide!(
        "storyboard",
        "Storyboarding a video without footage",
        "preview"
    ),
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

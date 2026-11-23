use std::collections::BTreeMap;

use rusqlite::params;
use serde::Serialize;

use crate::format_deliveries::read_settings;
use crate::generation_queue::read_run;
use crate::{
    CompletedVideoFeedback, GenerationActivity, GenerationPlan, GenerationRun, ProductionProgress,
    VerifiedDelivery, WorkspaceStore,
};

#[derive(Debug, Serialize)]
pub struct CompletedVideo {
    pub run: GenerationRun,
    pub title: String,
    pub delivery: VerifiedDelivery,
    pub supplemental_text: Vec<crate::supplemental_delivery::SupplementalText>,
    pub supplemental_media: Vec<crate::supplemental_delivery::SupplementalMedia>,
    pub output_dates: BTreeMap<i64, String>,
    pub plan: GenerationPlan,
    pub production: ProductionProgress,
    pub activity: Vec<GenerationActivity>,
    pub feedback: Vec<CompletedVideoFeedback>,
}

impl WorkspaceStore {
    pub fn completed_video(
        &self,
        workspace_id: i64,
        run_id: i64,
    ) -> rusqlite::Result<CompletedVideo> {
        let (run, title, plan) = self.0.query_row(
            "SELECT id, draft_id, state, snapshot_json, pending_review_id,
                    COALESCE(display_title, json_extract(snapshot_json, '$.draft.title')), plan_json, display_title
             FROM generation_runs WHERE workspace_id = ?1 AND id = ?2 AND state = 'completed'",
            params![workspace_id, run_id],
            |row| Ok((read_run(row)?, row.get(5)?, read_settings(row.get(6)?, 6)?)),
        )?;
        Ok(CompletedVideo {
            run,
            title,
            delivery: self.verified_delivery(workspace_id, run_id)?,
            supplemental_text: self.completed_supplemental_text(workspace_id, run_id)?,
            supplemental_media: self.completed_supplemental_media(workspace_id, run_id)?,
            output_dates: self.run_output_dates(workspace_id, run_id)?,
            plan,
            production: self.production_progress(workspace_id, run_id)?,
            activity: self.generation_activity(workspace_id, run_id)?,
            feedback: self.completed_video_feedback(workspace_id, run_id)?,
        })
    }

    /// A still that represents a finished video: its first generated thumbnail, or a frame of its
    /// first delivered video, kept next to the delivery so later views reuse it.
    pub fn completed_poster(
        &self,
        database: &std::path::Path,
        workspace_id: i64,
        run_id: i64,
    ) -> Result<(std::path::PathBuf, &'static str), Box<dyn std::error::Error>> {
        let video = self.completed_video(workspace_id, run_id)?;
        let thumbnail = video
            .supplemental_media
            .iter()
            .filter(|media| media.kind == "image")
            .find_map(|media| {
                let file = media.managed_file(database, workspace_id).ok()?;
                let mut head = [0u8; 16];
                std::io::Read::read_exact(&mut std::fs::File::open(&file).ok()?, &mut head).ok()?;
                Some((file, crate::image_type(&head)?))
            });
        if let Some(thumbnail) = thumbnail {
            return Ok(thumbnail);
        }
        let output = video
            .delivery
            .verified_outputs
            .first()
            .ok_or_else(|| std::io::Error::other("The video has no delivered output"))?;
        let source = crate::delivery_verification::managed_delivery_file(
            database,
            workspace_id,
            run_id,
            &output.video,
        )?;
        let directory = source
            .parent()
            .ok_or_else(|| std::io::Error::other("The delivery has no directory"))?;
        let poster = directory.join("poster.jpg");
        if !poster.is_file() {
            let pending = directory.join("poster-pending.jpg");
            let seconds = (output.duration_seconds * 0.2).min(3.0);
            let made = std::process::Command::new("ffmpeg")
                .args([
                    "-nostdin",
                    "-v",
                    "error",
                    "-ss",
                    &format!("{seconds:.2}"),
                    "-i",
                ])
                .arg(&source)
                .args(["-frames:v", "1", "-vf", "scale=480:-2", "-y"])
                .arg(&pending)
                .status()?;
            if !made.success() {
                return Err(std::io::Error::other("Could not take a frame of the video").into());
            }
            std::fs::rename(&pending, &poster)?;
        }
        Ok((poster, "image/jpeg"))
    }

    pub fn rename_completed_video(
        &self,
        workspace_id: i64,
        run_id: i64,
        title: &str,
    ) -> rusqlite::Result<String> {
        let title = title.trim();
        if title.is_empty() || title.contains('\0') {
            return Err(rusqlite::Error::InvalidQuery);
        }
        self.0.query_row(
            "UPDATE generation_runs SET display_title = ?3
             WHERE workspace_id = ?1 AND id = ?2 AND state = 'completed'
             RETURNING display_title",
            params![workspace_id, run_id, title],
            |row| row.get(0),
        )
    }
}

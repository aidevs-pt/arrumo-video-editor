use std::thread;

use rusqlite::OptionalExtension;
use serde::Serialize;

use crate::WorkspaceStore;

pub(crate) const LEGACY_PREVIEW_MAX_EDGE: u32 = 480;

#[derive(Debug, Serialize)]
pub struct PreviewSettings {
    pub override_max_edge: Option<u32>,
    pub recommended_max_edge: u32,
    pub effective_max_edge: u32,
}

impl WorkspaceStore {
    pub fn preview_settings(&self) -> rusqlite::Result<PreviewSettings> {
        let override_max_edge = self
            .0
            .query_row(
                "SELECT max_edge FROM preview_settings WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        let recommended_max_edge = match thread::available_parallelism().map(|count| count.get()) {
            Ok(1..=2) => 360,
            Ok(8..) => 720,
            _ => LEGACY_PREVIEW_MAX_EDGE,
        };
        Ok(PreviewSettings {
            override_max_edge,
            recommended_max_edge,
            effective_max_edge: override_max_edge.unwrap_or(recommended_max_edge),
        })
    }

    pub fn set_preview_max_edge(&self, max_edge: Option<u32>) -> rusqlite::Result<()> {
        self.0.execute(
            "INSERT INTO preview_settings (id, max_edge) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET max_edge = excluded.max_edge",
            [max_edge],
        )?;
        Ok(())
    }
}

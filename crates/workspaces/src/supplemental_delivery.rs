use crate::IoStep;
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{WorkspaceStore, format_deliveries::read_settings};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SupplementalTextKind {
    Title,
    Description,
    Tags,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SupplementalText {
    pub kind: SupplementalTextKind,
    pub format_id: Option<i64>,
    pub text: String,
}

impl SupplementalText {
    pub(crate) fn filename(&self, index: usize) -> String {
        let kind = match self.kind {
            SupplementalTextKind::Title => "title",
            SupplementalTextKind::Description => "description",
            SupplementalTextKind::Tags => "tags",
        };
        format!("supplemental-{kind}-{}.txt", index + 1)
    }
}

pub(crate) fn validate_supplemental_text(
    connection: &rusqlite::Connection,
    run_id: i64,
    value: &Value,
) -> rusqlite::Result<()> {
    let formats = run_format_ids(connection, run_id)?;
    let entries: Vec<SupplementalText> =
        serde_json::from_value(value.clone()).map_err(|_| rusqlite::Error::InvalidQuery)?;
    if entries.len() > 100
        || entries.iter().map(|entry| entry.text.len()).sum::<usize>() > 64 * 1024
        || entries.iter().any(|entry| {
            entry.text.trim().is_empty()
                || entry.text.contains('\0')
                || entry.format_id.is_some_and(|id| !formats.contains(&id))
        })
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(())
}

fn run_format_ids(
    connection: &rusqlite::Connection,
    run_id: i64,
) -> rusqlite::Result<std::collections::HashSet<i64>> {
    let mut formats = connection.prepare(
        "SELECT json_extract(format.value, '$.format.id')
         FROM generation_runs run, json_each(run.snapshot_json, '$.formats') format
         WHERE run.id = ?1",
    )?;
    formats
        .query_map([run_id], |row| row.get::<_, i64>(0))?
        .collect()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MediaReference {
    asset_id: i64,
    format_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct SupplementalMedia {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment_id: Option<i64>,
    pub asset_id: i64,
    pub format_id: Option<i64>,
    pub kind: String,
    pub name: String,
    pub byte_size: i64,
}

impl SupplementalMedia {
    pub fn managed_file(
        &self,
        database: &std::path::Path,
        workspace_id: i64,
    ) -> std::io::Result<std::path::PathBuf> {
        let file = crate::asset_path(
            &crate::asset_root(&database.canonicalize().io_step("Resolving a path")?),
            workspace_id,
            self.asset_id,
        );
        let metadata = file.symlink_metadata().io_step("Checking a file")?;
        if !metadata.is_file()
            || !crate::assets::is_canonical(&file)?
            || Some(metadata.len()) != u64::try_from(self.byte_size).ok()
        {
            return Err(std::io::Error::other("Supplemental media is unavailable"));
        }
        Ok(file)
    }

    pub(crate) fn filename(&self, index: usize) -> String {
        let extension = std::path::Path::new(&self.name)
            .extension()
            .and_then(|value| value.to_str())
            .filter(|value| {
                !value.is_empty()
                    && value.len() <= 8
                    && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
            })
            .unwrap_or("bin");
        let extension = extension.to_ascii_lowercase();
        match self.attachment_id {
            Some(id) => format!("attachment-{id}.{extension}"),
            None => format!("supplemental-media-{}.{extension}", index + 1),
        }
    }
}

pub(crate) fn validate_supplemental_media(
    connection: &rusqlite::Connection,
    workspace_id: i64,
    run_id: i64,
    value: &Value,
) -> rusqlite::Result<()> {
    let entries: Vec<MediaReference> =
        serde_json::from_value(value.clone()).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let formats = run_format_ids(connection, run_id)?;
    if entries.len() > 100 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let mut exists = connection.prepare(
        "SELECT EXISTS (SELECT 1 FROM workspace_assets
         WHERE workspace_id = ?1 AND id = ?2 AND superseded = 0 AND byte_size > 0)",
    )?;
    for entry in entries {
        if entry.format_id.is_some_and(|id| !formats.contains(&id))
            || !exists.query_row(params![workspace_id, entry.asset_id], |row| {
                row.get::<_, bool>(0)
            })?
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
    }
    Ok(())
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct PendingPayload {
    supplemental_text: Vec<SupplementalText>,
    supplemental_media: Vec<MediaReference>,
}

pub struct PendingSupplemental {
    pub text: Vec<SupplementalText>,
    pub media: Vec<SupplementalMedia>,
}

impl WorkspaceStore {
    pub(crate) fn completed_supplemental_text(
        &self,
        workspace_id: i64,
        run_id: i64,
    ) -> rusqlite::Result<Vec<SupplementalText>> {
        self.completed_supplemental_field(workspace_id, run_id, "$.supplemental_text")
    }

    pub(crate) fn completed_supplemental_media(
        &self,
        workspace_id: i64,
        run_id: i64,
    ) -> rusqlite::Result<Vec<SupplementalMedia>> {
        let references: Vec<MediaReference> =
            self.completed_supplemental_field(workspace_id, run_id, "$.supplemental_media")?;
        let mut attached = self.0.prepare(
            "SELECT media.id, media.asset_id, media.format_id, asset.kind, asset.name, asset.byte_size FROM completed_media AS media
             JOIN workspace_assets AS asset ON asset.id = media.asset_id AND asset.workspace_id = ?1
             JOIN generation_runs AS run ON run.id = media.run_id
             WHERE run.workspace_id = ?1 AND run.id = ?2 AND run.state = 'completed'
             ORDER BY media.id",
        )?;
        let entries = attached.query_map(params![workspace_id, run_id], |row| {
            Ok(SupplementalMedia {
                attachment_id: Some(row.get(0)?),
                asset_id: row.get(1)?,
                format_id: row.get(2)?,
                kind: row.get(3)?,
                name: row.get(4)?,
                byte_size: row.get(5)?,
            })
        })?;
        let mut media = self.supplemental_media_metadata(workspace_id, references)?;
        for entry in entries {
            media.push(entry?);
        }
        Ok(media)
    }

    pub fn attach_completed_media(
        &self,
        database: &std::path::Path,
        workspace_id: i64,
        run_id: i64,
        format_id: i64,
        asset_id: i64,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let transaction = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        let delivery = self.verified_delivery(workspace_id, run_id)?;
        if !delivery
            .verified_outputs
            .iter()
            .any(|output| output.format_id == format_id)
        {
            return Err(rusqlite::Error::QueryReturnedNoRows.into());
        }
        let exists: bool = transaction.query_row(
            "SELECT EXISTS (SELECT 1 FROM completed_media WHERE run_id = ?1 AND format_id = ?2 AND asset_id = ?3)",
            params![run_id, format_id, asset_id], |row| row.get(0),
        )?;
        if exists {
            return Ok(false);
        }
        let asset = self
            .supplemental_media_metadata(
                workspace_id,
                vec![MediaReference {
                    asset_id,
                    format_id: Some(format_id),
                }],
            )?
            .pop()
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        asset.managed_file(database, workspace_id)?;
        let added = transaction.execute(
            "INSERT INTO completed_media (run_id, format_id, asset_id)
             SELECT ?1, ?2, id FROM workspace_assets
             WHERE id = ?3 AND workspace_id = ?4 AND superseded = 0 AND byte_size > 0
               AND (SELECT count(*) FROM completed_media WHERE run_id = ?1) < 100",
            params![run_id, format_id, asset_id, workspace_id],
        )?;
        if added == 0 {
            return Err(rusqlite::Error::InvalidQuery.into());
        }
        transaction.commit()?;
        Ok(true)
    }

    pub fn detach_completed_media(
        &self,
        workspace_id: i64,
        run_id: i64,
        attachment_id: i64,
    ) -> rusqlite::Result<bool> {
        Ok(self.0.execute(
            "DELETE FROM completed_media WHERE id = ?3 AND run_id = ?2
             AND EXISTS (SELECT 1 FROM generation_runs
                         WHERE id = ?2 AND workspace_id = ?1 AND state = 'completed')",
            params![workspace_id, run_id, attachment_id],
        )? != 0)
    }

    pub fn pending_supplemental(
        &self,
        workspace_id: i64,
        run_id: i64,
        result_id: i64,
    ) -> rusqlite::Result<PendingSupplemental> {
        let result: PendingPayload = self.0.query_row(
            "SELECT result.result_json FROM generation_runs run
             JOIN production_results result ON result.run_id = run.id AND result.id = run.pending_review_id
             WHERE run.workspace_id = ?1 AND run.id = ?2 AND result.id = ?3 AND run.state = 'paused'
               AND json_extract(run.plan_json, '$.steps[' || result.stage_index || '].stage') = 'supplemental_outputs'",
            params![workspace_id, run_id, result_id],
            |row| read_settings(row.get(0)?, 0),
        )?;
        Ok(PendingSupplemental {
            text: result.supplemental_text,
            media: self.supplemental_media_metadata(workspace_id, result.supplemental_media)?,
        })
    }

    fn supplemental_media_metadata(
        &self,
        workspace_id: i64,
        references: Vec<MediaReference>,
    ) -> rusqlite::Result<Vec<SupplementalMedia>> {
        let mut metadata = self.0.prepare(
            "SELECT kind, name, byte_size FROM workspace_assets WHERE workspace_id = ?1 AND id = ?2",
        )?;
        references
            .into_iter()
            .map(|entry| {
                metadata.query_row(params![workspace_id, entry.asset_id], |row| {
                    Ok(SupplementalMedia {
                        attachment_id: None,
                        asset_id: entry.asset_id,
                        format_id: entry.format_id,
                        kind: row.get(0)?,
                        name: row.get(1)?,
                        byte_size: row.get(2)?,
                    })
                })
            })
            .collect()
    }

    pub fn completed_supplemental_media_file(
        &self,
        database: &std::path::Path,
        workspace_id: i64,
        run_id: i64,
        index: usize,
    ) -> Result<(SupplementalMedia, std::path::PathBuf), Box<dyn std::error::Error>> {
        let entry = self
            .completed_supplemental_media(workspace_id, run_id)?
            .into_iter()
            .nth(index)
            .filter(|entry| entry.attachment_id.is_none())
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        let file = entry.managed_file(database, workspace_id)?;
        Ok((entry, file))
    }

    pub fn completed_attachment_file(
        &self,
        database: &std::path::Path,
        workspace_id: i64,
        run_id: i64,
        attachment_id: i64,
    ) -> Result<(SupplementalMedia, std::path::PathBuf), Box<dyn std::error::Error>> {
        let entry = self
            .completed_supplemental_media(workspace_id, run_id)?
            .into_iter()
            .find(|entry| entry.attachment_id == Some(attachment_id))
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        let file = entry.managed_file(database, workspace_id)?;
        Ok((entry, file))
    }

    fn completed_supplemental_field<T: serde::de::DeserializeOwned>(
        &self,
        workspace_id: i64,
        run_id: i64,
        path: &str,
    ) -> rusqlite::Result<Vec<T>> {
        self.0.query_row(
            "SELECT json_extract(result.result_json, ?3)
             FROM production_results result JOIN generation_runs run ON run.id = result.run_id
             WHERE run.workspace_id = ?1 AND run.id = ?2 AND run.state = 'completed'
               AND json_extract(run.plan_json, '$.steps[' || result.stage_index || '].stage') = 'supplemental_outputs'
               AND (result.decision = 'approved' OR result.decision IS NULL)
             ORDER BY result.id DESC LIMIT 1",
            params![workspace_id, run_id, path],
            |row| read_settings(row.get(0)?, 0),
        ).optional().map(Option::unwrap_or_default)
    }
}

use rusqlite::{Row, params};
use serde::{Deserialize, Serialize};

use crate::WorkspaceStore;

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct OutputFormatSettings {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub enabled: bool,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct OutputFormat {
    pub id: i64,
    pub workspace_id: i64,
    #[serde(flatten)]
    pub settings: OutputFormatSettings,
}

impl WorkspaceStore {
    pub fn list_output_formats(&self, workspace_id: i64) -> rusqlite::Result<Vec<OutputFormat>> {
        self.0
            .prepare(
                "SELECT id, workspace_id, name, width, height, enabled FROM output_formats
                 WHERE workspace_id = ?1 ORDER BY id",
            )?
            .query_map([workspace_id], read_format)?
            .collect()
    }

    pub fn create_output_format(
        &self,
        workspace_id: i64,
        settings: &OutputFormatSettings,
    ) -> rusqlite::Result<OutputFormat> {
        self.0.query_row(
            "INSERT INTO output_formats (workspace_id, name, width, height, enabled)
             VALUES (?1, ?2, ?3, ?4, ?5) RETURNING id, workspace_id, name, width, height, enabled",
            params![
                workspace_id,
                settings.name.trim(),
                settings.width,
                settings.height,
                settings.enabled,
            ],
            read_format,
        )
    }

    pub fn update_output_format(
        &self,
        workspace_id: i64,
        format_id: i64,
        settings: &OutputFormatSettings,
    ) -> rusqlite::Result<OutputFormat> {
        self.0.query_row(
            "UPDATE output_formats SET name = ?3, width = ?4, height = ?5, enabled = ?6
             WHERE workspace_id = ?1 AND id = ?2
             RETURNING id, workspace_id, name, width, height, enabled",
            params![
                workspace_id,
                format_id,
                settings.name.trim(),
                settings.width,
                settings.height,
                settings.enabled,
            ],
            read_format,
        )
    }

    pub fn remove_output_format(
        &self,
        workspace_id: i64,
        format_id: i64,
    ) -> rusqlite::Result<bool> {
        self.0
            .execute(
                "DELETE FROM output_formats WHERE workspace_id = ?1 AND id = ?2",
                params![workspace_id, format_id],
            )
            .map(|deleted| deleted != 0)
    }
}

fn read_format(row: &Row<'_>) -> rusqlite::Result<OutputFormat> {
    Ok(OutputFormat {
        id: row.get(0)?,
        workspace_id: row.get(1)?,
        settings: OutputFormatSettings {
            name: row.get(2)?,
            width: row.get(3)?,
            height: row.get(4)?,
            enabled: row.get(5)?,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manages_formats_with_workspace_isolation() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let first = store.create("First").unwrap().id;
        let second = store.create("Second").unwrap().id;
        let defaults = store.list_output_formats(first).unwrap();
        assert_eq!(defaults.len(), 2);
        assert_eq!(
            (defaults[0].settings.width, defaults[0].settings.height),
            (1920, 1080)
        );
        assert_eq!(
            (defaults[1].settings.width, defaults[1].settings.height),
            (1080, 1920)
        );
        assert!(defaults.iter().all(|format| format.settings.enabled));

        let mut settings = OutputFormatSettings {
            name: " Square ".into(),
            width: 1080,
            height: 1080,
            enabled: true,
        };
        let square = store.create_output_format(first, &settings).unwrap();
        assert_eq!(square.settings.name, "Square");
        assert!(store.create_output_format(first, &settings).is_err());
        settings.enabled = false;
        assert!(
            store
                .update_output_format(second, square.id, &settings)
                .is_err()
        );
        assert!(!store.remove_output_format(second, square.id).unwrap());
        assert!(
            !store
                .update_output_format(first, square.id, &settings)
                .unwrap()
                .settings
                .enabled
        );
        assert!(store.remove_output_format(first, square.id).unwrap());
        assert_eq!(store.list_output_formats(first).unwrap(), defaults);

        settings.width = 0;
        assert!(store.create_output_format(first, &settings).is_err());
    }
}

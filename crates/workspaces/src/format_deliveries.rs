use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::WorkspaceStore;

#[derive(Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct SupplementalOutputs {
    pub thumbnails: u16,
    pub titles: u16,
    pub descriptions: u16,
    pub tags: bool,
    pub thumbnail_guidance: String,
    pub specifications: String,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RequiredOutput {
    FinalVideo,
    Subtitles,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct FormatDelivery {
    pub required: [RequiredOutput; 2],
    pub supplemental: SupplementalOutputs,
}

impl WorkspaceStore {
    pub fn format_delivery(
        &self,
        workspace_id: i64,
        format_id: i64,
    ) -> rusqlite::Result<FormatDelivery> {
        let json: Option<String> = self.0.query_row(
            "SELECT delivery.settings_json FROM output_formats AS format
             LEFT JOIN format_deliveries AS delivery ON delivery.format_id = format.id
             WHERE format.workspace_id = ?1 AND format.id = ?2",
            params![workspace_id, format_id],
            |row| row.get(0),
        )?;
        let supplemental = json
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?
            .unwrap_or_default();
        Ok(FormatDelivery {
            required: [RequiredOutput::FinalVideo, RequiredOutput::Subtitles],
            supplemental,
        })
    }

    pub fn set_format_delivery(
        &self,
        workspace_id: i64,
        format_id: i64,
        settings: &SupplementalOutputs,
    ) -> rusqlite::Result<()> {
        let json = serde_json::to_string(settings)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        self.0.query_row(
            "INSERT INTO format_deliveries (format_id, settings_json)
             SELECT id, ?3 FROM output_formats WHERE workspace_id = ?1 AND id = ?2
             ON CONFLICT (format_id) DO UPDATE SET settings_json = excluded.settings_json
             RETURNING format_id",
            params![workspace_id, format_id, json],
            |_| Ok(()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supplemental_settings_are_scoped_and_required_artifacts_are_fixed() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let studio = store.create("Studio").unwrap().id;
        let other = store.create("Other").unwrap().id;
        let format = store.list_output_formats(studio).unwrap()[0].id;
        let initial = store.format_delivery(studio, format).unwrap();
        assert_eq!(initial.supplemental, SupplementalOutputs::default());

        let settings = SupplementalOutputs {
            thumbnails: 3,
            titles: 3,
            tags: true,
            thumbnail_guidance: "## Show the product".into(),
            ..Default::default()
        };
        assert!(store.format_delivery(other, format).is_err());
        assert!(store.set_format_delivery(other, format, &settings).is_err());
        store
            .set_format_delivery(studio, format, &settings)
            .unwrap();
        let delivery = store.format_delivery(studio, format).unwrap();
        assert_eq!(delivery.supplemental, settings);
        assert_eq!(
            delivery.required,
            [RequiredOutput::FinalVideo, RequiredOutput::Subtitles]
        );
        assert!(serde_json::from_str::<SupplementalOutputs>(r#"{"required":[]}"#).is_err());

        store.remove_output_format(studio, format).unwrap();
        assert!(store.format_delivery(studio, format).is_err());
        let delivery_rows: i64 = store
            .0
            .query_row("SELECT count(*) FROM format_deliveries", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(delivery_rows, 0);
    }
}

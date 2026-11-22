use rusqlite::params;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::WorkspaceStore;

#[derive(Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct SupplementalOutputs {
    pub thumbnails: u16,
    pub titles: u16,
    pub descriptions: u16,
    pub tags: bool,
    pub thumbnail_guidance: String,
    pub thumbnail_reference_ids: Vec<i64>,
    pub specifications: String,
    pub portrait_derivatives: PortraitDerivatives,
    /// Also deliver a copy of the video with the subtitles drawn into the picture.
    pub burned_captions: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PortraitDerivatives {
    pub count: u16,
    pub source: PortraitSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PortraitSource {
    Manual {
        #[serde(default)]
        guidance: String,
        #[serde(default)]
        thumbnails_per_video: u16,
        #[serde(default)]
        thumbnail_guidance: String,
        #[serde(default)]
        thumbnail_reference_ids: Vec<i64>,
    },
    Profile {
        profile_id: i64,
    },
}

impl Default for PortraitSource {
    fn default() -> Self {
        Self::Manual {
            guidance: String::new(),
            thumbnails_per_video: 0,
            thumbnail_guidance: String::new(),
            thumbnail_reference_ids: Vec::new(),
        }
    }
}

impl PortraitDerivatives {
    pub(crate) fn source_profile(&self) -> Option<i64> {
        match self.source {
            PortraitSource::Profile { profile_id } => Some(profile_id),
            PortraitSource::Manual { .. } => None,
        }
    }
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
        Ok(assemble_delivery(read_settings(json, 0)?))
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
             AND (?4 IS NULL OR EXISTS (
                 SELECT 1 FROM profiles WHERE workspace_id = ?1 AND id = ?4
             ))
             AND NOT EXISTS (
                 SELECT 1 FROM (
                     SELECT value FROM json_each(?3, '$.thumbnail_reference_ids')
                     UNION ALL
                     SELECT value FROM json_each(?3, '$.portrait_derivatives.source.thumbnail_reference_ids')
                 ) AS ref
                 WHERE ref.value IS NOT NULL AND NOT EXISTS (
                     SELECT 1 FROM workspace_assets AS image
                     WHERE image.id = ref.value AND image.workspace_id = ?1 AND image.kind = 'image' AND image.superseded = 0
                   )
             )
             ON CONFLICT (format_id) DO UPDATE SET settings_json = excluded.settings_json
             RETURNING format_id",
            params![
                workspace_id,
                format_id,
                json,
                settings.portrait_derivatives.source_profile()
            ],
            |_| Ok(()),
        )
    }
}

pub(crate) fn read_settings<T: Default + DeserializeOwned>(
    json: Option<String>,
    column: usize,
) -> rusqlite::Result<T> {
    json.as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map(|settings| settings.unwrap_or_default())
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                column,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
}

pub(crate) fn assemble_delivery(supplemental: SupplementalOutputs) -> FormatDelivery {
    FormatDelivery {
        required: [RequiredOutput::FinalVideo, RequiredOutput::Subtitles],
        supplemental,
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

    #[test]
    fn references_require_workspace_images_and_can_be_cleared_by_profiles() {
        use crate::DeliveryOverrides;

        let store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let other = store.create("Other").unwrap().id;
        let profile = store
            .create_profile(workspace, "Reviews", "", None)
            .unwrap()
            .id;
        let format = store.list_output_formats(workspace).unwrap()[0].id;
        let create_asset = |workspace_id, kind| {
            store
                .0
                .query_row(
                    "INSERT INTO workspace_assets (workspace_id, name, kind, byte_size)
                 VALUES (?1, 'Test asset', ?2, 10) RETURNING id",
                    params![workspace_id, kind],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap()
        };
        let image = create_asset(workspace, "image");
        let audio = create_asset(workspace, "audio");
        let foreign = create_asset(other, "image");
        for bad in [audio, foreign, i64::MAX] {
            let settings = SupplementalOutputs {
                thumbnail_reference_ids: vec![bad],
                ..Default::default()
            };
            assert!(
                store
                    .set_format_delivery(workspace, format, &settings)
                    .is_err()
            );
            let overrides = DeliveryOverrides {
                portrait_derivatives: Some(PortraitDerivatives {
                    count: 1,
                    source: PortraitSource::Manual {
                        guidance: String::new(),
                        thumbnails_per_video: 1,
                        thumbnail_guidance: String::new(),
                        thumbnail_reference_ids: vec![bad],
                    },
                }),
                ..Default::default()
            };
            assert!(
                store
                    .set_profile_delivery(workspace, profile, format, &overrides)
                    .is_err()
            );
        }
        let settings = SupplementalOutputs {
            thumbnail_reference_ids: vec![image],
            ..Default::default()
        };
        store
            .set_format_delivery(workspace, format, &settings)
            .unwrap();
        assert_eq!(
            store
                .profile_delivery(workspace, profile, format)
                .unwrap()
                .effective
                .supplemental
                .thumbnail_reference_ids,
            vec![image]
        );
        store
            .set_profile_delivery(
                workspace,
                profile,
                format,
                &DeliveryOverrides {
                    thumbnail_reference_ids: Some(vec![]),
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(
            store
                .profile_delivery(workspace, profile, format)
                .unwrap()
                .effective
                .supplemental
                .thumbnail_reference_ids
                .is_empty()
        );
    }
}

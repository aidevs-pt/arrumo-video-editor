use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::format_deliveries::{assemble_delivery, read_settings};
use crate::{FormatDelivery, PortraitDerivatives, SupplementalOutputs, WorkspaceStore};

#[derive(Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct DeliveryOverrides {
    pub thumbnails: Option<u16>,
    pub titles: Option<u16>,
    pub descriptions: Option<u16>,
    pub tags: Option<bool>,
    pub thumbnail_guidance: Option<String>,
    pub thumbnail_reference_ids: Option<Vec<i64>>,
    pub specifications: Option<String>,
    pub portrait_derivatives: Option<PortraitDerivatives>,
    pub burned_captions: Option<bool>,
    pub caption_style: Option<crate::CaptionStyleOverrides>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ProfileDelivery {
    pub workspace: SupplementalOutputs,
    pub overrides: DeliveryOverrides,
    pub effective: FormatDelivery,
}

impl DeliveryOverrides {
    fn resolve(&self, workspace: &SupplementalOutputs) -> SupplementalOutputs {
        SupplementalOutputs {
            thumbnails: self.thumbnails.unwrap_or(workspace.thumbnails),
            titles: self.titles.unwrap_or(workspace.titles),
            descriptions: self.descriptions.unwrap_or(workspace.descriptions),
            tags: self.tags.unwrap_or(workspace.tags),
            thumbnail_guidance: self
                .thumbnail_guidance
                .as_ref()
                .unwrap_or(&workspace.thumbnail_guidance)
                .clone(),
            thumbnail_reference_ids: self
                .thumbnail_reference_ids
                .as_ref()
                .unwrap_or(&workspace.thumbnail_reference_ids)
                .clone(),
            specifications: self
                .specifications
                .as_ref()
                .unwrap_or(&workspace.specifications)
                .clone(),
            portrait_derivatives: self
                .portrait_derivatives
                .as_ref()
                .unwrap_or(&workspace.portrait_derivatives)
                .clone(),
            burned_captions: self.burned_captions.unwrap_or(workspace.burned_captions),
            caption_style: self.caption_style.as_ref().map_or_else(
                || workspace.caption_style.clone(),
                |style| style.apply(&workspace.caption_style),
            ),
        }
    }
}

impl WorkspaceStore {
    pub fn profile_delivery(
        &self,
        workspace_id: i64,
        profile_id: i64,
        format_id: i64,
    ) -> rusqlite::Result<ProfileDelivery> {
        self.0.query_row(
            "SELECT base.settings_json, custom.settings_json
             FROM profiles AS profile
             JOIN output_formats AS format ON format.workspace_id = profile.workspace_id
             LEFT JOIN format_deliveries AS base ON base.format_id = format.id
             LEFT JOIN profile_deliveries AS custom
               ON custom.workspace_id = profile.workspace_id
              AND custom.profile_id = profile.id AND custom.format_id = format.id
             WHERE profile.workspace_id = ?1 AND profile.id = ?2 AND format.id = ?3
               AND format.enabled = 1",
            params![workspace_id, profile_id, format_id],
            |row| {
                let workspace = read_settings(row.get(0)?, 0)?;
                let overrides: DeliveryOverrides = read_settings(row.get(1)?, 1)?;
                let effective = assemble_delivery(overrides.resolve(&workspace));
                Ok(ProfileDelivery {
                    workspace,
                    overrides,
                    effective,
                })
            },
        )
    }

    pub fn set_profile_delivery(
        &self,
        workspace_id: i64,
        profile_id: i64,
        format_id: i64,
        overrides: &DeliveryOverrides,
    ) -> rusqlite::Result<()> {
        if overrides
            .caption_style
            .as_ref()
            .is_some_and(|style| !style.apply(&crate::CaptionStyle::default()).valid())
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let settings = serde_json::to_string(overrides)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        self.0.query_row(
            "INSERT INTO profile_deliveries (workspace_id, profile_id, format_id, settings_json)
             SELECT profile.workspace_id, profile.id, format.id, ?4
             FROM profiles AS profile
             JOIN output_formats AS format ON format.workspace_id = profile.workspace_id
             WHERE profile.workspace_id = ?1 AND profile.id = ?2 AND format.id = ?3
               AND format.enabled = 1
               AND (?5 IS NULL OR EXISTS (
                   SELECT 1 FROM profiles WHERE workspace_id = ?1 AND id = ?5
               ))
             AND NOT EXISTS (
                 SELECT 1 FROM (
                     SELECT value FROM json_each(?4, '$.thumbnail_reference_ids')
                     UNION ALL
                     SELECT value FROM json_each(?4, '$.portrait_derivatives.source.thumbnail_reference_ids')
                 ) AS ref
                 WHERE ref.value IS NOT NULL AND NOT EXISTS (
                     SELECT 1 FROM workspace_assets AS image
                     WHERE image.id = ref.value AND image.workspace_id = ?1 AND image.kind = 'image' AND image.superseded = 0
                   )
             )
             ON CONFLICT (workspace_id, profile_id, format_id)
             DO UPDATE SET settings_json = excluded.settings_json
             RETURNING format_id",
            params![
                workspace_id,
                profile_id,
                format_id,
                settings,
                overrides
                    .portrait_derivatives
                    .as_ref()
                    .and_then(PortraitDerivatives::source_profile),
            ],
            |_| Ok(()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinguishes_inheritance_from_explicit_zero_false_and_empty() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let studio = store.create("Studio").unwrap().id;
        let profile = store
            .create_profile(studio, "Reviews", "", None)
            .unwrap()
            .id;
        let other = store.create("Other").unwrap().id;
        let format = store.list_output_formats(studio).unwrap().remove(0);
        let id = format.id;
        let mut base = SupplementalOutputs {
            thumbnails: 3,
            titles: 2,
            tags: true,
            specifications: "Workspace guidance".into(),
            ..Default::default()
        };
        store.set_format_delivery(studio, id, &base).unwrap();
        let overrides =
            serde_json::from_str(r#"{"thumbnails":0,"tags":false,"specifications":""}"#).unwrap();
        assert!(
            store
                .set_profile_delivery(other, profile, id, &overrides)
                .is_err()
        );
        assert!(store.profile_delivery(other, profile, id).is_err());
        store
            .set_profile_delivery(studio, profile, id, &overrides)
            .unwrap();
        base.titles = 4;
        store.set_format_delivery(studio, id, &base).unwrap();
        let resolved = store.profile_delivery(studio, profile, id).unwrap();
        assert_eq!(resolved.workspace, base);
        assert_eq!(resolved.overrides, overrides);
        assert_eq!(resolved.effective.supplemental.thumbnails, 0);
        assert_eq!(resolved.effective.supplemental.titles, 4);
        assert!(!resolved.effective.supplemental.tags);
        assert!(resolved.effective.supplemental.specifications.is_empty());
        assert_eq!(
            resolved.effective.required,
            store.format_delivery(studio, id).unwrap().required
        );
        store
            .set_profile_delivery(studio, profile, id, &DeliveryOverrides::default())
            .unwrap();
        assert_eq!(
            store
                .profile_delivery(studio, profile, id)
                .unwrap()
                .effective
                .supplemental,
            base
        );
        store.remove_output_format(studio, id).unwrap();
        assert!(store.profile_delivery(studio, profile, id).is_err());
        assert_eq!(
            store
                .0
                .query_row("SELECT count(*) FROM profile_deliveries", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn portrait_derivatives_require_a_source_profile_in_the_same_workspace() {
        use crate::PortraitSource;

        let store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let profile = store
            .create_profile(workspace, "Reviews", "", None)
            .unwrap()
            .id;
        let other = store.create("Other").unwrap().id;
        let foreign_profile = store.create_profile(other, "Foreign", "", None).unwrap().id;
        let format = store.list_output_formats(workspace).unwrap()[0].id;

        let mut settings = SupplementalOutputs {
            portrait_derivatives: PortraitDerivatives {
                count: 2,
                source: PortraitSource::Profile {
                    profile_id: foreign_profile,
                },
            },
            ..Default::default()
        };
        assert!(
            store
                .set_format_delivery(workspace, format, &settings)
                .is_err()
        );
        let invalid_override = DeliveryOverrides {
            portrait_derivatives: Some(settings.portrait_derivatives.clone()),
            ..Default::default()
        };
        assert!(
            store
                .set_profile_delivery(workspace, profile, format, &invalid_override)
                .is_err()
        );

        settings.portrait_derivatives.source = PortraitSource::Profile {
            profile_id: profile,
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
                .portrait_derivatives,
            settings.portrait_derivatives,
        );

        let manual: DeliveryOverrides = serde_json::from_str(r#"{
            "portrait_derivatives": {
                "count": 1,
                "source": {"type":"manual", "guidance":"Make a short demo", "thumbnails_per_video":2}
            }
        }"#).unwrap();
        store
            .set_profile_delivery(workspace, profile, format, &manual)
            .unwrap();
        assert_eq!(
            store
                .profile_delivery(workspace, profile, format)
                .unwrap()
                .effective
                .supplemental
                .portrait_derivatives,
            manual.portrait_derivatives.unwrap(),
        );
    }
}

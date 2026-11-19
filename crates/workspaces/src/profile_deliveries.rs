use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::format_deliveries::{assemble_delivery, read_settings};
use crate::{FormatDelivery, SupplementalOutputs, WorkspaceStore};

#[derive(Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct DeliveryOverrides {
    pub thumbnails: Option<u16>,
    pub titles: Option<u16>,
    pub descriptions: Option<u16>,
    pub tags: Option<bool>,
    pub thumbnail_guidance: Option<String>,
    pub specifications: Option<String>,
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
            specifications: self
                .specifications
                .as_ref()
                .unwrap_or(&workspace.specifications)
                .clone(),
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
        let settings = serde_json::to_string(overrides)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        self.0.query_row(
            "INSERT INTO profile_deliveries (workspace_id, profile_id, format_id, settings_json)
             SELECT profile.workspace_id, profile.id, format.id, ?4
             FROM profiles AS profile
             JOIN output_formats AS format ON format.workspace_id = profile.workspace_id
             WHERE profile.workspace_id = ?1 AND profile.id = ?2 AND format.id = ?3
               AND format.enabled = 1
             ON CONFLICT (workspace_id, profile_id, format_id)
             DO UPDATE SET settings_json = excluded.settings_json
             RETURNING format_id",
            params![workspace_id, profile_id, format_id, settings],
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
}

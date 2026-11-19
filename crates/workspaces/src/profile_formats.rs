use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::WorkspaceStore;

#[derive(Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DimensionOverrides {
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ProfileFormat {
    pub id: i64,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub overrides: DimensionOverrides,
    pub workspace_width: u32,
    pub workspace_height: u32,
}

impl WorkspaceStore {
    pub fn list_profile_formats(
        &self,
        workspace_id: i64,
        profile_id: i64,
    ) -> rusqlite::Result<Vec<ProfileFormat>> {
        self.0
            .prepare(
                "SELECT format.id, format.name,
                        COALESCE(custom.width, format.width),
                        COALESCE(custom.height, format.height),
                        custom.width, custom.height, format.width, format.height
                 FROM profiles AS profile
                 JOIN output_formats AS format ON format.workspace_id = profile.workspace_id
                 LEFT JOIN profile_format_overrides AS custom
                    ON custom.workspace_id = profile.workspace_id
                   AND custom.profile_id = profile.id
                   AND custom.format_id = format.id
                 WHERE profile.workspace_id = ?1 AND profile.id = ?2 AND format.enabled = 1
                 ORDER BY format.id",
            )?
            .query_map([workspace_id, profile_id], |row| {
                Ok(ProfileFormat {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    width: row.get(2)?,
                    height: row.get(3)?,
                    overrides: DimensionOverrides {
                        width: row.get(4)?,
                        height: row.get(5)?,
                    },
                    workspace_width: row.get(6)?,
                    workspace_height: row.get(7)?,
                })
            })?
            .collect()
    }

    pub fn set_profile_format_overrides(
        &self,
        workspace_id: i64,
        profile_id: i64,
        format_id: i64,
        overrides: &DimensionOverrides,
    ) -> rusqlite::Result<()> {
        let changed = self.0.execute(
            "INSERT INTO profile_format_overrides (workspace_id, profile_id, format_id, width, height)
             SELECT profile.workspace_id, profile.id, format.id, ?4, ?5
             FROM profiles AS profile
             JOIN output_formats AS format ON format.workspace_id = profile.workspace_id
             WHERE profile.workspace_id = ?1 AND profile.id = ?2 AND format.id = ?3
               AND format.enabled = 1
             ON CONFLICT (workspace_id, profile_id, format_id)
             DO UPDATE SET width = excluded.width, height = excluded.height",
            params![workspace_id, profile_id, format_id, overrides.width, overrides.height],
        )?;
        if changed == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OutputFormatSettings;

    #[test]
    fn inherits_each_dimension_and_tracks_workspace_enablement() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let profile = store
            .create_profile(workspace, "Reviews", "", None)
            .unwrap()
            .id;
        let other = store.create("Other studio").unwrap().id;
        let other_profile = store.create_profile(other, "Other", "", None).unwrap().id;

        let initial = store.list_profile_formats(workspace, profile).unwrap();
        assert_eq!(initial.len(), 2);
        let landscape = initial[0].id;
        let custom = DimensionOverrides {
            width: Some(1280),
            height: None,
        };
        assert!(
            store
                .set_profile_format_overrides(workspace, other_profile, landscape, &custom)
                .is_err()
        );
        let other_format = store.list_output_formats(other).unwrap()[0].id;
        assert!(
            store
                .set_profile_format_overrides(workspace, profile, other_format, &custom)
                .is_err()
        );
        store
            .set_profile_format_overrides(workspace, profile, landscape, &custom)
            .unwrap();

        let mut settings = OutputFormatSettings {
            name: "Landscape".into(),
            width: 2560,
            height: 1440,
            enabled: true,
        };
        store
            .update_output_format(workspace, landscape, &settings)
            .unwrap();
        let formats = store.list_profile_formats(workspace, profile).unwrap();
        assert_eq!((formats[0].width, formats[0].height), (1280, 1440));
        assert_eq!(formats[0].overrides, custom);
        assert_eq!(
            (formats[0].workspace_width, formats[0].workspace_height),
            (2560, 1440)
        );
        settings.enabled = false;
        store
            .update_output_format(workspace, landscape, &settings)
            .unwrap();
        assert_eq!(
            store
                .list_profile_formats(workspace, profile)
                .unwrap()
                .len(),
            1
        );
        assert!(
            store
                .set_profile_format_overrides(workspace, profile, landscape, &custom)
                .is_err()
        );
        settings.enabled = true;
        store
            .update_output_format(workspace, landscape, &settings)
            .unwrap();
        assert_eq!(
            store.list_profile_formats(workspace, profile).unwrap()[0].width,
            1280
        );

        store
            .set_profile_format_overrides(
                workspace,
                profile,
                landscape,
                &DimensionOverrides::default(),
            )
            .unwrap();
        assert_eq!(
            store.list_profile_formats(workspace, profile).unwrap()[0].width,
            2560
        );
        store.remove_output_format(workspace, landscape).unwrap();
        let recreated = store.create_output_format(workspace, &settings).unwrap();
        assert_ne!(landscape, recreated.id);
        assert_eq!(
            store.list_profile_formats(workspace, profile).unwrap()[1].overrides,
            DimensionOverrides::default()
        );
    }
}

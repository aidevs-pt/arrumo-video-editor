//! Profile-level organization: a short description, context that applies to one output format,
//! and workspace assets shared with a profile as a whole or with one of its formats.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use crate::{Profile, WorkspaceAsset, WorkspaceStore, assets::read_asset};

pub const MAX_PROFILE_DESCRIPTION: usize = 280;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ProfileAsset {
    #[serde(flatten)]
    pub asset: WorkspaceAsset,
    /// `None` when the asset is shared with every format of the profile.
    pub format_id: Option<i64>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct AssetProfileShare {
    pub profile_id: i64,
    pub format_id: Option<i64>,
}

impl WorkspaceStore {
    pub fn set_profile_description(
        &self,
        workspace_id: i64,
        profile_id: i64,
        description: &str,
    ) -> rusqlite::Result<Profile> {
        let description = description.trim();
        if description.chars().count() > MAX_PROFILE_DESCRIPTION || description.contains('\0') {
            return Err(rusqlite::Error::InvalidParameterName("description".into()));
        }
        self.0.query_row(
            "UPDATE profiles SET description = ?3 WHERE workspace_id = ?1 AND id = ?2
             RETURNING id, workspace_id, name, guidance, color, description",
            params![workspace_id, profile_id, description],
            Self::read_profile,
        )
    }

    /// Saves Markdown that applies only when the profile renders this format. Empty text clears it.
    pub fn set_profile_format_context(
        &self,
        workspace_id: i64,
        profile_id: i64,
        format_id: i64,
        markdown: &str,
    ) -> rusqlite::Result<()> {
        self.require_profile_format(workspace_id, profile_id, format_id)?;
        if markdown.trim().is_empty() {
            self.0.execute(
                "DELETE FROM profile_format_context
                 WHERE workspace_id = ?1 AND profile_id = ?2 AND format_id = ?3",
                params![workspace_id, profile_id, format_id],
            )?;
        } else {
            self.0.execute(
                "INSERT INTO profile_format_context (workspace_id, profile_id, format_id, markdown)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (workspace_id, profile_id, format_id) DO UPDATE SET markdown = excluded.markdown",
                params![workspace_id, profile_id, format_id, markdown],
            )?;
        }
        Ok(())
    }

    pub fn list_profile_assets(
        &self,
        workspace_id: i64,
        profile_id: i64,
    ) -> rusqlite::Result<Vec<ProfileAsset>> {
        self.require_profile(workspace_id, profile_id)?;
        self.0
            .prepare(
                "SELECT asset.id, asset.workspace_id, asset.name, asset.kind, asset.byte_size, asset.notes,
                        share.format_id
                 FROM asset_profile_shares AS share
                 JOIN workspace_assets AS asset
                   ON asset.workspace_id = share.workspace_id AND asset.id = share.asset_id
                 WHERE share.workspace_id = ?1 AND share.profile_id = ?2 AND asset.superseded = 0
                 ORDER BY asset.id, share.format_id IS NOT NULL, share.format_id",
            )?
            .query_map(params![workspace_id, profile_id], |row| {
                Ok(ProfileAsset {
                    asset: read_asset(row)?,
                    format_id: row.get(6)?,
                })
            })?
            .collect()
    }

    pub fn list_asset_profile_shares(
        &self,
        workspace_id: i64,
        asset_id: i64,
    ) -> rusqlite::Result<Vec<AssetProfileShare>> {
        self.0
            .prepare(
                "SELECT profile_id, format_id FROM asset_profile_shares
                 WHERE workspace_id = ?1 AND asset_id = ?2
                 ORDER BY profile_id, format_id IS NOT NULL, format_id",
            )?
            .query_map(params![workspace_id, asset_id], |row| {
                Ok(AssetProfileShare {
                    profile_id: row.get(0)?,
                    format_id: row.get(1)?,
                })
            })?
            .collect()
    }

    /// Shares a workspace asset with a profile. A profile-wide share replaces narrower format shares.
    pub fn share_asset_with_profile(
        &mut self,
        workspace_id: i64,
        asset_id: i64,
        profile_id: i64,
        format_id: Option<i64>,
    ) -> rusqlite::Result<()> {
        match format_id {
            Some(format_id) => self.require_profile_format(workspace_id, profile_id, format_id)?,
            None => self.require_profile(workspace_id, profile_id)?,
        }
        let tx = self.0.transaction()?;
        let live: Option<i64> = tx
            .query_row(
                "SELECT id FROM workspace_assets WHERE workspace_id = ?1 AND id = ?2 AND superseded = 0",
                params![workspace_id, asset_id],
                |row| row.get(0),
            )
            .optional()?;
        live.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        let covered: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM asset_profile_shares
             WHERE workspace_id = ?1 AND asset_id = ?2 AND profile_id = ?3 AND format_id IS NULL)",
            params![workspace_id, asset_id, profile_id],
            |row| row.get(0),
        )?;
        if format_id.is_none() {
            tx.execute(
                "DELETE FROM asset_profile_shares
                 WHERE workspace_id = ?1 AND asset_id = ?2 AND profile_id = ?3 AND format_id IS NOT NULL",
                params![workspace_id, asset_id, profile_id],
            )?;
        }
        if !covered {
            tx.execute(
                "INSERT OR IGNORE INTO asset_profile_shares (workspace_id, asset_id, profile_id, format_id)
                 VALUES (?1, ?2, ?3, ?4)",
                params![workspace_id, asset_id, profile_id, format_id],
            )?;
        }
        tx.commit()
    }

    /// Removes exactly one share scope. Returns whether a share existed.
    pub fn unshare_asset_from_profile(
        &self,
        workspace_id: i64,
        asset_id: i64,
        profile_id: i64,
        format_id: Option<i64>,
    ) -> rusqlite::Result<bool> {
        Ok(self.0.execute(
            "DELETE FROM asset_profile_shares
             WHERE workspace_id = ?1 AND asset_id = ?2 AND profile_id = ?3 AND format_id IS ?4",
            params![workspace_id, asset_id, profile_id, format_id],
        )? > 0)
    }

    fn require_profile(&self, workspace_id: i64, profile_id: i64) -> rusqlite::Result<()> {
        self.0.query_row(
            "SELECT 1 FROM profiles WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id, profile_id],
            |_| Ok(()),
        )
    }

    fn require_profile_format(
        &self,
        workspace_id: i64,
        profile_id: i64,
        format_id: i64,
    ) -> rusqlite::Result<()> {
        self.0.query_row(
            "SELECT 1 FROM profiles AS profile
             JOIN output_formats AS format ON format.workspace_id = profile.workspace_id
             WHERE profile.workspace_id = ?1 AND profile.id = ?2 AND format.id = ?3 AND format.enabled = 1",
            params![workspace_id, profile_id, format_id],
            |_| Ok(()),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::WorkspaceStore;

    fn fixture() -> (WorkspaceStore, i64, i64, i64, i64) {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let profile = store
            .create_profile(workspace, "Reviews", "", None)
            .unwrap()
            .id;
        let formats = store.list_output_formats(workspace).unwrap();
        let asset = store
            .0
            .query_row(
                "INSERT INTO workspace_assets (workspace_id, name, kind, byte_size, notes)
             VALUES (?1, 'intro.mp4', 'video', 1, '') RETURNING id",
                [workspace],
                |row| row.get(0),
            )
            .unwrap();
        (store, workspace, profile, formats[0].id, asset)
    }

    #[test]
    fn description_is_trimmed_bounded_and_scoped() {
        let (store, workspace, profile, _, _) = fixture();
        let other = store.create("Other").unwrap().id;
        let updated = store
            .set_profile_description(workspace, profile, "  Honest tests  ")
            .unwrap();
        assert_eq!(updated.description, "Honest tests");
        assert!(
            store
                .set_profile_description(workspace, profile, &"x".repeat(281))
                .is_err()
        );
        assert!(
            store
                .set_profile_description(other, profile, "Foreign")
                .is_err()
        );
        assert_eq!(
            store.list_profiles(workspace).unwrap()[0].description,
            "Honest tests"
        );
    }

    #[test]
    fn format_context_is_saved_cleared_and_scoped_to_enabled_formats() {
        let (store, workspace, profile, format, _) = fixture();
        store
            .set_profile_format_context(workspace, profile, format, "Chapters every 3 minutes")
            .unwrap();
        assert_eq!(
            store.list_profile_formats(workspace, profile).unwrap()[0].context,
            "Chapters every 3 minutes"
        );
        store
            .set_profile_format_context(workspace, profile, format, "  ")
            .unwrap();
        assert_eq!(
            store.list_profile_formats(workspace, profile).unwrap()[0].context,
            ""
        );
        let other = store.create("Other").unwrap().id;
        let foreign = store.list_output_formats(other).unwrap()[0].id;
        assert!(
            store
                .set_profile_format_context(workspace, profile, foreign, "No")
                .is_err()
        );
    }

    #[test]
    fn profile_wide_shares_replace_format_shares_and_unshare_is_exact() {
        let (mut store, workspace, profile, format, asset) = fixture();
        store
            .share_asset_with_profile(workspace, asset, profile, Some(format))
            .unwrap();
        store
            .share_asset_with_profile(workspace, asset, profile, Some(format))
            .unwrap();
        let shared = store.list_profile_assets(workspace, profile).unwrap();
        assert_eq!(shared.len(), 1);
        assert_eq!(shared[0].format_id, Some(format));

        store
            .share_asset_with_profile(workspace, asset, profile, None)
            .unwrap();
        let shares = store.list_asset_profile_shares(workspace, asset).unwrap();
        assert_eq!(shares.len(), 1);
        assert_eq!(shares[0].format_id, None);

        store
            .share_asset_with_profile(workspace, asset, profile, Some(format))
            .unwrap();
        assert_eq!(
            store
                .list_asset_profile_shares(workspace, asset)
                .unwrap()
                .len(),
            1
        );
        assert!(
            !store
                .unshare_asset_from_profile(workspace, asset, profile, Some(format))
                .unwrap()
        );
        assert!(
            store
                .unshare_asset_from_profile(workspace, asset, profile, None)
                .unwrap()
        );
        assert!(
            store
                .list_profile_assets(workspace, profile)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn shares_reject_foreign_and_missing_records_and_follow_deletion() {
        let (mut store, workspace, profile, _, asset) = fixture();
        let other = store.create("Other").unwrap().id;
        let foreign_profile = store.create_profile(other, "Theirs", "", None).unwrap().id;
        assert!(
            store
                .share_asset_with_profile(workspace, asset, foreign_profile, None)
                .is_err()
        );
        assert!(
            store
                .share_asset_with_profile(other, asset, foreign_profile, None)
                .is_err()
        );
        assert!(
            store
                .share_asset_with_profile(workspace, asset + 1, profile, None)
                .is_err()
        );
        store
            .share_asset_with_profile(workspace, asset, profile, None)
            .unwrap();
        store
            .0
            .execute("DELETE FROM profiles WHERE id = ?1", [profile])
            .unwrap();
        assert!(
            store
                .list_asset_profile_shares(workspace, asset)
                .unwrap()
                .is_empty()
        );
    }
}

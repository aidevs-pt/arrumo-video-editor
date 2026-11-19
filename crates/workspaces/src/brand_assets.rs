use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::WorkspaceStore;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrandAssetRole {
    Identity,
    Style,
}

impl BrandAssetRole {
    fn as_str(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Style => "style",
        }
    }
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct BrandAsset {
    pub asset_id: i64,
    pub name: String,
    pub role: String,
}

impl WorkspaceStore {
    pub fn list_brand_assets(&self, workspace_id: i64) -> rusqlite::Result<Vec<BrandAsset>> {
        self.0
            .prepare(
                "SELECT association.asset_id, asset.name, association.role
                 FROM workspace_brand_assets AS association
                 JOIN workspace_assets AS asset ON asset.workspace_id = association.workspace_id
                                              AND asset.id = association.asset_id
                 WHERE association.workspace_id = ?1
                 ORDER BY association.role, association.asset_id",
            )?
            .query_map([workspace_id], |row| {
                Ok(BrandAsset {
                    asset_id: row.get(0)?,
                    name: row.get(1)?,
                    role: row.get(2)?,
                })
            })?
            .collect()
    }

    pub fn attach_brand_asset(
        &self,
        workspace_id: i64,
        asset_id: i64,
        role: BrandAssetRole,
    ) -> rusqlite::Result<()> {
        self.0.query_row(
            "INSERT INTO workspace_brand_assets (workspace_id, asset_id, role)
             SELECT workspace_id, id, ?3 FROM workspace_assets
             WHERE workspace_id = ?1 AND id = ?2 AND kind = 'image'
             ON CONFLICT (workspace_id, role, asset_id) DO UPDATE SET role = excluded.role
             RETURNING asset_id",
            params![workspace_id, asset_id, role.as_str()],
            |_| Ok(()),
        )
    }

    pub fn detach_brand_asset(
        &self,
        workspace_id: i64,
        asset_id: i64,
        role: BrandAssetRole,
    ) -> rusqlite::Result<bool> {
        self.0
            .execute(
                "DELETE FROM workspace_brand_assets
                 WHERE workspace_id = ?1 AND asset_id = ?2 AND role = ?3",
                params![workspace_id, asset_id, role.as_str()],
            )
            .map(|count| count != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn associates_only_owned_images_and_detaches_independently() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let first = store.create("First").unwrap().id;
        let second = store.create("Second").unwrap().id;
        let asset = |workspace, kind| {
            store
                .0
                .query_row(
                    "INSERT INTO workspace_assets (workspace_id, name, kind, byte_size)
                 VALUES (?1, 'Media', ?2, 1) RETURNING id",
                    params![workspace, kind],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap()
        };
        let image = asset(first, "image");
        let audio = asset(first, "audio");
        let foreign = asset(second, "image");
        for id in [audio, foreign, i64::MAX] {
            assert!(
                store
                    .attach_brand_asset(first, id, BrandAssetRole::Style)
                    .is_err()
            );
        }
        store
            .attach_brand_asset(first, image, BrandAssetRole::Style)
            .unwrap();
        store
            .attach_brand_asset(first, image, BrandAssetRole::Style)
            .unwrap();
        store
            .attach_brand_asset(first, image, BrandAssetRole::Identity)
            .unwrap();
        assert_eq!(store.list_brand_assets(first).unwrap().len(), 2);
        assert!(store.list_brand_assets(second).unwrap().is_empty());
        assert!(
            !store
                .detach_brand_asset(second, image, BrandAssetRole::Style)
                .unwrap()
        );
        assert!(
            store
                .detach_brand_asset(first, image, BrandAssetRole::Style)
                .unwrap()
        );
        assert_eq!(store.list_brand_assets(first).unwrap()[0].role, "identity");
        assert_eq!(store.list_assets(first).unwrap().len(), 2);
        store
            .0
            .execute("DELETE FROM workspace_assets WHERE id = ?1", [image])
            .unwrap();
        assert!(store.list_brand_assets(first).unwrap().is_empty());
    }
}

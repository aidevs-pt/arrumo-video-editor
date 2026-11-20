use std::collections::{BTreeMap, HashSet};

use rusqlite::{params, types::Type};
use serde::{Deserialize, Serialize};

use crate::WorkspaceStore;

#[derive(Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AssetOrganization {
    pub group: Option<String>,
    pub labels: Vec<String>,
}

impl WorkspaceStore {
    pub fn asset_organization(
        &self,
        workspace_id: i64,
        asset_id: i64,
    ) -> rusqlite::Result<AssetOrganization> {
        self.0.query_row(
            "SELECT organization.group_name, COALESCE(organization.labels_json, '[]')
             FROM workspace_assets AS asset LEFT JOIN asset_organization AS organization
               ON organization.workspace_id = asset.workspace_id AND organization.asset_id = asset.id
             WHERE asset.workspace_id = ?1 AND asset.id = ?2",
            params![workspace_id, asset_id],
            |row| {
                let labels: String = row.get(1)?;
                Ok(AssetOrganization {
                    group: row.get(0)?,
                    labels: serde_json::from_str(&labels).map_err(|error|
                        rusqlite::Error::FromSqlConversionFailure(1, Type::Text, Box::new(error)))?,
                })
            },
        )
    }

    pub fn list_asset_organization(
        &self,
        workspace_id: i64,
    ) -> rusqlite::Result<BTreeMap<i64, AssetOrganization>> {
        self.0.prepare(
            "SELECT asset_id, group_name, labels_json FROM asset_organization WHERE workspace_id = ?1"
        )?.query_map([workspace_id], |row| {
            let labels: String = row.get(2)?;
            Ok((row.get(0)?, AssetOrganization {
                group: row.get(1)?,
                labels: serde_json::from_str(&labels).map_err(|error|
                    rusqlite::Error::FromSqlConversionFailure(2, Type::Text, Box::new(error)))?,
            }))
        })?.collect()
    }

    pub fn set_asset_organization(
        &self,
        workspace_id: i64,
        asset_id: i64,
        settings: AssetOrganization,
    ) -> rusqlite::Result<AssetOrganization> {
        let group = settings.group.map(normalize_name).transpose()?;
        let mut seen = HashSet::new();
        let mut labels = Vec::new();
        for label in settings.labels {
            let label = normalize_name(label)?;
            if seen.insert(label.to_lowercase()) {
                labels.push(label);
            }
        }
        let labels_json = serde_json::to_string(&labels)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        self.0.query_row(
            "INSERT INTO asset_organization (workspace_id, asset_id, group_name, labels_json)
             SELECT workspace_id, id, ?3, ?4 FROM workspace_assets WHERE workspace_id = ?1 AND id = ?2 AND superseded = 0
             ON CONFLICT(workspace_id, asset_id) DO UPDATE
             SET group_name = excluded.group_name, labels_json = excluded.labels_json
             RETURNING asset_id",
            params![workspace_id, asset_id, group, labels_json],
            |_| Ok(()),
        )?;
        Ok(AssetOrganization { group, labels })
    }
}

fn normalize_name(value: String) -> rusqlite::Result<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(value.to_owned())
}

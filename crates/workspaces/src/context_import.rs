//! Adds what context research found to the workspace once the creator accepts it: brand colours,
//! images (downloaded from their public pages, described, grouped and labelled), brand marks,
//! thumbnail references and profiles.

use std::{error::Error, fs, path::Path};

use serde::{Deserialize, Serialize};

use crate::{
    AssetOrganization, BrandAssetRole, WorkspaceStore,
    context_research::{SuggestedImage, SuggestedProfile},
    source_pages::{fetch_image, source_url},
};

/// The parts of a context draft the creator chose to keep.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextImport {
    #[serde(default)]
    pub colors: Vec<String>,
    #[serde(default)]
    pub images: Vec<SuggestedImage>,
    #[serde(default)]
    pub profiles: Vec<SuggestedProfile>,
}

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContextImportOutcome {
    pub colors: usize,
    pub images: usize,
    pub profiles: usize,
    /// Images that could not be downloaded or imported, by address.
    pub failed_images: Vec<String>,
}

/// A file name for a downloaded image from its suggested name.
fn image_file_name(name: &str, extension: &str) -> String {
    let stem: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .take(8)
        .collect::<Vec<_>>()
        .join("-");
    let stem = if stem.is_empty() {
        "image".to_owned()
    } else {
        stem.chars().take(60).collect()
    };
    format!("{stem}.{extension}")
}

impl WorkspaceStore {
    pub fn apply_context_import(
        &mut self,
        database: &Path,
        workspace_id: i64,
        import: &ContextImport,
    ) -> Result<ContextImportOutcome, Box<dyn Error>> {
        if import.images.len() > 40 || import.profiles.len() > 12 || import.colors.len() > 16 {
            return Err(std::io::Error::other("Too many items to import at once").into());
        }
        let mut outcome = ContextImportOutcome::default();

        if !import.colors.is_empty() {
            let before = self.brand_colors(workspace_id)?;
            let mut colors = before.clone();
            colors.extend(import.colors.iter().cloned());
            let after = self.set_brand_colors(workspace_id, &colors)?;
            outcome.colors = after.len().saturating_sub(before.len());
        }

        let existing: Vec<String> = self
            .list_profiles(workspace_id)?
            .into_iter()
            .map(|profile| profile.name.to_lowercase())
            .collect();
        for profile in &import.profiles {
            let name = profile.name.trim();
            if name.is_empty() || existing.contains(&name.to_lowercase()) {
                continue;
            }
            let guidance = if profile.guidance.trim().is_empty() {
                profile.description.clone()
            } else {
                format!(
                    "{}\n\n{}",
                    profile.description.trim(),
                    profile.guidance.trim()
                )
            };
            let created = self.create_profile(
                workspace_id,
                name,
                guidance.trim(),
                profile.color.as_deref(),
            );
            if created.is_ok() {
                outcome.profiles += 1;
            }
        }

        let root = crate::asset_root(database);
        let scratch = tempfile::tempdir()?;
        for image in &import.images {
            let imported = (|| -> Result<(), Box<dyn Error>> {
                let url = source_url(&image.url)
                    .map_err(|problem| std::io::Error::other(problem.to_string()))?;
                let (bytes, extension) = fetch_image(&url)
                    .map_err(|problem| std::io::Error::other(problem.to_string()))?;
                let file = scratch.path().join(image_file_name(&image.name, extension));
                fs::write(&file, bytes)?;
                let asset = if image.purpose == "thumbnail_reference" {
                    self.import_thumbnail_reference(&root, workspace_id, &file)
                } else {
                    self.import_asset(&root, workspace_id, &file)
                }
                .map_err(|error| std::io::Error::other(error.to_string()))?;
                let _ = fs::remove_file(&file);
                let mut notes = image.description.trim().to_owned();
                if image.purpose == "thumbnail_reference" {
                    notes = format!("Thumbnail reference. {notes}");
                }
                if !notes.is_empty() {
                    self.set_asset_notes(workspace_id, asset.id, notes.trim())?;
                }
                self.set_asset_organization(
                    workspace_id,
                    asset.id,
                    AssetOrganization {
                        group: image.group.clone(),
                        labels: image.labels.clone(),
                    },
                )?;
                if image.purpose == "brand" {
                    self.attach_brand_asset(workspace_id, asset.id, BrandAssetRole::Identity)?;
                }
                Ok(())
            })();
            match imported {
                Ok(()) => outcome.images += 1,
                Err(_) => outcome.failed_images.push(image.url.clone()),
            }
        }
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_and_profiles_merge_without_duplicates() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("studio.sqlite");
        let mut store = WorkspaceStore::open(&database).unwrap();
        let workspace = store.create("Studio").unwrap().id;
        store
            .set_brand_colors(workspace, &["#112233".into()])
            .unwrap();
        store
            .create_profile(workspace, "Reviews", "", None)
            .unwrap();
        let import = ContextImport {
            colors: vec!["#112233".into(), "#ffcc00".into()],
            images: Vec::new(),
            profiles: vec![
                SuggestedProfile {
                    name: "reviews".into(),
                    description: "dup".into(),
                    guidance: String::new(),
                    color: None,
                },
                SuggestedProfile {
                    name: "Speedruns".into(),
                    description: "Fast solves".into(),
                    guidance: "Keep cuts tight.".into(),
                    color: Some("#ff0000".into()),
                },
            ],
        };
        let outcome = store
            .apply_context_import(&database, workspace, &import)
            .unwrap();
        assert_eq!(
            (outcome.colors, outcome.profiles, outcome.images),
            (1, 1, 0)
        );
        assert_eq!(store.brand_colors(workspace).unwrap().len(), 2);
        let speedruns = store
            .list_profiles(workspace)
            .unwrap()
            .into_iter()
            .find(|profile| profile.name == "Speedruns")
            .unwrap();
        assert!(speedruns.guidance.contains("Keep cuts tight."));
    }

    #[test]
    fn image_names_become_safe_file_names() {
        assert_eq!(
            image_file_name("Channel logo (2026)!", "png"),
            "Channel-logo-2026.png"
        );
        assert_eq!(image_file_name("../../etc", "jpg"), "etc.jpg");
        assert_eq!(image_file_name("", "webp"), "image.webp");
    }
}

use std::{
    collections::HashSet,
    error::Error,
    io::{self, Read},
    time::Duration,
};

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::{
    CatalogPlugin, WorkspaceStore,
    plugin_catalog::{PLUGIN_MANIFEST_SCHEMA_VERSION, identifier},
    plugin_package::valid_sha256,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryReleaseSource {
    pub plugin_id: String,
    pub repository: String,
    pub version: String,
    pub package_url: String,
    pub manifest_schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginRegistry {
    schema_version: u32,
    pub plugins: Vec<RegistryPlugin>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryPlugin {
    pub id: String,
    pub repository: String,
    #[serde(rename = "default")]
    pub default: bool,
    #[serde(default)]
    pub releases: Vec<RegistryRelease>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegistryRelease {
    pub version: String,
    pub manifest_schema_version: u32,
    pub package_url: String,
    pub sha256: String,
}

impl RegistryRelease {
    pub fn compatible(&self) -> bool {
        self.manifest_schema_version == PLUGIN_MANIFEST_SCHEMA_VERSION
    }
}

pub(crate) struct InstalledRelease {
    pub sha256: String,
    pub raw_source: String,
}

impl WorkspaceStore {
    pub fn install_plugin_release(
        &self,
        catalog: impl Read,
        id: &str,
        version: &str,
        archive: impl Read,
    ) -> Result<CatalogPlugin, Box<dyn Error>> {
        let (source, digest) = read_plugin_registry(catalog)?.select_release(id, version)?;
        self.register_plugin_zip(archive, Some(&digest), Some(source), None)
    }

    pub fn download_plugin_release(
        &self,
        catalog_url: &str,
        id: &str,
        version: &str,
    ) -> Result<CatalogPlugin, Box<dyn Error>> {
        self.download_registry_release(catalog_url, id, version, None, None)
    }

    pub fn download_reviewed_plugin_release(
        &self,
        catalog_url: &str,
        id: &str,
        version: &str,
        repository: &str,
        release: &RegistryRelease,
    ) -> Result<CatalogPlugin, Box<dyn Error>> {
        self.download_registry_release(catalog_url, id, version, Some((repository, release)), None)
    }

    pub fn update_plugin_release(
        &self,
        id: &str,
        version: &str,
    ) -> Result<CatalogPlugin, Box<dyn Error>> {
        let previous = self
            .0
            .query_row(
                "SELECT package_sha256, release_source FROM installed_plugin_manifests
             WHERE plugin_id = ?1 AND release_source IS NOT NULL",
                [id],
                |row| {
                    Ok(InstalledRelease {
                        sha256: row.get(0)?,
                        raw_source: row.get(1)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| invalid_registry("Plugin has no installed registry release"))?;
        let source: RegistryReleaseSource = serde_json::from_str(&previous.raw_source)?;
        let catalog_url = source.catalog_url.as_deref().ok_or_else(|| {
            invalid_registry("Plugin was installed from a local registry snapshot")
        })?;
        if !semver::Version::parse(version)?
            .cmp_precedence(&semver::Version::parse(&source.version)?)
            .is_gt()
        {
            return Err(invalid_registry("Update must target a newer plugin version").into());
        }
        self.download_registry_release(
            catalog_url,
            id,
            version,
            None,
            Some((&previous, &source.repository)),
        )
    }

    fn download_registry_release(
        &self,
        catalog_url: &str,
        id: &str,
        version: &str,
        reviewed: Option<(&str, &RegistryRelease)>,
        previous: Option<(&InstalledRelease, &str)>,
    ) -> Result<CatalogPlugin, Box<dyn Error>> {
        validate_registry_url(catalog_url)?;
        let client = registry_client()?;
        let catalog = get_release_response(&client, catalog_url)?;
        let (mut source, digest) = read_plugin_registry(catalog)?.select_release(id, version)?;
        if let Some((repository, release)) = reviewed
            && (repository != source.repository
                || release.version != source.version
                || release.package_url != source.package_url
                || release.manifest_schema_version != source.manifest_schema_version
                || !release.sha256.eq_ignore_ascii_case(&digest))
        {
            return Err(invalid_registry("Plugin release changed since review").into());
        }
        if previous.is_some_and(|(_, repository)| repository != source.repository) {
            return Err(invalid_registry(
                "Plugin update source repository differs from the installed release",
            )
            .into());
        }
        let archive = get_release_response(&client, &source.package_url)?;
        source.catalog_url = Some(catalog_url.to_owned());
        self.register_plugin_zip(
            archive,
            Some(&digest),
            Some(source),
            previous.map(|(record, _)| record),
        )
    }
}

fn registry_client() -> Result<reqwest::blocking::Client, reqwest::Error> {
    reqwest::blocking::Client::builder()
        .https_only(true)
        .referer(false)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let url = attempt.url();
            if attempt.previous().len() > 5
                || url.scheme() != "https"
                || !url.username().is_empty()
                || url.password().is_some()
            {
                return attempt.error("Invalid plugin registry redirect");
            }
            attempt.follow()
        }))
        .build()
}

fn get_release_response(
    client: &reqwest::blocking::Client,
    url: &str,
) -> Result<reqwest::blocking::Response, Box<dyn Error>> {
    let response = client
        .get(url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(reqwest::Error::without_url)?;
    if response.status() != reqwest::StatusCode::OK {
        return Err(invalid_registry("Expected a complete HTTP 200 response").into());
    }
    Ok(response)
}

pub fn fetch_plugin_registry(catalog_url: &str) -> Result<PluginRegistry, Box<dyn Error>> {
    validate_registry_url(catalog_url)?;
    let client = registry_client()?;
    read_plugin_registry(get_release_response(&client, catalog_url)?)
}

pub fn read_plugin_registry(catalog: impl Read) -> Result<PluginRegistry, Box<dyn Error>> {
    const MAX_CATALOG_BYTES: u64 = 1024 * 1024;
    let mut bytes = Vec::new();
    catalog
        .take(MAX_CATALOG_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_CATALOG_BYTES {
        return Err(invalid_registry("Plugin registry exceeds 1 MiB").into());
    }
    let registry: PluginRegistry = serde_json::from_slice(&bytes)?;
    if registry.schema_version != 1 {
        return Err(invalid_registry("Unsupported plugin registry schema").into());
    }
    let mut plugin_ids = HashSet::new();
    for plugin in &registry.plugins {
        if !identifier(&plugin.id) || !plugin_ids.insert(&plugin.id) {
            return Err(invalid_registry("Invalid or duplicate plugin ID in registry").into());
        }
        validate_registry_url(&plugin.repository)?;
        let mut versions = HashSet::new();
        for release in &plugin.releases {
            semver::Version::parse(&release.version)?;
            if !versions.insert(&release.version) {
                return Err(
                    invalid_registry("Duplicate plugin release version in registry").into(),
                );
            }
            if release.manifest_schema_version == 0 || !valid_sha256(&release.sha256) {
                return Err(
                    invalid_registry("Invalid plugin release schema version or SHA-256").into(),
                );
            }
            validate_registry_url(&release.package_url)?;
        }
    }
    Ok(registry)
}

impl PluginRegistry {
    fn select_release(
        self,
        id: &str,
        version: &str,
    ) -> Result<(RegistryReleaseSource, String), Box<dyn Error>> {
        let plugin = self
            .plugins
            .into_iter()
            .find(|plugin| plugin.id == id)
            .ok_or_else(|| invalid_registry("Plugin is absent from registry"))?;
        let release = plugin
            .releases
            .into_iter()
            .find(|release| release.version == version)
            .ok_or_else(|| invalid_registry("Plugin release is absent from registry"))?;
        if !release.compatible() {
            return Err(
                invalid_registry("Plugin release requires an unsupported manifest schema").into(),
            );
        }
        let source = RegistryReleaseSource {
            plugin_id: plugin.id,
            repository: plugin.repository,
            version: release.version,
            package_url: release.package_url,
            manifest_schema_version: release.manifest_schema_version,
            catalog_url: None,
        };
        Ok((source, release.sha256))
    }
}

pub fn validate_registry_url(value: &str) -> Result<(), Box<dyn Error>> {
    let url = url::Url::parse(value)?;
    if !value.starts_with("https://")
        || value.chars().any(|c| c.is_whitespace() || c.is_control())
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.query().is_some()
    {
        return Err(invalid_registry("Registry URLs must be HTTPS URLs with a host without credentials, queries, or fragments").into());
    }
    Ok(())
}

fn invalid_registry(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

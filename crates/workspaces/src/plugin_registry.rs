use std::{
    error::Error,
    io::{self, Read},
};

use serde::{Deserialize, Serialize};

use crate::{CatalogPlugin, WorkspaceStore};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryReleaseSource {
    pub plugin_id: String,
    pub repository: String,
    pub version: String,
    pub package_url: String,
    pub manifest_schema_version: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Registry {
    schema_version: u32,
    plugins: Vec<RegistryPlugin>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryPlugin {
    id: String,
    repository: String,
    #[serde(rename = "default")]
    _default: bool,
    #[serde(default)]
    releases: Vec<RegistryRelease>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RegistryRelease {
    version: String,
    manifest_schema_version: u32,
    package_url: String,
    sha256: String,
}

impl WorkspaceStore {
    pub fn install_plugin_release(
        &self,
        catalog: impl Read,
        id: &str,
        version: &str,
        archive: impl Read,
    ) -> Result<CatalogPlugin, Box<dyn Error>> {
        const MAX_CATALOG_BYTES: u64 = 1024 * 1024;
        let mut bytes = Vec::new();
        catalog
            .take(MAX_CATALOG_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_CATALOG_BYTES {
            return Err(invalid_registry("Plugin registry exceeds 1 MiB").into());
        }
        let registry: Registry = serde_json::from_slice(&bytes)?;
        if registry.schema_version != 1 {
            return Err(invalid_registry("Unsupported plugin registry schema").into());
        }
        let mut plugins = registry
            .plugins
            .into_iter()
            .filter(|plugin| plugin.id == id);
        let plugin = plugins
            .next()
            .ok_or_else(|| invalid_registry("Plugin is absent from registry"))?;
        if plugins.next().is_some() {
            return Err(invalid_registry("Duplicate plugin ID in registry").into());
        }
        semver::Version::parse(version)?;
        let mut releases = plugin
            .releases
            .into_iter()
            .filter(|release| release.version == version);
        let release = releases
            .next()
            .ok_or_else(|| invalid_registry("Plugin release is absent from registry"))?;
        if releases.next().is_some() {
            return Err(invalid_registry("Duplicate plugin release version in registry").into());
        }
        if release.manifest_schema_version != 1 {
            return Err(
                invalid_registry("Plugin release requires an unsupported manifest schema").into(),
            );
        }
        validate_https_url(&plugin.repository)?;
        validate_https_url(&release.package_url)?;
        let source = RegistryReleaseSource {
            plugin_id: plugin.id,
            repository: plugin.repository,
            version: release.version,
            package_url: release.package_url,
            manifest_schema_version: release.manifest_schema_version,
        };
        self.register_plugin_zip(archive, Some(&release.sha256), Some(source))
    }
}

fn validate_https_url(value: &str) -> Result<(), Box<dyn Error>> {
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

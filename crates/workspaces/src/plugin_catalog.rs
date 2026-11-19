use std::{collections::BTreeMap, error::Error, io, sync::OnceLock};

use serde::{Deserialize, Serialize};

use crate::{PluginPackageInfo, RegistryReleaseSource, WorkspaceStore};

pub const PLUGIN_MANIFEST_SCHEMA_VERSION: u32 = 1;

pub const MAX_PLUGIN_MANIFEST_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginManifest {
    schema_version: u32,
    pub id: String,
    pub name: String,
    pub description: String,
    pub capabilities: BTreeMap<String, PluginCapability>,
    #[serde(
        default,
        deserialize_with = "deserialize_provider",
        skip_serializing_if = "Option::is_none"
    )]
    pub agent_provider: Option<ProviderDescriptor>,
}

fn deserialize_provider<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<ProviderDescriptor>, D::Error> {
    ProviderDescriptor::deserialize(deserializer).map(Some)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PluginCapability {
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderDescriptor {
    pub label: String,
    pub setup_url: String,
    pub(crate) cli: AgentCli,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AgentCli {
    pub executable: String,
    pub auth_status_args: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPlugin {
    pub bundled: bool,
    pub manifest: PluginManifest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<PluginPackageInfo>,
}

impl PluginManifest {
    fn parse(bytes: &[u8]) -> Result<Self, Box<dyn Error>> {
        if bytes.len() > MAX_PLUGIN_MANIFEST_BYTES {
            return Err(invalid_manifest("Plugin manifest exceeds 64 KiB").into());
        }
        let manifest: Self = serde_json::from_slice(bytes)?;
        if manifest.schema_version != PLUGIN_MANIFEST_SCHEMA_VERSION {
            return Err(invalid_manifest("Unsupported plugin manifest schema version").into());
        }
        if !identifier(&manifest.id) || blank(&manifest.name) || blank(&manifest.description) {
            return Err(invalid_manifest("Invalid plugin identity").into());
        }
        if manifest.capabilities.is_empty()
            || manifest.capabilities.iter().any(|(id, capability)| {
                !identifier(id) || blank(&capability.name) || blank(&capability.description)
            })
        {
            return Err(invalid_manifest("Invalid plugin capabilities").into());
        }
        if let Some(provider) = &manifest.agent_provider
            && (!manifest.capabilities.contains_key("agents")
                || blank(&provider.label)
                || provider.label.chars().count() > 80
                || !provider
                    .setup_url
                    .strip_prefix("https://")
                    .is_some_and(|url| !url.is_empty() && !url.chars().any(char::is_whitespace))
                || provider.setup_url.chars().count() > 2048
                || !identifier(&provider.cli.executable)
                || !(1..=4).contains(&provider.cli.auth_status_args.len())
                || !provider
                    .cli
                    .auth_status_args
                    .iter()
                    .all(|arg| identifier(arg)))
        {
            return Err(invalid_manifest("Invalid plugin agent provider descriptor").into());
        }
        Ok(manifest)
    }
}

pub(crate) fn bundled_manifests() -> &'static [PluginManifest] {
    static MANIFESTS: OnceLock<Vec<PluginManifest>> = OnceLock::new();
    MANIFESTS.get_or_init(|| {
        // Release inputs copied from the official SDK plugin repositories.
        [
            include_bytes!("../plugins/codex/manifest.json").as_slice(),
            include_bytes!("../plugins/claude-code/manifest.json").as_slice(),
            include_bytes!("../plugins/openai/manifest.json").as_slice(),
            include_bytes!("../plugins/higgsfield/manifest.json").as_slice(),
            include_bytes!("../plugins/gemini/manifest.json").as_slice(),
            include_bytes!("../plugins/elevenlabs/manifest.json").as_slice(),
            include_bytes!("../plugins/auto-dubbing/manifest.json").as_slice(),
        ]
        .into_iter()
        .map(|bytes| PluginManifest::parse(bytes).expect("valid bundled plugin manifest"))
        .collect()
    })
}

impl WorkspaceStore {
    pub fn install_plugin_manifest(&self, bytes: &[u8]) -> Result<CatalogPlugin, Box<dyn Error>> {
        self.register_plugin(bytes, None, None)
    }

    pub(crate) fn register_plugin(
        &self,
        bytes: &[u8],
        package: Option<(&PluginPackageInfo, &[u8])>,
        expected_id: Option<&str>,
    ) -> Result<CatalogPlugin, Box<dyn Error>> {
        let manifest = PluginManifest::parse(bytes)?;
        if expected_id.is_some_and(|id| id != manifest.id) {
            return Err(
                invalid_manifest("Package plugin ID differs from the registry entry").into(),
            );
        }
        if bundled_manifests()
            .iter()
            .any(|plugin| plugin.id == manifest.id)
        {
            return Err(invalid_manifest("Plugin ID belongs to a bundled plugin").into());
        }
        let (info, archive) = package.unzip();
        let release = info
            .and_then(|info| info.release.as_ref())
            .map(serde_json::to_string)
            .transpose()?;
        let added = self.0.execute(
            "INSERT INTO installed_plugin_manifests (plugin_id, manifest, package_sha256, package_bytes, release_source)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(plugin_id) DO NOTHING",
            rusqlite::params![manifest.id, bytes, info.map(|info| &info.sha256), archive, release],
        )?;
        if added == 0 {
            return Err(invalid_manifest("Plugin ID is already installed").into());
        }
        Ok(CatalogPlugin {
            bundled: false,
            manifest,
            package: info.cloned(),
        })
    }

    pub fn plugin_catalog(&self) -> Result<Vec<CatalogPlugin>, Box<dyn Error>> {
        let mut plugins: Vec<_> = bundled_manifests()
            .iter()
            .cloned()
            .map(|manifest| CatalogPlugin {
                bundled: true,
                manifest,
                package: None,
            })
            .collect();
        let mut query = self.0.prepare(
            "SELECT manifest, package_sha256, length(package_bytes), release_source
             FROM installed_plugin_manifests ORDER BY plugin_id",
        )?;
        let rows = query.query_map([], |row| {
            let manifest = row.get::<_, Vec<u8>>(0)?;
            let sha256 = row.get::<_, Option<String>>(1)?;
            let package = sha256
                .map(|sha256| {
                    Ok::<_, rusqlite::Error>((
                        sha256,
                        row.get::<_, i64>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                })
                .transpose()?;
            Ok((manifest, package))
        })?;
        for stored in rows {
            let (manifest, package) = stored?;
            let package = package
                .map(|(sha256, byte_length, release)| {
                    Ok::<_, serde_json::Error>(PluginPackageInfo {
                        sha256,
                        byte_length,
                        release: release
                            .as_deref()
                            .map(serde_json::from_str::<RegistryReleaseSource>)
                            .transpose()?,
                    })
                })
                .transpose()?;
            plugins.push(CatalogPlugin {
                bundled: false,
                manifest: PluginManifest::parse(&manifest)?,
                package,
            });
        }
        Ok(plugins)
    }

    pub fn remove_installed_plugin_manifest(&self, id: &str) -> rusqlite::Result<bool> {
        let removed = self.0.execute(
            "DELETE FROM installed_plugin_manifests WHERE plugin_id = ?1",
            [id],
        )?;
        Ok(removed == 1)
    }
}

fn blank(value: &str) -> bool {
    value.trim().is_empty()
}

fn invalid_manifest(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub(crate) fn identifier(value: &str) -> bool {
    value.split('-').enumerate().all(|(index, part)| {
        !part.is_empty()
            && (index > 0 || part.starts_with(|c: char| c.is_ascii_lowercase()))
            && part
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    })
}

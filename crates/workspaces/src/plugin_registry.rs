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
    plugin_catalog::{SUPPORTED_PLUGIN_MANIFEST_SCHEMAS, identifier},
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
        SUPPORTED_PLUGIN_MANIFEST_SCHEMAS.contains(&self.manifest_schema_version)
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
        let plugin = self.register_plugin_zip(archive, Some(&digest), Some(source), None)?;
        // Registry plugins are public, so their ID says nothing about the creator.
        self.track_usage("plugin_installed", &[("plugin", serde_json::json!(id))]);
        Ok(plugin)
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
        let (previous, source) = self.installed_release_for_update(id, version)?;
        let catalog_url = source.catalog_url.as_deref().ok_or_else(|| {
            invalid_registry("Plugin was installed from a local registry snapshot")
        })?;
        self.download_registry_release(
            catalog_url,
            id,
            version,
            None,
            Some((&previous, &source.repository)),
        )
    }

    pub fn update_reviewed_plugin_release(
        &self,
        catalog_url: &str,
        id: &str,
        repository: &str,
        release: &RegistryRelease,
    ) -> Result<CatalogPlugin, Box<dyn Error>> {
        let (previous, source) = self.installed_release_for_update(id, &release.version)?;
        if source.catalog_url.as_deref() != Some(catalog_url) || source.repository != repository {
            return Err(invalid_registry(
                "Update must use the installed plugin's registry and repository",
            )
            .into());
        }
        self.download_registry_release(
            catalog_url,
            id,
            &release.version,
            Some((repository, release)),
            Some((&previous, &source.repository)),
        )
    }

    pub fn update_plugin_release_from_snapshot(
        &self,
        catalog: impl Read,
        id: &str,
        version: &str,
        archive: impl Read,
    ) -> Result<CatalogPlugin, Box<dyn Error>> {
        let (previous, installed) = self.installed_release_for_update(id, version)?;
        if installed.catalog_url.is_some() {
            return Err(invalid_registry("Plugin was installed from a remote registry").into());
        }
        let (source, digest) = read_plugin_registry(catalog)?.select_release(id, version)?;
        validate_update_repository(&installed.repository, &source)?;
        self.register_plugin_zip(archive, Some(&digest), Some(source), Some(&previous))
    }

    fn installed_release_for_update(
        &self,
        id: &str,
        version: &str,
    ) -> Result<(InstalledRelease, RegistryReleaseSource), Box<dyn Error>> {
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
        if !semver::Version::parse(version)?
            .cmp_precedence(&semver::Version::parse(&source.version)?)
            .is_gt()
        {
            return Err(invalid_registry("Update must target a newer plugin version").into());
        }
        Ok((previous, source))
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
        if let Some((_, repository)) = previous {
            validate_update_repository(repository, &source)?;
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

fn validate_update_repository(repository: &str, source: &RegistryReleaseSource) -> io::Result<()> {
    if repository != source.repository {
        return Err(invalid_registry(
            "Plugin update source repository differs from the installed release",
        ));
    }
    Ok(())
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
    pub fn update_versions(
        &self,
        catalog_url: &str,
        source: &RegistryReleaseSource,
    ) -> Vec<String> {
        if source.catalog_url.as_deref() != Some(catalog_url) {
            return Vec::new();
        }
        let Some(plugin) = self
            .plugins
            .iter()
            .find(|plugin| plugin.id == source.plugin_id && plugin.repository == source.repository)
        else {
            return Vec::new();
        };
        let Ok(installed) = semver::Version::parse(&source.version) else {
            return Vec::new();
        };
        plugin
            .releases
            .iter()
            .filter(|release| {
                release.compatible()
                    && semver::Version::parse(&release.version)
                        .is_ok_and(|version| version.cmp_precedence(&installed).is_gt())
            })
            .map(|release| release.version.clone())
            .collect()
    }

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

/// The `owner/name` of a GitHub repository address such as
/// `https://github.com/owner/name`, `…/name.git` or `…/name/`.
fn github_repository(url: &str) -> io::Result<(String, String)> {
    let bad = || {
        invalid_registry("Enter a GitHub repository address, like https://github.com/owner/plugin")
    };
    let rest = url
        .trim()
        .strip_prefix("https://github.com/")
        .ok_or_else(bad)?;
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let (owner, name) = rest.split_once('/').ok_or_else(bad)?;
    let part = |text: &str| {
        (1..=100).contains(&text.len())
            && !matches!(text, "." | "..")
            && text
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    };
    if !part(owner) || !part(name) {
        return Err(bad());
    }
    Ok((owner.to_owned(), name.to_owned()))
}

#[derive(Deserialize)]
struct GithubRelease {
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    #[serde(default)]
    digest: Option<String>,
}

/// The plugin package among a release's assets: `plugin.zip`, or its only ZIP.
fn plugin_asset(assets: &[GithubAsset]) -> io::Result<&GithubAsset> {
    let zips: Vec<_> = assets
        .iter()
        .filter(|asset| asset.name.to_ascii_lowercase().ends_with(".zip"))
        .collect();
    zips.iter()
        .find(|asset| asset.name.eq_ignore_ascii_case("plugin.zip"))
        .or(match zips.as_slice() {
            [only] => Some(only),
            _ => None,
        })
        .copied()
        .ok_or_else(|| {
            invalid_registry(if zips.is_empty() {
                "The latest release has no plugin ZIP"
            } else {
                "The latest release has several ZIP files; name the plugin package plugin.zip"
            })
        })
}

impl WorkspaceStore {
    /// Installs the plugin package from the latest release of a public GitHub repository,
    /// checking `expected_sha256` and GitHub's SHA-256 digest of the file when either is known. The package goes
    /// through the same checks as a local ZIP and is a third-party plugin.
    pub fn install_plugin_from_repository(
        &self,
        repository: &str,
        expected_sha256: Option<&str>,
    ) -> Result<CatalogPlugin, Box<dyn Error>> {
        let (owner, name) = github_repository(repository)?;
        let client = reqwest::blocking::Client::builder()
            .https_only(true)
            .referer(false)
            .user_agent("Arrumo")
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                let url = attempt.url();
                let host = url.host_str().unwrap_or_default();
                if attempt.previous().len() > 5
                    || url.scheme() != "https"
                    || !url.username().is_empty()
                    || url.password().is_some()
                    || !(host == "github.com"
                        || host.ends_with(".github.com")
                        || host.ends_with(".githubusercontent.com"))
                {
                    return attempt.error("Invalid plugin download redirect");
                }
                attempt.follow()
            }))
            .build()?;
        let response = client
            .get(format!(
                "https://api.github.com/repos/{owner}/{name}/releases/latest"
            ))
            .header("accept", "application/vnd.github+json")
            .send()
            .map_err(reqwest::Error::without_url)?;
        match response.status().as_u16() {
            200 => {}
            404 => {
                return Err(invalid_registry(
                    "No public release found. Check the address, and that the repository is public and has a release",
                )
                .into());
            }
            403 | 429 => {
                return Err(invalid_registry(
                    "GitHub is limiting requests right now. Try again later",
                )
                .into());
            }
            status => {
                return Err(invalid_registry(&format!("GitHub returned HTTP {status}")).into());
            }
        }
        let mut body = Vec::new();
        response.take(1024 * 1024).read_to_end(&mut body)?;
        let release: GithubRelease = serde_json::from_slice(&body)
            .map_err(|_| invalid_registry("Could not read the release from GitHub"))?;
        let asset = plugin_asset(&release.assets)?;
        if asset.size > 16 * 1024 * 1024 {
            return Err(invalid_registry("The plugin package is over 16 MiB").into());
        }
        let digest = asset
            .digest
            .as_deref()
            .and_then(|digest| digest.strip_prefix("sha256:"))
            .filter(|digest| valid_sha256(digest));
        let url = reqwest::Url::parse(&asset.browser_download_url)
            .map_err(|_| invalid_registry("Invalid plugin download address"))?;
        if url.host_str() != Some("github.com")
            || !url
                .path()
                .starts_with(&format!("/{owner}/{name}/releases/download/"))
        {
            return Err(invalid_registry("The plugin download is not in that repository").into());
        }
        if let (Some(expected), Some(published)) = (expected_sha256, digest)
            && !expected.eq_ignore_ascii_case(published)
        {
            return Err(invalid_registry(
                "The release's package differs from the expected SHA-256",
            )
            .into());
        }
        let archive = get_release_response(&client, url.as_str())?;
        self.register_plugin_zip(archive, expected_sha256.or(digest), None, None)
    }
}

#[cfg(test)]
mod repository_tests {
    use super::*;

    #[test]
    fn reads_github_repository_addresses() {
        for url in [
            "https://github.com/aidevs-pt/arrumo-plugin",
            "https://github.com/aidevs-pt/arrumo-plugin/",
            "https://github.com/aidevs-pt/arrumo-plugin.git",
        ] {
            assert_eq!(
                github_repository(url).unwrap(),
                ("aidevs-pt".to_owned(), "arrumo-plugin".to_owned())
            );
        }
        for url in [
            "http://github.com/a/b",
            "https://gitlab.com/a/b",
            "https://github.com/a",
            "https://github.com/a/b/c",
            "https://github.com/../b",
        ] {
            assert!(github_repository(url).is_err(), "{url}");
        }
    }

    #[test]
    fn picks_the_plugin_package() {
        let asset = |name: &str| GithubAsset {
            name: name.into(),
            browser_download_url: String::new(),
            size: 1,
            digest: None,
        };
        assert_eq!(
            plugin_asset(&[asset("SHA256SUMS"), asset("x.zip")])
                .unwrap()
                .name,
            "x.zip"
        );
        assert_eq!(
            plugin_asset(&[asset("source.zip"), asset("plugin.zip")])
                .unwrap()
                .name,
            "plugin.zip"
        );
        assert!(plugin_asset(&[asset("a.zip"), asset("b.zip")]).is_err());
        assert!(plugin_asset(&[asset("notes.txt")]).is_err());
    }
}

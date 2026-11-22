use std::{
    collections::{BTreeMap, HashSet},
    error::Error,
    io,
    sync::OnceLock,
};

use serde::{Deserialize, Serialize};

use crate::{PluginCode, PluginPackageInfo, PluginSetting, RegistryReleaseSource, WorkspaceStore};

pub const SUPPORTED_PLUGIN_MANIFEST_SCHEMAS: [u32; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

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
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub settings: Option<Vec<PluginSetting>>,
    #[serde(
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub code: Option<PluginCode>,
    #[serde(
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub actions: Option<Vec<crate::PluginAction>>,
    #[serde(
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub agent_provider: Option<ProviderDescriptor>,
    /// What the plugin adds, shown once in the plugin tour after it arrives (schema 8).
    #[serde(
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub introduction: Option<PluginIntroduction>,
    /// A small SVG the plugin is shown with (schema 8).
    #[serde(
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub icon: Option<String>,
    /// The plugin's own text in other languages, keyed `es` and `pt` (schema 8).
    #[serde(
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub translations: Option<BTreeMap<String, crate::plugin_text::PluginTranslation>>,
}

/// Text in the app's languages, keyed `en`, `es` and `pt`; English is required and used when a
/// translation is missing.
pub type LocalizedText = BTreeMap<String, String>;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginIntroduction {
    pub headline: LocalizedText,
    pub body: LocalizedText,
}

impl PluginIntroduction {
    fn valid(&self) -> bool {
        let text = |map: &LocalizedText, limit: usize| {
            map.contains_key("en")
                && map.iter().all(|(language, value)| {
                    matches!(language.as_str(), "en" | "es" | "pt")
                        && !blank(value)
                        && value.chars().count() <= limit
                })
        };
        text(&self.headline, 80) && text(&self.body, 320)
    }
}

pub(crate) fn deserialize_present<'de, T: Deserialize<'de>, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_run: Option<StructuredRun>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_run: Option<ImageRun>,
}

/// An agent run that makes one image with the agent's own image tool. The prompt goes to stdin;
/// the agent prints JSON events, one of which names its session, and leaves the image in a folder
/// named after that session.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ImageRun {
    pub args: Vec<String>,
    /// JSON Pointer to the session ID in one of the printed events.
    pub session_pointer: String,
    pub output_directory: ImageOutputDirectory,
}

/// Where the agent leaves images: `path` (with `{session}`) under the directory named by the
/// environment variable `env`, or under `fallback` inside the home directory when it is unset.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ImageOutputDirectory {
    pub env: String,
    pub fallback: String,
    pub path: String,
}

impl ImageRun {
    fn valid(&self) -> bool {
        let segment = |part: &str| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        };
        let relative = |path: &str, session: bool| {
            path.len() <= 128
                && path
                    .split('/')
                    .all(|part| segment(part) || (session && part == "{session}"))
                && (!session || path.split('/').any(|part| part == "{session}"))
        };
        (1..=40).contains(&self.args.len())
            && self
                .args
                .iter()
                .all(|arg| arg.len() <= 512 && !arg.contains('\0'))
            && self.session_pointer.len() <= 128
            && self.session_pointer.starts_with('/')
            && self
                .session_pointer
                .split('/')
                .skip(1)
                .all(|part| !part.is_empty())
            && !self.output_directory.env.is_empty()
            && self.output_directory.env.len() <= 64
            && self
                .output_directory
                .env
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
            && relative(&self.output_directory.fallback, false)
            && relative(&self.output_directory.path, true)
    }
}

/// A non-interactive agent run declared by the provider: the prompt goes to stdin and the answer
/// must follow a JSON Schema passed through `{schema}` or `{schemaPath}`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StructuredRun {
    pub args: Vec<String>,
    /// Arguments chosen by the plugin's settings, inserted at `{settings}`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub setting_args: Vec<SettingArgs>,
    /// Arguments attaching one image, repeated per image at `{images}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_args: Option<Vec<String>>,
    /// Patterns that turn the agent's printed lines into steps creators can follow.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub progress: Vec<ProgressPattern>,
    pub result: StructuredResult,
    /// Arguments that stop the agent keeping its session, inserted at `{ephemeral}` when no
    /// follow-up is expected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ephemeral_args: Option<Vec<String>>,
    /// How to send a follow-up message to the same session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume: Option<ResumeRun>,
}

/// A follow-up run in an earlier session: `args` replace the run's arguments, with `{session}`
/// for the session ID read from the agent's output.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ResumeRun {
    pub args: Vec<String>,
    pub session: SessionSource,
}

/// Where the agent prints its session ID: a regular expression whose first group is the ID, or
/// a JSON Pointer into one-event-per-line output.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SessionSource {
    pub stream: SessionStream,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SessionStream {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProgressPattern {
    /// For agents printing one JSON event per line: the string at this pointer is matched instead
    /// of the whole line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer: Option<String>,
    pub pattern: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SettingArgs {
    pub setting: String,
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub otherwise: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StructuredResult {
    pub source: StructuredSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_pointer: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum StructuredSource {
    OutputFile,
    Stdout,
    /// The last line of standard output, for agents that print one JSON event per line.
    StdoutLastLine,
}

impl StructuredRun {
    fn valid(&self) -> bool {
        let has = |placeholder: &str| self.args.iter().any(|arg| arg == placeholder);
        let pointer = |value: &str, allow_root: bool| {
            value.len() <= 128
                && (allow_root && value.is_empty()
                    || value.starts_with('/')
                        && value.split('/').skip(1).all(|part| !part.is_empty()))
        };
        (1..=40).contains(&self.args.len())
            && self
                .args
                .iter()
                .all(|arg| arg.len() <= 512 && !arg.contains('\0'))
            && (has("{schema}") || has("{schemaPath}"))
            && (self.result.source != StructuredSource::OutputFile || has("{outputPath}"))
            && self
                .result
                .pointer
                .as_deref()
                .is_none_or(|value| pointer(value, true))
            && self
                .result
                .error_pointer
                .as_deref()
                .is_none_or(|value| pointer(value, false))
            && self.progress.iter().all(|entry| {
                entry
                    .pointer
                    .as_deref()
                    .is_none_or(|value| pointer(value, false))
            })
            && self.ephemeral_args.as_ref().is_none_or(|args| {
                (1..=4).contains(&args.len()) && args.iter().all(|arg| arg.len() <= 128)
            })
            && self.resume.as_ref().is_none_or(|resume| {
                let has = |placeholder: &str| resume.args.iter().any(|arg| arg == placeholder);
                (1..=40).contains(&resume.args.len())
                    && resume
                        .args
                        .iter()
                        .all(|arg| arg.len() <= 512 && !arg.contains('\0'))
                    && has("{session}")
                    && (has("{schema}") || has("{schemaPath}"))
                    && (self.result.source != StructuredSource::OutputFile || has("{outputPath}"))
                    && match (&resume.session.pattern, &resume.session.pointer) {
                        (Some(pattern), None) => {
                            pattern.len() <= 256
                                && regex::Regex::new(pattern)
                                    .is_ok_and(|compiled| compiled.captures_len() >= 2)
                        }
                        (None, Some(value)) => pointer(value, false),
                        _ => false,
                    }
            })
    }
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
    pub(crate) fn requests_workspace_context(&self) -> bool {
        self.backend_requests_workspace_context()
            || self.code.as_ref().is_some_and(|code| {
                code.frontend.iter().any(|surface| {
                    surface
                        .permissions
                        .contains(&crate::plugin_schema::PluginPermission::WorkspaceContextRead)
                })
            })
    }

    pub(crate) fn backend_requests_workspace_context(&self) -> bool {
        self.code
            .as_ref()
            .and_then(|code| code.backend.as_ref())
            .is_some_and(|backend| {
                backend
                    .permissions
                    .contains(&crate::plugin_schema::PluginPermission::WorkspaceContextRead)
            })
    }

    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, Box<dyn Error>> {
        if bytes.len() > MAX_PLUGIN_MANIFEST_BYTES {
            return Err(invalid_manifest("Plugin manifest exceeds 64 KiB").into());
        }
        let manifest: Self = serde_json::from_slice(bytes)?;
        if !SUPPORTED_PLUGIN_MANIFEST_SCHEMAS.contains(&manifest.schema_version) {
            return Err(invalid_manifest("Unsupported plugin manifest schema version").into());
        }
        if !identifier(&manifest.id) || blank(&manifest.name) || blank(&manifest.description) {
            return Err(invalid_manifest("Invalid plugin identity").into());
        }
        if manifest
            .introduction
            .as_ref()
            .is_some_and(|introduction| manifest.schema_version < 8 || !introduction.valid())
        {
            return Err(invalid_manifest("Invalid plugin introduction").into());
        }
        if (manifest.icon.is_some() || manifest.translations.is_some())
            && manifest.schema_version < 8
            || manifest
                .icon
                .as_deref()
                .is_some_and(|icon| !crate::plugin_text::valid_icon(icon))
        {
            return Err(invalid_manifest("Invalid plugin icon").into());
        }
        if manifest.capabilities.is_empty()
            || manifest.capabilities.iter().any(|(id, capability)| {
                !identifier(id) || blank(&capability.name) || blank(&capability.description)
            })
        {
            return Err(invalid_manifest("Invalid plugin capabilities").into());
        }
        let mut setting_ids = HashSet::new();
        if let Some(settings) = &manifest.settings
            && (manifest.schema_version < 2
                || settings.len() > 64
                || settings.iter().any(|setting| {
                    !identifier(&setting.id)
                        || !setting_ids.insert(&setting.id)
                        || blank(&setting.label)
                        || blank(&setting.description)
                }))
        {
            return Err(invalid_manifest("Invalid or duplicate plugin setting declaration").into());
        }
        if let Some(code) = &manifest.code {
            use crate::plugin_schema::valid_code_path;
            let mut surfaces = HashSet::new();
            if manifest.schema_version < 3
                || (code.backend.is_none() && code.frontend.is_empty())
                || code.backend.as_ref().is_some_and(|entry| {
                    !valid_code_path(&entry.component, ".wasm")
                        || entry.permissions.len() > 1
                        || (manifest.schema_version < 4 && !entry.permissions.is_empty())
                })
                || code.frontend.len() > 16
                || code.frontend.iter().any(|surface| {
                    !identifier(&surface.id)
                        || !surfaces.insert(&surface.id)
                        || surface.permissions.len() > 1
                        || (!surface.permissions.is_empty()
                            && (manifest.schema_version < 5
                                || surface.placement == crate::PluginSurfacePlacement::Settings))
                        || blank(&surface.title)
                        || !valid_code_path(&surface.document, ".html")
                })
            {
                return Err(invalid_manifest("Invalid plugin code declaration").into());
            }
        }
        if let Some(actions) = &manifest.actions {
            let mut ids = HashSet::new();
            if manifest.schema_version < 6
                || manifest
                    .code
                    .as_ref()
                    .is_none_or(|code| code.backend.is_none())
                || actions.is_empty()
                || actions.len() > 64
                || actions.iter().any(|action| {
                    !identifier(&action.id)
                        || !ids.insert(&action.id)
                        || blank(&action.label)
                        || blank(&action.description)
                })
            {
                return Err(invalid_manifest("Invalid plugin action declaration").into());
            }
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
                    .all(|arg| identifier(arg))
                || provider
                    .cli
                    .structured_run
                    .as_ref()
                    .is_some_and(|run| manifest.schema_version < 7 || !run.valid())
                || provider.cli.image_run.as_ref().is_some_and(|run| {
                    manifest.schema_version < 7
                        || !run.valid()
                        || !manifest.capabilities.contains_key("image-generation")
                }))
        {
            return Err(invalid_manifest("Invalid plugin agent provider descriptor").into());
        }
        if !crate::plugin_text::valid_translations(&manifest) {
            return Err(invalid_manifest("Invalid plugin translations").into());
        }
        Ok(manifest)
    }
}

pub(crate) struct BundledSource {
    manifest: &'static [u8],
    pub transcription: Option<&'static [u8]>,
    pub transcription_timed: Option<&'static [u8]>,
    pub speech_synthesis: Option<&'static [u8]>,
    pub image_generation: Option<&'static [u8]>,
    package: Option<&'static [u8]>,
    release: Option<&'static str>,
}

include!(concat!(env!("OUT_DIR"), "/bundled_plugins.rs"));

pub(crate) fn bundled_manifests() -> &'static [PluginManifest] {
    static MANIFESTS: OnceLock<Vec<PluginManifest>> = OnceLock::new();
    MANIFESTS.get_or_init(|| {
        BUNDLED
            .iter()
            .map(|source| {
                PluginManifest::parse(source.manifest).expect("valid bundled plugin manifest")
            })
            .collect()
    })
}

type BundledPackage = (
    &'static PluginManifest,
    &'static [u8],
    &'static PluginPackageInfo,
);

pub(crate) fn bundled_package(id: &str) -> Result<Option<BundledPackage>, &'static str> {
    static PACKAGES: OnceLock<Vec<Result<Option<PluginPackageInfo>, &'static str>>> =
        OnceLock::new();
    let Some(index) = bundled_manifests()
        .iter()
        .position(|manifest| manifest.id == id)
    else {
        return Ok(None);
    };
    let packages = PACKAGES.get_or_init(|| {
        BUNDLED
            .iter()
            .map(|source| {
                source
                    .package
                    .map(|bytes| {
                        use crate::plugin_package::{package_sha256, read_package_manifest};

                        let manifest = read_package_manifest(bytes)
                            .map_err(|_| "Invalid bundled plugin ZIP")?;
                        let packaged: serde_json::Value = serde_json::from_slice(&manifest)
                            .map_err(|_| "Invalid bundled package manifest")?;
                        let checkout: serde_json::Value =
                            serde_json::from_slice(source.manifest)
                                .map_err(|_| "Invalid bundled checkout manifest")?;
                        if packaged != checkout {
                            return Err("Bundled package manifest differs from the checkout");
                        }
                        Ok(PluginPackageInfo {
                            sha256: package_sha256(bytes)
                                .map_err(|_| "Could not hash bundled package")?,
                            byte_length: bytes.len() as i64,
                            release: source
                                .release
                                .map(serde_json::from_str)
                                .transpose()
                                .map_err(|_| "Invalid bundled release provenance")?,
                        })
                    })
                    .transpose()
            })
            .collect()
    });
    match &packages[index] {
        Ok(Some(info)) => Ok(BUNDLED[index]
            .package
            .map(|bytes| (&bundled_manifests()[index], bytes, info))),
        Ok(None) => Ok(None),
        Err(error) => Err(error),
    }
}

impl WorkspaceStore {
    pub(crate) fn plugin_manifest(&self, id: &str) -> Result<PluginManifest, Box<dyn Error>> {
        if let Some(plugin) = bundled_manifests().iter().find(|plugin| plugin.id == id) {
            return Ok(plugin.clone());
        }
        let bytes: Vec<u8> = self.0.query_row(
            "SELECT manifest FROM installed_plugin_manifests WHERE plugin_id = ?1",
            [id],
            |row| row.get(0),
        )?;
        PluginManifest::parse(&bytes)
    }

    pub fn install_plugin_manifest(&self, bytes: &[u8]) -> Result<CatalogPlugin, Box<dyn Error>> {
        self.register_plugin(bytes, None, None)
    }

    pub(crate) fn register_plugin(
        &self,
        bytes: &[u8],
        package: Option<(&PluginPackageInfo, &[u8])>,
        previous: Option<&crate::plugin_registry::InstalledRelease>,
    ) -> Result<CatalogPlugin, Box<dyn Error>> {
        let manifest = PluginManifest::parse(bytes)?;
        if manifest.code.is_some() && package.is_none() {
            return Err(invalid_manifest("Plugins with code require a ZIP package").into());
        }
        let (info, archive) = package.unzip();
        let source = info.and_then(|info| info.release.as_ref());
        if source.is_some_and(|source| source.plugin_id != manifest.id) {
            return Err(
                invalid_manifest("Package plugin ID differs from the registry entry").into(),
            );
        }
        if source.is_some_and(|source| source.manifest_schema_version != manifest.schema_version) {
            return Err(invalid_manifest(
                "Package manifest schema differs from the registry entry",
            )
            .into());
        }
        if bundled_manifests()
            .iter()
            .any(|plugin| plugin.id == manifest.id)
        {
            return Err(invalid_manifest("Plugin ID belongs to a bundled plugin").into());
        }
        let release = source.map(serde_json::to_string).transpose()?;
        if let Some(previous) = previous {
            let changed = self.0.execute(
                "UPDATE installed_plugin_manifests
                 SET manifest = ?2, package_sha256 = ?3, package_bytes = ?4, release_source = ?5
                 WHERE plugin_id = ?1 AND package_sha256 = ?6 AND release_source = ?7",
                rusqlite::params![
                    manifest.id,
                    bytes,
                    info.map(|info| &info.sha256),
                    archive,
                    release,
                    previous.sha256,
                    previous.raw_source
                ],
            )?;
            if changed == 0 {
                return Err(invalid_manifest("Installed plugin changed during update").into());
            }
        } else {
            let added = self.0.execute(
                "INSERT INTO installed_plugin_manifests (plugin_id, manifest, package_sha256, package_bytes, release_source)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(plugin_id) DO NOTHING",
                rusqlite::params![manifest.id, bytes, info.map(|info| &info.sha256), archive, release],
            )?;
            if added == 0 {
                return Err(invalid_manifest("Plugin ID is already installed").into());
            }
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
            .map(|manifest| {
                let package = bundled_package(&manifest.id)
                    .map_err(io::Error::other)?
                    .map(|(_, _, info)| info.clone());
                Ok(CatalogPlugin {
                    bundled: true,
                    manifest,
                    package,
                })
            })
            .collect::<io::Result<_>>()?;
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
            let manifest = PluginManifest::parse(&manifest)?;
            if !bundled_manifests()
                .iter()
                .any(|plugin| plugin.id == manifest.id)
            {
                plugins.push(CatalogPlugin {
                    bundled: false,
                    manifest,
                    package,
                });
            }
        }
        Ok(plugins)
    }

    pub fn remove_reviewed_plugin_package(
        &self,
        id: &str,
        package_sha256: &str,
    ) -> rusqlite::Result<bool> {
        self.0.execute(
            "DELETE FROM installed_plugin_manifests WHERE plugin_id = ?1 AND package_sha256 = ?2",
            [id, package_sha256],
        ).map(|deleted| deleted != 0)
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

use std::{fs::File, path::Path};

use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};

use crate::{
    RegistryRelease, THIRD_PARTY_PLUGIN_NOTICE, WorkspaceStore, fetch_plugin_registry,
    read_plugin_registry,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ActivationInput {
    plugin_id: String,
    active: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PluginInput {
    plugin_id: String,
    workspace_id: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SettingInput {
    plugin_id: String,
    workspace_id: Option<i64>,
    setting_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ValueInput {
    plugin_id: String,
    workspace_id: Option<i64>,
    setting_id: String,
    value: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PreviewInput {
    max_edge: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceInput {
    workspace_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContextGrantInput {
    workspace_id: i64,
    plugin_id: String,
    package_sha256: String,
    allowed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RepositoryInput {
    repository: String,
    package_sha256: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PackageInput {
    source: String,
    package_sha256: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RegistryPackageInput {
    catalog: String,
    source: String,
    plugin_id: String,
    version: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RegistryUrlInput {
    catalog_url: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryFileInput {
    catalog: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewedReleaseInput {
    catalog_url: String,
    plugin_id: String,
    repository: String,
    release: RegistryRelease,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RemovePackageInput {
    plugin_id: String,
    package_sha256: String,
}

pub(super) fn definitions() -> [Value; 21] {
    let workspace_id = json!({"type": "integer", "minimum": 1});
    let workspace = json!({"type": "integer", "minimum": 1, "description": "Omit for app scope."});
    let id = json!({"type": "string", "minLength": 1});
    let registry_package = json!({"type": "object", "properties": {
        "catalog": {"type": "string", "minLength": 1},
        "source": {"type": "string", "minLength": 1},
        "pluginId": id, "version": {"type": "string", "minLength": 1}
    }, "required": ["catalog", "source", "pluginId", "version"], "additionalProperties": false});
    let reviewed_release = json!({"type": "object", "properties": {
        "catalogUrl": {"type": "string", "minLength": 1}, "pluginId": id,
        "repository": {"type": "string", "minLength": 1},
        "release": {"type": "object", "properties": {
            "version": {"type": "string", "minLength": 1},
            "manifestSchemaVersion": {"type": "integer", "minimum": 1},
            "packageUrl": {"type": "string", "minLength": 1},
            "sha256": {"type": "string", "pattern": "^[0-9a-fA-F]{64}$"}
        }, "required": ["version", "manifestSchemaVersion", "packageUrl", "sha256"], "additionalProperties": false}
    }, "required": ["catalogUrl", "pluginId", "repository", "release"], "additionalProperties": false});
    [
        json!({"name": "get_plugin_settings", "description": "Read non-secret settings and their effective sources for a plugin.",
            "inputSchema": {"type": "object", "properties": {"pluginId": id, "workspaceId": workspace},
                "required": ["pluginId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "set_plugin_setting", "description": "Save a non-secret setting using the value type declared by the plugin.",
            "inputSchema": {"type": "object", "properties": {
                "pluginId": id, "workspaceId": workspace, "settingId": id,
                "value": {"type": ["boolean", "integer", "string"]}
            }, "required": ["pluginId", "settingId", "value"], "additionalProperties": false}}),
        json!({"name": "clear_plugin_setting", "description": "Clear a non-secret setting at this scope so it can inherit.",
            "inputSchema": {"type": "object", "properties": {"pluginId": id, "workspaceId": workspace, "settingId": id},
                "required": ["pluginId", "settingId"], "additionalProperties": false}}),
        json!({"name": "get_preview_settings", "description": "Read the app's recommended, overridden, and effective preview resolution limits.",
            "inputSchema": {"type": "object", "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "set_preview_max_edge", "description": "Set the app-wide preview long edge in pixels. Pass null to restore the recommendation.",
            "inputSchema": {"type": "object", "properties": {
                "maxEdge": {"type": ["integer", "null"], "minimum": 144, "maximum": 2160}
            }, "required": ["maxEdge"], "additionalProperties": false},
            "annotations": {"idempotentHint": true}}),
        json!({"name": "list_plugin_catalog", "description": "List bundled and installed plugins with declared capabilities, settings metadata, and package information.",
            "inputSchema": {"type": "object", "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "set_plugin_active", "description": "Turn an installed plugin on or off for the whole app. A plugin that is off keeps its settings but Arrumo does not run it: it leaves provider orders, actions, hooks and usage. Imported plugins start off. An agent in use by the app, a workspace or an unfinished video can't be turned off.",
            "inputSchema": {"type": "object", "properties": {
                "pluginId": id, "active": {"type": "boolean"}
            }, "required": ["pluginId", "active"], "additionalProperties": false},
            "annotations": {"idempotentHint": true}}),
        json!({"name": "get_provider_usage", "description": "Read how much of each keyed provider's allowance is used and left, with a level of ok, warning (20% or less left), danger (10% or less) or unknown.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": {"type": "integer", "minimum": 1}}, "required": ["workspaceId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true, "openWorldHint": true}}),
        json!({"name": "get_plugin_credential_status", "description": "Check whether a plugin credential is configured and where it comes from. Returns status only.",
            "inputSchema": {"type": "object", "properties": {"pluginId": id, "workspaceId": workspace, "settingId": id},
                "required": ["pluginId", "settingId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "clear_plugin_credential", "description": "Remove a plugin credential at the specified scope. A workspace can then inherit its app credential. Omit workspaceId to remove the app credential.",
            "inputSchema": {"type": "object", "properties": {"pluginId": id, "workspaceId": workspace, "settingId": id},
                "required": ["pluginId", "settingId"], "additionalProperties": false},
            "annotations": {"idempotentHint": true, "destructiveHint": true}}),
        json!({"name": "list_plugin_context_requests", "description": "List installed packages requesting workspace context and their current grant status.",
            "inputSchema": {"type": "object", "properties": {"workspaceId": workspace_id},
                "required": ["workspaceId"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "apply_reviewed_plugin_context_grant", "description": "Grant or revoke workspace context access after reviewing the specific plugin package. Use the digest from list_plugin_context_requests.",
            "inputSchema": {"type": "object", "properties": {
                "workspaceId": workspace_id, "pluginId": id,
                "packageSha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                "allowed": {"type": "boolean"}
            }, "required": ["workspaceId", "pluginId", "packageSha256", "allowed"], "additionalProperties": false},
            "annotations": {"idempotentHint": true}}),
        json!({"name": "install_plugin_zip", "description": format!("Install a reviewed local ZIP using its expected SHA-256 digest. {THIRD_PARTY_PLUGIN_NOTICE}"),
            "inputSchema": {"type": "object", "properties": {
                "source": {"type": "string", "minLength": 1},
                "packageSha256": {"type": "string", "pattern": "^[0-9a-fA-F]{64}$"}
            }, "required": ["source", "packageSha256"], "additionalProperties": false}}),
        json!({"name": "install_plugin_from_repository", "description": format!("Install the plugin package (plugin.zip, or the only ZIP) from the latest release of a public GitHub repository, after reviewing it, with its expected SHA-256 digest. {THIRD_PARTY_PLUGIN_NOTICE}"),
            "inputSchema": {"type": "object", "properties": {
                "repository": {"type": "string", "minLength": 1, "maxLength": 300, "description": "Like https://github.com/owner/plugin"},
                "packageSha256": {"type": "string", "pattern": "^[0-9a-fA-F]{64}$"}
            }, "required": ["repository", "packageSha256"], "additionalProperties": false}}),
        json!({"name": "install_plugin_release_from_snapshot", "description": format!("Install an exact plugin release using absolute paths to a reviewed local registry catalog and its ZIP. {THIRD_PARTY_PLUGIN_NOTICE}"),
            "inputSchema": registry_package}),
        json!({"name": "update_plugin_release_from_snapshot", "description": format!("Update an offline-installed plugin to a newer exact release using absolute paths to a reviewed local registry catalog and its ZIP. {THIRD_PARTY_PLUGIN_NOTICE}"),
            "inputSchema": registry_package}),
        json!({"name": "fetch_plugin_registry", "description": "Fetch and validate an HTTPS plugin registry catalog. Returns release metadata without installing packages.",
            "inputSchema": {"type": "object", "properties": {"catalogUrl": {"type": "string", "minLength": 1}},
                "required": ["catalogUrl"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "read_plugin_registry_snapshot", "description": "Read and validate a local registry catalog at an absolute path.",
            "inputSchema": {"type": "object", "properties": {"catalog": {"type": "string", "minLength": 1}},
                "required": ["catalog"], "additionalProperties": false},
            "annotations": {"readOnlyHint": true}}),
        json!({"name": "install_reviewed_plugin_release", "description": format!("Install an exact HTTPS release after reviewing its repository and release metadata. {THIRD_PARTY_PLUGIN_NOTICE}"),
            "inputSchema": reviewed_release}),
        json!({"name": "update_reviewed_plugin_release", "description": format!("Update an HTTPS-installed plugin to a newer reviewed release from the same registry and repository. {THIRD_PARTY_PLUGIN_NOTICE}"),
            "inputSchema": reviewed_release}),
        json!({"name": "remove_reviewed_plugin_package", "description": "Uninstall the installed package only if its digest matches the reviewed revision. Its saved settings and grants are removed.",
            "inputSchema": {"type": "object", "properties": {
                "pluginId": id, "packageSha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"}
            }, "required": ["pluginId", "packageSha256"], "additionalProperties": false},
            "annotations": {"destructiveHint": true, "idempotentHint": true}}),
    ]
}

pub(super) fn call(
    store: &WorkspaceStore,
    name: &str,
    arguments: &Map<String, Value>,
) -> Option<Result<Value, &'static str>> {
    Some(match name {
        "get_plugin_settings" => parse(arguments).and_then(|args: PluginInput| {
            store
                .plugin_settings(&args.plugin_id, args.workspace_id)
                .map(|settings| json!({"settings": settings}))
                .map_err(|_| "Could not read plugin settings")
        }),
        "set_plugin_setting" => parse(arguments).and_then(|args: ValueInput| {
            store
                .set_plugin_setting(&args.plugin_id, &args.setting_id, args.workspace_id, Some(&args.value))
                .map(|()| json!({"saved": true}))
                .map_err(|_| "Could not save plugin setting. Check its declaration, scope and value type")
        }),
        "clear_plugin_setting" => parse(arguments).and_then(|args: SettingInput| {
            store
                .set_plugin_setting(&args.plugin_id, &args.setting_id, args.workspace_id, None)
                .map(|()| json!({"saved": true}))
                .map_err(|_| "Could not clear plugin setting")
        }),
        "get_preview_settings" if arguments.is_empty() => store
            .preview_settings()
            .map(|settings| json!({"settings": settings}))
            .map_err(|_| "Could not read preview settings"),
        "get_preview_settings" => Err("Unexpected preview settings arguments"),
        "set_preview_max_edge" => parse(arguments).and_then(|args: PreviewInput| {
            let max_edge = serde_json::from_value::<Option<u32>>(args.max_edge)
                .map_err(|_| "Expected an integer preview edge or null")?;
            store
                .set_preview_max_edge(max_edge)
                .map(|()| json!({"saved": true}))
                .map_err(|_| "Could not save preview edge. Use 144 to 2160 pixels or null")
        }),
        "list_plugin_catalog" if arguments.is_empty() => store
            .plugin_catalog()
            .map(|plugins| json!({"plugins": plugins}))
            .map_err(|_| "Could not list plugin catalog"),
        "list_plugin_catalog" => Err("Unexpected plugin catalog arguments"),
        "set_plugin_active" => parse(arguments).and_then(|args: ActivationInput| {
            store
                .set_plugin_active(&args.plugin_id, args.active)
                .map(|()| json!({"pluginId": args.plugin_id, "active": args.active}))
                .map_err(|error| {
                    crate::plugin_activation::activation_refusal(error.as_ref())
                        .unwrap_or("Could not turn the plugin on or off")
                })
        }),
        "get_provider_usage" => serde_json::from_value::<UsageInput>(Value::Object(arguments.clone()))
            .map_err(|_| "Invalid usage arguments")
            .and_then(|args| {
                store
                    .provider_usage(args.workspace_id)
                    .map(|usage| json!({"providers": usage}))
                    .map_err(|_| "Could not read provider usage")
            }),
        "get_plugin_credential_status" => parse(arguments).and_then(|args: SettingInput| {
            store
                .plugin_credential_status(&args.plugin_id, &args.setting_id, args.workspace_id)
                .map(|status| json!({"status": status}))
                .map_err(|_| "Could not check credential status")
        }),
        "clear_plugin_credential" => parse(arguments).and_then(|args: SettingInput| {
            store
                .set_plugin_credential(&args.plugin_id, &args.setting_id, args.workspace_id, None)
                .map(|()| json!({"cleared": true}))
                .map_err(|_| "Could not clear plugin credential. Check the setting, scope and credential store")
        }),
        "list_plugin_context_requests" => parse(arguments).and_then(|args: WorkspaceInput| {
            store
                .plugin_context_requests(args.workspace_id)
                .map(|requests| json!({"requests": requests}))
                .map_err(|_| "Could not list workspace context requests")
        }),
        "apply_reviewed_plugin_context_grant" => parse(arguments).and_then(|args: ContextGrantInput| {
            store
                .apply_reviewed_plugin_context_grant(&args.plugin_id, args.workspace_id, &args.package_sha256, args.allowed)
                .map(|()| json!({"saved": true}))
                .map_err(|_| "Could not change context grant. Check workspace, package digest and requested permission")
        }),
        "install_plugin_zip" => parse(arguments).and_then(|args: PackageInput| {
            let archive = open_local_file(&args.source)?;
            store
                .install_verified_plugin_zip(archive, &args.package_sha256)
                .map(|plugin| json!({"plugin": plugin, "notice": THIRD_PARTY_PLUGIN_NOTICE}))
                .map_err(|_| "Could not install plugin ZIP. Check the package and expected digest")
        }),
        "install_plugin_from_repository" => parse(arguments).and_then(|args: RepositoryInput| {
            store
                .install_plugin_from_repository(&args.repository, Some(&args.package_sha256))
                .map(|plugin| json!({"plugin": plugin, "notice": THIRD_PARTY_PLUGIN_NOTICE}))
                .map_err(|_| "Could not install the plugin. Check the repository address, its latest release and the expected digest")
        }),
        "install_plugin_release_from_snapshot" | "update_plugin_release_from_snapshot" => {
            parse(arguments).and_then(|args: RegistryPackageInput| {
                let catalog = open_local_file(&args.catalog)?;
                let archive = open_local_file(&args.source)?;
                let result = if name == "update_plugin_release_from_snapshot" {
                    store.update_plugin_release_from_snapshot(catalog, &args.plugin_id, &args.version, archive)
                } else {
                    store.install_plugin_release(catalog, &args.plugin_id, &args.version, archive)
                };
                result
                    .map(|plugin| json!({"plugin": plugin, "notice": THIRD_PARTY_PLUGIN_NOTICE}))
                    .map_err(|_| "Could not register plugin release. Check the registry, version, and package")
            })
        }
        "fetch_plugin_registry" => parse(arguments).and_then(|args: RegistryUrlInput| {
            fetch_plugin_registry(&args.catalog_url)
                .map(|registry| json!({"registry": registry}))
                .map_err(|_| "Could not fetch HTTPS plugin registry")
        }),
        "read_plugin_registry_snapshot" => parse(arguments).and_then(|args: RegistryFileInput| {
            read_plugin_registry(open_local_file(&args.catalog)?)
                .map(|registry| json!({"registry": registry}))
                .map_err(|_| "Could not read plugin registry snapshot")
        }),
        "install_reviewed_plugin_release" | "update_reviewed_plugin_release" => {
            parse(arguments).and_then(|args: ReviewedReleaseInput| {
                let result = if name == "update_reviewed_plugin_release" {
                    store.update_reviewed_plugin_release(&args.catalog_url, &args.plugin_id, &args.repository, &args.release)
                } else {
                    store.download_reviewed_plugin_release(&args.catalog_url, &args.plugin_id, &args.release.version, &args.repository, &args.release)
                };
                result
                    .map(|plugin| json!({"plugin": plugin, "notice": THIRD_PARTY_PLUGIN_NOTICE}))
                    .map_err(|_| "Could not register reviewed release. Check the registry, repository, metadata and installed version")
            })
        }
        "remove_reviewed_plugin_package" => parse(arguments).and_then(|args: RemovePackageInput| {
            store
                .remove_reviewed_plugin_package(&args.plugin_id, &args.package_sha256)
                .map(|removed| json!({"removed": removed}))
                .map_err(|_| "Could not remove plugin package")
        }),
        _ => return None,
    })
}

fn parse<T: DeserializeOwned>(arguments: &Map<String, Value>) -> Result<T, &'static str> {
    serde_json::from_value(Value::Object(arguments.clone()))
        .map_err(|_| "Invalid settings arguments")
}

fn open_local_file(source: &str) -> Result<File, &'static str> {
    let path = Path::new(source);
    if !path.is_absolute() {
        return Err("Source must be an absolute local path");
    }
    if !path.metadata().is_ok_and(|metadata| metadata.is_file()) {
        return Err("Source must be a regular file");
    }
    File::open(path).map_err(|_| "Could not open local source")
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UsageInput {
    workspace_id: i64,
}

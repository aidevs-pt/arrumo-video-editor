use std::{
    env,
    error::Error,
    ffi::OsStr,
    fs::File,
    io::{self, IsTerminal, Read},
    path::Path,
};

use arrumo_workspaces::{
    BrandAssetRole, MAX_PLUGIN_MANIFEST_BYTES, RunControl, THIRD_PARTY_PLUGIN_NOTICE,
    WorkspaceStore, asset_root,
};

fn utf8(value: &OsStr) -> io::Result<&str> {
    value
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Arguments must be valid UTF-8"))
}

fn plugin_workspace(value: &OsStr) -> Result<Option<i64>, Box<dyn Error>> {
    match utf8(value)? {
        "app" => Ok(None),
        id => Ok(Some(id.parse()?)),
    }
}

fn read_plugin_invocation_input() -> Result<serde_json::Value, Box<dyn Error>> {
    let limit = arrumo_workspaces::MAX_PLUGIN_IO_BYTES;
    let mut input = Vec::new();
    io::stdin().take(limit as u64 + 1).read_to_end(&mut input)?;
    if input.len() > limit {
        return Err(
            io::Error::new(io::ErrorKind::InvalidInput, "Plugin input exceeds 64 KiB").into(),
        );
    }
    serde_json::from_slice(&input).map_err(|_| {
        io::Error::new(io::ErrorKind::InvalidInput, "Expected JSON plugin input").into()
    })
}

fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    match arguments.as_slice() {
        [database, command, port] if command == "a2a" => {
            if io::stdin().is_terminal() {
                return Err("Supply the A2A bearer token on redirected stdin".into());
            }
            let port = utf8(port)?.parse()?;
            let mut token = zeroize::Zeroizing::new(Vec::new());
            io::stdin().lock().take(259).read_to_end(&mut token)?;
            if token.last() == Some(&b'\n') {
                token.pop();
                if token.last() == Some(&b'\r') {
                    token.pop();
                }
            }
            let database = Path::new(database);
            let mut store = WorkspaceStore::open(database)?;
            arrumo_workspaces::serve_a2a(&mut store, database, port, token, io::stdout().lock())
                .map_err(io::Error::other)?;
        }
        [database, command] if command == "mcp" => {
            let mut store = WorkspaceStore::open(Path::new(database))?;
            arrumo_workspaces::serve_mcp(
                &mut store,
                Path::new(database),
                io::stdin().lock(),
                io::stdout().lock(),
            )?;
        }
        [database, command] if command == "list" => {
            let workspaces = WorkspaceStore::open(Path::new(database))?.list()?;
            println!("{}", serde_json::to_string(&workspaces)?);
        }
        [database, command] if command == "plugin-catalog" => {
            let plugins = WorkspaceStore::open(Path::new(database))?.plugin_catalog()?;
            println!("{}", serde_json::to_string(&plugins)?);
        }
        [database, command, workspace, wav] if command == "transcribe-wav" => {
            let transcript = WorkspaceStore::open(Path::new(database))?
                .transcribe_preferred_wav(utf8(workspace)?.parse()?, File::open(wav)?)?;
            println!("{}", serde_json::to_string(&transcript)?);
        }
        [database, command, workspace, provider, wav] if command == "transcribe-wav" => {
            let transcript = WorkspaceStore::open(Path::new(database))?.transcribe_wav(
                utf8(workspace)?.parse()?,
                utf8(provider)?,
                File::open(wav)?,
            )?;
            println!("{}", serde_json::to_string(&transcript)?);
        }
        [database, command, id, scope, setting] if command == "plugin-credential-status" => {
            let status = WorkspaceStore::open(Path::new(database))?.plugin_credential_status(
                utf8(id)?,
                utf8(setting)?,
                plugin_workspace(scope)?,
            )?;
            println!("{}", serde_json::to_string(&status)?);
        }
        [database, command, id, scope, setting] if command == "set-plugin-credential" => {
            if io::stdin().is_terminal() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Pass the credential through redirected stdin",
                )
                .into());
            }
            let mut secret = zeroize::Zeroizing::new(Vec::new());
            io::stdin()
                .take(arrumo_workspaces::MAX_PLUGIN_SECRET_BYTES as u64 + 3)
                .read_to_end(&mut secret)?;
            if secret.last() == Some(&b'\n') {
                secret.pop();
                if secret.last() == Some(&b'\r') {
                    secret.pop();
                }
            }
            WorkspaceStore::open(Path::new(database))?.set_plugin_credential(
                utf8(id)?,
                utf8(setting)?,
                plugin_workspace(scope)?,
                Some(secret),
            )?;
        }
        [database, command, id, scope, setting] if command == "clear-plugin-credential" => {
            WorkspaceStore::open(Path::new(database))?.set_plugin_credential(
                utf8(id)?,
                utf8(setting)?,
                plugin_workspace(scope)?,
                None,
            )?;
        }
        [database, command, capability, scope] if command == "provider-preferences" => {
            let result = WorkspaceStore::open(Path::new(database))?
                .provider_preferences(utf8(capability)?, plugin_workspace(scope)?)?;
            println!("{}", serde_json::to_string(&result)?);
        }
        [database, command, capability, scope, order] if command == "set-provider-preferences" => {
            let order: Vec<String> = serde_json::from_str(utf8(order)?)?;
            WorkspaceStore::open(Path::new(database))?.set_provider_preferences(
                utf8(capability)?,
                plugin_workspace(scope)?,
                Some(&order),
            )?;
        }
        [database, command, capability, scope] if command == "clear-provider-preferences" => {
            WorkspaceStore::open(Path::new(database))?.set_provider_preferences(
                utf8(capability)?,
                plugin_workspace(scope)?,
                None,
            )?;
        }
        [database, command, id, scope] if command == "plugin-settings" => {
            let workspace_id = plugin_workspace(scope)?;
            let settings = WorkspaceStore::open(Path::new(database))?
                .plugin_settings(utf8(id)?, workspace_id)?;
            println!("{}", serde_json::to_string(&settings)?);
        }
        [database, command, id, scope, setting, value] if command == "set-plugin-setting" => {
            let value = serde_json::from_str(utf8(value)?).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "Expected a JSON setting value")
            })?;
            WorkspaceStore::open(Path::new(database))?.set_plugin_setting(
                utf8(id)?,
                utf8(setting)?,
                plugin_workspace(scope)?,
                Some(&value),
            )?;
        }
        [database, command, id, scope, setting] if command == "clear-plugin-setting" => {
            WorkspaceStore::open(Path::new(database))?.set_plugin_setting(
                utf8(id)?,
                utf8(setting)?,
                plugin_workspace(scope)?,
                None,
            )?;
        }
        [database, command] if command == "plugin-actions" => {
            let actions = WorkspaceStore::open(Path::new(database))?.plugin_actions()?;
            println!("{}", serde_json::to_string(&actions)?);
        }
        [database, command, plugin_id, action_id, scope] if command == "invoke-plugin-action" => {
            let input = read_plugin_invocation_input()?;
            let result = WorkspaceStore::open(Path::new(database))?.invoke_plugin_action(
                utf8(plugin_id)?,
                utf8(action_id)?,
                plugin_workspace(scope)?,
                &input,
            )?;
            println!("{}", serde_json::to_string(&result)?);
        }
        [database, command, plugin_id, scope @ ..]
            if command == "invoke-plugin" && scope.len() <= 1 =>
        {
            let input = read_plugin_invocation_input()?;
            let store = WorkspaceStore::open(Path::new(database))?;
            let result = if let Some(workspace) = scope.first() {
                store.invoke_plugin_in_workspace(
                    utf8(plugin_id)?,
                    utf8(workspace)?.parse()?,
                    &input,
                )?
            } else {
                store.invoke_plugin_component(utf8(plugin_id)?, &input)?
            };
            println!("{}", serde_json::to_string(&result)?);
        }
        [database, command, plugin_id, workspace] if command == "plugin-context-grant" => {
            let allowed = WorkspaceStore::open(Path::new(database))?
                .plugin_context_granted(utf8(plugin_id)?, utf8(workspace)?.parse()?)?;
            println!("{allowed}");
        }
        [database, command, plugin_id, workspace]
            if command == "grant-plugin-context" || command == "revoke-plugin-context" =>
        {
            WorkspaceStore::open(Path::new(database))?.set_plugin_context_grant(
                utf8(plugin_id)?,
                utf8(workspace)?.parse()?,
                command == "grant-plugin-context",
            )?;
        }
        [database, command, manifest] if command == "install-plugin-manifest" => {
            eprintln!("{THIRD_PARTY_PLUGIN_NOTICE}");
            let mut bytes = Vec::new();
            File::open(manifest)?
                .take(MAX_PLUGIN_MANIFEST_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            let installed =
                WorkspaceStore::open(Path::new(database))?.install_plugin_manifest(&bytes)?;
            println!("{}", serde_json::to_string(&installed)?);
        }
        [database, command, archive] if command == "install-plugin-zip" => {
            eprintln!("{THIRD_PARTY_PLUGIN_NOTICE}");
            let installed = WorkspaceStore::open(Path::new(database))?
                .install_plugin_zip(File::open(archive)?)?;
            println!("{}", serde_json::to_string(&installed)?);
        }
        [database, command, archive, flag, checksum]
            if command == "install-plugin-zip" && flag == "--sha256" =>
        {
            eprintln!("{THIRD_PARTY_PLUGIN_NOTICE}");
            let installed = WorkspaceStore::open(Path::new(database))?
                .install_verified_plugin_zip(File::open(archive)?, utf8(checksum)?)?;
            println!("{}", serde_json::to_string(&installed)?);
        }
        [database, command, registry, id, version, archive]
            if command == "install-plugin-release" =>
        {
            eprintln!("{THIRD_PARTY_PLUGIN_NOTICE}");
            let installed = WorkspaceStore::open(Path::new(database))?.install_plugin_release(
                File::open(registry)?,
                utf8(id)?,
                utf8(version)?,
                File::open(archive)?,
            )?;
            println!("{}", serde_json::to_string(&installed)?);
        }
        [_database, command, source] if command == "read-plugin-registry" => {
            let catalog = arrumo_workspaces::read_plugin_registry(File::open(source)?)?;
            println!("{}", serde_json::to_string(&catalog)?);
        }
        [_database, command, url] if command == "fetch-plugin-registry" => {
            let catalog = arrumo_workspaces::fetch_plugin_registry(utf8(url)?)?;
            println!("{}", serde_json::to_string(&catalog)?);
        }
        [database, command, registry, id, version] if command == "download-plugin-release" => {
            eprintln!("{THIRD_PARTY_PLUGIN_NOTICE}");
            let installed = WorkspaceStore::open(Path::new(database))?.download_plugin_release(
                utf8(registry)?,
                utf8(id)?,
                utf8(version)?,
            )?;
            println!("{}", serde_json::to_string(&installed)?);
        }
        [database, command, registry, id, version, archive]
            if command == "update-plugin-release-from-snapshot" =>
        {
            eprintln!("{THIRD_PARTY_PLUGIN_NOTICE}");
            let updated = WorkspaceStore::open(Path::new(database))?
                .update_plugin_release_from_snapshot(
                    File::open(registry)?,
                    utf8(id)?,
                    utf8(version)?,
                    File::open(archive)?,
                )?;
            println!("{}", serde_json::to_string(&updated)?);
        }
        [database, command, id, version] if command == "update-plugin-release" => {
            eprintln!("{THIRD_PARTY_PLUGIN_NOTICE}");
            let updated = WorkspaceStore::open(Path::new(database))?
                .update_plugin_release(utf8(id)?, utf8(version)?)?;
            println!("{}", serde_json::to_string(&updated)?);
        }
        [database, command, id] if command == "remove-plugin-manifest" => {
            let removed = WorkspaceStore::open(Path::new(database))?
                .remove_installed_plugin_manifest(utf8(id)?)?;
            println!("{removed}");
        }
        [_database, command] if command == "agent-plugins" => {
            println!(
                "{}",
                serde_json::to_string(arrumo_workspaces::agent_plugins())?
            );
        }
        [_database, command, provider] if command == "agent-login-status" => {
            let provider = utf8(provider)?;
            println!(
                "{}",
                serde_json::to_string(&arrumo_workspaces::check_agent_login(provider))?
            );
        }
        [database, command, scope] if command == "agent-settings" => {
            let workspace_id = match utf8(scope)? {
                "app" => None,
                workspace => Some(workspace.parse()?),
            };
            let settings =
                WorkspaceStore::open(Path::new(database))?.agent_settings(workspace_id)?;
            println!("{}", serde_json::to_string(&settings)?);
        }
        [database, command, scope, provider] if command == "set-agent-provider" => {
            let workspace_id = match utf8(scope)? {
                "app" => None,
                workspace => Some(workspace.parse()?),
            };
            let provider = match utf8(provider)? {
                "unset" => None,
                provider => Some(provider),
            };
            let store = WorkspaceStore::open(Path::new(database))?;
            store.set_agent_provider(workspace_id, provider)?;
            println!(
                "{}",
                serde_json::to_string(&store.agent_settings(workspace_id)?)?
            );
        }
        [database, command] if command == "preview-settings" => {
            let settings = WorkspaceStore::open(Path::new(database))?.preview_settings()?;
            println!("{}", serde_json::to_string(&settings)?);
        }
        [database, command, value] if command == "set-preview-max-edge" => {
            let store = WorkspaceStore::open(Path::new(database))?;
            let max_edge = match utf8(value)? {
                "auto" => None,
                value => Some(value.parse()?),
            };
            store.set_preview_max_edge(max_edge)?;
            println!("{}", serde_json::to_string(&store.preview_settings()?)?);
        }
        [database, command, name] if command == "create" => {
            let name = utf8(name)?;
            let workspace = WorkspaceStore::open(Path::new(database))?.create(name)?;
            println!("{}", serde_json::to_string(&workspace)?);
        }
        [database, command, workspace_id] if command == "profiles" => {
            let profiles = WorkspaceStore::open(Path::new(database))?
                .list_profiles(utf8(workspace_id)?.parse()?)?;
            println!("{}", serde_json::to_string(&profiles)?);
        }
        [database, command, workspace_id, name, options @ ..]
            if command == "create-profile" && options.len() <= 2 =>
        {
            let guidance = options.first().map(|value| utf8(value)).transpose()?;
            let color = options.get(1).map(|value| utf8(value)).transpose()?;
            let profile = WorkspaceStore::open(Path::new(database))?.create_profile(
                utf8(workspace_id)?.parse()?,
                utf8(name)?,
                guidance.unwrap_or_default(),
                color,
            )?;
            println!("{}", serde_json::to_string(&profile)?);
        }
        [
            database,
            command,
            workspace_id,
            profile_id,
            name,
            guidance,
            options @ ..,
        ] if command == "update-profile" && options.len() <= 1 => {
            let color = options.first().map(|value| utf8(value)).transpose()?;
            let profile = WorkspaceStore::open(Path::new(database))?.update_profile(
                utf8(workspace_id)?.parse()?,
                utf8(profile_id)?.parse()?,
                utf8(name)?,
                utf8(guidance)?,
                color,
            )?;
            println!("{}", serde_json::to_string(&profile)?);
        }
        [database, command, workspace] if command == "formats" => {
            let formats = WorkspaceStore::open(Path::new(database))?
                .list_output_formats(utf8(workspace)?.parse()?)?;
            println!("{}", serde_json::to_string(&formats)?);
        }
        [database, command, workspace, settings] if command == "create-format" => {
            let format = WorkspaceStore::open(Path::new(database))?.create_output_format(
                utf8(workspace)?.parse()?,
                &serde_json::from_str(utf8(settings)?)?,
            )?;
            println!("{}", serde_json::to_string(&format)?);
        }
        [database, command, workspace, format, settings] if command == "update-format" => {
            let format = WorkspaceStore::open(Path::new(database))?.update_output_format(
                utf8(workspace)?.parse()?,
                utf8(format)?.parse()?,
                &serde_json::from_str(utf8(settings)?)?,
            )?;
            println!("{}", serde_json::to_string(&format)?);
        }
        [database, command, workspace, format] if command == "remove-format" => {
            let removed = WorkspaceStore::open(Path::new(database))?
                .remove_output_format(utf8(workspace)?.parse()?, utf8(format)?.parse()?)?;
            println!("{}", serde_json::to_string(&removed)?);
        }
        [database, command, workspace, profile] if command == "profile-formats" => {
            let formats = WorkspaceStore::open(Path::new(database))?
                .list_profile_formats(utf8(workspace)?.parse()?, utf8(profile)?.parse()?)?;
            println!("{}", serde_json::to_string(&formats)?);
        }
        [database, command, workspace, profile, format, overrides]
            if command == "set-profile-format" =>
        {
            WorkspaceStore::open(Path::new(database))?.set_profile_format_overrides(
                utf8(workspace)?.parse()?,
                utf8(profile)?.parse()?,
                utf8(format)?.parse()?,
                &serde_json::from_str(utf8(overrides)?)?,
            )?;
            println!("null");
        }
        [database, command, workspace, format] if command == "format-delivery" => {
            let delivery = WorkspaceStore::open(Path::new(database))?
                .format_delivery(utf8(workspace)?.parse()?, utf8(format)?.parse()?)?;
            println!("{}", serde_json::to_string(&delivery)?);
        }
        [database, command, workspace, format, settings] if command == "set-format-delivery" => {
            WorkspaceStore::open(Path::new(database))?.set_format_delivery(
                utf8(workspace)?.parse()?,
                utf8(format)?.parse()?,
                &serde_json::from_str(utf8(settings)?)?,
            )?;
            println!("null");
        }
        [database, command, workspace, profile, format] if command == "profile-delivery" => {
            let delivery = WorkspaceStore::open(Path::new(database))?.profile_delivery(
                utf8(workspace)?.parse()?,
                utf8(profile)?.parse()?,
                utf8(format)?.parse()?,
            )?;
            println!("{}", serde_json::to_string(&delivery)?);
        }
        [database, command, workspace, profile, format, overrides]
            if command == "set-profile-delivery" =>
        {
            WorkspaceStore::open(Path::new(database))?.set_profile_delivery(
                utf8(workspace)?.parse()?,
                utf8(profile)?.parse()?,
                utf8(format)?.parse()?,
                &serde_json::from_str(utf8(overrides)?)?,
            )?;
            println!("null");
        }
        [database, command, workspace, profile] if command == "motion-library" => {
            let profile = match utf8(profile)? {
                "workspace" => None,
                value => Some(value.parse()?),
            };
            let items = WorkspaceStore::open(Path::new(database))?
                .list_motion_library(utf8(workspace)?.parse()?, profile)?;
            println!("{}", serde_json::to_string(&items)?);
        }
        [database, command, workspace, settings] if command == "create-motion-item" => {
            let item = WorkspaceStore::open(Path::new(database))?.create_motion_library_item(
                utf8(workspace)?.parse()?,
                &serde_json::from_str(utf8(settings)?)?,
            )?;
            println!("{}", serde_json::to_string(&item)?);
        }
        [database, command, workspace, item, settings] if command == "update-motion-item" => {
            let item = WorkspaceStore::open(Path::new(database))?.update_motion_library_item(
                utf8(workspace)?.parse()?,
                utf8(item)?.parse()?,
                &serde_json::from_str(utf8(settings)?)?,
            )?;
            println!("{}", serde_json::to_string(&item)?);
        }
        [database, command, workspace, item, title] if command == "duplicate-motion-item" => {
            let item = WorkspaceStore::open(Path::new(database))?.duplicate_motion_library_item(
                utf8(workspace)?.parse()?,
                utf8(item)?.parse()?,
                utf8(title)?,
            )?;
            println!("{}", serde_json::to_string(&item)?);
        }
        [database, command, workspace, item, title] if command == "delete-motion-item" => {
            let deleted = WorkspaceStore::open(Path::new(database))?.delete_motion_library_item(
                utf8(workspace)?.parse()?,
                utf8(item)?.parse()?,
                utf8(title)?,
            )?;
            println!("{deleted}");
        }
        [database, command, workspace, item] if command == "motion-item-shares" => {
            let shares = WorkspaceStore::open(Path::new(database))?
                .motion_library_shares(utf8(workspace)?.parse()?, utf8(item)?.parse()?)?;
            println!("{}", serde_json::to_string(&shares)?);
        }
        [database, command, workspace, item, profiles] if command == "set-motion-item-shares" => {
            WorkspaceStore::open(Path::new(database))?.set_motion_library_shares(
                utf8(workspace)?.parse()?,
                utf8(item)?.parse()?,
                &serde_json::from_str::<Vec<i64>>(utf8(profiles)?)?,
            )?;
        }
        [database, command, workspace] if command == "assets" => {
            let assets = WorkspaceStore::open(Path::new(database))?
                .list_assets(utf8(workspace)?.parse()?)?;
            println!("{}", serde_json::to_string(&assets)?);
        }
        [database, command, workspace, source]
            if command == "import-asset" || command == "import-thumbnail-reference" =>
        {
            let database = Path::new(database);
            let mut store = WorkspaceStore::open(database)?;
            let workspace_id = utf8(workspace)?.parse()?;
            let root = asset_root(database);
            let asset = if command == "import-thumbnail-reference" {
                store.import_thumbnail_reference(&root, workspace_id, Path::new(source))
            } else {
                store.import_asset(&root, workspace_id, Path::new(source))
            }
            .map_err(io::Error::other)?;
            println!("{}", serde_json::to_string(&asset)?);
        }
        [database, command, workspace, asset, source] if command == "replace-asset" => {
            let database = Path::new(database);
            let replacement = WorkspaceStore::open(database)?
                .replace_asset(
                    &asset_root(database),
                    utf8(workspace)?.parse()?,
                    utf8(asset)?.parse()?,
                    Path::new(source),
                )
                .map_err(io::Error::other)?;
            println!("{}", serde_json::to_string(&replacement)?);
        }
        [database, command, workspace, asset, name] if command == "remove-asset" => {
            let database = Path::new(database);
            let removed = WorkspaceStore::open(database)?.remove_workspace_asset(
                database,
                utf8(workspace)?.parse()?,
                utf8(asset)?.parse()?,
                utf8(name)?,
            )?;
            println!("{removed}");
        }
        [database, command, workspace, asset] if command == "asset-organization" => {
            let settings = WorkspaceStore::open(Path::new(database))?
                .asset_organization(utf8(workspace)?.parse()?, utf8(asset)?.parse()?)?;
            println!("{}", serde_json::to_string(&settings)?);
        }
        [database, command, workspace, asset, settings] if command == "set-asset-organization" => {
            let settings = WorkspaceStore::open(Path::new(database))?.set_asset_organization(
                utf8(workspace)?.parse()?,
                utf8(asset)?.parse()?,
                serde_json::from_str(utf8(settings)?)?,
            )?;
            println!("{}", serde_json::to_string(&settings)?);
        }
        [database, command, workspace, ids, notes] if command == "set-batch-asset-notes" => {
            let ids: Vec<i64> = serde_json::from_str(utf8(ids)?)?;
            let assets = WorkspaceStore::open(Path::new(database))?.set_batch_asset_notes(
                utf8(workspace)?.parse()?,
                &ids,
                utf8(notes)?,
            )?;
            println!("{}", serde_json::to_string(&assets)?);
        }
        [database, command, workspace, asset, notes] if command == "set-asset-notes" => {
            let asset = WorkspaceStore::open(Path::new(database))?.set_asset_notes(
                utf8(workspace)?.parse()?,
                utf8(asset)?.parse()?,
                utf8(notes)?,
            )?;
            println!("{}", serde_json::to_string(&asset)?);
        }
        [database, command, workspace] if command == "context" => {
            let context = WorkspaceStore::open(Path::new(database))?
                .workspace_context(utf8(workspace)?.parse()?)?;
            println!("{}", serde_json::to_string(&context)?);
        }
        [database, command, workspace, markdown] if command == "set-context" => {
            WorkspaceStore::open(Path::new(database))?
                .set_workspace_context(utf8(workspace)?.parse()?, utf8(markdown)?)?;
            println!("null");
        }
        [database, command, workspace] if command == "feedback" => {
            let cards = WorkspaceStore::open(Path::new(database))?
                .list_workspace_feedback(utf8(workspace)?.parse()?)?;
            println!("{}", serde_json::to_string(&cards)?);
        }
        [database, command, workspace, title, markdown] if command == "add-feedback" => {
            let card = WorkspaceStore::open(Path::new(database))?.add_workspace_feedback(
                utf8(workspace)?.parse()?,
                utf8(title)?,
                utf8(markdown)?,
            )?;
            println!("{}", serde_json::to_string(&card)?);
        }
        [database, command, workspace, id, title, markdown] if command == "update-feedback" => {
            let card = WorkspaceStore::open(Path::new(database))?.update_workspace_feedback(
                utf8(workspace)?.parse()?,
                utf8(id)?.parse()?,
                utf8(title)?,
                utf8(markdown)?,
            )?;
            println!("{}", serde_json::to_string(&card)?);
        }
        [database, command, workspace] if command == "brand-colors" => {
            let colors = WorkspaceStore::open(Path::new(database))?
                .brand_colors(utf8(workspace)?.parse()?)?;
            println!("{}", serde_json::to_string(&colors)?);
        }
        [database, command, workspace, colors] if command == "set-brand-colors" => {
            let colors: Vec<String> = serde_json::from_str(utf8(colors)?)?;
            let stored = WorkspaceStore::open(Path::new(database))?
                .set_brand_colors(utf8(workspace)?.parse()?, &colors)?;
            println!("{}", serde_json::to_string(&stored)?);
        }
        [database, command, workspace] if command == "brand-assets" => {
            let assets = WorkspaceStore::open(Path::new(database))?
                .list_brand_assets(utf8(workspace)?.parse()?)?;
            println!("{}", serde_json::to_string(&assets)?);
        }
        [database, command, workspace, asset, role]
            if command == "attach-brand-asset" || command == "detach-brand-asset" =>
        {
            let role = match utf8(role)? {
                "identity" => BrandAssetRole::Identity,
                "style" => BrandAssetRole::Style,
                _ => return Err("Role must be identity or style".into()),
            };
            let store = WorkspaceStore::open(Path::new(database))?;
            let workspace_id = utf8(workspace)?.parse()?;
            let asset_id = utf8(asset)?.parse()?;
            if command == "attach-brand-asset" {
                store.attach_brand_asset(workspace_id, asset_id, role)?;
                println!("null");
            } else {
                println!(
                    "{}",
                    store.detach_brand_asset(workspace_id, asset_id, role)?
                );
            }
        }
        [database, command, workspace] if command == "drafts" => {
            let drafts = WorkspaceStore::open(Path::new(database))?
                .list_video_drafts(utf8(workspace)?.parse()?)?;
            println!("{}", serde_json::to_string(&drafts)?);
        }
        [database, command, workspace, settings] if command == "create-draft" => {
            let draft = WorkspaceStore::open(Path::new(database))?.create_video_draft(
                utf8(workspace)?.parse()?,
                serde_json::from_str(utf8(settings)?)?,
            )?;
            println!("{}", serde_json::to_string(&draft)?);
        }
        [database, command, workspace, draft, settings] if command == "update-draft" => {
            let draft = WorkspaceStore::open(Path::new(database))?.update_video_draft(
                utf8(workspace)?.parse()?,
                utf8(draft)?.parse()?,
                serde_json::from_str(utf8(settings)?)?,
            )?;
            println!("{}", serde_json::to_string(&draft)?);
        }
        [database, command, workspace, draft, title] if command == "delete-draft" => {
            let deleted = WorkspaceStore::open(Path::new(database))?.delete_video_draft(
                utf8(workspace)?.parse()?,
                utf8(draft)?.parse()?,
                utf8(title)?,
            )?;
            println!("{deleted}");
        }
        [database, command, workspace] if command == "generation-runs" => {
            let runs = WorkspaceStore::open(Path::new(database))?
                .list_generation_runs(utf8(workspace)?.parse()?)?;
            println!("{}", serde_json::to_string(&runs)?);
        }
        [database, command, workspace, draft, key, options] if command == "enqueue-draft" => {
            let run = WorkspaceStore::open(Path::new(database))?.enqueue_video_draft(
                utf8(workspace)?.parse()?,
                utf8(draft)?.parse()?,
                utf8(key)?,
                &serde_json::from_str(utf8(options)?)?,
            )?;
            println!("{}", serde_json::to_string(&run)?);
        }
        [database, command, workspace, source, key, mode] if command == "replay-generation" => {
            let run = WorkspaceStore::open(Path::new(database))?.replay_generation(
                utf8(workspace)?.parse()?,
                utf8(source)?.parse()?,
                utf8(key)?,
                serde_json::from_value(serde_json::json!(utf8(mode)?))?,
            )?;
            println!("{}", serde_json::to_string(&run)?);
        }
        [database, command, workspace] if command == "claim-generation" => {
            let claim = WorkspaceStore::open(Path::new(database))?
                .claim_generation(utf8(workspace)?.parse()?)?;
            println!("{}", serde_json::to_string(&claim)?);
        }
        [database, command, workspace, run, attempt, checkpoint]
            if command == "checkpoint-generation" =>
        {
            let expires = WorkspaceStore::open(Path::new(database))?.checkpoint_generation(
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(attempt)?.parse()?,
                &serde_json::from_str(utf8(checkpoint)?)?,
            )?;
            println!("{expires}");
        }
        [database, command, workspace, run, action] if command == "control-generation" => {
            let action = match utf8(action)? {
                "pause" => RunControl::Pause,
                "resume" => RunControl::Resume,
                "retry" => RunControl::Retry,
                "cancel" => RunControl::Cancel,
                _ => return Err("Action must be pause, resume, retry, or cancel".into()),
            };
            let run = WorkspaceStore::open(Path::new(database))?.control_generation(
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                action,
            )?;
            println!("{}", serde_json::to_string(&run)?);
        }
        [database, command, workspace, after] if command == "generation-notifications" => {
            let after = i64::try_from(utf8(after)?.parse::<u64>()?)?;
            let notifications = WorkspaceStore::open(Path::new(database))?
                .generation_notifications(utf8(workspace)?.parse()?, after)?;
            println!("{}", serde_json::to_string(&notifications)?);
        }
        [database, command, workspace, run] if command == "read-completion-notification" => {
            let changed = WorkspaceStore::open(Path::new(database))?
                .read_completion_notification(utf8(workspace)?.parse()?, utf8(run)?.parse()?)?;
            println!("{changed}");
        }
        [database, command, workspace, run] if command == "generation-activity" => {
            let events = WorkspaceStore::open(Path::new(database))?
                .generation_activity(utf8(workspace)?.parse()?, utf8(run)?.parse()?)?;
            println!("{}", serde_json::to_string(&events)?);
        }
        [database, command, workspace, run, attempt, detail] if command == "fail-generation" => {
            let run = WorkspaceStore::open(Path::new(database))?.fail_generation(
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(attempt)?.parse()?,
                utf8(detail)?,
            )?;
            println!("{}", serde_json::to_string(&run)?);
        }
        [database, command, workspace, run, attempt] if command == "prepare-generation-plan" => {
            let plan = WorkspaceStore::open(Path::new(database))?.prepare_generation_plan(
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(attempt)?.parse()?,
            )?;
            println!("{}", serde_json::to_string(&plan)?);
        }
        [database, command, workspace, run] if command == "generation-plan" => {
            let plan = WorkspaceStore::open(Path::new(database))?
                .generation_plan(utf8(workspace)?.parse()?, utf8(run)?.parse()?)?;
            println!("{}", serde_json::to_string(&plan)?);
        }
        [database, command, workspace, run] if command == "production-progress" => {
            let progress = WorkspaceStore::open(Path::new(database))?
                .production_progress(utf8(workspace)?.parse()?, utf8(run)?.parse()?)?;
            println!("{}", serde_json::to_string(&progress)?);
        }
        [database, command, workspace, run, attempt, stage, result]
            if command == "submit-stage" =>
        {
            let stage = serde_json::from_value(serde_json::json!(utf8(stage)?))?;
            let id = WorkspaceStore::open(Path::new(database))?.submit_production_result(
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(attempt)?.parse()?,
                stage,
                &serde_json::from_str(utf8(result)?)?,
            )?;
            println!("{id}");
        }
        [database, command, workspace, run, result, decision, notes]
            if command == "review-stage" =>
        {
            let decision = serde_json::from_value(serde_json::json!(utf8(decision)?))?;
            WorkspaceStore::open(Path::new(database))?.review_production_result(
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(result)?.parse()?,
                decision,
                utf8(notes)?,
            )?;
            println!("null");
        }
        [database, command, workspace, run, edit @ ..]
            if command == "process-media-stage" && edit.len() <= 1 =>
        {
            let path = Path::new(database);
            let edit = edit
                .first()
                .map(|value| Ok::<_, Box<dyn Error>>(serde_json::from_str(utf8(value)?)?))
                .transpose()?;
            let work = WorkspaceStore::open(path)?.process_media_stage(
                path,
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                edit.as_ref(),
            )?;
            println!("{}", serde_json::to_string(&work)?);
        }
        [database, command, workspace, run, format, date] if command == "set-output-schedule" => {
            let date = utf8(date)?;
            WorkspaceStore::open(Path::new(database))?.set_output_schedule(
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(format)?.parse()?,
                (date != "clear").then_some(date),
            )?;
            println!("null");
        }
        [database, command, workspace, start, end, profile @ ..]
            if command == "scheduled-outputs" && profile.len() <= 1 =>
        {
            let profile = profile
                .first()
                .map(|value| -> Result<i64, Box<dyn Error>> { Ok(utf8(value)?.parse()?) })
                .transpose()?;
            let outputs = WorkspaceStore::open(Path::new(database))?.scheduled_outputs(
                utf8(workspace)?.parse()?,
                profile,
                utf8(start)?,
                utf8(end)?,
            )?;
            println!("{}", serde_json::to_string(&outputs)?);
        }
        [database, command, workspace] if command == "recover-delivery-cleanup" => {
            let database = Path::new(database);
            let report = WorkspaceStore::open(database)?
                .recover_delivery_cleanup(database, utf8(workspace)?.parse()?)?;
            println!("{}", serde_json::to_string(&report)?);
        }
        [database, command, workspace, run, title] if command == "delete-generation" => {
            let database = Path::new(database);
            let deleted = WorkspaceStore::open(database)?.delete_generation(
                database,
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(title)?,
            )?;
            println!("{deleted}");
        }
        [database, command, workspace, run, title] if command == "delete-completed-video" => {
            let database = Path::new(database);
            let deleted = WorkspaceStore::open(database)?.delete_completed_generation(
                database,
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(title)?,
            )?;
            println!("{deleted}");
        }
        [database, command, workspace, run] if command == "completed-video" => {
            let video = WorkspaceStore::open(Path::new(database))?
                .completed_video(utf8(workspace)?.parse()?, utf8(run)?.parse()?)?;
            println!("{}", serde_json::to_string(&video)?);
        }
        [database, command, workspace, run, target, markdown]
            if command == "add-completed-video-feedback" =>
        {
            let result_id = match utf8(target)? {
                "run" => None,
                id => Some(id.parse()?),
            };
            let feedback = WorkspaceStore::open(Path::new(database))?
                .add_completed_video_feedback(
                    utf8(workspace)?.parse()?,
                    utf8(run)?.parse()?,
                    result_id,
                    utf8(markdown)?,
                )?;
            println!("{}", serde_json::to_string(&feedback)?);
        }
        [database, command, workspace, run, title] if command == "rename-completed-video" => {
            let title = WorkspaceStore::open(Path::new(database))?.rename_completed_video(
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(title)?,
            )?;
            println!("{}", serde_json::to_string(&title)?);
        }
        [database, command, workspace, run, destination] if command == "export-delivery" => {
            let database = Path::new(database);
            WorkspaceStore::open(database)?.export_generation_delivery(
                database,
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                Path::new(destination),
            )?;
            println!("null");
        }
        [database, command, workspace, run, format, source]
            if command == "import-delivery-subtitles" =>
        {
            let database = Path::new(database);
            let cues = WorkspaceStore::open(database)?.import_delivery_subtitles(
                database,
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(format)?.parse()?,
                Path::new(source),
            )?;
            println!("{cues}");
        }
        [database, command, workspace, run, attempt] if command == "verify-delivery" => {
            let path = Path::new(database);
            let run = WorkspaceStore::open(path)?.verify_generation_delivery(
                path,
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(attempt)?.parse()?,
            )?;
            println!("{}", serde_json::to_string(&run)?);
        }
        [database, command, workspace, run, attempt, edit] if command == "render-preview" => {
            let path = Path::new(database);
            let result = WorkspaceStore::open(path)?.render_preview_videos(
                path,
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(attempt)?.parse()?,
                &serde_json::from_str(utf8(edit)?)?,
            )?;
            println!("{result}");
        }
        [database, command, workspace, run, attempt] if command == "render-primary" => {
            let path = Path::new(database);
            let result = WorkspaceStore::open(path)?.render_primary_videos(
                path,
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(attempt)?.parse()?,
            )?;
            println!("{result}");
        }
        _ => {
            return Err(concat!(
                "Usage: arrumo-workspaces <database> ",
                "list | create <name> | profiles <workspace-id> | ",
                "create-profile <workspace-id> <name> [guidance] [color] | ",
                "update-profile <workspace-id> <profile-id> <name> <guidance> [color] | ",
                "formats <workspace-id> | create-format <workspace-id> <settings-json> | ",
                "update-format <workspace-id> <format-id> <settings-json> | ",
                "remove-format <workspace-id> <format-id> | ",
                "profile-formats <workspace-id> <profile-id> | ",
                "set-profile-format <workspace-id> <profile-id> <format-id> <overrides-json> | ",
                "format-delivery <workspace-id> <format-id> | ",
                "set-format-delivery <workspace-id> <format-id> <settings-json> | ",
                "profile-delivery <workspace-id> <profile-id> <format-id> | ",
                "set-profile-delivery <workspace-id> <profile-id> <format-id> <overrides-json> | ",
                "assets <workspace-id> | import-asset <workspace-id> <source-path> | ",
                "import-thumbnail-reference <workspace-id> <source-path> | ",
                "remove-asset <workspace-id> <asset-id> <confirmed-name> | asset-organization <workspace-id> <asset-id> | set-asset-organization <workspace-id> <asset-id> <settings-json> | ",
                "set-batch-asset-notes <workspace-id> <asset-ids-json> <markdown> | set-asset-notes <workspace-id> <asset-id> <notes> | ",
                "context <workspace-id> | set-context <workspace-id> <markdown> | ",
                "feedback <workspace-id> | add-feedback <workspace-id> <title> <markdown> | ",
                "update-feedback <workspace-id> <feedback-id> <title> <markdown> | ",
                "brand-colors <workspace-id> | set-brand-colors <workspace-id> <colors-json> | ",
                "brand-assets <workspace-id> | ",
                "attach-brand-asset <workspace-id> <asset-id> <identity|style> | ",
                "detach-brand-asset <workspace-id> <asset-id> <identity|style> | ",
                "drafts <workspace-id> | create-draft <workspace-id> <settings-json> | ",
                "update-draft <workspace-id> <draft-id> <settings-json> | ",
                "delete-draft <workspace-id> <draft-id> <confirmed-title> | generation-runs <workspace-id> | ",
                "enqueue-draft <workspace-id> <draft-id> <request-key> <options-json> | ",
                "replay-generation <workspace-id> <source-run-id> <request-key> <captured|current> | ",
                "claim-generation <workspace-id> | checkpoint-generation <workspace-id> <run-id> <attempt> <json> | ",
                "control-generation <workspace-id> <run-id> <pause|resume|retry|cancel> | ",
                "generation-activity <workspace-id> <run-id> | generation-notifications <workspace-id> <after-id> | ",
                "read-completion-notification <workspace-id> <run-id> | ",
                "fail-generation <workspace-id> <run-id> <attempt> <detail> | ",
                "prepare-generation-plan <workspace-id> <run-id> <attempt> | generation-plan <workspace-id> <run-id> | ",
                "production-progress <workspace-id> <run-id> | ",
                "submit-stage <workspace-id> <run-id> <attempt> <stage> <result-json> | ",
                "review-stage <workspace-id> <run-id> <result-id> <approved|rejected|revise> <notes> | ",
                "plugin-catalog | install-plugin-manifest <manifest.json> | install-plugin-zip <package.zip> [--sha256 <digest>] | remove-plugin-manifest <plugin-id> | ",
                "install-plugin-release <catalog.json> <plugin-id> <version> <package.zip> | ",
                "read-plugin-registry <catalog.json> | fetch-plugin-registry <catalog-url> | ",
                "download-plugin-release <catalog-url> <plugin-id> <version> | update-plugin-release <plugin-id> <version> | ",
                "update-plugin-release-from-snapshot <catalog.json> <plugin-id> <version> <package.zip> | ",
                "plugin-settings <plugin-id> <app|workspace-id> | set-plugin-setting <plugin-id> <app|workspace-id> <setting-id> <value-json> | clear-plugin-setting <plugin-id> <app|workspace-id> <setting-id> | ",
                "plugin-credential-status | set-plugin-credential | clear-plugin-credential <plugin-id> <app|workspace-id> <setting-id> | ",
                "provider-preferences <capability> <app|workspace-id> | set-provider-preferences <capability> <app|workspace-id> <order-json> | clear-provider-preferences <capability> <app|workspace-id> | ",
                "transcribe-wav <workspace-id> [plugin-id] <audio.wav> | ",
                "a2a <port> (bearer token on stdin) | mcp | plugin-actions | invoke-plugin-action <plugin-id> <action-id> <app|workspace-id> (JSON on stdin) | ",
                "invoke-plugin <plugin-id> [workspace-id] (JSON on stdin) | plugin-context-grant <plugin-id> <workspace-id> | grant-plugin-context <plugin-id> <workspace-id> | revoke-plugin-context <plugin-id> <workspace-id> | ",
                "agent-plugins | agent-login-status <plugin-id> | ",
                "agent-settings <app|workspace-id> | set-agent-provider <app|workspace-id> <plugin-id|unset> | ",
                "preview-settings | set-preview-max-edge <pixels|auto> | ",
                "process-media-stage <workspace-id> <run-id> [preview-edit-json] | ",
                "scheduled-outputs <workspace-id> <start-date> <end-date> [profile-id] | set-output-schedule <workspace-id> <run-id> <format-id> <date|clear> | ",
                "delete-generation <workspace-id> <run-id> <confirmed-title> | recover-delivery-cleanup <workspace-id> | delete-completed-video <workspace-id> <run-id> <confirmed-title> | completed-video <workspace-id> <run-id> | ",
                "add-completed-video-feedback <workspace-id> <run-id> <run|result-id> <markdown> | ",
                "rename-completed-video <workspace-id> <run-id> <title> | ",
                "import-delivery-subtitles <workspace-id> <run-id> <format-id> <source-srt> | ",
                "export-delivery <workspace-id> <run-id> <destination-zip> | ",
                "render-preview <workspace-id> <run-id> <attempt> <edit-json> | ",
                "render-primary <workspace-id> <run-id> <attempt> | verify-delivery <workspace-id> <run-id> <attempt>"
            )
            .into());
        }
    }
    Ok(())
}

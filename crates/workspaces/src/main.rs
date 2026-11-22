use std::{
    env,
    error::Error,
    ffi::OsStr,
    fs::File,
    io::{self, IsTerminal, Read},
    path::Path,
};

use arrumo_workspaces::{
    BrandAssetRole, MAX_PLUGIN_MANIFEST_BYTES, MotionWorkerPaths, RunControl,
    THIRD_PARTY_PLUGIN_NOTICE, WorkspaceStore, asset_root,
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
        [database, command, plugin, digest, output] if command == "export-plugin-package" => {
            WorkspaceStore::open(Path::new(database))?.export_reviewed_plugin_package(
                utf8(plugin)?,
                utf8(digest)?,
                Path::new(output),
            )?;
        }
        [database, command] if command == "plugin-catalog" => {
            let plugins = WorkspaceStore::open(Path::new(database))?.plugin_catalog()?;
            println!("{}", serde_json::to_string(&plugins)?);
        }
        [database, command, workspace, provider] if command == "speech-provider-status" => {
            let status = WorkspaceStore::open(Path::new(database))?
                .speech_provider_status(utf8(workspace)?.parse()?, utf8(provider)?)?;
            println!("{}", serde_json::to_string(&status)?);
        }
        [database, command, workspace, provider, voice, output]
            if command == "synthesize-speech" =>
        {
            let mut text = String::new();
            io::stdin().take(40_001).read_to_string(&mut text)?;
            if text.len() > 40_000 {
                return Err(io::Error::other("Speech input exceeds 40,000 bytes").into());
            }
            WorkspaceStore::open(Path::new(database))?.synthesize_speech_to_file(
                utf8(workspace)?.parse()?,
                utf8(provider)?,
                utf8(voice)?,
                &text,
                Path::new(output),
            )?;
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
        [database, command] if command == "queue-plugin-action-job" => {
            let request = serde_json::from_value(read_plugin_invocation_input()?)?;
            let job =
                WorkspaceStore::open(Path::new(database))?.queue_plugin_action_job(&request)?;
            println!("{}", serde_json::to_string(&job)?);
        }
        [database, command, scope] if command == "list-plugin-action-jobs" => {
            let jobs = WorkspaceStore::open(Path::new(database))?
                .list_plugin_action_jobs(plugin_workspace(scope)?)?;
            println!("{}", serde_json::to_string(&jobs)?);
        }
        [database, command, scope, id]
            if command == "plugin-action-job" || command == "process-plugin-action-job" =>
        {
            let store = WorkspaceStore::open(Path::new(database))?;
            let workspace = plugin_workspace(scope)?;
            let id = utf8(id)?.parse()?;
            let job = if command == "plugin-action-job" {
                store.plugin_action_job(workspace, id)?
            } else {
                store.process_plugin_action_job(workspace, id)?
            };
            println!("{}", serde_json::to_string(&job)?);
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
        [
            database,
            command,
            id,
            digest,
            worker,
            output,
            toolchain @ ..,
        ] if command == "run-installed-motion-renderer" && toolchain.len() <= 1 => {
            let digest = utf8(digest)?;
            let result = WorkspaceStore::open(Path::new(database))?.render_installed_motion(
                utf8(id)?,
                digest,
                &MotionWorkerPaths {
                    worker: Path::new(worker),
                    output: Path::new(output),
                    toolchain: toolchain.first().map(Path::new),
                },
            )?;
            use std::io::Write;
            io::stdout().write_all(&result)?;
        }
        [
            database,
            command,
            workspace,
            run,
            id,
            digest,
            worker,
            output,
            toolchain @ ..,
        ] if command == "render-run-motion" && toolchain.len() <= 1 => {
            let database = Path::new(database);
            let workspace_id = utf8(workspace)?.parse()?;
            let run_id = utf8(run)?.parse()?;
            let id = utf8(id)?;
            let digest = utf8(digest)?;
            let store = WorkspaceStore::open(database)?;
            let result_id = store.render_run_motion(
                database,
                workspace_id,
                run_id,
                id,
                digest,
                &MotionWorkerPaths {
                    worker: Path::new(worker),
                    output: Path::new(output),
                    toolchain: toolchain.first().map(Path::new),
                },
            )?;
            println!("{result_id}");
        }
        [database, command, id, state] if command == "set-plugin-active" => {
            let active = match utf8(state)? {
                "on" => true,
                "off" => false,
                _ => return Err("Use on or off".into()),
            };
            WorkspaceStore::open(Path::new(database))?.set_plugin_active(utf8(id)?, active)?;
            println!(
                "{}",
                serde_json::json!({"pluginId": utf8(id)?, "active": active})
            );
        }
        [database, command, id] if command == "remove-plugin-manifest" => {
            let removed = WorkspaceStore::open(Path::new(database))?
                .remove_installed_plugin_manifest(utf8(id)?)?;
            println!("{removed}");
        }
        [database, command] if command == "agent-plugins" => {
            let plugins = WorkspaceStore::open(Path::new(database))?.active_agent_plugins()?;
            println!("{}", serde_json::to_string(&plugins)?);
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
        [database, command, workspace_id, profile_id, description]
            if command == "set-profile-description" =>
        {
            let profile = WorkspaceStore::open(Path::new(database))?.set_profile_description(
                utf8(workspace_id)?.parse()?,
                utf8(profile_id)?.parse()?,
                utf8(description)?,
            )?;
            println!("{}", serde_json::to_string(&profile)?);
        }
        [
            database,
            command,
            workspace_id,
            profile_id,
            format_id,
            markdown,
        ] if command == "set-profile-format-context" => {
            WorkspaceStore::open(Path::new(database))?.set_profile_format_context(
                utf8(workspace_id)?.parse()?,
                utf8(profile_id)?.parse()?,
                utf8(format_id)?.parse()?,
                utf8(markdown)?,
            )?;
        }
        [database, command, workspace_id, profile_id] if command == "profile-assets" => {
            let assets = WorkspaceStore::open(Path::new(database))?
                .list_profile_assets(utf8(workspace_id)?.parse()?, utf8(profile_id)?.parse()?)?;
            println!("{}", serde_json::to_string(&assets)?);
        }
        [
            database,
            command,
            workspace_id,
            asset_id,
            profile_id,
            scope @ ..,
        ] if (command == "share-asset" || command == "unshare-asset") && scope.len() <= 1 => {
            let format_id: Option<i64> = match scope.first() {
                Some(value) => Some(utf8(value)?.parse()?),
                None => None,
            };
            let mut store = WorkspaceStore::open(Path::new(database))?;
            let (workspace_id, asset_id, profile_id) = (
                utf8(workspace_id)?.parse()?,
                utf8(asset_id)?.parse()?,
                utf8(profile_id)?.parse()?,
            );
            if command == "share-asset" {
                store.share_asset_with_profile(workspace_id, asset_id, profile_id, format_id)?;
            } else if !store.unshare_asset_from_profile(
                workspace_id,
                asset_id,
                profile_id,
                format_id,
            )? {
                return Err("No matching share".into());
            }
            println!(
                "{}",
                serde_json::to_string(&store.list_asset_profile_shares(workspace_id, asset_id)?)?
            );
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
        [database, command, workspace, run, result, index] if command == "save-motion-result" => {
            let item = WorkspaceStore::open(Path::new(database))?.save_motion_result(
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(result)?.parse()?,
                utf8(index)?.parse()?,
            )?;
            println!("{}", serde_json::to_string(&item)?);
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
        [database, command, workspace, sources, rest @ ..]
            if command == "draft-context" && rest.len() <= 1 =>
        {
            let sources: Vec<String> = serde_json::from_str(utf8(sources)?)?;
            let notes = rest
                .first()
                .map(|notes| utf8(notes))
                .transpose()?
                .unwrap_or("");
            let draft = WorkspaceStore::open(Path::new(database))?.draft_workspace_context(
                utf8(workspace)?.parse()?,
                &sources,
                notes,
            )?;
            println!("{}", serde_json::to_string(&draft)?);
        }
        [database, command, workspace, import] if command == "import-context" => {
            let import: arrumo_workspaces::ContextImport = serde_json::from_str(utf8(import)?)?;
            let database = Path::new(database);
            let outcome = WorkspaceStore::open(database)?.apply_context_import(
                database,
                utf8(workspace)?.parse()?,
                &import,
            )?;
            println!("{}", serde_json::to_string(&outcome)?);
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
        [database, command, workspace, id] if command == "remove-feedback" => {
            let removed = WorkspaceStore::open(Path::new(database))?
                .remove_workspace_feedback(utf8(workspace)?.parse()?, utf8(id)?.parse()?)?;
            if !removed {
                return Err("No matching feedback card".into());
            }
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
        [database, command] if command == "sweep-agent-sessions" => {
            let removed = WorkspaceStore::open(Path::new(database))?.sweep_agent_sessions()?;
            println!("{removed}");
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
                "stop" => RunControl::Stop,
                _ => return Err("Action must be pause, resume, retry, cancel or stop".into()),
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
        [database, command, workspace, request] if command == "generate-motion-item" => {
            let item = WorkspaceStore::open(Path::new(database))?.generate_motion_library_item(
                Path::new(database),
                utf8(workspace)?.parse()?,
                None,
                utf8(request)?,
                None,
            )?;
            println!("{}", serde_json::to_string(&item)?);
        }
        [database, command, workspace] if command == "agent-logs" => {
            let logs = WorkspaceStore::open(Path::new(database))?.agent_logs(
                utf8(workspace)?.parse()?,
                None,
                100,
            )?;
            println!("{}", serde_json::to_string(&logs)?);
        }
        [database, command, workspace, run] if command == "agent-logs" => {
            let logs = WorkspaceStore::open(Path::new(database))?.agent_logs(
                utf8(workspace)?.parse()?,
                Some(utf8(run)?.parse()?),
                100,
            )?;
            println!("{}", serde_json::to_string(&logs)?);
        }
        [database, command, workspace, log] if command == "agent-log" => {
            let log = WorkspaceStore::open(Path::new(database))?
                .agent_log(utf8(workspace)?.parse()?, utf8(log)?.parse()?)?
                .ok_or_else(|| io::Error::other("No such agent log in this workspace"))?;
            println!("{}", serde_json::to_string_pretty(&log)?);
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
        [database, command, workspace, run, attempt, artifacts]
            if command == "submit-motion-artifacts" =>
        {
            let database = Path::new(database);
            let id = WorkspaceStore::open(database)?
                .submit_motion_artifacts(
                    database,
                    utf8(workspace)?.parse()?,
                    utf8(run)?.parse()?,
                    utf8(attempt)?.parse()?,
                    &serde_json::from_str::<Vec<arrumo_workspaces::MotionArtifact>>(utf8(
                        artifacts,
                    )?)?,
                )
                .map_err(io::Error::other)?;
            println!("{id}");
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
        [
            database,
            command,
            workspace,
            run,
            result,
            decision,
            notes,
            selection @ ..,
        ] if command == "review-stage" && selection.len() <= 1 => {
            let decision = serde_json::from_value(serde_json::json!(utf8(decision)?))?;
            let indexes = selection
                .first()
                .map(|value| {
                    serde_json::from_str::<Vec<usize>>(utf8(value)?).map_err(io::Error::other)
                })
                .transpose()?;
            WorkspaceStore::open(Path::new(database))?.review_production_result_with_motion(
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(result)?.parse()?,
                decision,
                utf8(notes)?,
                indexes.as_deref(),
            )?;
            println!("null");
        }
        [database, command, workspace, run, result, decisions, notes]
            if command == "review-preview-formats" =>
        {
            WorkspaceStore::open(Path::new(database))?.review_preview_formats(
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(result)?.parse()?,
                &serde_json::from_str::<Vec<arrumo_workspaces::PreviewFormatDecision>>(utf8(
                    decisions,
                )?)?,
                utf8(notes)?,
            )?;
            println!("null");
        }
        [database, command, workspace, run, result, decisions, notes]
            if command == "review-supplemental-items" =>
        {
            WorkspaceStore::open(Path::new(database))?.review_supplemental_items(
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(result)?.parse()?,
                &serde_json::from_str(utf8(decisions)?)?,
                utf8(notes)?,
            )?;
            println!("null");
        }
        [database, command, workspace, run, format, revision @ ..]
            if command == "completed-subtitles" && revision.len() <= 1 =>
        {
            let database = Path::new(database);
            let revision = revision
                .first()
                .map(|value| -> Result<i64, Box<dyn Error>> { Ok(utf8(value)?.parse()?) })
                .transpose()?;
            let result = WorkspaceStore::open(database)?.completed_subtitles(
                database,
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(format)?.parse()?,
                revision,
            )?;
            println!("{}", serde_json::to_string(&result)?);
        }
        [database, command, workspace, run, format, revision, source]
            if command == "revise-completed-subtitles" =>
        {
            let content = arrumo_workspaces::read_subtitle_file(Path::new(source))?;
            let revision = WorkspaceStore::open(Path::new(database))?.revise_completed_subtitles(
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(format)?.parse()?,
                utf8(revision)?.parse()?,
                &content,
            )?;
            println!("{revision}");
        }
        [database, command, workspace, run, format]
            if command == "completed-subtitle-languages" =>
        {
            let languages = WorkspaceStore::open(Path::new(database))?
                .completed_subtitle_languages(
                    utf8(workspace)?.parse()?,
                    utf8(run)?.parse()?,
                    utf8(format)?.parse()?,
                )?;
            println!("{}", serde_json::to_string(&languages)?);
        }
        [
            database,
            command,
            workspace,
            run,
            format,
            language,
            revision @ ..,
        ] if command == "completed-subtitle-track" && revision.len() <= 1 => {
            let database = Path::new(database);
            let revision = revision
                .first()
                .map(|value| -> Result<i64, Box<dyn Error>> { Ok(utf8(value)?.parse()?) })
                .transpose()?;
            let result = WorkspaceStore::open(database)?.completed_subtitle_track(
                database,
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(format)?.parse()?,
                Some(utf8(language)?),
                revision,
            )?;
            println!("{}", serde_json::to_string(&result)?);
        }
        [
            database,
            command,
            workspace,
            run,
            format,
            language,
            revision,
            source,
        ] if command == "revise-completed-subtitle-track" => {
            let content = arrumo_workspaces::read_subtitle_file(Path::new(source))?;
            let revision = WorkspaceStore::open(Path::new(database))?
                .revise_completed_subtitle_track(
                    utf8(workspace)?.parse()?,
                    utf8(run)?.parse()?,
                    utf8(format)?.parse()?,
                    Some(utf8(language)?),
                    utf8(revision)?.parse()?,
                    &content,
                )?;
            println!("{revision}");
        }
        [database, command, destination] if command == "export-backup" => {
            let summary = WorkspaceStore::open(Path::new(database))?
                .export_backup(Path::new(database), Path::new(destination))?;
            println!("{}", serde_json::to_string(&summary)?);
        }
        [_, command, archive] if command == "verify-backup" => {
            let manifest = arrumo_workspaces::verify_backup(Path::new(archive))?;
            println!(
                "{}",
                serde_json::to_string(&arrumo_workspaces::BackupSummary::from(&manifest))?
            );
        }
        [database, command, archive] if command == "stage-backup-import" => {
            let summary =
                arrumo_workspaces::stage_backup_import(Path::new(database), Path::new(archive))?;
            println!("{}", serde_json::to_string(&summary)?);
        }
        [database, command] if command == "cancel-backup-import" => {
            arrumo_workspaces::cancel_backup_import(Path::new(database))?;
        }
        [database, command] if command == "apply-backup-import" => {
            let kept = arrumo_workspaces::apply_pending_import(Path::new(database))?;
            println!(
                "{}",
                serde_json::json!({"applied": kept.is_some(), "previousData": kept})
            );
        }
        [database, command, destination] if command == "snapshot-database" => {
            let snapshot = WorkspaceStore::open(Path::new(database))?
                .snapshot_database(Path::new(destination))?;
            println!("{}", serde_json::to_string(&snapshot)?);
        }
        [database, command] if command == "watch-plugin-action-queue" => {
            let store = WorkspaceStore::open(Path::new(database))?;
            loop {
                match store.process_next_plugin_action_job() {
                    Ok(Some(_)) => {}
                    Ok(None) => std::thread::sleep(std::time::Duration::from_millis(500)),
                    Err(_) => {
                        eprintln!("Arrumo could not process the plugin action queue.");
                        std::thread::sleep(std::time::Duration::from_secs(2));
                    }
                }
            }
        }
        [database, command] if command == "watch-delivery-queue" => {
            let database = Path::new(database);
            let store = WorkspaceStore::open(database)?;
            loop {
                let verified = store.process_next_ready_delivery(database).unwrap_or_else(|_| {
                    eprintln!("Arrumo could not verify a delivery. Check the run activity and output files.");
                    false
                });
                let transcribed = store
                    .process_next_automatic_subtitles(database)
                    .unwrap_or_else(|_| {
                        eprintln!("Arrumo could not process the automatic subtitle queue.");
                        false
                    });
                if !verified && !transcribed {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
            }
        }
        [database, command] if command == "watch-motion-queue" => {
            let database = Path::new(database);
            let worker = arrumo_workspaces::configured_motion_worker(None).ok_or_else(|| {
                io::Error::other("The host motion renderer worker is unavailable")
            })?;
            let store = WorkspaceStore::open(database)?;
            if arrumo_workspaces::recover_motion_scratch(database).is_err() {
                eprintln!("Arrumo could not retry pending motion scratch cleanup.");
            }
            loop {
                match store.process_next_motion_stage(database, &worker) {
                    Ok(Some(result)) => println!("{result}"),
                    Ok(None) => std::thread::sleep(std::time::Duration::from_secs(2)),
                    Err(_) => {
                        eprintln!(
                            "Arrumo could not dispatch the selected motion stage. Check the run and renderer installation."
                        );
                        std::thread::sleep(std::time::Duration::from_secs(2));
                    }
                }
            }
        }
        [database, command, workspace, run] if command == "process-motion-stage" => {
            let database = Path::new(database);
            let worker = arrumo_workspaces::configured_motion_worker(None).ok_or_else(|| {
                io::Error::other("The host motion renderer worker is unavailable")
            })?;
            let result = WorkspaceStore::open(database)?.process_queued_motion_stage(
                database,
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                &worker,
            )?;
            println!("{}", serde_json::to_string(&result)?);
        }
        [database, command, workspace, aspect, order, rest @ ..]
            if command == "generate-image" && rest.len() <= 1 =>
        {
            let references: Vec<i64> = rest
                .first()
                .map(|ids| {
                    serde_json::from_str(utf8(ids)?)
                        .map_err(Into::<Box<dyn std::error::Error>>::into)
                })
                .transpose()?
                .unwrap_or_default();
            let mut prompt = String::new();
            std::io::stdin()
                .take(16 * 1024)
                .read_to_string(&mut prompt)?;
            let order: Vec<String> = serde_json::from_str(utf8(order)?)?;
            let image = WorkspaceStore::open(Path::new(database))?.generate_image_asset(
                Path::new(database),
                utf8(workspace)?.parse()?,
                &order,
                &prompt,
                utf8(aspect)?,
                "generated-image",
                &references,
            )?;
            println!(
                "{}",
                serde_json::to_string(
                    &serde_json::json!({ "assetId": image.id, "provider": image.provider, "referencesSent": image.references })
                )?
            );
        }
        [database, command, workspace, run] if command == "process-supplemental-writing" => {
            let result = WorkspaceStore::open(Path::new(database))?.process_supplemental_writing(
                Path::new(database),
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
            )?;
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({ "resultId": result }))?
            );
        }
        [database, command, workspace, run] if command == "process-asset-plan" => {
            let result = WorkspaceStore::open(Path::new(database))?.process_asset_plan(
                Path::new(database),
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
            )?;
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({ "resultId": result }))?
            );
        }
        [database, command, workspace, run] if command == "process-preview-edit" => {
            let result = WorkspaceStore::open(Path::new(database))?.process_preview_edit(
                Path::new(database),
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
            )?;
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({ "resultId": result }))?
            );
        }
        [database, command, workspace, run] if command == "process-script-stage" => {
            let result = WorkspaceStore::open(Path::new(database))?.process_script_stage(
                Path::new(database),
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
            )?;
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({ "resultId": result }))?
            );
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
        [database, command, workspace, run, format, asset]
            if command == "attach-completed-media" =>
        {
            let database = Path::new(database);
            let added = WorkspaceStore::open(database)?.attach_completed_media(
                database,
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(format)?.parse()?,
                utf8(asset)?.parse()?,
            )?;
            println!("{added}");
        }
        [database, command, workspace, run, attachment] if command == "detach-completed-media" => {
            let removed = WorkspaceStore::open(Path::new(database))?.detach_completed_media(
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(attachment)?.parse()?,
            )?;
            println!("{removed}");
        }
        [database, command, workspace, run, format, source]
            if command == "import-completed-video-variant" =>
        {
            let database = Path::new(database);
            let asset = WorkspaceStore::open(database)?
                .import_completed_video_variant(
                    database,
                    utf8(workspace)?.parse()?,
                    utf8(run)?.parse()?,
                    utf8(format)?.parse()?,
                    Path::new(source),
                )
                .map_err(|error| io::Error::other(error.to_string()))?;
            println!("{}", serde_json::to_string(&asset)?);
        }
        [database, command, workspace, run, format] if command == "verified-video-source" => {
            let database = Path::new(database);
            let (video, _) = WorkspaceStore::open(database)?.verified_delivery_files(
                database,
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(format)?.parse()?,
            )?;
            println!("{}", serde_json::to_string(&video)?);
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
        [database, command, workspace, run, format, destination]
            if command == "export-captioned-output" =>
        {
            let database = Path::new(database);
            WorkspaceStore::open(database)?.export_captioned_output(
                database,
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(format)?.parse()?,
                Path::new(destination),
            )?;
            println!("null");
        }
        [database, command, workspace, run, format] if command == "generate-delivery-subtitles" => {
            let database = Path::new(database);
            let cues = WorkspaceStore::open(database)?.generate_delivery_subtitles(
                database,
                utf8(workspace)?.parse()?,
                utf8(run)?.parse()?,
                utf8(format)?.parse()?,
            )?;
            println!("{cues}");
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
                "set-profile-description <workspace-id> <profile-id> <description> | ",
                "set-profile-format-context <workspace-id> <profile-id> <format-id> <markdown> | ",
                "profile-assets <workspace-id> <profile-id> | ",
                "share-asset <workspace-id> <asset-id> <profile-id> [format-id] | ",
                "unshare-asset <workspace-id> <asset-id> <profile-id> [format-id] | ",
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
                "context <workspace-id> | set-context <workspace-id> <markdown> | draft-context <workspace-id> <sources-json> [notes] | import-context <workspace-id> <import-json> | ",
                "feedback <workspace-id> | add-feedback <workspace-id> <title> <markdown> | ",
                "update-feedback <workspace-id> <feedback-id> <title> <markdown> | ",
                "remove-feedback <workspace-id> <feedback-id> | ",
                "brand-colors <workspace-id> | set-brand-colors <workspace-id> <colors-json> | ",
                "brand-assets <workspace-id> | ",
                "attach-brand-asset <workspace-id> <asset-id> <identity|style> | ",
                "detach-brand-asset <workspace-id> <asset-id> <identity|style> | ",
                "drafts <workspace-id> | create-draft <workspace-id> <settings-json> | ",
                "update-draft <workspace-id> <draft-id> <settings-json> | ",
                "delete-draft <workspace-id> <draft-id> <confirmed-title> | generation-runs <workspace-id> | ",
                "enqueue-draft <workspace-id> <draft-id> <request-key> <options-json> | ",
                "replay-generation <workspace-id> <source-run-id> <request-key> <captured|current> | ",
                "claim-generation <workspace-id> | sweep-agent-sessions | checkpoint-generation <workspace-id> <run-id> <attempt> <json> | ",
                "control-generation <workspace-id> <run-id> <pause|resume|retry|cancel|stop> | ",
                "generation-activity <workspace-id> <run-id> | agent-logs <workspace-id> [run-id] | generate-motion-item <workspace-id> <request> | agent-log <workspace-id> <log-id> | generation-notifications <workspace-id> <after-id> | ",
                "read-completion-notification <workspace-id> <run-id> | ",
                "fail-generation <workspace-id> <run-id> <attempt> <detail> | ",
                "prepare-generation-plan <workspace-id> <run-id> <attempt> | generation-plan <workspace-id> <run-id> | ",
                "production-progress <workspace-id> <run-id> | ",
                "submit-motion-artifacts <workspace-id> <run-id> <attempt> <artifacts-json> | submit-stage <workspace-id> <run-id> <attempt> <stage> <result-json> | ",
                "review-preview-formats <workspace-id> <run-id> <result-id> <decisions-json> <notes> | ",
                "review-supplemental-items <workspace-id> <run-id> <result-id> <decisions-json> <notes> | ",
                "review-stage <workspace-id> <run-id> <result-id> <approved|rejected|revise> <notes> [motion-indexes-json] | ",
                "plugin-catalog | export-plugin-package <plugin-id> <sha256> <new-zip> | install-plugin-manifest <manifest.json> | install-plugin-zip <package.zip> [--sha256 <digest>] | completed-subtitles <workspace-id> <run-id> <format-id> [revision] | revise-completed-subtitles <workspace-id> <run-id> <format-id> <expected-revision> <srt-file> | completed-subtitle-languages <workspace-id> <run-id> <format-id> | completed-subtitle-track <workspace-id> <run-id> <format-id> <language> [revision] | revise-completed-subtitle-track <workspace-id> <run-id> <format-id> <language> <expected-revision> <srt-file> | watch-motion-queue | process-motion-stage <workspace-id> <run-id> | render-run-motion <workspace-id> <run-id> <plugin-id> <digest> <sdk-worker> <output-directory> [toolchain] | run-installed-motion-renderer <plugin-id> <digest> <sdk-worker> <output-directory> [toolchain] | remove-plugin-manifest <plugin-id> | ",
                "install-plugin-release <catalog.json> <plugin-id> <version> <package.zip> | ",
                "read-plugin-registry <catalog.json> | fetch-plugin-registry <catalog-url> | ",
                "download-plugin-release <catalog-url> <plugin-id> <version> | update-plugin-release <plugin-id> <version> | ",
                "update-plugin-release-from-snapshot <catalog.json> <plugin-id> <version> <package.zip> | ",
                "plugin-settings <plugin-id> <app|workspace-id> | set-plugin-setting <plugin-id> <app|workspace-id> <setting-id> <value-json> | clear-plugin-setting <plugin-id> <app|workspace-id> <setting-id> | ",
                "plugin-credential-status | set-plugin-credential | clear-plugin-credential <plugin-id> <app|workspace-id> <setting-id> | ",
                "provider-preferences <capability> <app|workspace-id> | set-provider-preferences <capability> <app|workspace-id> <order-json> | clear-provider-preferences <capability> <app|workspace-id> | ",
                "transcribe-wav <workspace-id> [plugin-id] <audio.wav> | speech-provider-status <workspace-id> <plugin-id> | synthesize-speech <workspace-id> <plugin-id> <voice-id> <new-audio-file> (text on stdin) | ",
                "a2a <port> (bearer token on stdin) | mcp | plugin-actions | watch-plugin-action-queue | queue-plugin-action-job (JSON on stdin) | list-plugin-action-jobs <app|workspace-id> | plugin-action-job <app|workspace-id> <job-id> | process-plugin-action-job <app|workspace-id> <job-id> | invoke-plugin-action <plugin-id> <action-id> <app|workspace-id> (JSON on stdin) | ",
                "invoke-plugin <plugin-id> [workspace-id] (JSON on stdin) | plugin-context-grant <plugin-id> <workspace-id> | grant-plugin-context <plugin-id> <workspace-id> | revoke-plugin-context <plugin-id> <workspace-id> | ",
                "agent-plugins | agent-login-status <plugin-id> | ",
                "agent-settings <app|workspace-id> | set-agent-provider <app|workspace-id> <plugin-id|unset> | ",
                "preview-settings | set-preview-max-edge <pixels|auto> | ",
                "snapshot-database <new-sqlite-file> | watch-delivery-queue | process-script-stage <workspace-id> <run-id> | process-supplemental-writing <workspace-id> <run-id> | generate-image <workspace-id> <16:9|9:16|1:1> <providers-json> [reference-asset-ids-json] (prompt on stdin) | process-asset-plan <workspace-id> <run-id> | process-preview-edit <workspace-id> <run-id> | process-media-stage <workspace-id> <run-id> [preview-edit-json] | ",
                "scheduled-outputs <workspace-id> <start-date> <end-date> [profile-id] | set-output-schedule <workspace-id> <run-id> <format-id> <date|clear> | ",
                "delete-generation <workspace-id> <run-id> <confirmed-title> | recover-delivery-cleanup <workspace-id> | delete-completed-video <workspace-id> <run-id> <confirmed-title> | completed-video <workspace-id> <run-id> | verified-video-source <workspace-id> <run-id> <format-id> | import-completed-video-variant <workspace-id> <run-id> <format-id> <source.mp4> | attach-completed-media <workspace-id> <run-id> <format-id> <asset-id> | detach-completed-media <workspace-id> <run-id> <attachment-id> | ",
                "add-completed-video-feedback <workspace-id> <run-id> <run|result-id> <markdown> | ",
                "rename-completed-video <workspace-id> <run-id> <title> | ",
                "generate-delivery-subtitles <workspace-id> <run-id> <format-id> | import-delivery-subtitles <workspace-id> <run-id> <format-id> <source-srt> | ",
                "export-delivery <workspace-id> <run-id> <destination-zip> | export-captioned-output <workspace-id> <run-id> <format-id> <destination-mp4> | ",
                "set-plugin-active <plugin-id> on|off | ",
                "render-preview <workspace-id> <run-id> <attempt> <edit-json> | ",
                "render-primary <workspace-id> <run-id> <attempt> | verify-delivery <workspace-id> <run-id> <attempt>"
            )
            .into());
        }
    }
    Ok(())
}

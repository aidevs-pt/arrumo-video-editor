use std::{env, error::Error, ffi::OsStr, io, path::Path};

use arrumo_workspaces::WorkspaceStore;

fn utf8(value: &OsStr) -> io::Result<&str> {
    value
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Arguments must be valid UTF-8"))
}

fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    match arguments.as_slice() {
        [database, command] if command == "list" => {
            let workspaces = WorkspaceStore::open(Path::new(database))?.list()?;
            println!("{}", serde_json::to_string(&workspaces)?);
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
                "set-profile-delivery <workspace-id> <profile-id> <format-id> <overrides-json>"
            )
            .into());
        }
    }
    Ok(())
}

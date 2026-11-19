use std::{env, error::Error, ffi::OsStr, io, path::Path};

use arrumo_workspaces::{BrandAssetRole, RunControl, WorkspaceStore, asset_root};

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
                "set-asset-notes <workspace-id> <asset-id> <notes> | ",
                "context <workspace-id> | set-context <workspace-id> <markdown> | ",
                "feedback <workspace-id> | add-feedback <workspace-id> <title> <markdown> | ",
                "update-feedback <workspace-id> <feedback-id> <title> <markdown> | ",
                "brand-colors <workspace-id> | set-brand-colors <workspace-id> <colors-json> | ",
                "brand-assets <workspace-id> | ",
                "attach-brand-asset <workspace-id> <asset-id> <identity|style> | ",
                "detach-brand-asset <workspace-id> <asset-id> <identity|style> | ",
                "drafts <workspace-id> | create-draft <workspace-id> <settings-json> | ",
                "update-draft <workspace-id> <draft-id> <settings-json> | ",
                "generation-runs <workspace-id> | ",
                "enqueue-draft <workspace-id> <draft-id> <request-key> <options-json> | ",
                "replay-generation <workspace-id> <source-run-id> <request-key> <captured|current> | ",
                "claim-generation <workspace-id> | checkpoint-generation <workspace-id> <run-id> <attempt> <json> | ",
                "control-generation <workspace-id> <run-id> <pause|resume|retry|cancel> | ",
                "generation-activity <workspace-id> <run-id> | ",
                "fail-generation <workspace-id> <run-id> <attempt> <detail> | ",
                "prepare-generation-plan <workspace-id> <run-id> <attempt> | generation-plan <workspace-id> <run-id> | ",
                "production-progress <workspace-id> <run-id> | ",
                "submit-stage <workspace-id> <run-id> <attempt> <stage> <result-json> | ",
                "review-stage <workspace-id> <run-id> <result-id> <approved|rejected|revise> <notes> | ",
                "preview-settings | set-preview-max-edge <pixels|auto> | ",
                "process-media-stage <workspace-id> <run-id> [preview-edit-json] | ",
                "completed-video <workspace-id> <run-id> | ",
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

# Arrumo Video Editor

The Tauri and Next.js desktop editor and shared application API for the UI, CLI, MCP, and A2A.

## Desktop development

Install the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/), Node, and the pnpm version in `package.json`.

```sh
pnpm install --frozen-lockfile
pnpm tauri dev
```

The first desktop screen creates and lists workspaces. It uses the system language when it is English, Spanish, or Portuguese, with English as the fallback. The app stores `workspaces.sqlite` in its platform-specific local application data directory. The same workspace store powers the CLI.

`pnpm tauri build --debug --no-bundle` builds a native executable from the static frontend. Installers and signing are tracked separately.

## Default plugin inputs

In the Arrumo workspace layout, the build reads the sibling `plugin-registry/catalog.json` and loads entries marked `default` from the sibling `plugins/<id>` checkouts. The checkouts may be symlinks. Set `ARRUMO_PLUGIN_REGISTRY` and `ARRUMO_PLUGIN_SOURCE_ROOT` to use a different registry file or checkout location. The build embeds the selected artifacts from those inputs without tracking copies in the editor repository.

Use `ARRUMO_CORE_ONLY=1` for isolated host verification without private checkout access. That build starts with an empty default catalog.

## Workspace CLI

From this repository, use Cargo to create and list workspaces in a local SQLite database.

```sh
cargo run --locked -- ./workspaces.sqlite create "My studio"
cargo run --locked -- ./workspaces.sqlite list
```

To create and list profiles for workspace ID 1:

```sh
cargo run --locked -- ./workspaces.sqlite create-profile 1 "Reviews" "## Keep hands-on demos" "#532A9D"
cargo run --locked -- ./workspaces.sqlite profiles 1
```

To update profile ID 1 within workspace ID 1:

```sh
cargo run --locked -- ./workspaces.sqlite update-profile 1 1 "Tutorials" "## Explain each step" "#532A9D"
```

Pass an empty guidance string to clear it, or omit color on update to clear the color. On creation, guidance and color are optional. Colors use `#RRGGBB`. The commands print JSON. The database persists across runs. Run the backend tests with `cargo test --locked`.

Copyright © 2026 DEVS AI Unipessoal LDA. Licensed under the [GNU Affero General Public License v3.0](LICENSE).

For custom AI solutions for your company, visit [DEVS AI](https://aidevs.pt).

Workspace output formats include Landscape (1920 × 1080) and Portrait (1080 × 1920) by default. For workspace ID 1:

```sh
cargo run --locked -- ./workspaces.sqlite formats 1
cargo run --locked -- ./workspaces.sqlite create-format 1 '{"name":"Square","width":1080,"height":1080,"enabled":true}'
```

Use `update-format <workspace-id> <format-id> <settings-json>` to edit or disable a format, and `remove-format <workspace-id> <format-id>` to remove it.

Profiles see enabled workspace formats and can override width and height independently. The listing returns resolved dimensions and the raw overrides. On save, absent or null fields inherit workspace values.

```sh
cargo run --locked -- ./workspaces.sqlite profile-formats 1 1
cargo run --locked -- ./workspaces.sqlite set-profile-format 1 1 1 '{"width":1280}'
cargo run --locked -- ./workspaces.sqlite set-profile-format 1 1 1 '{}'
```

Disabling a format hides it and preserves its overrides for re-enabling. Removing a format deletes its overrides.

Workspace output formats have supplemental delivery settings for thumbnail, title, and description counts, a tags toggle, and Markdown thumbnail guidance and specifications. Final video and subtitles stay required. Replacing the supplemental settings defaults omitted fields to zero, false, or empty text.

```sh
cargo run --locked -- ./workspaces.sqlite set-format-delivery 1 1 '{"thumbnails":3,"titles":3,"descriptions":1,"tags":true}'
cargo run --locked -- ./workspaces.sqlite format-delivery 1 1
```

Profile delivery settings can override individual workspace values for each enabled format. The read command returns workspace values, overrides, and effective delivery. Saving replaces the profile override set. Omitted and null fields inherit. Zero, false, and empty text are explicit values.

```sh
cargo run --locked -- ./workspaces.sqlite set-profile-delivery 1 1 1 '{"thumbnails":0,"tags":false}'
cargo run --locked -- ./workspaces.sqlite profile-delivery 1 1 1
```

Delivery settings can include `portrait_derivatives`. To request two portrait derivatives from profile ID 1, use `{"portrait_derivatives":{"count":2,"source":{"type":"profile","profile_id":1}}}` as part of the delivery settings JSON. The source profile must belong to the workspace. Use `{"type":"manual","guidance":"…","thumbnails_per_video":2,"thumbnail_guidance":"…"}` as the source to specify the derivative and thumbnail guidance directly. Count zero disables derivative requests. Profile delivery can override the portrait configuration as a unit.

Importing a workspace asset copies it into workspace-owned storage alongside the database at `<database>.media/<workspace-id>/<asset-id>`. The source file stays in place. File extensions classify supported image, audio, and video assets.

```sh
cargo run --locked -- ./workspaces.sqlite import-asset 1 ./reference.png
cargo run --locked -- ./workspaces.sqlite assets 1
cargo run --locked -- ./workspaces.sqlite set-asset-notes 1 1 "Keep the product centered"
```

Use `thumbnail_reference_ids` to select existing image assets in a delivery configuration, including within a manual portrait source. IDs are scoped to the workspace. In profile deliveries, omit the top-level field to inherit the workspace selection or pass an empty list to clear it.

The workspace Context view edits production guidance in Markdown. The editor supplies a localized template until the first save. The CLI returns JSON `null` for unset context and an empty string for context explicitly cleared by a user.

```sh
cargo run --locked -- ./workspaces.sqlite set-context 1 '## Audience'
cargo run --locked -- ./workspaces.sqlite context 1
```

Agent feedback cards live in workspace Context. A tool can record an observation locally using `add-feedback <workspace-id> <title> <markdown>`. Use `feedback <workspace-id>` to list cards and `update-feedback <workspace-id> <feedback-id> <title> <markdown>` to revise one. Cards can also be expanded and edited in the desktop app.

The workspace Brand view stores an ordered palette. Use `brand-colors <workspace-id>` to read it and `set-brand-colors <workspace-id> '["#532A9D","#FFFFFF"]'` to replace it. Colors use six-digit hex notation. Use an empty array to clear the palette.

The brand image catalog associates existing workspace images with `identity` (for example, logos or avatars) and `style` (visual reference) roles. An image can serve both roles without another copy of the file.

```sh
cargo run --locked -- ./workspaces.sqlite attach-brand-asset 1 1 identity
cargo run --locked -- ./workspaces.sqlite brand-assets 1
cargo run --locked -- ./workspaces.sqlite detach-brand-asset 1 1 identity
```

Create video drafts with an ordered list of workspace video asset IDs, or start from scratch with an intent brief. An optional `profile_id` must refer to a profile in the same workspace.

```sh
cargo run --locked -- ./workspaces.sqlite create-draft 1 '{"title":"Review","source_ids":[2,1],"profile_id":1}'
cargo run --locked -- ./workspaces.sqlite create-draft 1 '{"title":"Explainer","intent":"Show how a cube works"}'
cargo run --locked -- ./workspaces.sqlite drafts 1
```

Use `update-draft <workspace-id> <draft-id> <settings-json>` to replace the saved draft settings and source order. The desktop Drafts view also supports editing and cancelling. An invalid source group leaves the original draft intact.

Queue a saved draft with a request key and run options. Retrying with the same workspace, request key, draft, and options returns the existing run. Queued runs retain their submitted snapshot after later draft or workspace edits.

```sh
cargo run --locked -- ./workspaces.sqlite enqueue-draft 1 1 request-1 '{"autopilot":false,"platform":"YouTube"}'
cargo run --locked -- ./workspaces.sqlite generation-runs 1
```

A worker claims queued work with `claim-generation <workspace-id>`. The claim includes an attempt number and a 60-second lease. Use `checkpoint-generation <workspace-id> <run-id> <attempt> <json>` to save progress and renew the lease. Expired claims can be reclaimed with the last saved checkpoint. Use `control-generation <workspace-id> <run-id> <pause|resume|cancel>` to control runs and `generation-activity <workspace-id> <run-id>` to read their transition history.

## Interface translations

Copy `locales/en.json` to a new file such as `locales/fr.json` and translate the values. Keep the same keys. The desktop build discovers the file automatically and reports missing or unexpected keys. The app follows the system’s language preferences, tries the base language when a regional translation is unavailable, and falls back to English.

A worker can report a terminal failure with `fail-generation <workspace-id> <run-id> <attempt> <detail>`. Supply a short, user-facing reason without credentials or raw provider logs. The desktop activity history shows the reason. Use Retry in the desktop queue or `control-generation <workspace-id> <run-id> retry` to return a failed run to the queue with its checkpoint intact.

New runs save their production plan from the frozen snapshot at enqueue time. Use `generation-plan <workspace-id> <run-id>` to read it as soon as the run is queued. A worker receives the current step with its claim. For older runs with no saved plan, the worker can use `prepare-generation-plan <workspace-id> <run-id> <attempt>` under its active lease. Guided plans flag the stages that require review. Autopilot plans omit those review requirements.

Workspace Context uses the editorcn rich-text toolbar with Tiptap Markdown. Use the Markdown source toggle to edit raw Markdown directly, including syntax outside the rich toolbar's formatting options. The workspace continues to store Markdown.

A leased worker can use `submit-stage <workspace-id> <run-id> <attempt> <stage> <result-json>` to record the current plan step. A guided run waits for `review-stage <workspace-id> <run-id> <result-id> <approved|rejected|revise> <notes>`. Approval advances to the next stage. Rejection and revision requeue the current stage and preserve review history. Use `production-progress <workspace-id> <run-id>` to read results and decisions. Delivery verification has a separate completion path.

Delivery verification requires `ffprobe` on PATH. At the `verify_delivery` stage, place each configured format's final files under `<database>.deliveries/<workspace-id>/<run-id>/`, named `<format-id>.mp4` and `<format-id>.srt`. Run `verify-delivery <workspace-id> <run-id> <attempt>` to check each video’s dimensions and duration and its subtitle cues. A successful verification records metadata and completes the run while the lease is still active.

A leased worker on the Preview stage can use `render-preview <workspace-id> <run-id> <attempt> <edit-json>` for recorded-source runs. Pass the Preview result object as the edit. The renderer uses the optional cuts below and writes one MP4 per frozen output format, capped at the longest edge saved in the run snapshot. The recorded result includes these filenames in `preview_videos`. Files are stored in the run's delivery directory with the attempt number in the name so revisions retain separate artifacts.

Use `preview-settings` to inspect the app-wide resolution and `set-preview-max-edge <pixels|auto>` to set a cap from 144 to 2160 pixels or restore the CPU-based recommendation. This applies to new runs across every workspace. Runs saved before preview settings were introduced use a 480 px cap.

For recorded-source runs, the Final render stage can assemble managed clips using `render-primary <workspace-id> <run-id> <attempt>`. It requires `ffmpeg`, `ffprobe`, and the `libx264` encoder. It fits each clip to the frozen output dimensions without stretching, preserves source audio, and inserts silence for clips without audio. It writes the primary MP4s and advances to delivery verification. Supply subtitle files separately before verifying.

A Preview result can include `"cuts": [{"asset_id": 2, "start_ms": 500, "end_ms": 1500}]`. List cuts in playback order and use millisecond offsets into videos from the frozen draft source list. Final render consumes the approved Preview result in guided mode and the accepted Preview result in autopilot. It uses the same cuts across primary formats. Omit `cuts` to assemble whole source clips in draft order.

Use `process-media-stage <workspace-id> <run-id> [preview-edit-json]` to claim and execute one media stage for a specific run. Pass an edit object (or `{}` for whole clips) when the run is ready for Preview. Omit the edit for recorded-source Final render or delivery verification. The command prints the stage and attempt on success, or `null` if the run is unavailable for that operation. Failed media work records a short reason in activity history and can be retried after correction. Guided review still uses `review-stage`.

Use `export-delivery <workspace-id> <run-id> <destination-zip>` after delivery verification. It exports the primary MP4 and SRT files recorded as verified for the completed run. Choose a new ZIP path in an existing directory. An incomplete export leaves that path free.

Use `completed-video <workspace-id> <run-id>` to read the current title, captured inputs and settings, verified delivery, production decisions, and activity history together. Use `rename-completed-video <workspace-id> <run-id> <title>` to change the display title while preserving the title in the run snapshot.

Use `replay-generation <workspace-id> <source-run-id> <request-key> <captured|current>` to queue a fresh run from a completed video. Captured mode copies the original input snapshot. Current mode resolves the latest saved draft, workspace, profile, output and preview settings with the original run's platform and autopilot options. Both modes start at the beginning of the production plan. Repeat the same request key, source and mode to retrieve the same replay.

Use `add-completed-video-feedback <workspace-id> <run-id> <run|result-id> <markdown>` to comment on a completed video or one of its production results. The comments appear in the completed-video detail. New runs and current-settings replays of the same draft capture those comments as delivery feedback. Captured replays reuse the feedback in the original snapshot.

In the desktop sidebar, open **Completed videos** and select a video to review the verified file list, save a display title, or export the delivery through the native ZIP save dialog.

The completed-video panel also queues a replay with captured or current settings and saves Markdown feedback against the whole video or a selected production result.

The **Video** tab opens the verified player, shows a format picker when several outputs exist, and includes **Subtitle text**. Arrumo streams the selected file through its local loopback interface. Playback uses the desktop WebView's codecs. On Linux, WebKitGTK needs GStreamer H.264 and AAC decoders, such as those provided by `gstreamer1.0-libav` on Ubuntu.

In a guided run's production plan, open **Watch preview** to play a rendered MP4 before submitting the stage decision. Each replacement preview appears with its own attempt number.

The desktop production plan can render a whole-clip preview for recorded-source runs, render the accepted edit at final resolution, and verify delivery after the SRT files are present. Each action claims only the stage shown in the plan.

At delivery verification, use **Import SRT** beside an output format or run `import-delivery-subtitles <workspace-id> <run-id> <format-id> <source-srt>`. The importer checks UTF-8 text and cue timing against the rendered MP4 before replacing the delivery subtitle file. The CLI prints the number of accepted cues.

The desktop frontend exports separate App Router pages for Profiles, Context, Brand, Assets, Outputs, Drafts, Generations, and Completed videos. Workspace IDs are created in the local database after the app is built, so an exported page takes the ID as a query parameter, for example `/brand?workspace=2`. Profile-to-draft navigation also carries the profile ID in the URL. The studio layout resolves those IDs through Tauri.

Completed runs open at `/completed-video?workspace=<workspace-id>&run=<run-id>`. The app resolves the video from the selected local workspace, so the link works with the matching Arrumo database.

Desktop deep links use `arrumo://open/<section>?workspace=<id>`. Profile and run pages also take `profile=<id>` or `run=<id>`, respectively. For example, `arrumo://open/generation-run?workspace=2&run=8` opens the matching run in the local database. Linux registers the scheme on launch, and Windows debug builds register it for testing. Installed package registration follows Tauri's desktop scheme configuration.

Use `agent-settings <app|workspace-id>` to inspect the configured agent and where it came from. Use `set-agent-provider <app|workspace-id> <plugin-id|unset>` to change the choice. Unsetting a workspace override restores the app choice, if configured. New runs freeze the resolved selection and provenance. Selecting a provider is separate from connecting or running its agent.

Open **Settings** in the desktop sidebar to choose the app's production agent. Use the settings scope picker to configure a workspace override or restore inheritance. The effective provider and its source appear under the selector.

On a fresh install, creating the first workspace opens agent setup if no app provider is selected. Settings can check whether the selected CLI is on Arrumo's PATH and open the provider's installation guide in the system browser. This check reports local CLI availability. Authentication is handled through the provider's own setup.

Use **Check sign-in** in Settings or `agent-login-status <plugin-id>` in the CLI to ask the installed provider about its local authentication state. The check makes no generation request and returns only a status category. It times out after five seconds. Windows detection and sign-in checks use a native executable on PATH.


Use `agent-plugins` to list the bundled agent providers. Their IDs, labels, setup guides, and authentication probes come from the provider repositories' SDK manifests. Builds import the registry defaults from their separate plugin checkouts. Runtime installation and trust grants are separate host features.

The app database also holds a plugin manifest catalog. With the database path as the first CLI argument, use `plugin-catalog` to inspect bundled and locally registered SDK manifests, `install-plugin-manifest <manifest.json>` to register a manifest, and `remove-plugin-manifest <plugin-id>` to remove a local entry. The catalog validates SDK v1 through v6 metadata and reserves the IDs of bundled plugins. Local entries are metadata only. The agent selector and sign-in probes use the bundled agent adapters until the host supports runtime grants.

Use `install-plugin-zip <package.zip>` to register a local ZIP containing a root SDK `manifest.json`. The catalog reports the package size and SHA-256 digest and retains the original ZIP in the app database. Removing the local entry removes its stored ZIP. The CLI displays the third-party plugin notice on registration. ZIP contents remain inert.

Pass `--sha256 <digest>` after the ZIP path to compare the package against an independently obtained release checksum before registration. A malformed or mismatched digest prevents registration. A checksum establishes that the package matches the expected bytes. Publisher trust still depends on how you obtained the checksum.

Use `install-plugin-release <catalog.json> <plugin-id> <version> <package.zip>` to verify a local ZIP against a selected release in a local registry snapshot. Arrumo verifies supported manifest compatibility, the ZIP checksum, and the manifest's plugin ID, then records the declared version and source URLs in the catalog. This command accepts the two files locally. Registry synchronization and remote downloading remain separate work.

Use `download-plugin-release <catalog-url> <plugin-id> <version>` to fetch an exact release from an HTTPS registry and register it locally. Arrumo verifies the release using the same checks as local snapshot registration and records the registry URL. The command expects a registry and package that can be fetched without authentication.

List plugin IDs, source repositories, defaults, and available release versions with `read-plugin-registry <catalog.json>` or `fetch-plugin-registry <catalog-url>`. This host supports manifest schemas 1 through 6. Entries with an empty release list have no downloadable version in that catalog. Both commands validate the catalog and can run without opening the app database.

In desktop Settings, open **Browse registry** under **Plugins** and load an HTTPS catalog URL. Expand a plugin to review its source repository and available versions. Arrumo checks the reviewed release again before downloading. If its source or checksum changed, reload the catalog and review the release again.

Use `update-plugin-release <plugin-id> <version>` to replace a remotely installed package with a newer SemVer release from its recorded HTTPS catalog and source repository. Arrumo validates the replacement before saving it and checks that the installed release did not change during the download. Updates to bundled plugins ship with Arrumo.

For packages installed from a local registry snapshot, use `update-plugin-release-from-snapshot <catalog.json> <plugin-id> <version> <package.zip>`. Use a catalog snapshot obtained independently of the ZIP. Arrumo requires the recorded source repository and a newer compatible release and verifies the ZIP before replacing the installed package. Remotely installed packages continue to use their recorded HTTPS catalog.

When you load the same registry in desktop Settings, installed packages show **Update** for newer compatible releases from their recorded repository. Review the release and confirm the update. Local-snapshot packages use the offline CLI update command.

Use `plugin-settings <plugin-id> <app|workspace-id>` to read declared non-secret values and their effective source. Use `set-plugin-setting <plugin-id> <app|workspace-id> <setting-id> <value-json>` to store a string, boolean, or signed 64-bit integer. Use `clear-plugin-setting <plugin-id> <app|workspace-id> <setting-id>` to unset a value. Clearing a workspace override restores its app default. Empty strings, `false`, and `0` are explicit values. Secret fields require separate credential storage and are excluded from these commands.

Desktop Settings shows **Plugin settings** when an installed manifest declares editable non-secret fields. Choose the app scope to set defaults or a workspace scope to override workspace fields. **Use app default** clears an override. The form shows where each effective value comes from.

Use `plugin-credential-status <plugin-id> <app|workspace-id> <setting-id>` to read configuration flags and the effective scope for a bundled provider's declared secret. Use `set-plugin-credential <plugin-id> <app|workspace-id> <setting-id>` with the credential on redirected standard input, or `clear-plugin-credential <plugin-id> <app|workspace-id> <setting-id>` to remove it. A workspace credential overrides the app credential. Secret bytes are stored in the system credential store. On Linux this requires an available Secret Service session, and building from source requires D-Bus development headers. CLI output reports status only.

In desktop Settings, expand a bundled provider under **Plugin settings** to save or remove its credential at the app or workspace scope. Arrumo shows whether a credential is configured and where it comes from. Removing a workspace credential restores the app credential when one is configured.

Use `provider-preferences <capability> <app|workspace-id>` to inspect the catalog candidates and effective provider order. Use `set-provider-preferences <capability> <app|workspace-id> <order-json>` with an ordered JSON array of plugin IDs. An empty array explicitly selects no providers. Use `clear-provider-preferences <capability> <app|workspace-id>` to inherit. The default order contains only bundled plugins. Eligibility here reflects declared capabilities. Runtime readiness and execution are separate.

New generation runs capture the effective provider order and its source for each capability. A replay using captured settings preserves that order. A replay using current settings resolves it again. Agent selection continues to use the separately captured agent settings. Snapshots created before provider order capture remain readable.

The `enqueue-draft` options JSON can include `provider_overrides`, a map from capability ID to an ordered array of provider plugin IDs. An omitted capability inherits the workspace or app order. An empty array disables providers for that capability for this run. Both replay modes preserve the requested per-run overrides. A replay with current settings revalidates the requested providers. Agent selection uses the agent settings.

In the desktop generation form, expand **Providers for this run** to include or exclude individual providers. **Use saved order** clears a capability override. Closing the disclosure preserves your selections for that draft. Submitting the run leaves the workspace preferences unchanged.

Use `transcribe-wav <workspace-id> [plugin-id] <audio.wav>` to submit a WAV to a bundled transcription adapter. Omit the plugin ID to use the effective provider order. Each plugin declares its limits and transcript fields. The host skips unavailable adapters, oversized inputs for a particular adapter, and missing credentials before upload. An HTTP 401, 403, or 429 permits fallback. Other failures stop the request. This is an explicit cloud upload.

The bundled plugin index is generated from the official registry defaults and local plugin checkouts. Provider-specific transcription settings live in the plugin's SDK descriptor. Manifest schema v3 accepts ZIPs with declared WebAssembly backend and HTML frontend entrypoints, validates their presence, and retains the code. Installed backend components support explicit CLI invocation. Installed frontend panels render from declared documents. Workspace-scoped panels can request explicitly granted context access.

Use `invoke-plugin <plugin-id>` with JSON on standard input to invoke an installed package's declared WebAssembly backend. The runtime instantiates the component with no host imports or WASI and limits its size, execution fuel, linear memory and JSON payloads. The result is JSON on standard output.

A plugin that declares workspace context access needs a separate grant. Use `grant-plugin-context <plugin-id> <workspace-id>`, `plugin-context-grant <plugin-id> <workspace-id>`, and `revoke-plugin-context <plugin-id> <workspace-id>` to grant, check, or revoke access. Pass the workspace ID to `invoke-plugin <plugin-id> <workspace-id>` to use the grant. Revocation and package updates affect subsequent invocations. Each invocation sees only the context selected at its start.

Desktop Settings, workspace Context, and completed video details can show **Plugin panels** from installed ZIPs. Opening a panel loads its declared HTML from a dedicated local protocol and checks the package revision. A sandbox isolates the panel from the editor and its native commands.

In desktop Settings, choose a workspace and open **Plugin permissions** to grant or revoke context access for a requesting plugin. The confirmation names the plugin and workspace, and the host verifies the package revision before applying your choice.

Workspace and completed video plugin panels can request the host’s selected workspace context if their manifest declares that permission and the operator grants it. The bridge handles one request per panel opening and rechecks the package revision and grant before returning the context.

Use `plugin-actions` to discover named backend actions. Invoke an action with `invoke-plugin-action <plugin-id> <action-id> <app|workspace-id>` and pass JSON on standard input. The host validates the action's declared scope before running the component.

In desktop Settings, expand an action under **Plugin actions**. The Settings scope determines which actions are shown. Enter JSON, select **Run action**, and review the result. The editor checks that the package matches the version shown when the form was opened.

### Local MCP

Configure a local MCP client to launch `arrumo-workspaces <database-path> mcp` over stdio. This server supports MCP `2025-11-25` and exposes `list_workspaces`, `list_plugin_actions`, and `invoke_plugin_action`. Pass the package digest returned by the action listing, and pass a workspace ID for a workspace action. The client process has the same workspace access as an operator running the local CLI. Action execution applies the existing sandbox and plugin grants. Pass an absolute database path when configuring the client.

To queue a saved draft, call `list_video_drafts` with the workspace ID, then call `enqueue_video_draft` with the draft ID and a request key. Reuse the key for retries of the same submission. Use `list_generation_runs` and `get_generation_run` to inspect runs and captured settings, and `control_generation` to pause, resume, retry, or cancel a run. Use `get_production_progress` to identify a pending stage result and `review_production_result` to submit an approval, rejection, or revision request.

Workers can call `claim_generation` for the next run and use the returned attempt number with `checkpoint_generation` to save progress and renew the lease. Submit stage results with `submit_production_result` while the lease is active. Delivery verification uses a separate workflow. Use `get_generation_activity` to read the run's recorded transitions.

At the `verify_delivery` step, call `verify_generation_delivery` with the workspace ID, run ID, and current attempt. The server checks the managed MP4 and SRT files with `ffprobe` using the database path configured at launch. After a successful verification, call `get_completed_video` for verified output metadata and production history.

Call `export_generation_delivery` with the workspace ID, completed run ID, and an absolute local `destination` path in an existing directory. The shared exporter writes a ZIP of the verified video and subtitle files and refuses to replace an existing path.

Poll for review and completion events with `list_generation_notifications` and a workspace ID, or with CLI `generation-notifications <workspace-id> <after-id>`. Start with a cursor of zero and pass the last returned event ID when polling again. Each response contains at most 100 events in ascending ID order. Events include the run ID and describe transitions that may have been acted on already.

Completion events include a nullable `read_at` timestamp. To acknowledge a completion, call MCP `read_completion_notification` with the workspace and run IDs or use CLI `read-completion-notification <workspace-id> <run-id>`. Both report whether the state changed. Acknowledgement retains the original event and ID. Refresh from an earlier cursor to observe its updated read state.

Use `get_generation_plan` to inspect a run's prepared steps. A worker can use `fail_generation` with its current attempt and a short diagnostic for the activity log. Once a run completes, `replay_generation` can queue a new run in `captured` or `current` mode. Reuse the same request key and mode when retrying a replay submission.

Use `create_video_draft` and `update_video_draft` to save drafts before enqueueing. Pass the full `settings` object with a title and either an intent or an ordered `source_ids` list of managed video assets in the workspace. An optional `profile_id` selects a workspace profile. Keep the returned draft ID for updates and enqueueing.

Use `import_workspace_asset` with a workspace ID and an absolute local `source` path to copy a media file into managed storage. Use `list_workspace_assets` to find asset IDs for draft sources.

Use `import_thumbnail_reference` to import a local image with thumbnail reference notes. Use `set_asset_notes` with the workspace ID, asset ID, and Markdown notes to change the asset's usage guidance. Pass an empty string to clear the notes.

Use `rename_completed_video` to change a completed run's display title. Use `add_completed_video_feedback` to save Markdown feedback about the full run or a specific production result. Pass the result ID from `get_completed_video` for stage feedback.

Use `create_workspace` to initialize a workspace, then `get_workspace_context` and `set_workspace_context` to read or replace its Markdown context. Use `list_profiles`, `create_profile`, and `update_profile` to manage production profiles. Pass a profile's ID as `profile_id` in draft settings. The queue captures the saved context and profile when it creates the run.

Use `list_output_formats`, `create_output_format`, and `update_output_format` to configure workspace output dimensions. Use `get_format_delivery` and `set_format_delivery` for supplemental deliverables. Sending a replacement resets omitted supplemental fields to their defaults. Final video and subtitles remain required.

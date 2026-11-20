# Arrumo Video Editor

The Tauri and Next.js desktop editor and shared application API for the UI, CLI, MCP, and A2A.

## User guide

Create, review, and export a video. [English](docs/user/en/first-video.md) · [Español](docs/user/es/first-video.md) · [Português](docs/user/pt/first-video.md)

## Developer quickstart

Connect through CLI, MCP, or A2A. [English](docs/developer/en/connect.md) · [Español](docs/developer/es/connect.md) · [Português](docs/developer/pt/connect.md)

## Official plugins

Find setup and capability guides in the [official plugin documentation index](https://github.com/aidevs-pt/arrumo-plugin-registry#plugin-guides).

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

For local debug builds, place an SDK-built `plugin.zip` in a selected sibling plugin checkout. The host validates the ZIP and requires its manifest to match the checkout before exposing its actions or frontend surfaces. Workspace context still requires a grant for that package digest.

For registry release builds, set `ARRUMO_PLUGIN_RELEASE_ROOT` to a directory containing the staged `<id>.zip` files. Each staged ZIP must match exactly one release SHA-256 in the selected registry and its manifest must match the corresponding checkout. The bundled catalog retains the release version, source repository and package URL. Release builds only embed executable ZIPs and provider adapter descriptors from this verified staging path. Debug builds without release staging may use sibling checkout descriptors. A default without a staged ZIP has no bundled provider adapter in release builds.

Use `export-plugin-package <plugin-id> <sha256> <new-zip>` to copy a bundled
or installed package from the local catalog. The editor recomputes the digest
and refuses to overwrite an existing file. Obtain the expected digest from
the package entry you reviewed in `plugin-catalog`.

Use `ARRUMO_CORE_ONLY=1` for isolated host verification without private checkout access. That build starts with an empty default catalog.

## Workspace CLI

From this repository, use Cargo to create and list workspaces in a local SQLite database.

```sh
cargo run --locked -- ./workspaces.sqlite create "My studio"
cargo run --locked -- ./workspaces.sqlite list
```

Use `arrumo-workspaces <database-path> snapshot-database <new-sqlite-file>`
to capture committed SQLite state, including writes in the source WAL. The
command creates a new file and reports its SHA-256 and byte length. The
snapshot includes SQLite records and stored plugin ZIPs. Managed media,
delivery files, and system-store credentials require separate backup steps.

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

Copy `locales/en.json` to a new file such as `locales/fr.json` and translate the values. Keep the same keys and preserve substitutions such as `{name}`. The desktop build discovers new catalogs automatically, requires English, Spanish, and Portuguese, and reports key or placeholder differences. The app follows the system’s language preferences, tries the base language when a regional translation is unavailable, and falls back to English.

A worker can report a terminal failure with `fail-generation <workspace-id> <run-id> <attempt> <detail>`. Supply a short, user-facing reason without credentials or raw provider logs. The desktop activity history shows the reason. Use Retry in the desktop queue or `control-generation <workspace-id> <run-id> retry` to return a failed run to the queue with its checkpoint intact.

New runs save their production plan from the frozen snapshot at enqueue time. Use `generation-plan <workspace-id> <run-id>` to read it as soon as the run is queued. A worker receives the current step with its claim. For older runs with no saved plan, the worker can use `prepare-generation-plan <workspace-id> <run-id> <attempt>` under its active lease. Guided plans flag the stages that require review. Autopilot plans omit those review requirements.

Workspace Context uses the editorcn rich-text toolbar with Tiptap Markdown. Use the Markdown source toggle to edit raw Markdown directly, including syntax outside the rich toolbar's formatting options. The workspace continues to store Markdown.

A leased worker can use `submit-stage <workspace-id> <run-id> <attempt> <stage> <result-json>` to record the current plan step. A guided run waits for `review-stage <workspace-id> <run-id> <result-id> <approved|rejected|revise> <notes>`. Approval advances to the next stage. Rejection and revision requeue the current stage and preserve review history. Use `production-progress <workspace-id> <run-id>` to read results and decisions. Delivery verification has a separate completion path.

Use `generate-delivery-subtitles <workspace-id> <run-id> <format-id>` to
transcribe a rendered format while the run is at delivery verification. This
command sends the format's extracted audio to a configured transcription
provider from the run's captured order. It requires FFmpeg, ffprobe, and a
provider that returns timed words or segments. It validates the generated SRT before
publishing it and preserves any existing subtitle file. MCP and A2A expose
`generate_delivery_subtitles` with `workspaceId`, `runId`, and `formatId`.
Only one subtitle generation request can run per output at a time. In the
desktop delivery step, select **Generate subtitles** for the rendered format
or **Import SRT** to supply your own subtitles.

In the desktop queue form, turn on **Generate missing subtitles automatically**
for the selected draft. To opt in when submitting a run through the CLI, MCP,
or A2A, set
`generate_delivery_subtitles: true` in its run options. While the desktop is
open, it attempts transcription once per rendered format missing an SRT.
An interrupted or failed automatic attempt requires an explicit retry via
**Generate subtitles**, or you can import an SRT. Existing SRTs are kept. The
desktop delivery step shows the last automatic attempt for each format. The
shared production progress response exposes the same states in
`automatic_subtitles`, keyed by format ID.

Run `arrumo-workspaces <database-path> watch-delivery-queue` as a foreground
worker when the desktop is closed. It verifies ready deliveries and generates
missing subtitles for runs that opted in. It uses the same database and claim
rules as the desktop. Stop the worker with Ctrl+C. Failed or interrupted
automatic subtitle attempts still require an explicit retry.

Delivery verification requires `ffprobe` on PATH. At the `verify_delivery` stage, place each configured format's final files under `<database>.deliveries/<workspace-id>/<run-id>/`, named `<format-id>.mp4` and `<format-id>.srt`. Run `verify-delivery <workspace-id> <run-id> <attempt>` to check each video’s dimensions and duration and its subtitle cues. A successful verification records metadata and completes the run while the lease is still active. While the desktop is open, it automatically verifies eligible runs once every captured output format has a managed MP4 and SRT. Runs with missing files stay queued so you can import the remaining subtitles.

A leased worker on the Preview stage can use `render-preview <workspace-id> <run-id> <attempt> <edit-json>` for recorded-source runs. Pass the Preview result object as the edit. The renderer uses the optional cuts below and writes one MP4 per frozen output format, capped at the longest edge saved in the run snapshot. The recorded result includes these filenames in `preview_videos`. Files are stored in the run's delivery directory with the attempt number in the name so revisions retain separate artifacts.

Use `preview-settings` to inspect the app-wide resolution and `set-preview-max-edge <pixels|auto>` to set a cap from 144 to 2160 pixels or restore the CPU-based recommendation. This applies to new runs across every workspace. Runs saved before preview settings were introduced use a 480 px cap.

For recorded-source runs, the Final render stage can assemble managed clips using `render-primary <workspace-id> <run-id> <attempt>`. It requires `ffmpeg`, `ffprobe`, and the `libx264` encoder. It fits each clip to the frozen output dimensions without stretching, preserves source audio, and inserts silence for clips without audio. It writes the primary MP4s and advances to delivery verification. Supply subtitle files separately before verifying.

A Preview result can include `"cuts": [{"asset_id": 2, "start_ms": 500, "end_ms": 1500}]`. List cuts in playback order and use millisecond offsets into videos from the frozen draft source list. Final render consumes the approved Preview result in guided mode and the accepted Preview result in autopilot. Use `format_cuts` to override the shared cuts for a frozen output format, for example `{"format_cuts":{"2":[{"asset_id":2,"start_ms":500,"end_ms":1500}]}}`. A format value of `null` uses whole source clips for that format. Formats without an override use `cuts` or whole source clips in draft order if `cuts` is omitted. Preview and final render use the same format-specific edits.

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

Use `replace_workspace_asset` through MCP or A2A with `workspaceId`, the active `assetId`, and an absolute local `source` path to media of the same kind. The returned `asset` has a new ID. Use that ID for subsequent operations on the replacement.

Use `remove_workspace_asset` through MCP or A2A with `workspaceId`, `assetId`, and the exact saved filename as `confirmedName`. It returns `removed: false` when the asset is missing, referenced, or the filename differs. Removing an asset leaves the original import in place.

Use `import_thumbnail_reference` to import a local image with thumbnail reference notes. Use `set_asset_notes` with the workspace ID, asset ID, and Markdown notes to change the asset's usage guidance. Pass an empty string to clear the notes.

Use `rename_completed_video` to change a completed run's display title. Use `add_completed_video_feedback` to save Markdown feedback about the full run or a specific production result. Pass the result ID from `get_completed_video` for stage feedback.

Use `create_workspace` to initialize a workspace, then `get_workspace_context` and `set_workspace_context` to read or replace its Markdown context. Use `list_profiles`, `create_profile`, and `update_profile` to manage production profiles. Pass a profile's ID as `profile_id` in draft settings. The queue captures the saved context and profile when it creates the run.

Use `list_output_formats`, `create_output_format`, and `update_output_format` to configure workspace output dimensions. Use `get_format_delivery` and `set_format_delivery` for supplemental deliverables. Sending a replacement resets omitted supplemental fields to their defaults. Final video and subtitles remain required.

Use `list_profile_formats` and `get_profile_delivery` to inspect inherited and effective format settings. Use `set_profile_format_overrides` and `set_profile_delivery` to replace one format's overrides for a profile. Omit a field to inherit its workspace value, or send an empty overrides object to inherit all fields in that group. Explicit zero and false values remain overrides.

Use `process_media_stage` to claim and execute a ready media stage for one run. Pass `previewEdit` at Preview, using an empty object for whole clips. Omit it for final rendering or delivery verification. For each rendered format, call `import_delivery_subtitles` with its format ID and an absolute local SRT `source` before verifying delivery.

Use `list_workspace_feedback`, `add_workspace_feedback`, and `update_workspace_feedback` to manage workspace guidance cards. Use `get_brand_colors` and `set_brand_colors` to read or replace the workspace palette. New generation runs capture both inputs.

Use `list_provider_preferences` or `get_provider_preferences` to inspect provider availability, effective ordering, and provenance. Use `set_provider_preferences` to save an order and `clear_provider_preferences` to restore inheritance. Omit `workspaceId` for app scope. Passing an empty order explicitly disables providers for the capability at that scope.

Use `get_plugin_settings`, `set_plugin_setting`, and `clear_plugin_setting` to manage SDK-declared non-secret settings. Omit `workspaceId` for app scope. Clearing a workspace override restores inheritance. Stored credentials are excluded from these tools.

Use `list_agent_plugins` to discover bundled production agents and `get_agent_settings` to read the effective choice. Use `set_agent_provider` or `clear_agent_provider` at app or workspace scope. Omit `workspaceId` for app scope. Changes affect future run snapshots.

Use `get_preview_settings` to inspect preview resolution. Set `maxEdge` with `set_preview_max_edge` to a value between 144 and 2160 pixels or pass null to restore the app recommendation. Queued runs retain their captured preview settings.

Use `list_plugin_catalog` to discover plugin IDs and declared settings. Use `get_plugin_credential_status` for a declared credential and optional workspace ID. The response reports configuration flags and the effective source. Set credentials through desktop Settings or the CLI's redirected-stdin command.

Use `clear_plugin_credential` with a plugin and setting ID to remove an app credential. Include `workspaceId` to clear only that workspace's credential and restore inheritance.

Use `list_plugin_context_requests` with a workspace ID to inspect packages that request workspace context access. After reviewing a package, call `apply_reviewed_plugin_context_grant` with the workspace ID, plugin ID, the listed package SHA-256, and the desired `allowed` value. The host rejects a digest that no longer matches the installed package.

Use `install_plugin_zip` with an absolute local source path and the expected `packageSha256` to install a reviewed package. The digest must match the archive before registration. The response includes the installed catalog entry and the third-party plugin notice. Workspace context access requires a separate grant.

Use `install_plugin_release_from_snapshot` or `update_plugin_release_from_snapshot` with absolute local `catalog` and ZIP `source` paths, the plugin ID, and an exact version. The host verifies the archive against the catalog and records release provenance. Offline updates require a newer release from the same repository.

Use `fetch_plugin_registry` to inspect an HTTPS registry catalog, or `read_plugin_registry_snapshot` to validate a catalog at an absolute local path. Both return release metadata for review before installation.

Use `install_reviewed_plugin_release` and `update_reviewed_plugin_release` with the catalog URL, plugin ID, repository, and exact release object returned by registry discovery. The host checks the metadata again before downloading the package and verifies the archive digest before registration. Updates require a newer release from the original registry and repository.

### Local A2A

Run `arrumo-workspaces <database-path> a2a <port>` with a randomly generated 32–256 character alphanumeric bearer token on redirected stdin. Use port `0` to let the OS choose a port. The command binds only to `127.0.0.1` and prints its endpoint when ready. Anyone with the token has the same access to that database and local operations as the CLI operator.

Read `/.well-known/agent-card.json` at the printed origin for the [A2A 1.0 JSON-RPC interface](https://a2a-protocol.org/latest/specification/#9-json-rpc-protocol-binding). Send requests to the printed endpoint with `Authorization: Bearer <token>`, `Content-Type: application/json`, and `A2A-Version: 1.0`. For example, send this JSON body to discover operation names and arguments.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "SendMessage",
  "params": {
    "message": {
      "messageId": "discover-operations-1",
      "role": "ROLE_USER",
      "parts": [{"data": {"operation": "list_operations", "arguments": {}}}]
    }
  }
}
```

Use the same message format with a listed operation name and arguments. The reply contains a data part with `result` or an operation `error`. Most operations return direct messages. Enqueue and replay return a task. Use its ID with `GetTask` or `CancelTask`. Task status reflects the generation queue, and completed tasks include verified output metadata. Use `ListTasks` to browse tasks, optionally filtering by `contextId` (such as `workspace-1`), `status`, or `statusTimestampAfter`. Pass `nextPageToken` back with the same filters to fetch the next page. Concurrent state changes can reorder the list between pages. Set `includeArtifacts` to include verified outputs for completed tasks. Use `SendStreamingMessage` with the same message body to receive Server-Sent Events, or `SubscribeToTask` with an existing active task ID. Streams send the initial result and then task snapshots as the observed state changes. Streams close at a terminal state or after 60 seconds. Reconnect to an active task for a fresh snapshot. The server accepts up to eight concurrent streams.

To submit a decision on a task waiting for review, send a follow-up message with its `taskId` and the `review_production_result` operation. Pass the current `pendingReviewId` as `resultId`, plus the decision and optional notes. The task ID supplies the workspace and run. If you include `contextId`, it must match the task. A successful decision returns the updated task.

Use `remove_output_format` with a workspace and format ID to remove that format from future runs. Captured generation settings retain their original formats.

Use `list_brand_assets`, `attach_brand_asset`, and `detach_brand_asset` through MCP or A2A to manage which workspace images serve as identity assets or style references. Removing a role keeps the image in the workspace library.

Use `remove_reviewed_plugin_package` through MCP or A2A with the plugin ID and its installed package SHA-256. A mismatched digest leaves the installed revision intact. Uninstalling removes that package's stored settings and grants.

Use `get_agent_login_status` through MCP or A2A with the bundled agent plugin ID to check local sign-in readiness. The host returns a status and discards the probe's output.

Delete an unqueued draft with `delete-draft <workspace-id> <draft-id> <confirmed-title>` in the CLI or `delete_video_draft` through MCP and A2A. Pass the exact saved title. A draft referenced by a run stays available for that run and its replays. Managed sources remain in the workspace library.

Use `delete-completed-video <workspace-id> <run-id> <confirmed-title>` in the CLI to remove a completed run and its managed delivery directory. Use the exact current display title. A run referenced by a replay stays available. If file cleanup is interrupted, rerun the same command to finish the pending cleanup. Shared source assets stay in the workspace library.

Through MCP or A2A, use `delete_completed_video` with `workspaceId`, `runId`, and the exact `confirmedTitle`. It uses the same deletion and resumable file cleanup as the CLI command.

Use `recover-delivery-cleanup <workspace-id>` in the CLI or `recover_delivery_cleanup` in MCP and A2A to retry pending cleanup for previously deleted runs. The report lists completed cleanup and entries that still require attention.

Use `delete-generation <workspace-id> <run-id> <confirmed-title>` or the shared `delete_generation` operation to delete a queued, active, or terminal run. A successful deletion invalidates any worker lease and removes the managed delivery directory. The draft and shared assets remain available. External workers must stop using run paths when their lease becomes invalid.

Schedule a verified output with `set-output-schedule <workspace-id> <run-id> <format-id> <YYYY-MM-DD|clear>`. Each output format can have its own calendar date. Use `scheduled-outputs <workspace-id> <start-date> <end-date> [profile-id]` to list an inclusive range. Calendar entries include the saved profile color and output dimensions.

Through MCP or A2A, use `set_output_schedule` with a workspace, run, and format ID. Pass an ISO calendar `date` or null to clear it. Use `list_scheduled_outputs` with `start` and `end` dates and an optional `profileId` to browse the schedule.

Open the desktop calendar at a specific month and profile with a link such as `arrumo://open/calendar?workspace=1&profile=2&month=2028-02`. Omit the profile for all workspace outputs. Completed-video links can include a `format` ID to select a specific delivered output.

Use `set-batch-asset-notes <workspace-id> <asset-ids-json> <markdown>` in the CLI, or `set_batch_asset_notes` in MCP and A2A, to replace notes for multiple workspace assets together. A missing or foreign asset rolls back the entire update.

Read asset labels and grouping with `asset-organization <workspace-id> <asset-id>` or `get_asset_organization` through MCP and A2A. Replace them with `set-asset-organization <workspace-id> <asset-id> <settings-json>` or `set_asset_organization`. Settings contain an optional `group` and a `labels` array. An empty settings object clears both without changing the asset or its references.

Use `replace-asset <workspace-id> <asset-id> <source-path>` to import a same-kind replacement. It returns the new asset ID, carries forward usage notes and organization, and updates draft, brand, and thumbnail references. The previous media stays available to generation runs that captured it. Original files remain in place.

Use `remove-asset <workspace-id> <asset-id> <confirmed-name>` to remove an unused asset and its managed file. Use the exact saved filename. Draft sources, brand assignments, thumbnail references, and captured run references protect their assets from removal. If file cleanup is interrupted, repeat the same command to finish it. At startup, the desktop app retries pending asset cleanup and reclaims superseded managed files without remaining references.

### Animation and transition library

Use `create-motion-item <workspace-id> <settings-json>` to register existing managed media as a reusable library item. The settings accept `kind` (`animation` or `transition`), `title`, `usage_markdown`, and `preview_asset_id` (video). Optional `thumbnail_asset_id` (image) and `sound_asset_id` (audio) refer to assets in the same workspace. Omit `profile_id` for workspace-wide use.

Use `motion-library <workspace-id> workspace` for workspace-wide items, or pass a profile ID to include items scoped to that profile. Several entries can refer to the same media without copying it.

Use `update-motion-item <workspace-id> <item-id> <settings-json>` to replace its settings with the same fields as creation. Use `duplicate-motion-item <workspace-id> <item-id> <title>` to create a new entry with the source scope, usage notes, and media. Use `delete-motion-item <workspace-id> <item-id> <confirmed-title>` with the exact saved title. Deletion removes the library entry and keeps its assets in the workspace.

MCP and A2A expose the same library through `list_motion_library`, `create_motion_library_item`, `update_motion_library_item`, `duplicate_motion_library_item`, and `delete_motion_library_item`. Use `workspaceId` and, where applicable, `profileId`, `itemId`, or `confirmedTitle` as top-level arguments. The nested `settings` uses the same field names as the CLI.

Read a profile-scoped item's additional recipients with `motion-item-shares <workspace-id> <item-id>`. Replace the recipient list with `set-motion-item-shares <workspace-id> <item-id> <profile-ids-json>`. Use an empty array to clear sharing. The owning profile is implicit, and workspace-wide entries are already visible to every profile. Duplicating a profile item copies its recipients.

In MCP and A2A, call `get_motion_library_shares` with `workspaceId` and `itemId`. Call `set_motion_library_shares` with those arguments and `profileIds` to replace the additional recipients, or pass an empty array to clear them.

In the desktop Motion library, select a profile to see its entries alongside workspace-wide items. Use **Share with profiles** on a profile-scoped item to add or remove recipients. A link such as `arrumo://open/motion-library?workspace=1&profile=2` opens that profile's library view.

Use **Play preview** to open a looping video preview and audition linked sound separately. Playback depends on the desktop webview supporting the media codec.

An Assets stage result can include a `motion_items` array. Each entry uses the same settings fields as `create-motion-item` and refers to managed assets and, optionally, a profile in the run's workspace. The host rejects the entire submission if a reference is invalid. Saved results protect their referenced media from cleanup until the run is removed.

Use `save-motion-result <workspace-id> <run-id> <result-id> <index>` to save one accepted Assets stage motion result to the library. The index is zero-based. Repeating the command returns the existing entry while it remains in the library. Saved entries retain the run and result IDs and share the exact managed media, including earlier versions that were replaced after the run.

In MCP and A2A, call `save_motion_result` with `workspaceId`, `runId`, `resultId`, and the zero-based `index` in the accepted Assets result's `motion_items` array. The response includes the saved `item`.

Accepted motion results are saved to the library by default when an Assets stage advances, including in autopilot. Set `save_motion_to_library` to `false` in the run options or turn off **Save motion to library** when queuing a desktop run. You can still save an item manually from its accepted result.

During guided Assets review, select which animations and transitions to keep before approving. In MCP and A2A, pass `saveMotionIndexes` to `review_production_result`. In the CLI, append a JSON array of indexes to `review-stage`. Omit the selection to use the run default, or pass an empty array to save none. Indexes are zero-based.

A local renderer worker can submit finished files with `submit-motion-artifacts <workspace-id> <run-id> <attempt> <artifacts-json>` while it holds the Assets-stage lease. Pass an array of objects with `kind`, `title`, `usage_markdown`, and an absolute local `preview` video path. Optional fields are `profile_id`, `thumbnail` (image path), and `sound` (audio path). The host imports the files and submits their managed IDs together. A rejected batch leaves the run at its current stage and does not publish catalog entries. The input files remain with the worker.

MCP and A2A expose `submit_motion_artifacts` with `workspaceId`, `runId`, `attempt`, and `artifacts`. Each artifact uses the CLI's field names and absolute paths on the machine running Arrumo. The response includes `resultId`.

On a queued desktop run at the Assets stage, use **Submit motion files** to add one or more finished animations and transitions, each with a video preview and optional thumbnail and sound. Use **Add another item** to include more in the same submission. Submitting completes the stage or sends it to guided review, according to the run options.

Motion artifact submission requires `ffprobe`. The host probes each managed copy with the demuxer for its extension and checks for the expected video or audio stream before publishing the batch.

On Linux, run an installed motion-rendering package with `run-installed-motion-renderer <plugin-id> <digest> <sdk-worker> <output-directory> [toolchain]`. Use the installed package digest from the plugin catalog and point `sdk-worker` at your trusted local Plugin SDK checkout's `tools/run_motion_package.py`. Arrumo checks that the installed ZIP still matches the digest and declares `motion-render`. The SDK worker runs it in Bubblewrap and returns a JSON artifact batch. Submit that batch separately with the active Assets-stage lease.

For a queued Assets stage, use `render-run-motion <workspace-id> <run-id> <plugin-id> <digest> <sdk-worker> <output-directory> [toolchain]` to render and submit in one command. Arrumo claims the queued stage before rendering and renews the lease until the worker exits. If the lease is lost, the command stops its worker or refuses publication. Rendered files remain in the output directory. A successful command prints the production result ID.

For desktop development on Linux, launch Arrumo with `ARRUMO_MOTION_SDK_WORKER` set to the absolute path of the trusted SDK checkout's `tools/run_motion_package.py`. Optionally set `ARRUMO_MOTION_TOOLCHAIN` to a local toolchain directory. At a queued Assets stage, **Render motion** runs the selected installed renderer and submits its files through the same operation as the CLI. The desktop host owns the temporary output directory and removes it when the operation ends.

In MCP and A2A, call `render_run_motion` with `workspaceId`, `runId`, `pluginId`, and `packageSha256`. Launch the transport host with the same Linux worker configuration as the desktop. The host creates and cleans up its own temporary output directory. The call waits for publication and returns `resultId`. A2A accepts up to two concurrent render requests and continues serving task status and cancellation during rendering.

To build a Linux Debian package with the SDK worker, download the worker ZIP from a trusted Plugin SDK build. Run `python3 scripts/stage_motion_worker.py <worker.zip> <expected-sha256>` from a clean editor checkout, then run `pnpm tauri build --bundles deb --config src-tauri/tauri.motion-worker.conf.json`. The package declares Python, JSON Schema, Bubblewrap, util-linux, and FFmpeg dependencies. The installed desktop discovers the worker in its application resources. A process-level `ARRUMO_MOTION_SDK_WORKER` setting takes precedence for development and testing. Headless hosts still use the explicit worker setting. [Tauri resources](https://v2.tauri.app/develop/resources/)

Use `process-motion-stage <workspace-id> <run-id>` for one-shot worker dispatch with `ARRUMO_MOTION_SDK_WORKER` configured. At queue time, Arrumo captures the installed package digest for the first provider in the effective `motion-render` preference. To pick a renderer for one run, set `provider_overrides` to an object such as `{"motion-render":["my-renderer"]}`. An explicit empty order disables installed motion renderers for that run. The one-shot command returns the result ID after publishing or `null` if there is no queued Assets stage with a captured renderer. If the captured package is missing or has changed, the command fails and leaves the stage queued.

On Linux, the desktop also watches the motion queue while the app is open if it
finds the bundled SDK worker or an explicit `ARRUMO_MOTION_SDK_WORKER`. It
automatically starts eligible Assets stages using the renderer captured at
queue time. The result appears in the run's existing review or autopilot flow.
If the host exits mid-render, Linux stops the SDK worker. Arrumo can recover
the stage once the original attempt lease expires. Desktop and queue-worker
startup remove abandoned managed motion scratch beside the database. File
locks protect scratch directories still in use by another Arrumo process.

Run `watch-motion-queue` as a foreground Linux worker with `ARRUMO_MOTION_SDK_WORKER` configured. It polls the shared database, starts the oldest eligible Assets stage with a captured installed package, and prints each published result ID. Multiple worker processes use the same attempt lease, so only one can publish a particular stage. An expired Assets lease can be reclaimed after a restart. Runs whose captured renderer package is missing or has changed stay queued while other eligible runs proceed.

From a completed video's output, select **Subtitle text** to open the SRT file alongside its video. Search the transcript or select a cue to seek to it. A link such as `arrumo://open/completed-video?workspace=1&run=3&format=2&file=subtitles` opens that output's transcript directly. Linux WebKit playback of H.264 requires a compatible GStreamer decoder. The editor's Debian motion-worker package declares `gstreamer1.0-libav`.

The CLI command `completed-subtitles <workspace-id> <run-id> <format-id> [revision]` returns the subtitle text and revision number as JSON. Omit the revision for the latest version or pass `0` for the verified original. Use `revise-completed-subtitles <workspace-id> <run-id> <format-id> <expected-revision> <srt-file>` to append a version. The command validates cue timing against the verified video and rejects a stale expected revision. The completed-video viewer and ZIP export read the latest revision.

Additional language tracks are scoped to one verified output format and have independent revision histories. Use `completed-subtitle-languages <workspace-id> <run-id> <format-id>` to list them and `completed-subtitle-track <workspace-id> <run-id> <format-id> <language> [revision]` to read one. Use `revise-completed-subtitle-track <workspace-id> <run-id> <format-id> <language> <expected-revision> <srt-file>` with expected revision zero to add a track. Language tags such as `pt-PT` are stored in lowercase. The delivery ZIP includes the latest revision of each language as `<format-id>.<language>.srt` alongside the primary subtitle.

MCP and A2A provide `list_completed_subtitle_languages` and accept an optional `language` on `get_completed_subtitles` and `revise_completed_subtitles`.

In the desktop subtitle viewer, use **Subtitle language** to switch between the primary subtitles and additional tracks. **Add language** creates a track from a language tag and translated SRT. **Edit subtitles** opens the selected track for editing. **Save version** adds a validated revision, and the history arrows browse that track’s revisions.

MCP and A2A expose `get_completed_subtitles` with `workspaceId`, `runId`, `formatId`, and optional `revision`. Submit `revise_completed_subtitles` with those three IDs, `expectedRevision` from the latest read, and `subtitles` containing the complete SRT text. Both transports have a 128 KiB request envelope. Use the desktop or CLI to save larger SRT files within the 2 MiB subtitle limit.

At the `supplemental_outputs` stage, workers can submit `supplemental_text` in the stage result. Each item has a `kind` of `title`, `description`, or `tags`, a UTF-8 `text` value, and an optional `format_id` from the frozen run. Omit the format for text shared by the whole video. Arrumo accepts up to 100 entries and 64 KiB of combined text. The accepted stage result supplies the completed-video file links and adds corresponding `supplemental-<kind>-<index>.txt` files to the delivery ZIP. Indexes start at 1 and follow submission order.

A `supplemental_outputs` result can also include up to 100 `supplemental_media` entries with an `asset_id` and optional frozen `format_id`. Import each image, video, or audio file into the run's workspace before submitting its asset ID. The run retains those exact asset versions even after a library replacement. Completed-video reads expose the accepted media metadata and ZIP export includes numbered `supplemental-media-<index>.<extension>` files.

The completed-video file viewer opens supplemental images, audio and video from the captured asset reference. For example, `arrumo://open/completed-video?workspace=1&run=3&file=media-1` opens the first supplemental media item. Playback depends on the desktop WebView's installed codecs.

For selective approval, use `review-supplemental-items <workspace-id> <run-id> <result-id> <decisions-json> <notes>`. The JSON has `text` and `media` arrays, each in the order of the corresponding submitted items. Each decision is `approved`, `revise`, or `rejected`. Include an empty array when a category has no items. The run advances only when every item is approved. Otherwise, the next Supplemental Outputs submission supplies only replacements for the unresolved items, in their original order within each category. Text replacements keep their kind and scope. Media replacements keep the media type and scope. Arrumo merges the replacements into their original positions and retains approved entries. Workers can read the decisions and shared notes from production progress.

In desktop guided Outputs review, choose a decision per item or use a batch button to set all items at once. You can override any item before sending. Arrumo stages your choices and shared notes locally until you select **Send**.

MCP and A2A support `review_supplemental_items` with `workspaceId`, `runId`, `resultId`, `review` (the `text` and `media` decision arrays), and optional `notes`. In an A2A task message, the task ID supplies the workspace and run scope.

Check individual outputs to apply a batch decision only to the selected rows. After a partial revision, prior approvals fill in automatically and the replacement items need a new decision. Review choices stay editable until Send.

A supplemental review may include `review.item_notes` with `text` and `media` arrays aligned with the decision arrays. Use `null` for an item without a note. Each note has a 4 KiB UTF-8 limit. The desktop note buttons stage feedback on the corresponding row until Send. Workers receive the notes with the saved review in production progress.

When a worker claims a Supplemental Outputs stage after a selective review, the claim includes `supplemental_revision`. This carries the source result ID, shared notes, and separate `text` and `media` arrays of unresolved items. Each item has its zero-based slot index, decision, optional note, and previous submitted value. Submit one replacement per listed item in array order. The same request survives lease expiry and takeover. Other claims omit this field.

Use `review-preview-formats <workspace-id> <run-id> <result-id> <decisions-json> <notes>` to review each rendered preview. Supply an array such as `[{"format_id":1,"decision":"approved"},{"format_id":2,"decision":"revise"}]` with one decision per frozen format. If any format needs replacement, claim the stage and use `render-preview` again. The renderer keeps approved files and cuts and rerenders only unresolved formats. A partial revision requires the managed renderer. Once every format is approved, final rendering uses each format's retained or revised cuts.

Desktop Preview review offers individual and selected-format batch decisions for rendered previews that cover all frozen formats. Approved formats are preselected after a partial revision. You can attach a note to a format before Send, or include an optional `note` in each CLI format decision.

MCP and A2A expose the same operation as `review_preview_formats`. A worker claim after a selective review includes `preview_revision`, with the source result ID, shared notes, and only the unresolved formats. Each entry includes the decision, optional format note, previous preview filename, and effective cuts. The request survives lease takeover. To produce the replacement files, use the managed renderer through `render-preview` or the desktop.

To add a file produced after completion, import it into the run's workspace and use
`attach-completed-media <workspace-id> <run-id> <format-id> <asset-id>`.
MCP and A2A expose the same operation as `attach_completed_media` with
`workspaceId`, `runId`, `formatId`, and `assetId`. A repeated attachment
returns false in the CLI or `{"added":false}` through MCP and A2A.
Attachments appear in supplemental files with a stable `attachment_id`, which
also identifies their delivery ZIP entry as `attachment-<id>.<extension>`.
Use `detach-completed-media <workspace-id> <run-id> <attachment-id>` or
`detach_completed_media` with `workspaceId`, `runId`, and `attachmentId`
to remove the attachment while keeping the asset in the workspace. A repeated
removal returns false. Other attachment links keep their IDs.

In the desktop Video tab, select the output format and use **Attach file** under **Other files in this video** to choose a managed workspace asset. In the desktop file viewer, **Detach file** removes a post-completion attachment from the delivery and returns to the video. The managed file remains in workspace Assets.

Use `synthesize-speech <workspace-id> <plugin-id> <voice-id> <new-audio-file>` to send UTF-8 text on stdin to a trusted bundled speech provider. The output extension must match the adapter's response format. The editor resolves the configured workspace credential with app fallback, validates the returned audio with FFprobe, and saves it without overwriting an existing file. This command makes one provider request per invocation.

The local CLI command `verified-video-source <workspace-id> <run-id> <format-id>` prints the verified MP4 path as JSON for workflows running on the same machine.

Use `speech-provider-status <workspace-id> <plugin-id>` to read the bundled
provider's response format, maximum cue length, adapter digest, and configured credential scope
before synthesis. The command makes no provider request.

Use `import-completed-video-variant <workspace-id> <run-id> <format-id> <source.mp4>`
to add a rendered alternate video. The editor verifies the managed copy's
dimensions and duration against the selected completed output and requires an
audio track before committing the asset and attachment together.

MCP and A2A expose the same import as `import_completed_video_variant` with
`workspaceId`, `runId`, `formatId`, and an absolute local `source` path.
Identical bytes submitted again to the same run and format return the original managed asset. Detaching the file allows a fresh import. Attachments created before digest tracking retain their existing links.

The desktop Video tab exposes this verification under **Import alternate MP4**.
Select the intended output format before opening the file picker.

### Durable plugin actions

Send a JSON object with `id` (a caller-generated UUID), `pluginId`,
`actionId`, `packageSha256`, `input`, and `workspaceId` for workspace
actions to `queue-plugin-action-job` on stdin. Repeating the same request
returns the existing job. Reusing the UUID with changed input or scope fails.

Read a job with `plugin-action-job <app|workspace-id> <job-id>`. Run a queued
job with `process-plugin-action-job <app|workspace-id> <job-id>`. Processing
checks the reviewed package again, retains the result, and can reclaim a job
after its two-minute lease expires. MCP and A2A expose these as
`queue_plugin_action_job`, `get_plugin_action_job`, and
`process_plugin_action_job`. These jobs run declared Wasm actions in the
existing sandbox. Job requests persist their input, so include only the data
the action needs.

Run `arrumo-workspaces <database-path> watch-plugin-action-queue` as a
foreground worker for queued Wasm actions when the desktop is closed. It
claims queued jobs and recovers expired leases. Read each outcome through
the same job status commands. The worker writes no job input or result to
its console. Stop it with Ctrl+C.

The desktop **Plugin actions** panel records each run as a job and includes
recent jobs in the selected scope. While the desktop app is open, it processes
queued jobs and recovers interrupted jobs after their leases expire. The panel
refreshes when a job finishes. Use **Process job** to claim a specific job when
it is available. To list jobs from the CLI, use
`list-plugin-action-jobs <app|workspace-id>`. MCP and A2A expose
`list_plugin_action_jobs`.

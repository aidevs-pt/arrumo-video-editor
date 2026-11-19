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

After claiming a run, use `prepare-generation-plan <workspace-id> <run-id> <attempt>` to persist its stage plan from the frozen snapshot. The operation requires an active lease and returns the same plan on repeat attempts. Use `generation-plan <workspace-id> <run-id>` to read it. Guided plans flag the stages that require review. Autopilot plans omit those review requirements.

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

In the desktop Generation queue, open **Completed video details** to review the verified file list, save a display title, or export the delivery through the native ZIP save dialog.

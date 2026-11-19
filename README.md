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

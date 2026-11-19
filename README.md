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

Both commands print JSON. The database persists across runs. Run the backend tests with `cargo test --locked`.

Copyright © 2026 DEVS AI Unipessoal LDA. Licensed under the [GNU Affero General Public License v3.0](LICENSE).

For custom AI solutions for your company, visit [DEVS AI](https://aidevs.pt).

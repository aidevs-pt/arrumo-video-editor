# Arrumo Video Editor

The Tauri and Next.js desktop editor and shared application API for the UI, CLI, MCP, and A2A.

User, developer and plugin guides live in [Arrumo Docs](https://github.com/aidevs-pt/arrumo-docs), in English, Spanish and Portuguese.

## Guides

- Create, review and export a video: [English](https://github.com/aidevs-pt/arrumo-docs/blob/main/content/docs/en/user/first-video.md) · [Español](https://github.com/aidevs-pt/arrumo-docs/blob/main/content/docs/es/user/first-video.md) · [Português](https://github.com/aidevs-pt/arrumo-docs/blob/main/content/docs/pt/user/first-video.md)
- Connect through the CLI, MCP or A2A: [English](https://github.com/aidevs-pt/arrumo-docs/blob/main/content/docs/en/developer/connect.md) · [Español](https://github.com/aidevs-pt/arrumo-docs/blob/main/content/docs/es/developer/connect.md) · [Português](https://github.com/aidevs-pt/arrumo-docs/blob/main/content/docs/pt/developer/connect.md)
- Official plugins: [plugin guides](https://github.com/aidevs-pt/arrumo-plugin-registry#plugin-guides)

Agents that drive Arrumo over MCP can load [`skills/arrumo/SKILL.md`](skills/arrumo/SKILL.md), the editing workflow and review checklist, as a Claude Code or Codex skill.

## Reference

Every command, MCP tool and A2A operation, by domain ([English](https://github.com/aidevs-pt/arrumo-docs/blob/main/content/docs/en/developer/reference/index.md) · [Español](https://github.com/aidevs-pt/arrumo-docs/blob/main/content/docs/es/developer/reference/index.md) · [Português](https://github.com/aidevs-pt/arrumo-docs/blob/main/content/docs/pt/developer/reference/index.md)):

| Page | Covers |
| --- | --- |
| [Build and run the editor](https://github.com/aidevs-pt/arrumo-docs/blob/main/content/docs/en/developer/reference/build.md) | Desktop development, bundled plugin inputs, translations, deep links, usage counts and crash reports |
| [Workspaces, profiles and assets](https://github.com/aidevs-pt/arrumo-docs/blob/main/content/docs/en/developer/reference/workspaces.md) | Backups, profiles, output formats and deliverables, assets, generated library media, context and brand |
| [Drafts, runs and review](https://github.com/aidevs-pt/arrumo-docs/blob/main/content/docs/en/developer/reference/runs.md) | Drafts, the queue and workers, the editing agent, agent logs, stage results, completed videos, schedule, notifications |
| [Rendering, subtitles and delivery](https://github.com/aidevs-pt/arrumo-docs/blob/main/content/docs/en/developer/reference/rendering.md) | Previews, cuts, final render and loudness, delivery checks, subtitles, export, attachments |
| [Animation and transition library](https://github.com/aidevs-pt/arrumo-docs/blob/main/content/docs/en/developer/reference/motion.md) | Library items, agent-designed animations, installed motion renderers |
| [Plugins and providers](https://github.com/aidevs-pt/arrumo-docs/blob/main/content/docs/en/developer/reference/plugins.md) | Catalog, installation and the on/off switch, settings and credentials, provider order, runtime, actions, jobs and hooks |
| [MCP and A2A](https://github.com/aidevs-pt/arrumo-docs/blob/main/content/docs/en/developer/reference/transports.md) | Connecting a client to each transport |

## Develop

Install the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/), Node, and the pnpm version in `package.json`.

```sh
pnpm install --frozen-lockfile
pnpm tauri dev
```

Run the backend tests with `cargo test --locked`. In the Arrumo workspace layout, the build loads the default plugins from the sibling `plugin-registry` and `plugins/<id>` checkouts; set `ARRUMO_CORE_ONLY=1` to build without them. See [Build and run the editor](https://github.com/aidevs-pt/arrumo-docs/blob/main/content/docs/en/developer/reference/build.md) for release builds and the other variables.

Copyright © 2026 DEVS AI Unipessoal LDA. Licensed under the [GNU Affero General Public License v3.0](LICENSE).

For custom AI solutions for your company, visit [DEVS AI](https://aidevs.pt).

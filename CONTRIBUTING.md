# Contributing

Thank you for helping improve Arrumo AI Video Editor, the desktop app. Bug reports, fixes, translations, documentation and
ideas are all welcome. You can write in English, Spanish or Portuguese.

## Before you start

- **Found a security problem?** Don't open an issue. Follow [SECURITY.md](SECURITY.md).
- **Questions and ideas** go to [Discussions](https://github.com/aidevs-pt/arrumo-video-editor/discussions). **Bugs** go to this repository's
  issues, one per problem.
- **For anything larger than a small fix,** open an issue first so we can agree on the approach
  before you spend time on it.
- Everyone taking part follows our [Code of Conduct](CODE_OF_CONDUCT.md).

## Develop

Install the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/), Node and the
pnpm version in `package.json`. Clone this repository next to
[`arrumo-plugin-registry`](https://github.com/aidevs-pt/arrumo-plugin-registry) and the
default plugins under `plugins/<id>`, or set `ARRUMO_CORE_ONLY=1` to build without them.

```sh
pnpm install --frozen-lockfile
pnpm tauri dev
```

Run the same checks as CI before opening a pull request:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
pnpm build
```

## Guidelines

- **Business rules live in `crates/workspaces`**, behind the application API shared by the
  desktop UI, the CLI, MCP and A2A. A new feature should work through all of them, with a test.
- **Provider-specific behaviour belongs in a plugin**, not in the editor. The editor implements
  the generic Plugin SDK contracts.
- **Database changes are new SQLite migrations**; never edit a migration that has shipped.
- **Interface text is translated** in `locales/en.json`, `es.json` and `pt.json`. Spanish uses
  "tú" and "complementos"; European Portuguese uses "tu".
- **Small, focused commits** with [Conventional Commits](https://www.conventionalcommits.org/)
  messages, such as `fix(render): keep the last frame when a clip ends early`.
- **Tests for behaviour changes.** Bug fixes include a test that fails without the fix.
- **Never commit credentials, footage, exports or machine-specific settings.**

## Pull requests

1. Fork the repository and create a branch from `main`.
2. Make your change, with tests and documentation.
3. Open a pull request and fill in the template. CI must pass.
4. The first time you contribute, a bot asks you to sign the
   [Contributor License Agreement](CLA.md) by replying with a comment. You sign it once per
   repository.
5. A maintainer reviews the change. We squash-merge, so the pull request title becomes the commit
   message.

## License

This project is licensed under the [GNU Affero General Public License v3.0](LICENSE) and owned by DEVS.AI UNIPESSOAL LDA. The [CLA](CLA.md) lets DEVS AI distribute your contribution under this
license and under other terms in the future; you keep the copyright to your work.

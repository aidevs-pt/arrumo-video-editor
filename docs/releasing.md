# Releasing Arrumo

Releases are built by the **Release** workflow (`.github/workflows/release.yml`). Run it by hand from the repository's Actions tab. It:

1. sets the new version with `scripts/set-version.mjs` (`patch`, `minor`, `major`, or an exact version), commits `chore(release): vX.Y.Z` as the person who ran it, tags `vX.Y.Z` and drafts a GitHub release;
2. builds and signs Linux (`.deb`, `.rpm`, AppImage), Windows (NSIS and MSI) and macOS (universal `.dmg`) on GitHub's runners;
3. bundles the default plugins from their published registry releases, after checking each ZIP's digest against `plugin-registry/catalog.json`;
4. uploads the installers, the signed updater packages and `latest.json`, then publishes the release.

Installed copies of Arrumo read `latest.json` from the latest release, check its signature against the public key built into the app, and offer the update.

## One-time setup

1. **Enable Actions** for `aidevs-pt/arrumo-video-editor` (Settings › Actions › General).
2. **Create the updater signing key on your own computer.** Keep the private key and its password in your password manager, and keep an offline backup. If they are lost, installed copies can never be updated again.

   ```sh
   pnpm tauri signer generate -w ~/.tauri/arrumo-updater.key
   ```

3. **Create the `release` environment** (Settings › Environments). Add yourself as a required reviewer if every release should wait for your approval.
4. **Add the variables** (Settings › Secrets and variables › Actions › Variables):
   - `RELEASE_ACTORS`: the GitHub accounts allowed to release, separated by commas (for example `IAntonioDev`).
   - `ARRUMO_UPDATER_PUBKEY`: the contents of `~/.tauri/arrumo-updater.key.pub`. The public key is not secret.
5. **Add the secrets**:

   | Secret | What it is |
   | --- | --- |
   | `TAURI_SIGNING_PRIVATE_KEY` | The contents of `~/.tauri/arrumo-updater.key` |
   | `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Its password |
   | `ARRUMO_CRASH_DSN` | The Sentry project DSN |
   | `ARRUMO_USAGE_KEY` | The Aptabase app key |
   | `ARRUMO_REPOS_TOKEN` | A fine-grained token with read access to the plugin and registry repositories. Remove it once they are public. |
   | `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY` | The Developer ID Application certificate (base64 `.p12`), its password and its identity name |
   | `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | For notarization; `APPLE_PASSWORD` is an app-specific password |

   | `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID` | The Azure app registration allowed to sign (Windows) |

6. **Set up Windows signing** with [Azure Artifact Signing](https://learn.microsoft.com/azure/artifact-signing/), which is open to organisations in the EU. Without it the Windows build is unsigned, and SmartScreen warns on first install.
   1. In the Azure portal, create an **Artifact Signing account**. Note its region's endpoint, for example `https://weu.codesigning.azure.net` for West Europe.
   2. In the account, complete **Identity validation** for DEVS AI Unipessoal LDA as a public organisation. Microsoft checks the company's official records; this can take a few days.
   3. Create a **Public Trust** certificate profile on the validated identity.
   4. In Microsoft Entra ID, create an **App registration** and a client secret under Certificates & secrets.
   5. On the Artifact Signing account (Access control), give that app the **Artifact Signing Certificate Profile Signer** role (called *Trusted Signing Certificate Profile Signer* on older pages).
   6. Add the variables `AZURE_ARTIFACT_SIGNING_ENDPOINT`, `AZURE_ARTIFACT_SIGNING_ACCOUNT` (the account name) and `AZURE_ARTIFACT_SIGNING_PROFILE` (the certificate profile name), and the three `AZURE_*` secrets above.

   The release build installs [`artifact-signing-cli`](https://github.com/Levminer/artifact-signing-cli). Tauri runs it on the app executable and both installers, before it signs the update files, so updates install the signed build.

## Before each release

- Publish a registry release for any default plugin that changed. The build stops if a plugin has no release, or if a release ZIP differs from its catalog digest.
- Run the local checks: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `pnpm build`.

## When a release breaks

Publish a fixed release as usual. Arrumo marks each launch before anything else runs, and removes the mark once the interface has stayed up for 30 seconds, or on a normal quit after the interface appeared. If a launch fails before then, the next one checks for an update before opening any data, and installs it without asking (waiting up to 90 seconds), so a release that crashes on start can still be replaced.

Crash reports follow the creator's saved choice from the first moment, even when the data can't be opened. They never include native crash dumps or breadcrumbs.

Builds made without `ARRUMO_UPDATER_PUBKEY` (contributor builds), or with `ARRUMO_STORE_BUILD` set (store builds), never check for updates. Builds without `ARRUMO_CRASH_DSN` or `ARRUMO_USAGE_KEY` send nothing.

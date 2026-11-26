# Releasing Arrumo

Releases are built by the **Release** workflow (`.github/workflows/release.yml`). Run it by hand from the repository's Actions tab. It:

1. sets the new version with `scripts/set-version.mjs` (`patch`, `minor`, `major`, or an exact version), commits `chore(release): vX.Y.Z` as the person who ran it, tags `vX.Y.Z` and drafts a GitHub release;
2. builds Linux (`.deb`, `.rpm`, AppImage), Windows (NSIS and MSI) and macOS (universal `.dmg`) on GitHub's runners, signing Windows and macOS when their credentials are set (see [Previews and unsigned builds](#previews-and-unsigned-builds)). The macOS job gives the disk image a Retina background and Arrumo's install-box volume icon, both drawn from sources in `src-tauri/dmg`;
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
   - `SENTRY_ORG` and `SENTRY_PROJECT`: the Sentry organization and project slugs (`arrumo-video-editor`), for uploading debug files and source maps.
5. **Add the secrets**:

   | Secret | What it is |
   | --- | --- |
   | `TAURI_SIGNING_PRIVATE_KEY` | The contents of `~/.tauri/arrumo-updater.key` |
   | `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Its password |
   | `ARRUMO_CRASH_DSN` | The Sentry project DSN |
   | `ARRUMO_USAGE_KEY` | The Aptabase app key |
   | `SENTRY_AUTH_TOKEN` | A Sentry organization token (Settings › Developer Settings › Organization Tokens). With it, each release uploads the app's debug files (dSYM, PDB) and the interface's source maps, so crash reports show file and line. Once the release is published, it is finalized in Sentry with its commits (linking commits needs Sentry's GitHub integration). Without it, nothing is uploaded and the source maps are still left out of the app. |
   | `ARRUMO_REPOS_TOKEN` | Only while the plugin and registry repositories are private: a fine-grained token with read access to them. Not needed once they are public. |
   | `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY` | The Developer ID Application certificate (base64 `.p12`), its password and its identity name |
   | `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | For notarization; `APPLE_PASSWORD` is an app-specific password |
   | `RELEASE_DEPLOY_KEY` | The private half of a deploy key with write access (see below). Needed once the repository is public |
   | `AZURE_CLIENT_ID`, `AZURE_TENANT_ID` | The Azure app registration allowed to sign (Windows); it needs no client secret |

   Once the repository is public, the "Protect main" ruleset only lets you and one deploy key push to `main`, and the release workflow pushes its version commit with that key. Create it on your computer with `ssh-keygen -t ed25519 -N "" -C "release" -f release-deploy-key`. Add `release-deploy-key.pub` under Settings › Deploy keys with **Allow write access**, save `release-deploy-key` as the `RELEASE_DEPLOY_KEY` secret, then delete both files. `arrumo-plugin-registry` needs its own key the same way, for **Add plugin release**.

6. **Set up Windows signing** with [Azure Artifact Signing](https://learn.microsoft.com/azure/artifact-signing/), which is open to organisations in the EU. Without it the Windows build is unsigned, and SmartScreen warns on first install.
   1. In the Azure portal, create an **Artifact Signing account**. Note its region's endpoint: `https://neu.codesigning.azure.net` for North Europe, which DEVS AI uses (West Europe wasn't accepting new customers in November 2026). The endpoint must match the account's region, or signing fails with 403.
   2. In the account, complete **Identity validation** for DEVS.AI UNIPESSOAL LDA as a public organisation. Microsoft checks the company's official records; this can take a few days.
   3. Create a **Public Trust** certificate profile on the validated identity.
   4. In Microsoft Entra ID, create an **App registration**. Under **Certificates & secrets › Federated credentials**, add a **GitHub Actions** credential for your organisation and repository, with entity type **Environment** and environment `release`. If the form asks for the organisation and repository IDs, `gh api repos/<owner>/<repo> --jq '.owner.id, .id'` prints them. The release build signs in with a token GitHub issues for each run, so there is no client secret to store or rotate.
   5. On the Artifact Signing account (Access control), give that app the **Artifact Signing Certificate Profile Signer** role (called *Trusted Signing Certificate Profile Signer* on older pages).
   6. Add the variables `AZURE_ARTIFACT_SIGNING_ENDPOINT`, `AZURE_ARTIFACT_SIGNING_ACCOUNT` (the account name) and `AZURE_ARTIFACT_SIGNING_PROFILE` (the certificate profile name), and the two `AZURE_*` secrets above.

   The release build downloads Microsoft's Artifact Signing client for `signtool` (checked against its SHA-256) and signs with `scripts/windows-sign.ps1`, which asks GitHub for a fresh token for each file. Tauri runs it on the app executable and both installers, before it signs the update files, so updates install the signed build.

## Before each release

- Publish a release of every default plugin that changed, in two clicks:
  1. In the plugin's repository, run **Actions › Release** with the new version. It packages the plugin with the Plugin SDK and publishes `<plugin>-<version>.zip` as a GitHub release.
  2. In `arrumo-plugin-registry`, run **Actions › Add plugin release** with the plugin and version. It downloads that exact ZIP, records its version, schema and SHA-256 in `catalog.json`, verifies it and commits.

  The editor build bundles the latest catalog release of each default plugin. It stops if a plugin has no release, or if a ZIP differs from its catalog digest or from the plugin's manifest at that tag.
- Run the local checks: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `pnpm build`.

## Changelog

Each release writes its notes from the [Conventional Commits](https://www.conventionalcommits.org/) since the previous release tag (`scripts/release-notes.mjs`): `feat` commits under **New**, `fix` and `perf` under **Fixes**, in the words of the commit subject. Other types, such as `docs`, `ci` and `test`, are left out. The notes go at the top of `CHANGELOG.md` in the release commit, into the GitHub release, and into `latest.json`, so the update dialog shows them with a link to the release page. Write commit subjects for the people who use Arrumo.

## Messages to everyone using Arrumo

To tell everyone something at their next launch, such as asking people on a broken version to update by hand, run **Actions › Announcements** with **add**. Give a new id (ids are never reused), the English text (Spanish and Portuguese are optional, and English is shown where a translation is missing), an optional `https://` link, and optionally the versions it applies to: from `from` up to, but not including, `before`. Run it again with **remove** and the id once the message no longer applies.

The workflow edits `announcements/announcements.json`, raises its serial and signs the whole file with the updater key. Only the accounts in `RELEASE_ACTORS` can run it. At launch, Arrumo downloads the list and its signature, verifies the exact bytes against the public key built into the app, refuses any list with a lower serial than one it has already seen, and shows at most one message per launch: the first that applies to its version and wasn't closed before. Closed messages stay closed on that computer. Builds without the updater key never show messages. Debug builds can read a local list from `ARRUMO_ANNOUNCEMENTS_URL` to try a message before publishing it.

## Previews and unsigned builds

Tick **Preview** when you run the workflow to publish a pre-release. It's on the Releases page for testers, but `releases/latest` and the updater keep pointing at the last full release, so the public download and installed copies don't move to it. Testers on a preview update to the next full release with a higher version.

Missing signing credentials don't stop a release:

- **Windows** without the Azure Artifact Signing variables builds unsigned installers. SmartScreen asks people to choose **More info › Run anyway**.
- **macOS** without `APPLE_CERTIFICATE` gets an ad-hoc signature and no notarization. macOS asks people to allow the app once in **System Settings › Privacy & Security › Open Anyway**.

When either platform is unsigned, the release notes start with these steps (`scripts/unsigned-notes.mjs`). Updates are verified with the updater key either way, so copies installed from an unsigned build update normally to the next signed one. Once the certificates arrive, add the secrets and publish the next version; earlier releases don't need re-signing.

## When a platform fails to build

The other platforms still upload to the draft, and nothing is published. Fix the cause on `main`, then run **Actions › Release** again with **Resume** set to the draft's version, such as `0.1.0`, and **Platforms** set to the one that failed. It rebuilds only that platform from the existing tag, keeps the draft's notes and preview setting, adds its files to the update manifest and publishes once every platform is in it. The workflow comes from `main`, so a fix to the workflow applies; a fix to the app's code needs a new version instead.

## CI

Rust and Desktop run on pull requests and on `main`, not on pull request branches or tags. Each starts with a gate that skips the build when only documentation changed, or when a pull request already verified the exact code `main` now has (a squash merge of an up-to-date pull request). The checks still report, so required checks pass. A newer commit on the same pull request or on `main` cancels the run still in progress, so merging several pull requests at once builds only the last. Run either workflow by hand to force a full build.

## When a release breaks

Publish a fixed release as usual. Arrumo marks each launch before anything else runs, and removes the mark once the interface has stayed up for 30 seconds, or on a normal quit after the interface appeared. If a launch fails before then, the next one checks for an update before opening any data, and installs it without asking (waiting up to 90 seconds), so a release that crashes on start can still be replaced.

Crash reports follow the creator's saved choice from the first moment, even when the data can't be opened. They never include native crash dumps or breadcrumbs.

Builds made without `ARRUMO_UPDATER_PUBKEY` (contributor builds), or with `ARRUMO_STORE_BUILD` set (store builds), never check for updates. Builds without `ARRUMO_CRASH_DSN` or `ARRUMO_USAGE_KEY` send nothing.

// Runs after `next build`. With SENTRY_AUTH_TOKEN, SENTRY_ORG and SENTRY_PROJECT set (release
// builds), it gives each script a debug ID and uploads its source map, so frontend crash reports
// show the original code. The maps are then deleted either way, so they never ship in the app.
import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync, rmSync } from "node:fs";
import { join } from "node:path";

const OUT = "out";
const SENTRY_CLI = "@sentry/cli@2.39.1";

function mapFiles(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return mapFiles(path);
    return entry.name.endsWith(".map") ? [path] : [];
  });
}

const { SENTRY_AUTH_TOKEN, SENTRY_ORG, SENTRY_PROJECT } = process.env;
if (SENTRY_AUTH_TOKEN && SENTRY_ORG && SENTRY_PROJECT) {
  // The desktop crate's version, as the Rust reporter names the release.
  const version = readFileSync("src-tauri/Cargo.toml", "utf8").match(/^version = "([^"]+)"/m)?.[1];
  if (!version) throw new Error("No version in src-tauri/Cargo.toml");
  const cli = (...args) => execFileSync("pnpm", ["dlx", SENTRY_CLI, ...args], { stdio: "inherit", shell: process.platform === "win32" });
  cli("sourcemaps", "inject", OUT);
  cli("sourcemaps", "upload", "--release", `arrumo-desktop@${version}`, OUT);
}
const maps = mapFiles(OUT);
for (const map of maps) rmSync(map);
console.log(`Removed ${maps.length} source maps from the app bundle.`);

#!/usr/bin/env node
// Writes public/third-party-licenses.json, the notices Settings › About shows: every Rust crate
// and npm package that ships in the app, the bundled fonts, with their license files, plus the
// bundled FFmpeg release. Identical license texts are stored once.
//
// `pnpm build` runs it. Without network or tools it warns and the About page says the list is
// missing; with ARRUMO_REQUIRE_LICENSES=1, as in the release workflow, it fails the build instead.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

const output = "public/third-party-licenses.json";
const required = process.env.ARRUMO_REQUIRE_LICENSES === "1";
/** npm packages that only run while building, such as image tooling and browser data. */
const buildOnly = /^(@img\/|sharp$|caniuse-lite$|@next\/swc-)/;
/** Packages that state their license only in prose. */
const statedLicense = { "@editorcn/editor": "MIT (stated in its README)" };
const licenseFile = /^(licen[cs]e|copying|notice|copyright|unlicense)([-._].*)?$/i;

const texts = {};
function addText(text) {
  const id = createHash("sha256").update(text).digest("hex").slice(0, 16);
  texts[id] = text;
  return id;
}

function licenseTexts(folder) {
  if (!folder || !existsSync(folder)) return [];
  return readdirSync(folder)
    .filter((name) => licenseFile.test(name) && statSync(join(folder, name)).isFile() && statSync(join(folder, name)).size < 200_000)
    .sort()
    .map((name) => addText(readFileSync(join(folder, name), "utf8").trim()));
}

function rustPackages() {
  const metadata = JSON.parse(execFileSync("cargo", ["metadata", "--format-version", "1", "--locked"], { encoding: "utf8", maxBuffer: 1 << 28 }));
  const members = new Set(metadata.workspace_members);
  const nodes = new Map(metadata.resolve.nodes.map((node) => [node.id, node]));
  const root = metadata.packages.find((pkg) => pkg.name === "arrumo-desktop" && members.has(pkg.id));
  const shipped = new Set();
  const queue = [root.id];
  while (queue.length) {
    for (const dep of nodes.get(queue.pop())?.deps ?? []) {
      if (!dep.dep_kinds.some((kind) => kind.kind === null) || shipped.has(dep.pkg)) continue;
      shipped.add(dep.pkg);
      queue.push(dep.pkg);
    }
  }
  return metadata.packages
    .filter((pkg) => shipped.has(pkg.id) && !members.has(pkg.id))
    .map((pkg) => ({
      name: pkg.name,
      version: pkg.version,
      license: pkg.license ?? (pkg.license_file ? "See license file" : "Unknown"),
      url: pkg.repository ?? `https://crates.io/crates/${pkg.name}`,
      kind: "rust",
      texts: licenseTexts(dirname(pkg.manifest_path)),
    }));
}

function npmPackages() {
  const listed = JSON.parse(execFileSync("pnpm", ["licenses", "list", "--prod", "--json"], { encoding: "utf8", maxBuffer: 1 << 28, shell: process.platform === "win32" }));
  return Object.values(listed)
    .flat()
    .filter((pkg) => !buildOnly.test(pkg.name))
    .flatMap((pkg) => pkg.versions.map((version, index) => ({
      name: pkg.name,
      version,
      license: statedLicense[pkg.name] ?? pkg.license,
      url: pkg.homepage ?? `https://www.npmjs.com/package/${pkg.name}`,
      kind: "npm",
      texts: licenseTexts(pkg.paths[index]),
    })));
}

function fonts() {
  return [
    ["Anybody", "licenses/anybody-OFL.txt", "https://github.com/Etcetera-Type-Co/Anybody"],
    ["Atkinson Hyperlegible Next", "licenses/atkinsonhyperlegiblenext-OFL.txt", "https://github.com/googlefonts/atkinson-hyperlegible-next"],
  ].map(([name, file, url]) => ({ name, version: "", license: "OFL-1.1", url, kind: "font", texts: [addText(readFileSync(file, "utf8").trim())] }));
}

function ffmpeg() {
  const pinned = readFileSync("src-tauri/media-tools.txt", "utf8");
  const release = /^release (\S+)/m.exec(pinned)?.[1];
  const version = /ffmpeg-([\d.]+)-/.exec(pinned)?.[1];
  return release && version ? { version, release, url: `https://github.com/aidevs-pt/ffmpeg-builds/releases/tag/${release}` } : null;
}

try {
  const packages = [...rustPackages(), ...npmPackages(), ...fonts()].sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version));
  mkdirSync(dirname(output), { recursive: true });
  writeFileSync(output, JSON.stringify({ ffmpeg: ffmpeg(), packages, texts }));
  console.log(`Wrote ${output}: ${packages.length} packages, ${Object.keys(texts).length} license texts.`);
} catch (error) {
  if (required) throw error;
  console.warn(`Skipped ${output}: ${error.message.split("\n")[0]}`);
}

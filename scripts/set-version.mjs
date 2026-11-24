#!/usr/bin/env node
// Sets the app version everywhere it is declared: `node scripts/set-version.mjs 0.2.0`, or
// `patch`, `minor` or `major` to bump the current one. Prints the new version.
import { readFileSync, writeFileSync } from "node:fs";

const files = {
  tauri: "src-tauri/tauri.conf.json",
  npm: "package.json",
  cargo: ["src-tauri/Cargo.toml", "crates/workspaces/Cargo.toml"],
  metainfo: "src-tauri/linux/pt.aidevs.arrumo.metainfo.xml",
};
const current = JSON.parse(readFileSync(files.tauri, "utf8")).version;
const [major, minor, patch] = current.split(".").map(Number);
const asked = process.argv[2];
const next = {
  major: `${major + 1}.0.0`,
  minor: `${major}.${minor + 1}.0`,
  patch: `${major}.${minor}.${patch + 1}`,
}[asked] ?? asked;
if (!/^\d+\.\d+\.\d+$/.test(next ?? "")) {
  console.error("Give a version like 0.2.0, or patch, minor or major.");
  process.exit(1);
}
for (const path of [files.tauri, files.npm]) {
  writeFileSync(path, readFileSync(path, "utf8").replace(/"version": "[^"]+"/, `"version": "${next}"`));
}
for (const path of files.cargo) {
  writeFileSync(path, readFileSync(path, "utf8").replace(/^version = "[^"]+"/m, `version = "${next}"`));
}
// Linux software centres list each release from the metainfo, newest first.
const metainfo = readFileSync(files.metainfo, "utf8");
if (!metainfo.includes(`<release version="${next}"`)) {
  const date = new Date().toISOString().slice(0, 10);
  writeFileSync(files.metainfo, metainfo.replace("<releases>", `<releases>\n    <release version="${next}" date="${date}"/>`));
}
console.log(next);

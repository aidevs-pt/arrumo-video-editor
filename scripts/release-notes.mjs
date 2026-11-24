#!/usr/bin/env node
// Writes the changelog for a release from the Conventional Commits since the previous tag:
// new features (`feat`) and fixes (`fix`, `perf`), in plain sentences. Other commits, such as
// docs, CI and tests, are left out.
//
//   node scripts/release-notes.mjs <version>             prints the notes
//   node scripts/release-notes.mjs <version> --changelog  also adds them to CHANGELOG.md
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";

const [version, flag] = process.argv.slice(2);
if (!/^\d+\.\d+\.\d+$/.test(version ?? "")) {
  console.error("Give the version, such as 0.2.0.");
  process.exit(1);
}

const git = (...args) => execFileSync("git", args, { encoding: "utf8" }).trim();
const tags = git("tag", "--list", "v*", "--sort=-v:refname").split("\n").filter((tag) => tag && tag !== `v${version}`);
const previous = tags[0];

let notes;
if (!previous) {
  notes = "The first release of Arrumo AI Video Editor.";
} else {
  const sections = { feat: [], fix: [] };
  for (const line of git("log", "--no-merges", "--format=%s", `${previous}..HEAD`).split("\n")) {
    const match = /^(feat|fix|perf)(\([^)]*\))?(!)?: (.+)$/.exec(line);
    if (!match) continue;
    const text = match[4].charAt(0).toUpperCase() + match[4].slice(1);
    sections[match[1] === "feat" ? "feat" : "fix"].push(`- ${match[3] ? "**Breaking:** " : ""}${text}`);
  }
  const parts = [];
  if (sections.feat.length) parts.push(`### New\n\n${sections.feat.join("\n")}`);
  if (sections.fix.length) parts.push(`### Fixes\n\n${sections.fix.join("\n")}`);
  notes = parts.length ? parts.join("\n\n") : "Maintenance and small improvements.";
}

if (flag === "--changelog") {
  const date = new Date().toISOString().slice(0, 10);
  const heading = "# Changelog\n\nWhat changed in each release of Arrumo AI Video Editor.\n";
  const existing = existsSync("CHANGELOG.md") ? readFileSync("CHANGELOG.md", "utf8").replace(heading, "").trimStart() : "";
  writeFileSync("CHANGELOG.md", `${heading}\n## ${version} (${date})\n\n${notes}\n${existing ? `\n${existing}` : ""}`);
}
process.stdout.write(`${notes}\n`);

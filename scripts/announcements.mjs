#!/usr/bin/env node
// Adds or removes a message in announcements/announcements.json and raises its serial, so apps
// never go back to an older list. The Announcements workflow runs it, then signs the file.
//
//   node scripts/announcements.mjs add <id> <english> [spanish] [portuguese] [link] [from] [before]
//   node scripts/announcements.mjs remove <id>
//
// Empty arguments are skipped. Versions are plain x.y.z; a message applies from `from` up to,
// but not including, `before`.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

const file = process.env.ANNOUNCEMENTS_FILE ?? "announcements/announcements.json";
const args = process.argv.slice(2);
const [action, id, english, spanish, portuguese, link, from, before] = Array.from({ length: 8 }, (_, index) => (args[index] ?? "").trim());

function fail(message) {
  console.error(message);
  process.exit(1);
}

const board = existsSync(file) ? JSON.parse(readFileSync(file, "utf8")) : { serial: 0, messages: [], retired: [] };
board.retired ??= [];
if (!/^[a-z0-9][a-z0-9-]{0,63}$/.test(id ?? "")) fail("Give an id of lowercase letters, digits and dashes, such as 2026-12-update-by-hand.");

if (action === "add") {
  if (board.messages.some((message) => message.id === id) || board.retired.includes(id)) {
    fail(`The id ${id} is already used. Use a new one; apps remember the messages people closed.`);
  }
  if (!english) fail("The English text is required.");
  for (const text of [english, spanish, portuguese]) {
    if (text.length > 500) fail("Keep each text under 500 characters and link to the details.");
  }
  if (link && !/^https:\/\/\S+$/.test(link)) fail("The link must start with https://.");
  for (const version of [from, before]) {
    if (version && !/^\d+\.\d+\.\d+$/.test(version)) fail(`${version} isn't a version such as 0.2.0.`);
  }
  const text = Object.fromEntries(Object.entries({ en: english, es: spanish, pt: portuguese }).filter(([, value]) => value));
  const versions = Object.fromEntries(Object.entries({ from, before }).filter(([, value]) => value));
  board.messages.push({ id, text, ...(link ? { link } : {}), ...(Object.keys(versions).length ? { versions } : {}) });
} else if (action === "remove") {
  const count = board.messages.length;
  board.messages = board.messages.filter((message) => message.id !== id);
  if (board.messages.length === count) fail(`The list has no message ${id}.`);
  board.retired.push(id);
} else {
  fail("Use add or remove.");
}

board.serial += 1;
mkdirSync(dirname(file), { recursive: true });
writeFileSync(file, `${JSON.stringify(board, null, 2)}\n`);
console.log(`${action === "add" ? "Added" : "Removed"} ${id}; the list is now at serial ${board.serial} with ${board.messages.length} message(s).`);

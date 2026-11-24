#!/usr/bin/env node
// Prints the release-notes section that explains how to open installers built without a
// code-signing certificate: `node scripts/unsigned-notes.mjs <windows-signed> <macos-signed>`,
// each `true` or `false`. Prints nothing when every platform is signed.
const [windowsSigned, macosSigned] = process.argv.slice(2).map((value) => value === "true");

const sections = [];
if (!macosSigned) {
  sections.push(`**macOS:** open the disk image and drag Arrumo into Applications. The first time you open it, macOS says it can't verify the app. Choose **Done**, open **System Settings → Privacy & Security**, scroll to Security and choose **Open Anyway** next to Arrumo, then confirm. macOS remembers your choice.`);
}
if (!windowsSigned) {
  sections.push(`**Windows:** when SmartScreen says "Windows protected your PC", choose **More info**, then **Run anyway**.`);
}

if (sections.length > 0) {
  process.stdout.write(`## Opening this build

This build isn't signed with a code-signing certificate yet, so your system asks you to confirm before opening it the first time. The download comes from this page and its updates are verified by Arrumo, and signed builds will follow.

${sections.join("\n\n")}

`);
}

import type { LocaleMessages } from "@/lib/locales";

export type AssetUse = { kind: "open_video" | "finished_video" | "animation"; name: string };
export type AssetRemoval = { state: "removed" } | { state: "missing" } | { state: "in_use"; uses: AssetUse[] };

type Copy = LocaleMessages["assets"]["removal"];

/** What still needs an asset, as a sentence fragment in the app's language. */
export function describeUses(uses: AssetUse[], copy: Copy, locale: string) {
  const phrase = { open_video: copy.openVideo, finished_video: copy.finishedVideo, animation: copy.animation };
  return new Intl.ListFormat(locale, { type: "conjunction" })
    .format(uses.map((use) => phrase[use.kind].replace("{name}", () => use.name)));
}

/** `true` once removed, otherwise the reason to show in the removal dialog. */
export function removalOutcome(result: AssetRemoval, copy: Copy, locale: string): true | string {
  if (result.state === "removed") return true;
  if (result.state === "missing") return copy.missing;
  return copy.inUse.replace("{uses}", () => describeUses(result.uses, copy, locale));
}

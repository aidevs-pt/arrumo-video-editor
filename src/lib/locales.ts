import type english from "../../locales/en.json";

export type LocaleMessages = typeof english;
export type LocaleCatalog = Record<string, LocaleMessages>;

export const defaultLocale = "en";

export function preferredLocale(languages: readonly string[], catalog: LocaleCatalog): string {
  for (const language of languages) {
    let tag = language.toLowerCase();
    while (tag) {
      if (Object.hasOwn(catalog, tag)) return tag;
      const boundary = tag.lastIndexOf("-");
      tag = boundary === -1 ? "" : tag.slice(0, boundary);
    }
  }
  return defaultLocale;
}

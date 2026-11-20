import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import english from "../../locales/en.json";
import { type LocaleCatalog, type LocaleMessages } from "./locales";

const requiredLocales = ["en", "es", "pt"];

function placeholders(message: string): string[] {
  return (message.match(/\{[A-Za-z][A-Za-z0-9_]*\}/g) ?? []).sort();
}

function validateTranslations(value: unknown, reference: unknown, location: string): void {
  if (typeof reference === "string") {
    if (typeof value !== "string") throw new Error(`Expected translation string at ${location}`);
    const expected = placeholders(reference);
    const actual = placeholders(value);
    if (actual.length !== expected.length || actual.some((token, index) => token !== expected[index])) {
      throw new Error(`Translation placeholders differ at ${location}: expected ${expected.join(", ") || "none"}`);
    }
    return;
  }
  if (!value || typeof value !== "object" || Array.isArray(value) || !reference || typeof reference !== "object") {
    throw new Error(`Expected translation object at ${location}`);
  }
  const entries = Object.entries(reference);
  const translated = value as Record<string, unknown>;
  for (const [key, child] of entries) {
    validateTranslations(translated[key], child, `${location}.${key}`);
  }
  if (Object.keys(translated).length !== entries.length) {
    throw new Error(`Unexpected translation keys at ${location}`);
  }
}

export function getLocaleCatalog(): LocaleCatalog {
  const folder = join(process.cwd(), "locales");
  const entries: [string, LocaleMessages][] = [];
  for (const file of readdirSync(folder).sort()) {
    if (!file.endsWith(".json")) continue;
    const [canonical] = Intl.getCanonicalLocales(file.slice(0, -5));
    const locale = canonical.toLowerCase();
    if (entries.some(([tag]) => tag === locale)) throw new Error(`Duplicate locale in ${file}`);
    const messages: unknown = JSON.parse(readFileSync(join(folder, file), "utf8"));
    validateTranslations(messages, english, file);
    entries.push([locale, messages as LocaleMessages]);
  }
  const catalog = Object.fromEntries(entries);
  for (const locale of requiredLocales) {
    if (!Object.hasOwn(catalog, locale)) throw new Error(`Missing required locale: ${locale}`);
  }
  return catalog;
}

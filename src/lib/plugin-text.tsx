"use client";

import { invoke } from "@tauri-apps/api/core";
import { Bot } from "lucide-react";
import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";

import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";

type Named = { name: string; description: string };
type Labelled = { label: string; description: string };
type SettingText = { label?: string; description?: string; choices?: Record<string, string> };
type Translation = {
  name?: string;
  description?: string;
  provider?: { label: string };
  capabilities?: Record<string, Named>;
  settings?: Record<string, SettingText>;
  actions?: Record<string, Labelled>;
};
type CatalogManifest = { id: string; icon?: string; translations?: Record<string, Translation> };

export const PLUGINS_CHANGED = "arrumo:plugins-changed";

type PluginText = {
  /** The plugin's name in the creator's language, or `fallback` (the manifest's English). */
  name: (pluginId: string, fallback: string) => string;
  description: (pluginId: string, fallback: string) => string;
  providerLabel: (pluginId: string, fallback: string) => string;
  capability: (pluginId: string, capabilityId: string, field: keyof Named, fallback: string) => string;
  setting: (pluginId: string, settingId: string, field: "label" | "description", fallback: string) => string;
  choice: (pluginId: string, settingId: string, value: string) => string;
  action: (pluginId: string, actionId: string, field: keyof Labelled, fallback: string) => string;
  icon: (pluginId: string) => string | undefined;
  /** A setting declaration with its label, description and choice labels translated. */
  field: <T extends SettingDeclaration>(pluginId: string, field: T) => T & { choiceLabels: Record<string, string> };
};

type SettingDeclaration = { id: string; label: string; description: string; choices?: string[] };

const PluginTextContext = createContext<PluginText | null>(null);

/** Plugin text and icons come from the plugins themselves (their manifest `translations` and
 * `icon`); English stays in the fields every API already returns. */
export function PluginTextProvider({ children }: { children: ReactNode }) {
  const { locale } = useLocale();
  const [manifests, setManifests] = useState<Map<string, CatalogManifest>>(new Map());

  const load = useCallback(() => {
    invoke<{ manifest: CatalogManifest }[]>("list_plugin_catalog").then(
      (plugins) => setManifests(new Map(plugins.map(({ manifest }) => [manifest.id, manifest]))),
      () => undefined,
    );
  }, []);

  useEffect(() => {
    load();
    window.addEventListener(PLUGINS_CHANGED, load);
    return () => window.removeEventListener(PLUGINS_CHANGED, load);
  }, [load]);

  const value = useMemo<PluginText>(() => {
    const entry = (id: string, language = locale) => manifests.get(id)?.translations?.[language];
    return {
      name: (id, fallback) => entry(id)?.name ?? fallback,
      description: (id, fallback) => entry(id)?.description ?? fallback,
      providerLabel: (id, fallback) => entry(id)?.provider?.label ?? fallback,
      capability: (id, capability, field, fallback) => entry(id)?.capabilities?.[capability]?.[field] ?? fallback,
      setting: (id, setting, field, fallback) => entry(id)?.settings?.[setting]?.[field] ?? fallback,
      choice: (id, setting, choice) =>
        entry(id)?.settings?.[setting]?.choices?.[choice] ?? entry(id, "en")?.settings?.[setting]?.choices?.[choice] ?? choice,
      action: (id, action, field, fallback) => entry(id)?.actions?.[action]?.[field] ?? fallback,
      icon: (id) => manifests.get(id)?.icon,
      field: (id, field) => ({
        ...field,
        label: entry(id)?.settings?.[field.id]?.label ?? field.label,
        description: entry(id)?.settings?.[field.id]?.description ?? field.description,
        choiceLabels: Object.fromEntries((field.choices ?? []).map((choice) => [choice,
          entry(id)?.settings?.[field.id]?.choices?.[choice] ?? entry(id, "en")?.settings?.[field.id]?.choices?.[choice] ?? choice])),
      }),
    };
  }, [manifests, locale]);

  return <PluginTextContext.Provider value={value}>{children}</PluginTextContext.Provider>;
}

const english: PluginText = {
  name: (_, fallback) => fallback,
  description: (_, fallback) => fallback,
  providerLabel: (_, fallback) => fallback,
  capability: (_, __, ___, fallback) => fallback,
  setting: (_, __, ___, fallback) => fallback,
  choice: (_, __, choice) => choice,
  action: (_, __, ___, fallback) => fallback,
  icon: () => undefined,
  field: (_, field) => ({ ...field, choiceLabels: Object.fromEntries((field.choices ?? []).map((choice) => [choice, choice])) }),
};

export function usePluginText(): PluginText {
  return useContext(PluginTextContext) ?? english;
}

/** The icon a plugin provides, shown as an image so it cannot run anything. Plugins without
 * one get a neutral placeholder; never the Arrumo face, which stands for the app. */
export function PluginIcon({ pluginId, className, tone = "light" }: {
  pluginId: string | null | undefined;
  className?: string;
  /** The surface it sits on, for the placeholder's contrast. */
  tone?: "light" | "dark";
}) {
  const { icon } = usePluginText();
  const svg = pluginId ? icon(pluginId) : undefined;
  if (svg) {
    // eslint-disable-next-line @next/next/no-img-element
    return <img src={`data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`} alt="" aria-hidden="true"
      className={cn("size-8 shrink-0 rounded-md", className)} />;
  }
  return (
    <span aria-hidden="true" className={cn("grid size-8 shrink-0 place-items-center rounded-md",
      tone === "dark" ? "bg-white/15 text-white" : "bg-muted text-muted-foreground", className)}>
      <Bot className="size-[55%]" />
    </span>
  );
}

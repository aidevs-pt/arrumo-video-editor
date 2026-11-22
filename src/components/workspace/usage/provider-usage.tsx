"use client";

import { invoke } from "@tauri-apps/api/core";
import { TriangleAlert, X } from "lucide-react";
import Link from "next/link";
import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { useLocale } from "@/lib/locale-provider";
import { PLUGINS_CHANGED, PluginIcon, usePluginText } from "@/lib/plugin-text";
import { cn } from "@/lib/utils";

export type ProviderUsage = {
  pluginId: string;
  unit: "characters" | "credits" | "requests" | "seconds";
  used: number | null;
  limit: number | null;
  remainingPercent: number | null;
  resetsAt: number | null;
  level: "ok" | "warning" | "danger" | "unknown";
  problem: string | null;
};

type CatalogEntry = { manifest: { id: string; name: string } };

const REFRESH_MS = 30 * 60 * 1000;

/** Each keyed provider's allowance in this workspace, refreshed every half hour and when plugins change. */
export function useProviderUsage(workspaceId: number | undefined) {
  const [usage, setUsage] = useState<{ workspaceId: number; providers: ProviderUsage[]; names: Record<string, string> }>();
  const [revision, setRevision] = useState(0);

  useEffect(() => {
    const changed = () => setRevision((value) => value + 1);
    window.addEventListener(PLUGINS_CHANGED, changed);
    const timer = setInterval(changed, REFRESH_MS);
    return () => { window.removeEventListener(PLUGINS_CHANGED, changed); clearInterval(timer); };
  }, []);

  useEffect(() => {
    if (workspaceId === undefined) return;
    let active = true;
    Promise.all([
      invoke<ProviderUsage[]>("provider_usage", { workspaceId }),
      invoke<CatalogEntry[]>("list_plugin_catalog").catch(() => [] as CatalogEntry[]),
    ]).then(([providers, catalog]) => {
      if (active) setUsage({ workspaceId, providers, names: Object.fromEntries(catalog.map(({ manifest }) => [manifest.id, manifest.name])) });
    }, () => {});
    return () => { active = false; };
  }, [workspaceId, revision]);

  return usage?.workspaceId === workspaceId ? usage : undefined;
}

function dismissKey(workspaceId: number, entry: ProviderUsage) {
  return `arrumo.usage-dismissed.${workspaceId}.${entry.pluginId}.${entry.level}`;
}

function wasDismissed(key: string) {
  try { return window.localStorage.getItem(key) === "1"; } catch { return false; }
}

/** Formats "12%" in the app's language. */
function percent(value: number, locale: string) {
  return new Intl.NumberFormat(locale, { style: "percent", maximumFractionDigits: 0 }).format(value / 100);
}

/** The warning across the top of the app when a provider has 20% (then 10%) of its allowance left.
 * Closing it hides that provider's warning at that level until it gets worse. */
export function UsageBanner({ workspaceId }: { workspaceId: number }) {
  const { locale, messages: { usage: copy } } = useLocale();
  const pluginText = usePluginText();
  const usage = useProviderUsage(workspaceId);
  const [closed, setClosed] = useState<Set<string>>(new Set());

  const entry = usage?.providers
    .filter((item) => (item.level === "warning" || item.level === "danger") && item.remainingPercent !== null)
    .filter((item) => { const key = dismissKey(workspaceId, item); return !closed.has(key) && !wasDismissed(key); })
    .sort((a, b) => (a.remainingPercent ?? 0) - (b.remainingPercent ?? 0))[0];
  if (!entry || !usage) return null;

  const name = pluginText.name(entry.pluginId, usage.names[entry.pluginId] ?? entry.pluginId);
  const danger = entry.level === "danger";
  function close() {
    const key = dismissKey(workspaceId, entry!);
    try { window.localStorage.setItem(key, "1"); } catch {}
    setClosed((current) => new Set(current).add(key));
  }

  return (
    <div role="alert" className={cn("flex items-center gap-3 px-3 py-2.5 text-sm sm:px-5", danger ? "bg-destructive text-white" : "bg-attention text-ink")}>
      <TriangleAlert className="size-5 shrink-0" aria-hidden="true" />
      <p className="min-w-0 flex-1">
        <strong>{copy.bannerTitle.replace("{provider}", name).replace("{percent}", percent(entry.remainingPercent!, locale)).replace("{unit}", copy.units[entry.unit])}</strong>{" "}
        {copy.bannerBody}
      </p>
      <Button asChild size="sm" variant="outline" className={cn("shrink-0 bg-transparent", danger ? "border-white text-white hover:bg-white/15 hover:text-white" : "border-ink text-ink hover:bg-ink/10 hover:text-ink")}>
        <Link href={`/settings?workspace=${workspaceId}&scope=workspace&section=services`}>{copy.seeUsage}</Link>
      </Button>
      <Button type="button" size="icon" variant="ghost" className="size-8 shrink-0 hover:bg-black/10 hover:text-current" aria-label={copy.close} onClick={close}>
        <X />
      </Button>
    </div>
  );
}

/** Settings › Services and keys: how much each keyed provider has left. */
export function UsagePanel({ workspaceId }: { workspaceId: number }) {
  const { locale, messages: { usage: copy } } = useLocale();
  const pluginText = usePluginText();
  const usage = useProviderUsage(workspaceId);
  if (!usage?.providers.length) return null;
  const count = new Intl.NumberFormat(locale, { notation: "compact", maximumFractionDigits: 1 });
  const day = new Intl.DateTimeFormat(locale, { dateStyle: "medium" });

  return (
    <Card size="sm">
      <CardHeader>
        <CardTitle>{copy.panelTitle}</CardTitle>
        <CardDescription>{copy.panelDescription}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        {usage.providers.map((entry) => {
          const name = pluginText.name(entry.pluginId, usage.names[entry.pluginId] ?? entry.pluginId);
          const left = entry.remainingPercent;
          return (
            <div key={entry.pluginId} className="flex items-start gap-3">
              <PluginIcon pluginId={entry.pluginId} className="size-8" />
              <div className="flex min-w-0 flex-1 flex-col gap-1.5">
                <div className="flex items-baseline justify-between gap-3">
                  <span className="font-semibold">{name}</span>
                  {left !== null && <span className={cn("text-sm font-semibold tabular-nums",
                    entry.level === "danger" ? "text-destructive" : entry.level === "warning" ? "text-foreground" : "text-muted-foreground")}>
                    {copy.left.replace("{percent}", percent(left, locale))}
                  </span>}
                </div>
                {left !== null ? <>
                  <Progress value={left} aria-label={copy.left.replace("{percent}", percent(left, locale))}
                    className={cn(entry.level === "danger" && "*:data-[slot=progress-indicator]:bg-destructive", entry.level === "warning" && "*:data-[slot=progress-indicator]:bg-attention")} />
                  <span className="text-xs text-muted-foreground">
                    {copy.usedOf.replace("{used}", count.format(entry.used ?? 0)).replace("{limit}", count.format(entry.limit ?? 0)).replace("{unit}", copy.units[entry.unit])}
                    {entry.resetsAt ? ` · ${copy.resets.replace("{date}", day.format(new Date(entry.resetsAt * 1000)))}` : ""}
                  </span>
                </> : <span className="text-sm text-muted-foreground">{copy.unknown.replace("{reason}", entry.problem ?? "")}</span>}
              </div>
            </div>
          );
        })}
      </CardContent>
    </Card>
  );
}

/** Before generating: providers at 20% or less, which might stop this video partway. Not dismissible. */
export function UsageNotice({ workspaceId }: { workspaceId: number }) {
  const { locale, messages: { usage: copy } } = useLocale();
  const pluginText = usePluginText();
  const usage = useProviderUsage(workspaceId);
  const low = usage?.providers.filter((item) => (item.level === "warning" || item.level === "danger") && item.remainingPercent !== null) ?? [];
  if (!usage || !low.length) return null;
  const danger = low.some((item) => item.level === "danger");
  return (
    <div role="status" className={cn("flex gap-2.5 rounded-lg border p-3 text-sm", danger ? "border-destructive/40 bg-destructive/8" : "border-attention bg-attention/15")}>
      <TriangleAlert className={cn("mt-0.5 size-4 shrink-0", danger && "text-destructive")} aria-hidden="true" />
      <div className="flex flex-col gap-1">
        {low.map((entry) => <p key={entry.pluginId}>
          <strong>{copy.bannerTitle.replace("{provider}", pluginText.name(entry.pluginId, usage.names[entry.pluginId] ?? entry.pluginId))
            .replace("{percent}", percent(entry.remainingPercent!, locale)).replace("{unit}", copy.units[entry.unit])}</strong>
        </p>)}
        <p className="text-muted-foreground">{copy.beforeGenerating}</p>
      </div>
    </div>
  );
}

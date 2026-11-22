"use client";

import { invoke } from "@tauri-apps/api/core";
import { TriangleAlert, X } from "lucide-react";
import Link from "next/link";
import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
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
const SHARED_MS = 60 * 1000;
type UsageRead = { providers: ProviderUsage[]; names: Record<string, string> };
const reads = new Map<number, { at: number; read: Promise<UsageRead> }>();

/** One provider read per workspace for every component asking within a minute, so each key is used once. */
function readUsage(workspaceId: number) {
  const cached = reads.get(workspaceId);
  if (cached && Date.now() - cached.at < SHARED_MS) return cached.read;
  const read = Promise.all([
    invoke<ProviderUsage[]>("provider_usage", { workspaceId }),
    invoke<CatalogEntry[]>("list_plugin_catalog").catch(() => [] as CatalogEntry[]),
  ]).then(([providers, catalog]) => ({ providers, names: Object.fromEntries(catalog.map(({ manifest }) => [manifest.id, manifest.name])) }));
  read.catch(() => reads.delete(workspaceId));
  reads.set(workspaceId, { at: Date.now(), read });
  return read;
}

/** Each keyed provider's allowance in this workspace, refreshed every half hour and when plugins change. */
export function useProviderUsage(workspaceId: number | undefined) {
  const [usage, setUsage] = useState<{ workspaceId: number; providers: ProviderUsage[]; names: Record<string, string> }>();
  const [revision, setRevision] = useState(0);

  useEffect(() => {
    const changed = () => { reads.clear(); setRevision((value) => value + 1); };
    window.addEventListener(PLUGINS_CHANGED, changed);
    const timer = setInterval(changed, REFRESH_MS);
    return () => { window.removeEventListener(PLUGINS_CHANGED, changed); clearInterval(timer); };
  }, []);

  useEffect(() => {
    if (workspaceId === undefined) return;
    let active = true;
    readUsage(workspaceId).then((read) => { if (active) setUsage({ workspaceId, ...read }); }, () => {});
    return () => { active = false; };
  }, [workspaceId, revision]);

  return usage?.workspaceId === workspaceId ? usage : undefined;
}

export function dismissKey(workspaceId: number, entry: ProviderUsage) {
  return `arrumo.usage-dismissed.${workspaceId}.${entry.pluginId}.${entry.level}`;
}

export function wasDismissed(key: string) {
  try { return window.localStorage.getItem(key) === "1"; } catch { return false; }
}

/** Formats "12%" in the app's language. */
export const USAGE_DISMISSED = "arrumo:usage-dismissed";

/** Hides these provider warnings in the banner and the notifications until they get worse. */
export function dismissUsage(keys: string[]) {
  for (const key of keys) try { window.localStorage.setItem(key, "1"); } catch {}
  window.dispatchEvent(new CustomEvent<string[]>(USAGE_DISMISSED, { detail: keys }));
}

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
  useEffect(() => {
    const dismissed = (event: Event) => setClosed((current) => new Set([...current, ...(event as CustomEvent<string[]>).detail]));
    window.addEventListener(USAGE_DISMISSED, dismissed);
    return () => window.removeEventListener(USAGE_DISMISSED, dismissed);
  }, []);

  const entry = usage?.providers
    .filter((item) => (item.level === "warning" || item.level === "danger") && item.remainingPercent !== null)
    .filter((item) => { const key = dismissKey(workspaceId, item); return !closed.has(key) && !wasDismissed(key); })
    .sort((a, b) => (a.remainingPercent ?? 0) - (b.remainingPercent ?? 0))[0];
  if (!entry || !usage) return null;

  const name = pluginText.name(entry.pluginId, usage.names[entry.pluginId] ?? entry.pluginId);
  const danger = entry.level === "danger";
  function close() {
    dismissUsage([dismissKey(workspaceId, entry!)]);
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

/** Settings › Services and keys: how much each keyed provider has left, one row per provider. */
export function UsagePanel({ workspaceId, keysHref }: { workspaceId: number; keysHref: string }) {
  const { locale, messages: { usage: copy } } = useLocale();
  const pluginText = usePluginText();
  const usage = useProviderUsage(workspaceId);
  if (!usage?.providers.length) return null;
  const count = new Intl.NumberFormat(locale, { notation: "compact", maximumFractionDigits: 1 });
  const day = new Intl.DateTimeFormat(locale, { day: "numeric", month: "short" });

  return (
    <section aria-labelledby="usage-title">
      <h2 id="usage-title" className="mb-3 text-[1.5rem]">{copy.panelTitle}</h2>
      <ul className="divide-y rounded-xl border bg-card">
        {usage.providers.map((entry) => {
          const name = pluginText.name(entry.pluginId, usage.names[entry.pluginId] ?? entry.pluginId);
          const left = entry.remainingPercent;
          const tone = entry.level === "danger" ? "text-destructive" : entry.level === "warning" ? "text-[oklch(52%_.13_65)]" : "text-[oklch(50%_.13_150)]";
          return (
            <li key={entry.pluginId} className="grid items-center gap-x-5 gap-y-2 p-4 sm:grid-cols-[minmax(0,10rem)_minmax(0,11rem)_minmax(0,1fr)_auto]">
              <span className="flex min-w-0 items-center gap-2.5">
                <PluginIcon pluginId={entry.pluginId} className="size-7 shrink-0" />
                <span className="truncate font-semibold">{name}</span>
              </span>
              {left !== null ? <span className="flex flex-col gap-1.5">
                <span className={cn("text-sm font-semibold tabular-nums", tone)}>{copy.left.replace("{percent}", percent(left, locale))}</span>
                <Progress value={left} aria-label={copy.left.replace("{percent}", percent(left, locale))} className={cn(
                  "h-1.5",
                  entry.level === "danger" ? "*:data-[slot=progress-indicator]:bg-destructive"
                    : entry.level === "warning" ? "*:data-[slot=progress-indicator]:bg-[oklch(60%_.14_65)]" : "*:data-[slot=progress-indicator]:bg-[oklch(55%_.14_150)]")} />
              </span> : <span className="text-sm text-muted-foreground">{copy.unreadable}</span>}
              <span className={cn("text-sm", left !== null && entry.level !== "ok" ? tone : "text-muted-foreground")}>
                {left !== null
                  ? <>{copy.usedOf.replace("{used}", count.format(entry.used ?? 0)).replace("{limit}", count.format(entry.limit ?? 0)).replace("{unit}", copy.units[entry.unit])}
                    {entry.resetsAt ? `, ${copy.resets.replace("{date}", day.format(new Date(entry.resetsAt * 1000)))}` : ""}</>
                  : copy.unknown.replace("{reason}", entry.problem ?? "")}
              </span>
              <Button asChild size="sm" variant="outline" className="justify-self-start sm:justify-self-end"><Link href={keysHref}>{copy.manageKey}</Link></Button>
            </li>
          );
        })}
      </ul>
      <p className="mt-3 text-sm text-muted-foreground">{copy.panelDescription} {copy.vault}</p>
    </section>
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

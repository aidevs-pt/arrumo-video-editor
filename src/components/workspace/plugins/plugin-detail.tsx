"use client";

import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft, Play } from "lucide-react";
import Link from "next/link";
import { useEffect, useState } from "react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { LayerGauge } from "@/components/workspace/layer-gauge";
import { PluginCredentialField } from "@/components/workspace/settings/plugin-credential-field";
import { PluginFields, type PluginSettingDeclaration } from "@/components/workspace/settings/plugin-preferences";
import { useStudioTrail } from "@/components/workspace/home/studio-trail";
import { replayPluginTour } from "./plugin-tour";
import { useLocale } from "@/lib/locale-provider";
import { PluginIcon, usePluginText } from "@/lib/plugin-text";

type Entry = {
  bundled: boolean;
  manifest: {
    id: string;
    name: string;
    description: string;
    capabilities: Record<string, { name: string; description: string }>;
    settings?: PluginSettingDeclaration[];
    actions?: { id: string; label: string; description: string; scope: string }[];
    agentProvider?: unknown;
    introduction?: unknown;
  };
  package?: { release?: { version: string } };
};

/** One plugin's page: what it adds, its settings and the actions agents can call. */
export function PluginDetail({ pluginId, backHref }: { pluginId: string; backHref: string }) {
  const { messages: { pluginDetail: copy, pluginCatalog, pluginTour } } = useLocale();
  const pluginText = usePluginText();
  const [entry, setEntry] = useState<Entry | null>();
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let active = true;
    invoke<Entry[]>("list_plugin_catalog").then(
      (catalog) => { if (active) setEntry(catalog.find(({ manifest }) => manifest.id === pluginId) ?? null); },
      () => { if (active) setFailed(true); },
    );
    return () => { active = false; };
  }, [pluginId]);

  const name = entry ? pluginText.name(pluginId, entry.manifest.name) : "";
  useStudioTrail(entry ? [{ label: pluginCatalog.title, href: backHref }, { label: name }] : null);

  if (failed || entry === null) return <Alert variant="destructive"><AlertDescription>{copy.missing}</AlertDescription>
    <Button asChild variant="outline"><Link href={backHref}>{copy.back}</Link></Button></Alert>;
  if (!entry) return <Skeleton className="h-64" aria-label={pluginCatalog.loading} />;

  const { manifest } = entry;
  const fields = (manifest.settings ?? []).filter(({ kind }) => kind !== "secret" || entry.bundled).map((field) => pluginText.field(pluginId, field));
  const ordinary = fields.filter(({ kind }) => kind !== "secret");
  const credentials = fields.filter(({ kind }) => kind === "secret");
  const actions = manifest.actions ?? [];
  const capabilities = Object.entries(manifest.capabilities);

  return (
    <div className="flex flex-col gap-8">
      <Button asChild variant="ghost" className="self-start"><Link href={backHref}><ArrowLeft data-icon="inline-start" />{copy.back}</Link></Button>
      <header className="flex flex-wrap items-end justify-between gap-6">
        <div className="flex min-w-0 max-w-2xl flex-col gap-3">
          <p className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
            <Badge variant="secondary">{entry.bundled ? copy.bundled : copy.installed}</Badge>
            {entry.package?.release && <span>{copy.version.replace("{version}", entry.package.release.version)}</span>}
          </p>
          <h1 className="flex items-center gap-4 text-[clamp(2.25rem,5vw,3.25rem)] leading-[1] font-[850] [font-variation-settings:'wdth'_135]">
            <PluginIcon pluginId={pluginId} className="size-12" />{name}
          </h1>
          <p className="text-lg text-muted-foreground">{pluginText.description(pluginId, manifest.description)}</p>
          {pluginText.iconNotice(pluginId) && <p className="text-xs text-muted-foreground">{pluginText.iconNotice(pluginId)}</p>}
        </div>
        {Boolean(manifest.introduction) && <Button variant="outline" onClick={() => replayPluginTour([pluginId])}>
          <Play data-icon="inline-start" />{pluginTour.replayOne}
        </Button>}
      </header>
      <div className="grid items-start gap-10 lg:grid-cols-[minmax(0,1fr)_18rem]">
        <Tabs defaultValue={fields.length ? "settings" : "actions"} className="min-w-0 gap-6">
          <TabsList variant="line" className="justify-start">
            <TabsTrigger value="settings">{copy.settings}</TabsTrigger>
            <TabsTrigger value="actions">{copy.actions} <span className="text-muted-foreground tabular-nums">{actions.length}</span></TabsTrigger>
          </TabsList>
          <TabsContent value="settings" className="flex flex-col gap-5">
            {fields.length === 0 ? <p className="text-muted-foreground">{copy.noSettings}</p> : <>
              {ordinary.length > 0 && <PluginFields pluginId={pluginId} fields={ordinary} workspaceId={null} />}
              {credentials.map((field) => <PluginCredentialField key={field.id} field={field} pluginId={pluginId} workspaceId={null} />)}
              <div className="flex flex-col gap-1 border-t pt-4">
                <p className="font-semibold">{copy.layer}</p>
                <p className="text-sm text-muted-foreground">{copy.layerHint}</p>
                <LayerGauge layer="app" />
              </div>
            </>}
          </TabsContent>
          <TabsContent value="actions">
            {actions.length === 0 ? <p className="text-muted-foreground">{copy.noActions}</p> : (
              <ul className="divide-y rounded-xl border bg-card">
                {actions.map((action) => (
                  <li key={action.id} className="flex flex-col gap-1 p-4">
                    <span className="flex flex-wrap items-baseline justify-between gap-2">
                      <span className="font-semibold">{pluginText.action(pluginId, action.id, "label", action.label)}</span>
                      <code className="text-xs text-muted-foreground">{action.id}</code>
                    </span>
                    <span className="text-sm text-muted-foreground">{pluginText.action(pluginId, action.id, "description", action.description)}</span>
                  </li>
                ))}
              </ul>
            )}
          </TabsContent>
        </Tabs>
        <aside className="flex flex-col gap-6">
          <section>
            <h2 className="mb-2 text-[1.35rem]">{copy.adds}</h2>
            <ul className="flex flex-col gap-2 text-sm">
              {capabilities.map(([id, capability]) => (
                <li key={id}>
                  <span className="font-semibold">{pluginText.capability(pluginId, id, "name", capability.name)}</span>
                  <span className="text-muted-foreground">: {pluginText.capability(pluginId, id, "description", capability.description)}</span>
                </li>
              ))}
            </ul>
          </section>
          {actions.length > 0 && <section>
            <h2 className="mb-2 text-[1.35rem]">{copy.agentsCall}</h2>
            <p className="text-sm text-muted-foreground">
              {actions.map((action, index) => <span key={action.id}>{index > 0 && ", "}<code className="font-semibold text-foreground">{action.id}</code></span>)}
              {" "}{copy.agentsCallHint}
            </p>
          </section>}
        </aside>
      </div>
    </div>
  );
}

"use client";

import { PluginIcon, usePluginText } from "@/lib/plugin-text";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useId, useState, type FormEvent } from "react";

import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "@/components/ui/accordion";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { useLocale } from "@/lib/locale-provider";
import { PluginCredentialField, SettingHelpLink } from "./plugin-credential-field";
import { ProviderOrder } from "./provider-order";

type Scope = "app" | "workspace";
export type PluginSettingDeclaration = {
  id: string;
  label: string;
  description: string;
  kind: "string" | "integer" | "boolean" | "secret";
  scope: Scope;
  choices?: string[];
  default?: string | boolean | number;
  /** A page explaining where to get the value, opened by the host from the installed manifest. */
  helpUrl?: string;
};
export type PluginSettingsManifest = { id: string; name: string; settings?: PluginSettingDeclaration[] };
type CatalogEntry = { bundled: boolean; manifest: PluginSettingsManifest };
type Values = {
  app: string | null;
  workspaceOverride: string | null;
  effective: string | null;
  source: Scope | null;
};

export function PluginPreferences({
  plugins,
  workspaceId,
}: {
  plugins: CatalogEntry[];
  workspaceId: number | null;
}) {
  const { messages: { pluginPreferences: copy } } = useLocale();
  const pluginText = usePluginText();
  const entries = plugins.map(({ manifest, bundled }) => ({
    ...manifest,
    name: pluginText.name(manifest.id, manifest.name),
    fields: manifest.settings?.filter(({ kind, scope }) =>
      (kind !== "secret" || bundled) && (workspaceId === null || scope === "workspace"),
    ).map((field) => pluginText.field(manifest.id, field)) ?? [],
  })).filter(({ fields }) => fields.length > 0);
  if (entries.length === 0) return null;

  return (
    <Card>
      <CardHeader>
        <CardTitle>{copy.title}</CardTitle>
        <CardDescription>{workspaceId === null ? copy.appIntro : copy.workspaceIntro}</CardDescription>
      </CardHeader>
      <CardContent>
        <Accordion type="single" collapsible>
          {entries.map(({ id, name, fields }) => {
            const ordinary = fields.filter((field) => field.kind !== "secret");
            const credentials = fields.filter((field) => field.kind === "secret");
            return (
              <AccordionItem key={id} value={id}>
                <AccordionTrigger className="items-center wrap-anywhere">
                  <span className="flex min-w-0 items-center gap-3"><PluginIcon pluginId={id} className="size-7" />{name}</span>
                </AccordionTrigger>
                <AccordionContent>
                  <div className="flex flex-col gap-4">
                    {ordinary.length > 0 && <PluginFields pluginId={id} fields={ordinary} workspaceId={workspaceId} />}
                    {credentials.map((field) => <PluginCredentialField key={field.id} field={field} pluginId={id} workspaceId={workspaceId} />)}
                  </div>
                </AccordionContent>
              </AccordionItem>
            );
          })}
        </Accordion>
      </CardContent>
    </Card>
  );
}

export function ScopedPluginPreferences({ workspaceId }: { workspaceId: number | null }) {
  const { messages: { pluginPreferences: copy, workspaces } } = useLocale();
  const [manifests, setManifests] = useState<CatalogEntry[]>();
  const [error, setError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let active = true;
    invoke<CatalogEntry[]>("list_plugin_catalog").then(
      (catalog) => { if (active) setManifests(catalog); },
      () => { if (active) setError(true); },
    );
    return () => { active = false; };
  }, [attempt]);

  if (error) return <Alert variant="destructive">
    <AlertDescription>{copy.loadError}</AlertDescription>
    <Button variant="outline" onClick={() => { setError(false); setAttempt((value) => value + 1); }}>{workspaces.retry}</Button>
  </Alert>;
  if (!manifests) return <Skeleton className="h-12" aria-label={copy.loading} />;
  return <>
    <ProviderOrder catalog={manifests} workspaceId={workspaceId} />
    <PluginPreferences plugins={manifests} workspaceId={workspaceId} />
  </>;
}

/** The agent plugin's own settings, such as web search, model and reasoning effort. */
export function AgentCapabilities({ pluginId, workspaceId }: { pluginId: string; workspaceId: number | null }) {
  const { messages: { pluginPreferences: copy } } = useLocale();
  const [fields, setFields] = useState<PluginSettingDeclaration[]>();
  useEffect(() => {
    let active = true;
    invoke<CatalogEntry[]>("list_plugin_catalog").then((catalog) => {
      if (!active) return;
      const manifest = catalog.find((entry) => entry.manifest.id === pluginId)?.manifest;
      setFields(manifest?.settings?.filter(({ kind, scope }) =>
        kind !== "secret" && (workspaceId === null || scope === "workspace")) ?? []);
    }, () => { if (active) setFields([]); });
    return () => { active = false; };
  }, [pluginId, workspaceId]);
  if (!fields?.length) return null;
  return (
    <Card>
      <CardHeader>
        <CardTitle>{copy.agentTitle}</CardTitle>
        <CardDescription>{copy.agentIntro}</CardDescription>
      </CardHeader>
      <CardContent>
        <PluginFields key={`${pluginId}:${workspaceId ?? "app"}`} pluginId={pluginId} fields={fields} workspaceId={workspaceId} />
      </CardContent>
    </Card>
  );
}

export function PluginFields({ pluginId, fields, workspaceId }: {
  pluginId: string;
  fields: PluginSettingDeclaration[];
  workspaceId: number | null;
}) {
  const { messages: { pluginPreferences: copy, workspaces } } = useLocale();
  const pluginText = usePluginText();
  const [values, setValues] = useState<Record<string, Values>>();
  const [loadFailed, setLoadFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [saving, setSaving] = useState(false);
  // Changes waiting for Save: a JSON value, or null to go back to the inherited one.
  const [drafts, setDrafts] = useState<Record<string, string | null>>({});
  const [feedback, setFeedback] = useState<"saved" | "saveError" | "invalid">();
  const workspace = workspaceId !== null;

  useEffect(() => {
    let active = true;
    invoke<Record<string, Values>>("read_plugin_settings", { pluginId, workspaceId }).then(
      (result) => { if (active) setValues(result); },
      () => { if (active) setLoadFailed(true); },
    );
    return () => { active = false; };
  }, [pluginId, workspaceId, attempt]);

  function change(id: string, value: string | null) {
    setFeedback(undefined);
    setDrafts((current) => ({ ...current, [id]: value }));
  }

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const pending = Object.entries(drafts);
    if (pending.some(([id, value]) => fields.find((field) => field.id === id)?.kind === "integer"
      && value !== null && !/^-?(0|[1-9][0-9]*)$/.test(value))) {
      setFeedback("invalid");
      return;
    }
    setSaving(true);
    setFeedback(undefined);
    try {
      for (const [settingId, value] of pending) {
        const field = fields.find((entry) => entry.id === settingId);
        const valueJson = value === null ? null : field?.kind === "string" ? JSON.stringify(value) : value;
        await invoke("write_plugin_setting", { pluginId, settingId, workspaceId, valueJson });
      }
      setDrafts({});
      setFeedback("saved");
    } catch {
      setFeedback("saveError");
    } finally {
      setValues(await invoke<Record<string, Values>>("read_plugin_settings", { pluginId, workspaceId }).catch(() => values));
      setSaving(false);
    }
  }

  if (loadFailed) return <Alert variant="destructive">
    <AlertDescription>{copy.loadError}</AlertDescription>
    <Button variant="outline" onClick={() => { setLoadFailed(false); setAttempt((value) => value + 1); }}>{workspaces.retry}</Button>
  </Alert>;
  if (!values) return <Skeleton className="h-24" aria-label={copy.loading} />;

  const changed = Object.keys(drafts).length > 0;
  return <form className="flex flex-col gap-4" onSubmit={save}>
    <div className="flex flex-col divide-y">
      {fields.map((field) => values[field.id] && (
        <PreferenceField key={field.id} field={pluginText.field(pluginId, field)} values={values[field.id]} workspace={workspace} pluginId={pluginId}
          draft={field.id in drafts ? drafts[field.id] : undefined} disabled={saving}
          onChange={(value) => change(field.id, value)} />
      ))}
    </div>
    <div className="flex flex-wrap items-center gap-3">
      <Button type="submit" size="sm" disabled={saving || !changed}>{saving ? copy.saving : copy.save}</Button>
      {changed && !saving && <Button type="button" size="sm" variant="ghost" onClick={() => { setDrafts({}); setFeedback(undefined); }}>{copy.discard}</Button>}
      {feedback && <p role={feedback === "saved" ? "status" : "alert"}
        className={feedback === "saved" ? "text-sm text-muted-foreground" : "text-sm text-destructive"}>{copy[feedback]}</p>}
    </div>
  </form>;
}

function PreferenceField({ field, values, workspace, pluginId, draft, disabled, onChange }: {
  pluginId: string;
  field: PluginSettingDeclaration & { choiceLabels: Record<string, string> };
  values: Values;
  workspace: boolean;
  /** A pending JSON value, null to inherit, or undefined when unchanged. */
  draft: string | null | undefined;
  disabled: boolean;
  onChange: (value: string | null) => void;
}) {
  const { messages: { pluginPreferences: copy } } = useLocale();
  const id = useId();
  const own = workspace ? values.workspaceOverride : values.app;
  const saved = field.kind === "string" && values.effective !== null
    ? JSON.parse(values.effective) as string
    : values.effective ?? "";
  const fallback = field.default === undefined ? undefined
    : typeof field.default === "boolean" ? String(field.default) : String(field.default);
  const shown = draft === undefined ? saved : draft === null ? "" : draft;
  const effective = shown === "" ? fallback ?? "" : shown;
  const resetting = draft === null;
  const source = draft !== undefined ? copy.unsaved
    : values.source ? copy[values.source] : fallback !== undefined ? copy.pluginDefault : copy.unset;
  const canReset = !resetting && (own !== null || (draft !== undefined && draft !== null));

  const control = field.kind === "boolean" ? (
    <Switch id={id} aria-describedby={id + "-description"} disabled={disabled}
      checked={effective === "true"} onCheckedChange={(checked) => onChange(String(checked))} />
  ) : field.choices ? (
    <Select value={shown} onValueChange={(value) => onChange(value)} disabled={disabled}>
      <SelectTrigger id={id} aria-describedby={id + "-description"} className="w-44"><SelectValue placeholder={fallback !== undefined ? field.choiceLabels[fallback] ?? fallback : copy.unset} /></SelectTrigger>
      <SelectContent><SelectGroup>
        {field.choices.map((choice) => <SelectItem key={choice} value={choice}>{field.choiceLabels[choice] ?? choice}</SelectItem>)}
      </SelectGroup></SelectContent>
    </Select>
  ) : (
    <Input id={id} aria-describedby={id + "-description"} className="w-full" placeholder={fallback}
      inputMode={field.kind === "integer" ? "numeric" : undefined}
      value={shown} onChange={(event) => onChange(event.target.value)} disabled={disabled} />
  );

  const status = <p className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
    <span>{source}</span>
    {canReset && <button type="button" className="underline underline-offset-2 hover:text-foreground" disabled={disabled}
      onClick={() => onChange(null)}>{workspace ? copy.inherit : copy.clear}</button>}
  </p>;
  // Text values stack like the key fields; switches and choices sit beside their label.
  if (field.kind !== "boolean" && !field.choices) return <div className="flex flex-col gap-2 py-3 first:pt-0 last:pb-0">
    <Label htmlFor={id} className="font-semibold">{field.label}</Label>
    <p id={id + "-description"} className="text-sm text-muted-foreground">{field.description}</p>
    {control}
    {status}
    <SettingHelpLink pluginId={pluginId} settingId={field.id} helpUrl={field.helpUrl} />
  </div>;
  return <div className="flex flex-wrap items-start justify-between gap-x-6 gap-y-2 py-3 first:pt-0 last:pb-0">
    <div className="flex min-w-0 flex-1 basis-64 flex-col gap-1">
      <Label htmlFor={id} className="font-semibold">{field.label}</Label>
      <p id={id + "-description"} className="text-sm text-muted-foreground">{field.description}</p>
      {status}
      <SettingHelpLink pluginId={pluginId} settingId={field.id} helpUrl={field.helpUrl} />
    </div>
    <div className="shrink-0 pt-0.5">{control}</div>
  </div>;
}

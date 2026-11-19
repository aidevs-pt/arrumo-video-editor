"use client";

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
import { useLocale } from "@/lib/locale-provider";
import { PluginCredentialField } from "./plugin-credential-field";

type Scope = "app" | "workspace";
export type PluginSettingDeclaration = {
  id: string;
  label: string;
  description: string;
  kind: "string" | "integer" | "boolean" | "secret";
  scope: Scope;
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
  const entries = plugins.map(({ manifest, bundled }) => ({
    ...manifest,
    fields: manifest.settings?.filter(({ kind, scope }) =>
      (kind !== "secret" || bundled) && (workspaceId === null || scope === "workspace"),
    ) ?? [],
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
                <AccordionTrigger className="wrap-anywhere">{name}</AccordionTrigger>
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

export function WorkspacePluginPreferences({ workspaceId }: { workspaceId: number }) {
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
  return <PluginPreferences plugins={manifests} workspaceId={workspaceId} />;
}

function PluginFields({ pluginId, fields, workspaceId }: {
  pluginId: string;
  fields: PluginSettingDeclaration[];
  workspaceId: number | null;
}) {
  const { messages: { pluginPreferences: copy, workspaces } } = useLocale();
  const [values, setValues] = useState<Record<string, Values>>();
  const [loadFailed, setLoadFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    let active = true;
    invoke<Record<string, Values>>("read_plugin_settings", { pluginId, workspaceId }).then(
      (result) => { if (active) setValues(result); },
      () => { if (active) setLoadFailed(true); },
    );
    return () => { active = false; };
  }, [pluginId, workspaceId, attempt]);

  async function write(settingId: string, valueJson: string | null) {
    setSaving(true);
    try {
      await invoke("write_plugin_setting", { pluginId, settingId, workspaceId, valueJson });
      setValues(await invoke<Record<string, Values>>("read_plugin_settings", { pluginId, workspaceId }));
    } finally {
      setSaving(false);
    }
  }

  if (loadFailed) return <Alert variant="destructive">
    <AlertDescription>{copy.loadError}</AlertDescription>
    <Button variant="outline" onClick={() => { setLoadFailed(false); setAttempt((value) => value + 1); }}>{workspaces.retry}</Button>
  </Alert>;
  if (!values) return <Skeleton className="h-24" aria-label={copy.loading} />;

  return <div className="flex flex-col divide-y">
    {fields.map((field) => values[field.id] && (
      <PreferenceField key={field.id} field={field} values={values[field.id]} workspace={workspaceId !== null}
        saving={saving} write={(value) => write(field.id, value)} />
    ))}
  </div>;
}

function PreferenceField({ field, values, workspace, saving, write }: {
  field: PluginSettingDeclaration;
  values: Values;
  workspace: boolean;
  saving: boolean;
  write: (value: string | null) => Promise<void>;
}) {
  const { messages: { pluginPreferences: copy } } = useLocale();
  const id = useId();
  const own = workspace ? values.workspaceOverride : values.app;
  const present = field.kind === "string" && values.effective !== null
    ? JSON.parse(values.effective) as string
    : values.effective ?? "";
  const [draft, setDraft] = useState<string>();
  const [feedback, setFeedback] = useState<"invalid" | "saveError" | "saved">();

  async function save(value: string | null) {
    setFeedback(undefined);
    try {
      await write(value);
      setDraft(undefined);
      setFeedback("saved");
    } catch {
      setFeedback("saveError");
    }
  }

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const value = draft ?? present;
    if (field.kind === "integer" && !/^-?(0|[1-9][0-9]*)$/.test(value)) {
      setFeedback("invalid");
      return;
    }
    if (field.kind === "boolean" && value === "") {
      setFeedback("invalid");
      return;
    }
    void save(field.kind === "string" ? JSON.stringify(value) : value);
  }

  return <form className="flex flex-col gap-2 py-3 first:pt-0 last:pb-0" onSubmit={submit}>
    <Field>
      <FieldLabel htmlFor={id}>{field.label}</FieldLabel>
      <FieldDescription id={id + "-description"}>{field.description}</FieldDescription>
      {field.kind === "boolean" ? (
        <Select value={draft ?? present} onValueChange={setDraft} disabled={saving}>
          <SelectTrigger id={id} aria-describedby={id + "-description"}><SelectValue placeholder={copy.unset} /></SelectTrigger>
          <SelectContent><SelectGroup>
            <SelectItem value="true">{copy.yes}</SelectItem>
            <SelectItem value="false">{copy.no}</SelectItem>
          </SelectGroup></SelectContent>
        </Select>
      ) : <Input id={id} aria-describedby={id + "-description"} inputMode={field.kind === "integer" ? "numeric" : undefined}
        value={draft ?? present} onChange={(event) => setDraft(event.target.value)} disabled={saving} />}
      <FieldDescription>{copy[values.source ?? "unset"]}</FieldDescription>
    </Field>
    <div className="flex flex-wrap gap-2">
      <Button type="submit" size="sm" disabled={saving || draft === undefined}>{copy.save}</Button>
      {own !== null && <Button type="button" size="sm" variant="outline" disabled={saving} onClick={() => void save(null)}>
        {workspace ? copy.inherit : copy.clear}
      </Button>}
    </div>
    {feedback && <p role={feedback === "saved" ? "status" : "alert"} className={feedback === "saved" ? "text-sm text-muted-foreground" : "text-sm text-destructive"}>{copy[feedback]}</p>}
  </form>;
}

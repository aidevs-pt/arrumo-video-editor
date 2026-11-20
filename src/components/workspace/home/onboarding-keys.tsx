"use client";

import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft } from "lucide-react";
import { useEffect, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { PluginSettingsManifest } from "@/components/workspace/settings/plugin-preferences";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";

type Service = PluginSettingsManifest & { description: string };
type Status = { appConfigured: boolean };
type Preferences = Record<string, { effective: string[] }>;

/** Collects app-wide keys for bundled services in one pass. The default transcription provider is required. */
export function OnboardingKeys({ services, onBack, onDone }: {
  services: Service[];
  onBack: () => void;
  onDone: () => void;
}) {
  const { messages: { onboarding: copy } } = useLocale();
  const [required, setRequired] = useState<string | null>(null);
  const [saved, setSaved] = useState<Record<string, boolean>>({});
  const [values, setValues] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);
  const [failed, setFailed] = useState(false);
  const fields = services.flatMap((service) => (service.settings ?? [])
    .filter(({ kind }) => kind === "secret").map((field) => ({ service, field, key: `${service.id}/${field.id}` })));

  useEffect(() => {
    let active = true;
    invoke<Preferences>("list_provider_preferences", { workspaceId: null })
      .then((preferences) => { if (active) setRequired(preferences.transcription?.effective[0] ?? null); }, () => {});
    for (const { service, field, key } of fields) {
      invoke<Status>("read_plugin_credential_status", { pluginId: service.id, settingId: field.id, workspaceId: null })
        .then((status) => { if (active) setSaved((current) => ({ ...current, [key]: status.appConfigured })); }, () => {});
    }
    return () => { active = false; };
    // Field identities come from the bundled catalog, which does not change while onboarding is open.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const requiredMissing = fields.some(({ service, key }) => service.id === required && !saved[key] && !values[key]?.trim());

  async function save() {
    setSaving(true);
    setFailed(false);
    let ok = true;
    for (const { service, field, key } of fields) {
      const secret = values[key]?.trim();
      if (!secret) continue;
      try {
        await invoke("write_plugin_credential", { pluginId: service.id, settingId: field.id, workspaceId: null, secret });
        setSaved((current) => ({ ...current, [key]: true }));
        setValues((current) => ({ ...current, [key]: "" }));
      } catch {
        ok = false;
      }
    }
    setSaving(false);
    if (!ok) setFailed(true);
    return ok;
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (await save()) onDone();
  }

  return (
    <form onSubmit={submit} className="flex flex-1 flex-col">
      <div className="divide-y border-y">
        {[...fields].sort((a, b) => Number(b.service.id === required) - Number(a.service.id === required)).map(({ service, field, key }) => {
          const isRequired = service.id === required;
          return (
            <section key={key} aria-labelledby={`service-${key}`} className="grid gap-3 py-5 md:grid-cols-2 md:gap-8">
              <div>
                <h2 id={`service-${key}`} className="flex flex-wrap items-center gap-2.5 font-heading text-[1.375rem] font-[850] [font-variation-settings:'wdth'_112]">
                  {service.name}
                  <span className={cn("rounded-[5px] px-2 py-0.5 font-sans text-sm [font-variation-settings:normal]",
                    isRequired ? "bg-attention font-bold text-ink" : "font-normal text-muted-foreground")}>
                    {isRequired ? copy.required : copy.optional}
                  </span>
                </h2>
                <p className="mt-1.5 text-muted-foreground">{service.description}</p>
              </div>
              <div className="flex flex-col gap-2">
                <label htmlFor={`key-${key}`} className="font-semibold">{field.label}</label>
                <Input id={`key-${key}`} type="password" autoComplete="new-password" spellCheck={false} maxLength={2048}
                  placeholder={saved[key] ? "••••••••••••••" : copy.pasteKey} disabled={saving}
                  aria-describedby={`key-${key}-hint`} value={values[key] ?? ""}
                  onChange={(event) => setValues((current) => ({ ...current, [key]: event.target.value }))} className="h-11" />
                <p id={`key-${key}-hint`} className="text-sm text-muted-foreground">
                  {saved[key] ? <span className="flex items-center gap-2 font-semibold text-approval">
                    <span aria-hidden="true" className="size-2.5 rounded-[2px] bg-approval" />{copy.keySaved}</span> : null}
                </p>
              </div>
            </section>
          );
        })}
      </div>
      <p className="mt-6 text-sm text-muted-foreground">{copy.servicesFooter}</p>
      {failed && <Alert variant="destructive" className="mt-4"><AlertDescription>{copy.keysError}</AlertDescription></Alert>}
      <footer className="mt-auto flex flex-wrap items-center justify-between gap-3 pt-10">
        <Button type="button" variant="ghost" disabled={saving} onClick={onBack}><ArrowLeft data-icon="inline-start" />{copy.back}</Button>
        <div className="flex flex-wrap gap-3">
          <Button type="button" variant="outline" size="lg" disabled={saving} onClick={onDone}>
            {requiredMissing ? copy.skipAll : copy.skipOptional}
          </Button>
          <Button type="submit" size="lg" disabled={saving || !Object.values(values).some((value) => value.trim())}>
            {saving ? copy.savingKeys : copy.saveKeys}
          </Button>
        </div>
      </footer>
    </form>
  );
}

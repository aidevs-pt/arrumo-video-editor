"use client";

import { SettingHelpLink } from "@/components/workspace/settings/plugin-credential-field";
import { usePluginText } from "@/lib/plugin-text";
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

/** Collects app-wide keys for bundled services in one pass. At least one transcription provider needs a key; the preferred one comes first. */
export function OnboardingKeys({ services, onBack, onDone }: {
  services: Service[];
  onBack: () => void;
  onDone: () => void;
}) {
  const pluginText = usePluginText();
  const { locale, messages: { onboarding: copy } } = useLocale();
  const [transcribers, setTranscribers] = useState<string[]>([]);
  const [saved, setSaved] = useState<Record<string, boolean>>({});
  const [values, setValues] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);
  const [failed, setFailed] = useState(false);
  const fields = services.flatMap((service) => (service.settings ?? [])
    .filter(({ kind }) => kind === "secret").map((field) => ({ service, field, key: `${service.id}/${field.id}` })));

  useEffect(() => {
    let active = true;
    invoke<Preferences>("list_provider_preferences", { workspaceId: null })
      .then((preferences) => { if (active) setTranscribers(preferences.transcription?.effective ?? []); }, () => {});
    for (const { service, field, key } of fields) {
      invoke<Status>("read_plugin_credential_status", { pluginId: service.id, settingId: field.id, workspaceId: null })
        .then((status) => { if (active) setSaved((current) => ({ ...current, [key]: status.appConfigured })); }, () => {});
    }
    return () => { active = false; };
    // Field identities come from the bundled catalog, which does not change while onboarding is open.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const transcriberFields = fields.filter(({ service }) => transcribers.includes(service.id));
  const requiredMissing = transcriberFields.length > 0 && !transcriberFields.some(({ key }) => saved[key] || values[key]?.trim());
  const rank = (id: string) => { const index = transcribers.indexOf(id); return index < 0 ? transcribers.length : index; };

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
      {transcriberFields.length > 1 && <p className="mb-4 max-w-[64ch] rounded-lg bg-attention/20 px-3 py-2 text-sm">
        {copy.oneTranscriber.replace("{providers}", new Intl.ListFormat(locale, { type: "disjunction" })
          .format(transcriberFields.map(({ service }) => pluginText.name(service.id, service.name))))}
      </p>}
      <div className="divide-y border-y">
        {[...fields].sort((a, b) => rank(a.service.id) - rank(b.service.id)).map(({ service, field, key }) => {
          const isRequired = transcribers.includes(service.id);
          const preferred = transcribers[0] === service.id;
          return (
            <section key={key} aria-labelledby={`service-${key}`} className="grid gap-3 py-5 md:grid-cols-2 md:gap-8">
              <div>
                <h2 id={`service-${key}`} className="flex flex-wrap items-center gap-2.5 font-heading text-[1.375rem] font-[850] [font-variation-settings:'wdth'_112]">
                  {pluginText.name(service.id, service.name)}
                  <span className={cn("rounded-[5px] px-2 py-0.5 font-sans text-sm [font-variation-settings:normal]",
                    isRequired ? "bg-attention font-bold text-ink" : "font-normal text-muted-foreground")}>
                    {!isRequired ? copy.optional : preferred ? copy.recommended : copy.orThisOne}
                  </span>
                </h2>
                <p className="mt-1.5 text-muted-foreground">{pluginText.description(service.id, service.description)}</p>
              </div>
              <div className="flex flex-col gap-2">
                <label htmlFor={`key-${key}`} className="font-semibold">{pluginText.setting(service.id, field.id, "label", field.label)}</label>
                <Input id={`key-${key}`} type="password" autoComplete="new-password" spellCheck={false} maxLength={2048}
                  placeholder={saved[key] ? "••••••••••••••" : copy.pasteKey} disabled={saving}
                  aria-describedby={`key-${key}-hint`} value={values[key] ?? ""}
                  onChange={(event) => setValues((current) => ({ ...current, [key]: event.target.value }))} className="h-11" />
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <p id={`key-${key}-hint`} className="text-sm text-muted-foreground">
                    {saved[key] ? <span className="flex items-center gap-2 font-semibold text-approval">
                      <span aria-hidden="true" className="size-2.5 rounded-[2px] bg-approval" />{copy.keySaved}</span> : null}
                  </p>
                  <SettingHelpLink pluginId={service.id} settingId={field.id} helpUrl={field.helpUrl} className="ms-auto" />
                </div>
              </div>
            </section>
          );
        })}
      </div>
      <p className="mt-6 text-sm text-muted-foreground">{copy.servicesFooter}</p>
      {failed && <Alert variant="destructive" className="mt-4"><AlertDescription>{copy.keysError}</AlertDescription></Alert>}
      <footer className="sticky bottom-0 z-10 mt-auto flex flex-wrap items-center justify-between gap-3 border-t bg-background/95 py-4 backdrop-blur-sm mt-10">
        <Button type="button" variant="ghost" size="lg" disabled={saving} onClick={onBack}><ArrowLeft data-icon="inline-start" />{copy.back}</Button>
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

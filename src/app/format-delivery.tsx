"use client";

import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { deliveryCopy } from "./delivery-copy";

type Supplemental = {
  thumbnails: number;
  titles: number;
  descriptions: number;
  tags: boolean;
  thumbnail_guidance: string;
  specifications: string;
};

type Delivery = {
  required: ("final_video" | "subtitles")[];
  supplemental: Supplemental & { portrait_derivatives: unknown };
};

type ProfileDelivery = {
  workspace: Supplemental;
  effective: Delivery;
  overrides: { [Key in keyof Supplemental]: Supplemental[Key] | null } & { portrait_derivatives: unknown };
};

const countFields = ["thumbnails", "titles", "descriptions"] as const;
const textFields = ["thumbnail_guidance", "specifications"] as const;

export function FormatDelivery({
  workspaceId, profileId, format, locale, onBack,
}: {
  workspaceId: number;
  profileId?: number;
  format: { id: number; name: string };
  locale: keyof typeof deliveryCopy;
  onBack: () => void;
}) {
  const [delivery, setDelivery] = useState<Delivery>();
  const [profileDelivery, setProfileDelivery] = useState<ProfileDelivery>();
  const [overrides, setOverrides] = useState<Partial<Record<keyof Supplemental, boolean>>>({});
  const isProfile = profileId !== undefined;
  const [loadError, setLoadError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [pending, setPending] = useState(false);
  const [result, setResult] = useState<"saved" | "saveError" | null>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const copy = deliveryCopy[locale];

  useEffect(() => { heading.current?.focus(); }, []);
  useEffect(() => {
    let active = true;
    async function load() {
      try {
        if (profileId === undefined) {
          const value = await invoke<Delivery>("get_format_delivery", { workspaceId, formatId: format.id });
          if (active) setDelivery(value);
        } else {
          const value = await invoke<ProfileDelivery>("get_profile_delivery", { workspaceId, profileId, formatId: format.id });
          if (!active) return;
          setProfileDelivery(value);
          setDelivery(value.effective);
        }
      } catch {
        if (active) setLoadError(true);
      }
    }
    void load();
    return () => { active = false; };
  }, [workspaceId, profileId, format.id, attempt]);

  function overridden(field: keyof Supplemental) {
    return overrides[field] ?? (profileDelivery?.overrides[field] != null);
  }

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending) return;
    const fields = new FormData(event.currentTarget);
    const settings: Supplemental = {
      thumbnails: Number(fields.get("thumbnails")),
      titles: Number(fields.get("titles")),
      descriptions: Number(fields.get("descriptions")),
      tags: fields.has("tags"),
      thumbnail_guidance: String(fields.get("thumbnail_guidance") ?? ""),
      specifications: String(fields.get("specifications") ?? ""),
    };
    setPending(true);
    setResult(null);
    try {
      if (isProfile) {
        const custom = Object.fromEntries((Object.keys(settings) as (keyof Supplemental)[])
          .filter(overridden).map((field) => [field, settings[field]]));
        await invoke("set_profile_delivery", { workspaceId, profileId, formatId: format.id, overrides: { ...custom, portrait_derivatives: profileDelivery?.overrides.portrait_derivatives } });
      } else {
        await invoke("set_format_delivery", { workspaceId, formatId: format.id, settings: { ...settings, portrait_derivatives: delivery?.supplemental.portrait_derivatives } });
      }
      setResult("saved");
    } catch {
      setResult("saveError");
    } finally {
      setPending(false);
    }
  }

  function overrideControl(field: keyof Supplemental) {
    if (!profileDelivery) return null;
    const value = profileDelivery.workspace[field];
    const display = typeof value === "boolean" ? (value ? copy.on : copy.off) : String(value) || copy.empty;
    return (
      <>
        <Field orientation="horizontal">
          <input id={`override-${field}`} type="checkbox" checked={overridden(field)}
            aria-label={`${copy.override} · ${copy[field]}`}
            onChange={(event) => setOverrides({ ...overrides, [field]: event.target.checked })} />
          <FieldLabel htmlFor={`override-${field}`}>{copy.override}</FieldLabel>
        </Field>
        <p className="profile-guidance">{copy.workspaceValue} · {display}</p>
      </>
    );
  }

  return (
    <main className="workspace-shell profile-shell">
      <Button type="button" variant="ghost" onClick={onBack} disabled={pending}>
        <ArrowLeft data-icon="inline-start" />
        {isProfile ? copy.backProfile : copy.back}
      </Button>
      <div className="workspace-heading">
        <h1 ref={heading} tabIndex={-1}>{copy.title}</h1>
        <p>{format.name}</p>
      </div>
      {loadError ? (
        <Alert variant="destructive">
          <AlertDescription>{copy.loadError}</AlertDescription>
          <Button type="button" variant="outline" onClick={() => {
            setLoadError(false);
            setAttempt((value) => value + 1);
          }}>{copy.retry}</Button>
        </Alert>
      ) : delivery ? (
        <div className="workspace-layout delivery-layout">
          <section aria-labelledby="delivery-required">
            <h2 id="delivery-required">{copy.required}</h2>
            <ul className="workspace-rows">
              {delivery.required.map((output) => <li key={output}>{copy[output]}</li>)}
            </ul>
          </section>
          <form className="workspace-form" aria-labelledby="delivery-supplemental" aria-busy={pending}
            onSubmit={save} onChange={() => setResult(null)}>
            <h2 id="delivery-supplemental">{copy.supplemental}</h2>
            <fieldset disabled={pending}>
              <FieldGroup>
                <div className="delivery-counts">
                  {countFields.map((field) => (
                    <Field key={field}>
                      <FieldLabel htmlFor={field}>{copy[field]}</FieldLabel>
                      <Input id={field} name={field} type="number" min={0} max={65535} required hidden={isProfile && !overridden(field)} disabled={isProfile && !overridden(field)}
                        defaultValue={delivery.supplemental[field]} />
                      {overrideControl(field)}
                    </Field>
                  ))}
                </div>
                <Field orientation="horizontal">
                  <input id="tags" name="tags" type="checkbox" hidden={isProfile && !overridden("tags")} disabled={isProfile && !overridden("tags")}
                    defaultChecked={delivery.supplemental.tags} />
                  <FieldLabel htmlFor="tags">{copy.tags}</FieldLabel>
                </Field>
                {overrideControl("tags")}
                {textFields.map((field) => (
                  <Field key={field}>
                    <FieldLabel htmlFor={field}>{copy[field]}</FieldLabel>
                    <Textarea id={field} name={field} hidden={isProfile && !overridden(field)}
                      disabled={isProfile && !overridden(field)} defaultValue={delivery.supplemental[field]} />
                    {overrideControl(field)}
                  </Field>
                ))}
                {result === "saved" && <p role="status">{copy.saved}</p>}
                {result === "saveError" && <Alert variant="destructive"><AlertDescription>{copy.saveError}</AlertDescription></Alert>}
                <Button type="submit">{copy.save}</Button>
              </FieldGroup>
            </fieldset>
          </form>
        </div>
      ) : <p role="status">{copy.loading}</p>}
    </main>
  );
}

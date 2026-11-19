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

type Delivery = {
  required: ("final_video" | "subtitles")[];
  supplemental: {
    thumbnails: number;
    titles: number;
    descriptions: number;
    tags: boolean;
    thumbnail_guidance: string;
    specifications: string;
  };
};

const countFields = ["thumbnails", "titles", "descriptions"] as const;
const textFields = ["thumbnail_guidance", "specifications"] as const;

export function FormatDelivery({
  workspaceId, format, locale, onBack,
}: {
  workspaceId: number;
  format: { id: number; name: string };
  locale: keyof typeof deliveryCopy;
  onBack: () => void;
}) {
  const [delivery, setDelivery] = useState<Delivery>();
  const [loadError, setLoadError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [pending, setPending] = useState(false);
  const [result, setResult] = useState<"saved" | "saveError" | null>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const copy = deliveryCopy[locale];

  useEffect(() => { heading.current?.focus(); }, []);
  useEffect(() => {
    let active = true;
    invoke<Delivery>("get_format_delivery", { workspaceId, formatId: format.id }).then(
      (value) => { if (active) setDelivery(value); },
      () => { if (active) setLoadError(true); },
    );
    return () => { active = false; };
  }, [workspaceId, format.id, attempt]);

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending) return;
    const fields = new FormData(event.currentTarget);
    const settings: Delivery["supplemental"] = {
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
      await invoke("set_format_delivery", { workspaceId, formatId: format.id, settings });
      setResult("saved");
    } catch {
      setResult("saveError");
    } finally {
      setPending(false);
    }
  }

  return (
    <main className="workspace-shell profile-shell">
      <Button type="button" variant="ghost" onClick={onBack} disabled={pending}>
        <ArrowLeft data-icon="inline-start" />
        {copy.back}
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
                      <Input id={field} name={field} type="number" min={0} max={65535} required defaultValue={delivery.supplemental[field]} />
                    </Field>
                  ))}
                </div>
                <Field orientation="horizontal">
                  <input id="tags" name="tags" type="checkbox" defaultChecked={delivery.supplemental.tags} />
                  <FieldLabel htmlFor="tags">{copy.tags}</FieldLabel>
                </Field>
                {textFields.map((field) => (
                  <Field key={field}>
                    <FieldLabel htmlFor={field}>{copy[field]}</FieldLabel>
                    <Textarea id={field} name={field} defaultValue={delivery.supplemental[field]} />
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

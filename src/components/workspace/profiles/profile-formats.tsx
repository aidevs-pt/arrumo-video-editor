"use client";

import { useLocale } from "@/lib/locale-provider";
import { WorkspaceHeading, WorkspaceRows, WorkspaceShell } from "@/components/workspace/layout";
import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft, PanelsTopLeft } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { FormatDelivery } from "@/components/workspace/outputs/format-delivery";

type ProfileFormat = {
  id: number;
  name: string;
  width: number;
  height: number;
  workspace_width: number;
  workspace_height: number;
  overrides: { width: number | null; height: number | null };
};

export function ProfileFormats({
  workspaceId, profile, onBack, onOutputs,
}: {
  workspaceId: number;
  profile: { id: number; name: string };

  onBack: () => void;
  onOutputs: () => void;
}) {
  const [deliveryFormat, setDeliveryFormat] = useState<ProfileFormat | null>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const [formats, setFormats] = useState<ProfileFormat[]>();
  const [loadError, setLoadError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [pending, setPending] = useState<number | null>(null);
  const [result, setResult] = useState<{ id: number; saved: boolean } | null>(null);
  const { messages } = useLocale();
  const copy = messages.profileFormats;

  useEffect(() => { heading.current?.focus(); }, []);
  useEffect(() => {
    let current = true;
    invoke<ProfileFormat[]>("list_profile_formats", { workspaceId, profileId: profile.id }).then(
      (rows) => { if (current) setFormats(rows); },
      () => { if (current) setLoadError(true); },
    );
    return () => { current = false; };
  }, [workspaceId, profile.id, attempt]);

  async function save(event: FormEvent<HTMLFormElement>, formatId: number) {
    event.preventDefault();
    if (pending !== null) return;
    const data = new FormData(event.currentTarget);
    const width = data.get("width");
    const height = data.get("height");
    setResult(null);
    setPending(formatId);
    try {
      await invoke("set_profile_format_overrides", {
        workspaceId,
        profileId: profile.id,
        formatId,
        overrides: { width: width ? Number(width) : null, height: height ? Number(height) : null },
      });
      setResult({ id: formatId, saved: true });
    } catch {
      setResult({ id: formatId, saved: false });
    } finally {
      setPending(null);
    }
  }

  if (deliveryFormat) {
    return <FormatDelivery
      workspaceId={workspaceId}
      profileId={profile.id}
      format={deliveryFormat}
      onBack={() => setDeliveryFormat(null)}
    />;
  }

  return (
    <WorkspaceShell>
      <Button type="button" variant="ghost" onClick={onBack} disabled={pending !== null}>
        <ArrowLeft data-icon="inline-start" />
        {copy.back}
      </Button>
      <Button type="button" variant="outline" onClick={onOutputs} disabled={pending !== null}>{copy.manage}</Button>
      <WorkspaceHeading icon={PanelsTopLeft}>
        <h1 ref={heading} tabIndex={-1}>{copy.title}</h1>
        <p>{profile.name}</p>
      </WorkspaceHeading>
      {loadError ? (
        <Alert variant="destructive">
          <AlertDescription>{copy.loadError}</AlertDescription>
          <Button type="button" variant="outline" onClick={() => {
            setLoadError(false);
            setAttempt((value) => value + 1);
          }}>{copy.retry}</Button>
        </Alert>
      ) : !formats ? (
        <p role="status">{copy.loading}</p>
      ) : formats.length === 0 ? (
        <p>{copy.empty}</p>
      ) : (
        <>
          <p id="format-inheritance">{copy.inherit}</p>
          <WorkspaceRows className="mt-[18px]">
            {formats.map((format) => (
              <li key={format.id}>
                <form onSubmit={(event) => void save(event, format.id)} aria-labelledby={`profile-format-${format.id}`}
                  onChange={() => { if (result?.id === format.id) setResult(null); }}>
                  <div className="flex flex-wrap items-baseline justify-between gap-2.5">
                    <h2 id={`profile-format-${format.id}`} className="flex min-w-0 items-baseline gap-2.5 font-[680]">{format.name}</h2>
                    <Button type="button" size="sm" variant="outline" disabled={pending !== null}
                      aria-label={`${messages.delivery.title} · ${format.name}`}
                      onClick={() => setDeliveryFormat(format)}>{messages.delivery.title}</Button>
                  </div>
                  <fieldset disabled={pending !== null}>
                    <FieldGroup className="my-3 grid grid-cols-2 items-end min-[681px]:grid-cols-[repeat(2,minmax(0,1fr))_auto] max-[680px]:[&>button]:col-span-full max-[680px]:[&>button]:justify-self-start">
                      <Field>
                        <FieldLabel htmlFor={`profile-format-width-${format.id}`}>{copy.width}</FieldLabel>
                        <Input id={`profile-format-width-${format.id}`} name="width" type="number" min={1} max={4294967295}
                          aria-describedby="format-inheritance" placeholder={String(format.workspace_width)}
                          defaultValue={format.overrides.width ?? ""} />
                      </Field>
                      <Field>
                        <FieldLabel htmlFor={`profile-format-height-${format.id}`}>{copy.height}</FieldLabel>
                        <Input id={`profile-format-height-${format.id}`} name="height" type="number" min={1} max={4294967295}
                          aria-describedby="format-inheritance" placeholder={String(format.workspace_height)}
                          defaultValue={format.overrides.height ?? ""} />
                      </Field>
                      <Button type="submit" aria-label={`${copy.save} · ${format.name}`}>{copy.save}</Button>
                    </FieldGroup>
                  </fieldset>
                  {result?.id === format.id && (
                    result.saved ? <p role="status">{copy.saved}</p> : <p role="alert">{copy.saveError}</p>
                  )}
                </form>
              </li>
            ))}
          </WorkspaceRows>
        </>
      )}
    </WorkspaceShell>
  );
}

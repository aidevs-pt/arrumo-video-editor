"use client";

import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldDescription, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { profileCopy } from "./profile-copy";

type Profile = { id: number; name: string; guidance: string; color: string | null };

export function WorkspaceProfiles({
  workspace,
  locale,
  onBack,
}: {
  workspace: { id: number; name: string };
  locale: keyof typeof profileCopy;
  onBack: () => void;
}) {
  const heading = useRef<HTMLHeadingElement>(null);
  const [attempt, setAttempt] = useState(0);
  const [status, setStatus] = useState<"loading" | "ready" | "error">("loading");
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [name, setName] = useState("");
  const [guidance, setGuidance] = useState("");
  const [color, setColor] = useState("");
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState(false);
  const copy = profileCopy[locale];
  const disabled = saving || status !== "ready";

  useEffect(() => {
    heading.current?.focus();
  }, []);

  useEffect(() => {
    let active = true;
    invoke<Profile[]>("list_profiles", { workspaceId: workspace.id }).then(
      (results) => {
        if (!active) return;
        setProfiles(results);
        setStatus("ready");
      },
      () => {
        if (active) setStatus("error");
      },
    );
    return () => { active = false; };
  }, [workspace.id, attempt]);

  async function createProfile(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (disabled || !name.trim()) return;
    setSaving(true);
    setSaveError(false);
    try {
      const profile = await invoke<Profile>("create_profile", {
        workspaceId: workspace.id,
        name,
        guidance,
        color: color || null,
      });
      setProfiles((current) => [...current, profile]);
      setName("");
      setGuidance("");
      setColor("");
    } catch {
      setSaveError(true);
    } finally {
      setSaving(false);
    }
  }

  return (
    <main className="workspace-shell profile-shell">
      <Button type="button" variant="ghost" onClick={onBack} disabled={saving}>
        <ArrowLeft data-icon="inline-start" />
        {copy.back}
      </Button>
      <div className="workspace-heading">
        <h1 ref={heading} tabIndex={-1}>{workspace.name}</h1>
      </div>
      <div className="workspace-layout">
        <section className="workspace-list" aria-labelledby="profile-list-title">
          <h2 id="profile-list-title">{copy.title}</h2>
          {status === "loading" && <p role="status">{copy.loading}</p>}
          {status === "error" && (
            <Alert variant="destructive">
              <AlertDescription>{copy.loadError}</AlertDescription>
              <Button type="button" variant="outline" onClick={() => {
                setStatus("loading");
                setAttempt((current) => current + 1);
              }}>{copy.retry}</Button>
            </Alert>
          )}
          {status === "ready" && (profiles.length ? (
            <ul className="workspace-rows" aria-live="polite">
              {profiles.map((profile) => (
                <li key={profile.id}>
                  <h3 className="profile-name">
                    {profile.color && <span className="profile-color" style={{ backgroundColor: profile.color }} aria-hidden="true" />}
                    {profile.name}
                  </h3>
                  {profile.guidance && <p className="profile-guidance">{profile.guidance}</p>}
                </li>
              ))}
            </ul>
          ) : (
            <Empty className="workspace-empty">
              <EmptyHeader>
                <EmptyTitle>{copy.empty}</EmptyTitle>
                <EmptyDescription>{copy.emptyHint}</EmptyDescription>
              </EmptyHeader>
            </Empty>
          ))}
        </section>
        <form className="workspace-form" onSubmit={createProfile} aria-labelledby="new-profile-title">
          <h2 id="new-profile-title">{copy.form}</h2>
          <FieldGroup>
            <Field data-disabled={disabled}>
              <FieldLabel htmlFor="profile-name">{copy.name}</FieldLabel>
              <Input id="profile-name" value={name} onChange={(event) => setName(event.target.value)} disabled={disabled} required />
            </Field>
            <Field data-disabled={disabled}>
              <FieldLabel htmlFor="profile-guidance">{copy.guidance}</FieldLabel>
              <Textarea id="profile-guidance" value={guidance} onChange={(event) => setGuidance(event.target.value)} disabled={disabled} />
            </Field>
            <Field data-disabled={disabled}>
              <FieldLabel htmlFor="profile-color">{copy.color}</FieldLabel>
              <Input id="profile-color" value={color} onChange={(event) => setColor(event.target.value)} disabled={disabled}
                pattern="#[0-9A-Fa-f]{6}" aria-describedby="profile-color-hint" />
              <FieldDescription id="profile-color-hint">{copy.colorHint}</FieldDescription>
            </Field>
            {saveError && <Alert variant="destructive" role="alert"><AlertDescription>{copy.createError}</AlertDescription></Alert>}
            <Button type="submit" disabled={disabled || !name.trim()}>
              {saving ? copy.saving : copy.submit}
            </Button>
          </FieldGroup>
        </form>
      </div>
    </main>
  );
}

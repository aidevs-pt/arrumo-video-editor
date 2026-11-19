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
import { ProfileFormats } from "./profile-formats";
import { profileCopy } from "./profile-copy";

type Profile = { id: number; name: string; guidance: string; color: string | null };
type Draft = { name: string; guidance: string; color: string };
const emptyDraft: Draft = { name: "", guidance: "", color: "" };

export function WorkspaceProfiles({
  workspace,
  locale,
  onBack,
  onOutputs,
  onAssets,
  onContext,
  onBrand,
}: {
  workspace: { id: number; name: string };
  locale: keyof typeof profileCopy;
  onBack: () => void;
  onOutputs: () => void;
  onAssets: () => void;
  onContext: () => void;
  onBrand: () => void;
}) {
  const heading = useRef<HTMLHeadingElement>(null);
  const [attempt, setAttempt] = useState(0);
  const [status, setStatus] = useState<"loading" | "ready" | "error">("loading");
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const nameInput = useRef<HTMLInputElement>(null);
  const editButton = useRef<HTMLButtonElement>(null);
  const [newDraft, setNewDraft] = useState<Draft>(emptyDraft);
  const [formatProfile, setFormatProfile] = useState<Profile | null>(null);
  const [editing, setEditing] = useState<{ id: number; draft: Draft } | null>(null);
  const editId = editing?.id;
  const draft = editing?.draft ?? newDraft;
  const { name, guidance, color } = draft;
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState(false);
  const copy = profileCopy[locale];
  const disabled = saving || status !== "ready";

  useEffect(() => {
    heading.current?.focus();
  }, []);

  useEffect(() => {
    if (editId !== undefined) {
      nameInput.current?.focus();
    } else {
      editButton.current?.focus();
    }
  }, [editId]);

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

  function setField(field: keyof Draft, value: string) {
    const next = { ...draft, [field]: value };
    if (editing) {
      setEditing({ ...editing, draft: next });
    } else {
      setNewDraft(next);
    }
  }

  async function saveProfile(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (disabled || !name.trim()) return;
    setSaving(true);
    setSaveError(false);
    try {
      const profile = await invoke<Profile>(editing ? "update_profile" : "create_profile", {
        workspaceId: workspace.id,
        ...(editing && { profileId: editing.id }),
        name,
        guidance,
        color: color || null,
      });
      if (editing) {
        setProfiles((current) => current.map((row) => row.id === profile.id ? profile : row));
        setEditing(null);
      } else {
        setProfiles((current) => [...current, profile]);
        setNewDraft(emptyDraft);
      }
    } catch {
      setSaveError(true);
    } finally {
      setSaving(false);
    }
  }

  if (formatProfile) {
    return (
      <ProfileFormats
        workspaceId={workspace.id}
        profile={formatProfile}
        locale={locale}
        onBack={() => setFormatProfile(null)}
        onOutputs={onOutputs}
      />
    );
  }

  return (
    <main className="workspace-shell profile-shell">
      <Button type="button" variant="ghost" onClick={onBack} disabled={saving}>
        <ArrowLeft data-icon="inline-start" />
        {copy.back}
      </Button>
      <Button type="button" variant="outline" onClick={onOutputs} disabled={saving || editing !== null}>{copy.outputs}</Button>
      <Button type="button" variant="outline" onClick={onAssets} disabled={saving || editing !== null}>{copy.assets}</Button>
      <Button type="button" variant="outline" onClick={onContext} disabled={saving || editing !== null}>{copy.context}</Button>
      <Button type="button" variant="outline" onClick={onBrand} disabled={saving || editing !== null}>{copy.brand}</Button>
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
                  <div className="profile-row-heading">
                    <h3 className="profile-name">
                      {profile.color && <span className="profile-color" style={{ backgroundColor: profile.color }} aria-hidden="true" />}
                      {profile.name}
                    </h3>
                    <div className="profile-edit-actions">
                      <Button type="button" size="sm" variant="outline"
                        aria-label={`${copy.formats} · ${profile.name}`}
                        disabled={saving || editing !== null}
                        onClick={() => setFormatProfile(profile)}>
                        {copy.formats}
                      </Button>
                      <Button
                        type="button"
                        size="sm"
                        variant="outline"
                        aria-label={`${copy.edit} ${profile.name}`}
                        disabled={saving || editing !== null}
                        onClick={(event) => {
                          editButton.current = event.currentTarget;
                          setSaveError(false);
                          setEditing({
                            id: profile.id,
                            draft: { name: profile.name, guidance: profile.guidance, color: profile.color ?? "" },
                          });
                        }}
                      >
                        {copy.edit}
                      </Button>
                    </div>
                  </div>
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
        <form className="workspace-form" onSubmit={saveProfile} aria-labelledby="profile-form-title">
          <h2 id="profile-form-title">{editing ? copy.editTitle : copy.form}</h2>
          <FieldGroup>
            <Field data-disabled={disabled}>
              <FieldLabel htmlFor="profile-name">{copy.name}</FieldLabel>
              <Input ref={nameInput} id="profile-name" value={name} onChange={(event) => setField("name", event.target.value)} disabled={disabled} required />
            </Field>
            <Field data-disabled={disabled}>
              <FieldLabel htmlFor="profile-guidance">{copy.guidance}</FieldLabel>
              <Textarea id="profile-guidance" value={guidance} onChange={(event) => setField("guidance", event.target.value)} disabled={disabled} />
            </Field>
            <Field data-disabled={disabled}>
              <FieldLabel htmlFor="profile-color">{copy.color}</FieldLabel>
              <Input id="profile-color" value={color} onChange={(event) => setField("color", event.target.value)} disabled={disabled}
                pattern="#[0-9A-Fa-f]{6}" aria-describedby="profile-color-hint" />
              <FieldDescription id="profile-color-hint">{copy.colorHint}</FieldDescription>
            </Field>
            {saveError && <Alert variant="destructive" role="alert"><AlertDescription>{editing ? copy.updateError : copy.createError}</AlertDescription></Alert>}
            {editing ? (
              <div className="profile-edit-actions">
                <Button type="submit" disabled={disabled || !name.trim()}>{copy.save}</Button>
                <Button type="button" variant="outline" disabled={saving} onClick={() => {
                  setEditing(null);
                  setSaveError(false);
                }}>{copy.cancel}</Button>
              </div>
            ) : (
              <Button type="submit" disabled={disabled || !name.trim()}>
                {saving ? copy.saving : copy.submit}
              </Button>
            )}
          </FieldGroup>
        </form>
      </div>
    </main>
  );
}

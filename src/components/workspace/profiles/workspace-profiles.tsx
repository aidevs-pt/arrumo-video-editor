"use client";

import { Layers, Plus, Settings2 } from "lucide-react";
import { useLocale } from "@/lib/locale-provider";
import { WorkspaceColumns, WorkspaceForm, WorkspaceHeading, WorkspaceRows, WorkspaceShell } from "@/components/workspace/layout";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldDescription, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Textarea } from "@/components/ui/textarea";
import { useWorkspaceCatalog } from "../home/workspace-router";
import Link from "next/link";

type Profile = { id: number; name: string; guidance: string; color: string | null };
type Draft = { name: string; guidance: string; color: string };
const emptyDraft: Draft = { name: "", guidance: "", color: "" };

export function WorkspaceProfiles({
  workspace,
  onDrafts,
}: {
  workspace: { id: number; name: string };

  onDrafts: (profileId?: number) => void;
}) {
  const heading = useRef<HTMLHeadingElement>(null);
  const [attempt, setAttempt] = useState(0);
  const [status, setStatus] = useState<"loading" | "ready" | "error">("loading");
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const nameInput = useRef<HTMLInputElement>(null);
  const editButton = useRef<HTMLButtonElement>(null);
  const [newDraft, setNewDraft] = useState<Draft>(emptyDraft);
  const [editing, setEditing] = useState<{ id: number; draft: Draft } | null>(null);
  const editId = editing?.id;
  const draft = editing?.draft ?? newDraft;
  const { name, guidance, color } = draft;
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState(false);
  const { messages } = useLocale();
  const copy = messages.profiles;
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
      window.dispatchEvent(new Event("arrumo:profiles-changed"));
    } catch {
      setSaveError(true);
    } finally {
      setSaving(false);
    }
  }

  return (
    <WorkspaceShell>
      <WorkspaceHeading icon={Layers}>
        <h1 ref={heading} tabIndex={-1}>{copy.title}</h1>
      </WorkspaceHeading>
      <WorkspaceColumns>
        <section className="min-w-0 [&>p]:mt-3 [&>p]:max-w-[60ch] [&>p]:text-muted-foreground" aria-labelledby="profile-list-title">
          <h2 id="profile-list-title">{copy.list}</h2>
          {status === "loading" && <Skeleton className="h-32 w-full" role="status" aria-label={copy.loading} />}
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
            <WorkspaceRows  aria-live="polite">
              {profiles.map((profile) => (
                <li key={profile.id}>
                  <div className="flex flex-wrap items-baseline justify-between gap-2.5">
                    <h3 className="flex min-w-0 items-baseline gap-2.5 font-[680]">
                      {profile.color && <span className="size-2.5 shrink-0 rounded-full outline-1 outline-border" style={{ backgroundColor: profile.color }} aria-hidden="true" />}
                      {profile.name}
                    </h3>
                    <div className="flex flex-wrap items-baseline gap-2.5">
                      <Button type="button" size="sm" variant="outline" disabled={saving || editing !== null}
                        aria-label={`${copy.drafts} · ${profile.name}`} onClick={() => onDrafts(profile.id)}>
                        {copy.drafts}
                      </Button>
                      {saving || editing !== null ? (
                        <Button type="button" size="sm" variant="outline" disabled>{copy.formats}</Button>
                      ) : (
                        <Button asChild size="sm" variant="outline">
                          <Link href={`/profile?workspace=${workspace.id}&profile=${profile.id}`}
                            aria-label={`${copy.formats} · ${profile.name}`}><Settings2 data-icon="inline-start" />{copy.formats}</Link>
                        </Button>
                      )}
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
                  {profile.guidance && <p className="mt-1.25 whitespace-pre-wrap text-muted-foreground">{profile.guidance}</p>}
                </li>
              ))}
            </WorkspaceRows>
          ) : (
            <Empty className="min-h-44 border border-dashed">
              <EmptyHeader>
                <EmptyMedia variant="icon"><Layers /></EmptyMedia>
                <EmptyTitle>{copy.empty}</EmptyTitle>
                <EmptyDescription>{copy.emptyHint}</EmptyDescription>
              </EmptyHeader>
            </Empty>
          ))}
        </section>
        <WorkspaceForm  onSubmit={saveProfile} aria-labelledby="profile-form-title">
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
              <div className="flex flex-wrap items-baseline gap-2.5">
                <Button type="submit" disabled={disabled || !name.trim()}>{copy.save}</Button>
                <Button type="button" variant="outline" disabled={saving} onClick={() => {
                  setEditing(null);
                  setSaveError(false);
                }}>{copy.cancel}</Button>
              </div>
            ) : (
              <Button type="submit" disabled={disabled || !name.trim()}>
                <Plus data-icon="inline-start" />{saving ? copy.saving : copy.submit}
              </Button>
            )}
          </FieldGroup>
        </WorkspaceForm>
      </WorkspaceColumns>
    </WorkspaceShell>
  );
}


export function WorkspaceProfilesRoute() {
  const { workspace, navigate } = useWorkspaceCatalog();
  return workspace ? <WorkspaceProfiles key={workspace.id} workspace={workspace}
    onDrafts={(profile) => navigate("drafts", profile)} /> : null;
}

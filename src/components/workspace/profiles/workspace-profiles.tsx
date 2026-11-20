"use client";

import { ChevronRight, Layers, Plus, Settings2 } from "lucide-react";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";
import { WorkspaceHeading, WorkspaceShell } from "@/components/workspace/layout";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldDescription, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Textarea } from "@/components/ui/textarea";
import { useWorkspaceCatalog } from "../home/workspace-router";
import Link from "next/link";
import { profileHref } from "../home/workspace-links";

type Profile = { id: number; name: string; guidance: string; color: string | null };
type Draft = { name: string; guidance: string; color: string };
const emptyDraft: Draft = { name: "", guidance: "", color: "" };
const profileColors = ["#E55D4D", "#C68820", "#138778", "#6974D6", "#AB5980"];

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
  const [formOpen, setFormOpen] = useState(false);
  const [newDraft, setNewDraft] = useState<Draft>(emptyDraft);
  const [editing, setEditing] = useState<{ id: number; draft: Draft } | null>(null);
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
      setFormOpen(false);
      window.dispatchEvent(new Event("arrumo:profiles-changed"));
    } catch {
      setSaveError(true);
    } finally {
      setSaving(false);
    }
  }

  return (
    <WorkspaceShell>
      <div className="flex flex-wrap items-start justify-between gap-3">
        <WorkspaceHeading>
          <h1 ref={heading} tabIndex={-1}>{copy.title}</h1>
          <p>{copy.emptyHint}</p>
        </WorkspaceHeading>
        <Button type="button" disabled={disabled} onClick={(event) => {
          editButton.current = event.currentTarget;
          setEditing(null);
          setSaveError(false);
          setFormOpen(true);
        }}>
          <Plus data-icon="inline-start" />{copy.form}
        </Button>
      </div>
      <section className="min-w-0" aria-label={copy.list}>
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
          <ul className="divide-y border-y" aria-live="polite">
            {profiles.map((profile) => (
              <li key={profile.id} className="px-1 py-3">
                <div className="flex flex-wrap items-baseline justify-between gap-2.5">
                  <Link href={profileHref(workspace.id, profile.id)}
                    className="group inline-flex min-w-0 items-center gap-2.5 rounded-sm font-semibold hover:text-primary">
                    <span className="size-3 shrink-0 rounded-[2px] bg-muted outline-1 outline-border"
                      style={profile.color ? { backgroundColor: profile.color } : undefined} aria-hidden="true" />
                    <span className="wrap-anywhere">{profile.name}</span>
                    <ChevronRight className="size-3.5 text-muted-foreground group-hover:text-primary" aria-hidden="true" />
                  </Link>
                  <div className="flex flex-wrap items-baseline gap-2.5">
                    <Button type="button" size="sm" variant="outline" disabled={saving || editing !== null}
                      aria-label={`${copy.drafts} · ${profile.name}`} onClick={() => onDrafts(profile.id)}>
                      {copy.drafts}
                    </Button>
                    {saving || editing !== null ? (
                      <Button type="button" size="sm" variant="outline" disabled>{copy.formats}</Button>
                    ) : (
                      <Button asChild size="sm" variant="outline">
                        <Link href={`${profileHref(workspace.id, profile.id)}&tab=outputs`}
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
                        setFormOpen(true);
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
                {profile.guidance && <p className="mt-1 line-clamp-2 whitespace-pre-wrap text-sm text-muted-foreground">{profile.guidance}</p>}
              </li>
            ))}
          </ul>
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
      <Dialog open={formOpen} onOpenChange={(next) => {
        if (saving) return;
        setFormOpen(next);
        if (!next) {
          setEditing(null);
          setSaveError(false);
        }
      }}>
        <DialogContent onCloseAutoFocus={(event) => {
          if (editButton.current?.isConnected) {
            event.preventDefault();
            editButton.current.focus();
          }
        }}>
          <DialogHeader>
            <DialogTitle>{editing ? copy.editTitle : copy.form}</DialogTitle>
            <DialogDescription>{copy.emptyHint}</DialogDescription>
          </DialogHeader>
          <form onSubmit={saveProfile}>
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
                <div className="flex gap-2" role="group" aria-label={copy.color}>
                  {profileColors.map((value) => <Button key={value} type="button" size="icon-sm" variant="outline"
                    aria-label={value} aria-pressed={color.toUpperCase() === value}
                    disabled={disabled} style={{ backgroundColor: value }} onClick={() => setField("color", value)}
                    className={cn("rounded-sm hover:outline-1 hover:outline-foreground", color.toUpperCase() === value && "outline-2 outline-offset-2 outline-foreground")} />)}
                </div>
                <Input id="profile-color" value={color} onChange={(event) => setField("color", event.target.value)} disabled={disabled}
                  pattern="#[0-9A-Fa-f]{6}" aria-describedby="profile-color-hint" />
                <FieldDescription id="profile-color-hint">{copy.colorHint}</FieldDescription>
              </Field>
              {saveError && <Alert variant="destructive" role="alert"><AlertDescription>{editing ? copy.updateError : copy.createError}</AlertDescription></Alert>}
              <DialogFooter>
                <Button type="button" variant="outline" disabled={saving} onClick={() => {
                  setFormOpen(false);
                  setEditing(null);
                  setSaveError(false);
                }}>{copy.cancel}</Button>
                <Button type="submit" disabled={disabled || !name.trim()}>
                  {saving ? copy.saving : editing ? copy.save : copy.submit}
                </Button>
              </DialogFooter>
            </FieldGroup>
          </form>
        </DialogContent>
      </Dialog>
    </WorkspaceShell>
  );
}


export function WorkspaceProfilesRoute() {
  const { workspace, navigate } = useWorkspaceCatalog();
  return workspace ? <WorkspaceProfiles key={workspace.id} workspace={workspace}
    onDrafts={(profile) => navigate("drafts", profile)} /> : null;
}

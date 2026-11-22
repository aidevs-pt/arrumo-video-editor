"use client";

import { ArrowUpRight, Clapperboard, Layers, Pencil, Plus, Settings2 } from "lucide-react";
import { ColorPicker, randomSwatch } from "@/components/workspace/color-picker";
import { ItemContextMenu } from "@/components/workspace/item-context-menu";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";
import { WorkspaceHeading, WorkspaceShell } from "@/components/workspace/layout";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { useSearchParams } from "next/navigation";
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
const emptyDraft = (): Draft => ({ name: "", guidance: "", color: randomSwatch() });

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
  const editButton = useRef<HTMLElement | null>(null);
  const [formOpen, setFormOpen] = useState(useSearchParams().get("new") === "1");
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
        setNewDraft(emptyDraft());
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
      <div className="mb-6 flex flex-wrap items-center justify-between gap-3">
        <WorkspaceHeading className="mb-0 flex-1">
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
          <ul className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3" aria-live="polite">
            {profiles.map((profile) => {
              const edit = (target?: HTMLElement) => {
                editButton.current = target ?? null;
                setSaveError(false);
                setFormOpen(true);
                setEditing({ id: profile.id, draft: { name: profile.name, guidance: profile.guidance, color: profile.color ?? "" } });
              };
              const busy = saving || editing !== null;
              return <ItemContextMenu key={profile.id} label={profile.name} items={[
                { label: messages.profileHub.openProfile, icon: ArrowUpRight, href: profileHref(workspace.id, profile.id) },
                { label: copy.drafts, icon: Clapperboard, onSelect: () => onDrafts(profile.id) },
                { label: copy.formats, icon: Settings2, href: `${profileHref(workspace.id, profile.id)}&tab=outputs` },
                { label: copy.edit, icon: Pencil, onSelect: () => edit(), group: true, disabled: busy },
              ]}>
                <li className="group/card relative flex min-h-44 flex-col rounded-xl border bg-card p-5 transition-colors hover:border-foreground/40 motion-reduce:transition-none">
                  <span aria-hidden="true" className="mb-4 h-1.5 w-14 rounded-[2px] bg-muted"
                    style={profile.color ? { backgroundColor: profile.color } : undefined} />
                  <h2 className="text-[1.375rem] leading-tight font-[850] wrap-anywhere [font-variation-settings:'wdth'_125]">
                    <Link href={profileHref(workspace.id, profile.id)}
                      className="rounded-sm outline-none after:absolute after:inset-0 after:rounded-xl focus-visible:after:ring-2 focus-visible:after:ring-ring">
                      {profile.name}
                    </Link>
                  </h2>
                  <p className={cn("mt-1.5 line-clamp-2 text-sm", profile.guidance ? "text-muted-foreground" : "text-muted-foreground/70 italic")}>
                    {profile.guidance.trim() || copy.noGuidance}
                  </p>
                  <div className="relative z-[1] mt-auto flex items-center gap-2 pt-5">
                    <Button type="button" size="sm" disabled={busy} onClick={() => onDrafts(profile.id)}
                      aria-label={`${copy.drafts} · ${profile.name}`}>
                      <Clapperboard data-icon="inline-start" />{copy.drafts}
                    </Button>
                    <Button asChild size="sm" variant="outline">
                      <Link href={`${profileHref(workspace.id, profile.id)}&tab=outputs`} aria-label={`${copy.formats} · ${profile.name}`}>
                        <Settings2 data-icon="inline-start" />{copy.formats}
                      </Link>
                    </Button>
                    <Button type="button" size="icon-sm" variant="ghost" className="ms-auto" disabled={busy}
                      aria-label={`${copy.edit} ${profile.name}`} onClick={(event) => edit(event.currentTarget)}>
                      <Pencil />
                    </Button>
                  </div>
                </li>
              </ItemContextMenu>;
            })}
            <li>
              <button type="button" disabled={disabled} onClick={(event) => {
                editButton.current = event.currentTarget;
                setEditing(null);
                setSaveError(false);
                setFormOpen(true);
              }} className="flex size-full min-h-44 flex-col items-center justify-center gap-2 rounded-xl border border-dashed p-5 text-center text-muted-foreground transition-colors hover:border-primary hover:text-primary focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none disabled:opacity-50 motion-reduce:transition-none">
                <Plus className="size-5" aria-hidden="true" />
                <span className="font-semibold">{copy.form}</span>
                <span className="max-w-56 text-sm">{copy.emptyHint}</span>
              </button>
            </li>
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
                <FieldLabel>{copy.color}</FieldLabel>
                <ColorPicker value={color || null} onChange={(value) => setField("color", value ?? "")} disabled={disabled}
                  label={copy.color} customLabel={messages.profileHub.customColor} noneLabel={messages.profileHub.noColor} />
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

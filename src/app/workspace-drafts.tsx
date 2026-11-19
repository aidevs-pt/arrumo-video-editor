"use client";

import { useLocale } from "@/lib/locale-provider";
import { WorkspaceColumns, WorkspaceForm, WorkspaceHeading, WorkspaceRows, WorkspaceShell } from "@/components/workspace/layout";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { ArrowLeft } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldDescription, FieldGroup, FieldLabel, FieldLegend, FieldSet } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { GenerationQueue } from "./generation-queue";

type CatalogEntry = { id: number; name: string };
type Asset = CatalogEntry & { kind: string };
type Draft = { id: number; title: string; intent: string; profile_id: number | null; source_ids: number[] };

export function WorkspaceDrafts({ workspace, initialProfile, onBack }: {
  workspace: CatalogEntry;
  initialProfile: number | null;

  onBack: () => void;
}) {
  const [queueView, setQueueView] = useState<{ draftId: number | null } | null>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const [drafts, setDrafts] = useState<Draft[]>();
  const [assets, setAssets] = useState<Asset[]>([]);
  const [profiles, setProfiles] = useState<CatalogEntry[]>([]);
  const [editing, setEditing] = useState<number | null>(null);
  const [title, setTitle] = useState("");
  const titleInput = useRef<HTMLInputElement>(null);
  const newDraft = useRef<Omit<Draft, "id"> | null>(null);
  const [profile, setProfile] = useState(initialProfile);
  const [intent, setIntent] = useState("");
  const [sources, setSources] = useState<number[]>([]);
  const [attempt, setAttempt] = useState(0);
  const [loadError, setLoadError] = useState(false);
  const [error, setError] = useState<"saveError" | "importError" | null>(null);
  const [pending, setPending] = useState<"saving" | "importing" | null>(null);
  const workspaceId = workspace.id;
  const { messages } = useLocale();
  const copy = messages.drafts;
  const names = new Map(assets.map((asset) => [asset.id, asset.name]));

  useEffect(() => { heading.current?.focus(); }, []);
  useEffect(() => {
    let active = true;
    Promise.all([
      invoke<Draft[]>("list_video_drafts", { workspaceId }),
      invoke<Asset[]>("list_assets", { workspaceId }),
      invoke<CatalogEntry[]>("list_profiles", { workspaceId }),
    ]).then(([items, media, entries]) => {
      if (!active) return;
      setDrafts(items);
      setAssets(media.filter((asset) => asset.kind === "video"));
      setProfiles(entries);
    }, () => { if (active) setLoadError(true); });
    return () => { active = false; };
  }, [workspaceId, attempt]);

  async function importClips() {
    if (pending) return;
    setPending("importing");
    setError(null);
    try {
      const files = await open({ title: copy.import, multiple: true,
        filters: [{ name: copy.sources, extensions: ["mp4", "mov", "webm", "mkv", "avi"] }] });
      for (const source of files ?? []) {
        try {
          const asset = await invoke<Asset>("import_asset", { workspaceId, source });
          if (asset.kind !== "video") {
            setError("importError");
            continue;
          }
          setAssets((current) => [...current, asset]);
          setSources((current) => [...current, asset.id]);
        } catch {
          setError("importError");
        }
      }
    } catch {
      setError("importError");
    } finally {
      setPending(null);
    }
  }

  function move(index: number, offset: number) {
    setSources((current) => {
      const next = [...current];
      [next[index], next[index + offset]] = [next[index + offset], next[index]];
      return next;
    });
  }

  function edit(draft: Draft) {
    newDraft.current = { title, intent, profile_id: profile, source_ids: sources };
    setEditing(draft.id);
    setTitle(draft.title);
    setProfile(draft.profile_id);
    setIntent(draft.intent);
    setSources(draft.source_ids);
    setError(null);
    titleInput.current?.focus();
  }

  function reset() {
    setEditing(null);
    const previous = newDraft.current;
    newDraft.current = null;
    setTitle(previous?.title ?? "");
    setProfile(previous ? previous.profile_id : initialProfile);
    setIntent(previous?.intent ?? "");
    setSources(previous?.source_ids ?? []);
    setError(null);
  }

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending || !title.trim() || (!sources.length && !intent.trim())) return;
    setPending("saving");
    setError(null);
    try {
      const draft = await invoke<Draft>(editing === null ? "create_video_draft" : "update_video_draft", {
        workspaceId,
        ...(editing !== null && { draftId: editing }),
        settings: { title, intent, profile_id: profile, source_ids: sources },
      });
      setDrafts((current) => {
        const rows = current ?? [];
        return editing === null ? [...rows, draft] : rows.map((row) => row.id === draft.id ? draft : row);
      });
      reset();
    } catch {
      setError("saveError");
    } finally {
      setPending(null);
    }
  }

  if (queueView && drafts) {
    return <GenerationQueue
      workspaceId={workspaceId}
      drafts={drafts}
      initialDraftId={queueView.draftId}
      onBack={() => setQueueView(null)}
    />;
  }

  return (
    <WorkspaceShell>
      <Button type="button" variant="ghost" disabled={pending !== null} onClick={onBack}>
        <ArrowLeft data-icon="inline-start" />{copy.back}
      </Button>
      <Button type="button" variant="outline" disabled={!drafts || pending !== null || editing !== null}
        onClick={() => setQueueView({ draftId: null })}>{messages.queue.title}</Button>
      <WorkspaceHeading>
        <h1 ref={heading} tabIndex={-1}>{copy.title}</h1><p>{workspace.name}</p>
      </WorkspaceHeading>
      {loadError ? <Alert variant="destructive">
        <AlertDescription>{copy.loadError}</AlertDescription>
        <Button type="button" variant="outline" onClick={() => {
          setLoadError(false);
          setAttempt((value) => value + 1);
        }}>{copy.retry}</Button>
      </Alert> : !drafts ? <p role="status">{copy.loading}</p> : (
        <WorkspaceColumns>
          <section aria-label={copy.title}>
            {drafts.length === 0 ? <p>{copy.empty}</p> : (
              <WorkspaceRows  aria-live="polite">
                {drafts.map((draft) => (
                  <li key={draft.id}>
                    <div className="flex flex-wrap items-baseline justify-between gap-2.5">
                      <h2>{draft.title}</h2>
                      <div className="flex flex-wrap items-baseline gap-2.5">
                        <Button type="button" size="sm" variant="outline" disabled={pending !== null || editing !== null}
                          aria-label={`${copy.edit} · ${draft.title}`} onClick={() => edit(draft)}>{copy.edit}</Button>
                        <Button type="button" size="sm" variant="outline" disabled={pending !== null || editing !== null}
                          aria-label={`${messages.queue.submit} · ${draft.title}`}
                          onClick={() => setQueueView({ draftId: draft.id })}>{messages.queue.submit}</Button>
                      </div>
                    </div>
                    {draft.profile_id !== null && <p>{profiles.find((entry) => entry.id === draft.profile_id)?.name}</p>}
                    {draft.intent && <p className="mt-1.25 whitespace-pre-wrap text-muted-foreground">{draft.intent}</p>}
                    {draft.source_ids.length > 0 && <ol className="list-decimal ps-6 [&>li]:py-0.75">
                      {draft.source_ids.map((id) => <li key={id}>{names.get(id) ?? id}</li>)}
                    </ol>}
                  </li>
                ))}
              </WorkspaceRows>
            )}
          </section>
          <WorkspaceForm  onSubmit={save} aria-labelledby="new-draft-title" aria-busy={pending !== null}>
            <h2 id="new-draft-title">{editing === null ? copy.newDraft : copy.edit}</h2>
            <fieldset disabled={pending !== null}>
              <FieldGroup>
                <Field>
                  <FieldLabel htmlFor="draft-title">{copy.name}</FieldLabel>
                  <Input ref={titleInput} id="draft-title" name="title" required value={title} onChange={(event) => setTitle(event.target.value)} />
                </Field>
                <Field>
                  <FieldLabel htmlFor="draft-profile">{copy.profile}</FieldLabel>
                  <NativeSelect id="draft-profile" className="w-full"
                    value={profile ?? ""} onChange={(event) => setProfile(event.target.value ? Number(event.target.value) : null)}>
                    <NativeSelectOption value="">{copy.noProfile}</NativeSelectOption>
                    {profiles.map((entry) => <NativeSelectOption key={entry.id} value={entry.id}>{entry.name}</NativeSelectOption>)}
                  </NativeSelect>
                </Field>
                <Field>
                  <FieldLabel htmlFor="draft-intent">{copy.intent}</FieldLabel>
                  <Textarea id="draft-intent" value={intent} aria-describedby="draft-intent-hint" onChange={(event) => setIntent(event.target.value)} />
                  <FieldDescription id="draft-intent-hint">{copy.intentHint}</FieldDescription>
                </Field>
                <FieldSet>
                  <FieldLegend>{copy.sources}</FieldLegend>
                  <Button type="button" variant="outline" onClick={() => void importClips()}>{copy.import}</Button>
                  <FieldGroup>
                    {assets.map((asset) => (
                      <Field key={asset.id} orientation="horizontal">
                        <input id={`draft-asset-${asset.id}`} type="checkbox" checked={sources.includes(asset.id)}
                          onChange={(event) => {
                            const checked = event.target.checked;
                            setSources((current) => checked
                              ? [...current, asset.id] : current.filter((id) => id !== asset.id));
                          }} />
                        <FieldLabel htmlFor={`draft-asset-${asset.id}`}>{asset.name}</FieldLabel>
                      </Field>
                    ))}
                  </FieldGroup>
                </FieldSet>
                {sources.length > 0 && <ol className="list-decimal ps-6 [&>li]:py-0.75" aria-label={copy.order}>
                  {sources.map((id, index) => (
                    <li key={id}>
                      {names.get(id) ?? id}
                      <div className="flex flex-wrap items-baseline gap-2.5">
                        <Button type="button" size="sm" variant="outline" disabled={index === 0}
                          aria-label={`${copy.up} · ${names.get(id) ?? id}`} onClick={() => move(index, -1)}>{copy.up}</Button>
                        <Button type="button" size="sm" variant="outline" disabled={index === sources.length - 1}
                          aria-label={`${copy.down} · ${names.get(id) ?? id}`} onClick={() => move(index, 1)}>{copy.down}</Button>
                      </div>
                    </li>
                  ))}
                </ol>}
                {error && <Alert variant="destructive"><AlertDescription>{copy[error]}</AlertDescription></Alert>}
                <div className="flex flex-wrap items-baseline gap-2.5">
                  <Button type="submit" disabled={!title.trim() || (!sources.length && !intent.trim())}>
                    {pending === "saving" ? copy.saving : copy.save}
                  </Button>
                  {editing !== null && <Button type="button" variant="outline" onClick={reset}>{copy.cancel}</Button>}
                </div>
              </FieldGroup>
            </fieldset>
          </WorkspaceForm>
        </WorkspaceColumns>
      )}
    </WorkspaceShell>
  );
}

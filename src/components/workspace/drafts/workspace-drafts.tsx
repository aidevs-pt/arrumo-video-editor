"use client";

import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Clapperboard, Film, ListVideo, Pencil, Plus } from "lucide-react";
import { useSearchParams } from "next/navigation";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldDescription, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { MarkdownEditor } from "@/components/markdown/editor";
import { GenerationQueue } from "@/components/workspace/generation/generation-queue";
import { WorkspaceHeading, WorkspaceShell } from "@/components/workspace/layout";
import { useLocale } from "@/lib/locale-provider";
import { useWorkspaceCatalog } from "../home/workspace-router";
import { DraftSourceClips } from "./draft-source-clips";

type CatalogEntry = { id: number; name: string };
type Asset = CatalogEntry & { kind: string };
type Draft = { id: number; title: string; intent: string; profile_id: number | null; source_ids: number[] };

export function WorkspaceDrafts({ workspace, initialProfile, startInQueue = false, startComposing = false, onBack }: {
  workspace: CatalogEntry;
  startInQueue?: boolean;
  startComposing?: boolean;
  initialProfile: number | null;

  onBack: () => void;
}) {
  const [queueView, setQueueView] = useState<{ draftId: number | null } | null>(startInQueue ? { draftId: null } : null);
  const heading = useRef<HTMLHeadingElement>(null);
  const [tab, setTab] = useState<"compose" | "saved">();
  const [notice, setNotice] = useState<string>();
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
  const canSave = Boolean(title.trim() && (sources.length || intent.trim()));
  const activeTab = tab ?? (startComposing || !drafts?.length ? "compose" : "saved");

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

  function edit(draft: Draft) {
    newDraft.current = { title, intent, profile_id: profile, source_ids: sources };
    setEditing(draft.id);
    setTab("compose");
    setNotice(undefined);
    setTitle(draft.title);
    setProfile(draft.profile_id);
    setIntent(draft.intent);
    setSources(draft.source_ids);
    setError(null);
    window.requestAnimationFrame(() => titleInput.current?.focus());
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
    if (pending || !canSave) return;
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
      setNotice(draft.title);
      setTab("saved");
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
      onBack={startInQueue ? onBack : () => setQueueView(null)}
    />;
  }

  return (
    <WorkspaceShell>
      <WorkspaceHeading icon={Clapperboard}>
        <h1 ref={heading} tabIndex={-1}>{copy.title}</h1>
        <p>{copy.description}</p>
      </WorkspaceHeading>
      {loadError ? (
        <Alert variant="destructive">
          <AlertDescription>{copy.loadError}</AlertDescription>
          <Button type="button" variant="outline" size="sm" onClick={() => {
            setLoadError(false);
            setAttempt((value) => value + 1);
          }}>{copy.retry}</Button>
        </Alert>
      ) : !drafts ? (
        <div className="grid gap-3" role="status" aria-label={copy.loading}>
          <Skeleton className="h-9 w-52" />
          <Skeleton className="h-52 w-full" />
        </div>
      ) : (
        <Tabs value={activeTab} onValueChange={(value) => {
          setNotice(undefined);
          setTab(value === "saved" ? "saved" : "compose");
        }} className="gap-5">
          <TabsList aria-label={copy.title} className="w-fit max-w-full">
            <TabsTrigger value="compose"><Plus />{editing === null ? copy.newDraft : copy.edit}</TabsTrigger>
            <TabsTrigger value="saved"><ListVideo />{copy.savedDrafts}
              {drafts.length > 0 && <Badge variant="secondary" className="hidden sm:inline-flex">{drafts.length}</Badge>}
            </TabsTrigger>
          </TabsList>
          <TabsContent value="compose">
            <form onSubmit={save} aria-busy={pending !== null} className="@container grid gap-4">
              <div className="grid items-start gap-4 @min-[46rem]:grid-cols-2">
                <DraftSourceClips assets={assets} sources={sources} disabled={pending !== null}
                  importing={pending === "importing"} onImport={() => void importClips()} onChange={setSources} />
                <Card>
                  <CardHeader>
                    <CardTitle><h2>{copy.details}</h2></CardTitle>
                  </CardHeader>
                  <CardContent>
                    <FieldGroup>
                      <Field>
                        <FieldLabel htmlFor="draft-title">{copy.name}</FieldLabel>
                        <Input ref={titleInput} id="draft-title" name="title" required value={title}
                          disabled={pending !== null} onChange={(event) => setTitle(event.target.value)} />
                      </Field>
                      <Field>
                        <FieldLabel htmlFor="draft-profile">{copy.profile}</FieldLabel>
                        <Select value={profile === null ? "none" : String(profile)} disabled={pending !== null}
                          onValueChange={(value) => setProfile(value === "none" ? null : Number(value))}>
                          <SelectTrigger id="draft-profile" className="w-full"><SelectValue /></SelectTrigger>
                          <SelectContent>
                            <SelectItem value="none">{copy.noProfile}</SelectItem>
                            {profiles.map((entry) => <SelectItem key={entry.id} value={String(entry.id)}>{entry.name}</SelectItem>)}
                          </SelectContent>
                        </Select>
                      </Field>
                      <Field>
                        <FieldLabel htmlFor="draft-intent">{copy.intent}</FieldLabel>
                        <MarkdownEditor id="draft-intent" label={copy.intent} value={intent}
                          disabled={pending !== null} onChange={setIntent} />
                        <FieldDescription>{copy.intentHint}</FieldDescription>
                      </Field>
                    </FieldGroup>
                  </CardContent>
                </Card>
              </div>
              {error && <Alert variant="destructive"><AlertDescription>{copy[error]}</AlertDescription></Alert>}
              <div className="flex flex-wrap items-center justify-end gap-2 border-t pt-4">
                {editing !== null && <Button type="button" variant="outline" disabled={pending !== null}
                  onClick={() => { reset(); setTab("saved"); }}>{copy.cancel}</Button>}
                <Button type="submit" disabled={pending !== null || !canSave}>
                  {pending === "saving" ? copy.saving : copy.save}
                </Button>
              </div>
            </form>
          </TabsContent>
          <TabsContent value="saved" className="grid gap-3">
            {notice && <Alert><AlertDescription role="status">{copy.saved} · {notice}</AlertDescription></Alert>}
            {drafts.length === 0 ? (
              <Empty>
                <EmptyHeader>
                  <EmptyMedia variant="icon"><Clapperboard /></EmptyMedia>
                  <EmptyTitle>{copy.empty}</EmptyTitle>
                  <EmptyDescription>{copy.emptyHint}</EmptyDescription>
                </EmptyHeader>
                <EmptyContent><Button type="button" onClick={() => setTab("compose")}>
                  <Plus data-icon="inline-start" />{copy.newDraft}
                </Button></EmptyContent>
              </Empty>
            ) : (
              <ul aria-label={copy.savedDrafts} className="grid gap-3">
                {drafts.map((draft) => {
                  const draftProfile = profiles.find((entry) => entry.id === draft.profile_id);
                  return (
                    <li key={draft.id}>
                      <Card size="sm">
                        <CardHeader>
                          <CardTitle className="wrap-anywhere"><h2>{draft.title}</h2></CardTitle>
                          {draft.intent && <CardDescription className="line-clamp-2 wrap-anywhere">{draft.intent}</CardDescription>}
                        </CardHeader>
                        <CardContent className="flex flex-wrap gap-2">
                          {draftProfile && <Badge variant="secondary">{draftProfile.name}</Badge>}
                          {draft.source_ids.length > 0 ? <Badge variant="outline">
                            <Film />{copy.sources} · {draft.source_ids.length}
                          </Badge> : <Badge variant="outline">{copy.intentOnly}</Badge>}
                        </CardContent>
                        <CardFooter className="flex flex-wrap gap-2">
                          <Button type="button" size="sm" variant="outline" disabled={pending !== null || editing !== null}
                            aria-label={`${copy.edit} · ${draft.title}`} onClick={() => edit(draft)}>
                            <Pencil data-icon="inline-start" />{copy.edit}
                          </Button>
                          <Button type="button" size="sm" disabled={pending !== null || editing !== null}
                            aria-label={`${messages.queue.submit} · ${draft.title}`}
                            onClick={() => setQueueView({ draftId: draft.id })}>{messages.queue.submit}</Button>
                        </CardFooter>
                      </Card>
                    </li>
                  );
                })}
              </ul>
            )}
          </TabsContent>
        </Tabs>
      )}
    </WorkspaceShell>
  );
}


export function WorkspaceDraftsRoute({ queue = false }: { queue?: boolean }) {
  const { workspace, draftProfile, navigate } = useWorkspaceCatalog();
  const compose = useSearchParams().get("compose") === "1" || draftProfile !== null;
  return workspace ? <WorkspaceDrafts key={`${workspace.id}-${queue}-${draftProfile}-${compose}`}
    workspace={workspace} initialProfile={draftProfile} startInQueue={queue} startComposing={compose}
    onBack={() => navigate("drafts")} /> : null;
}

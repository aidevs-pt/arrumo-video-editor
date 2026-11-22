"use client";

import { invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow, type DragDropEvent } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { ArrowLeft, ArrowRight, Clapperboard } from "lucide-react";
import { useRouter, useSearchParams } from "next/navigation";
import { useEffect, useEffectEvent, useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldLabel } from "@/components/ui/field";
import { Skeleton } from "@/components/ui/skeleton";
import { MarkdownEditor } from "@/components/markdown/editor";
import { GenerationQueue } from "@/components/workspace/generation/generation-queue";
import { WorkspaceHeading, WorkspaceShell } from "@/components/workspace/layout";
import { useLocale } from "@/lib/locale-provider";
import { useWorkspaceCatalog } from "../home/workspace-router";
import { ConfirmTitleDelete } from "../confirm-title-delete";
import { DraftSourceClips } from "./draft-source-clips";
import { RawFootageBatch } from "./raw-footage-batch";
import { VideoDetails } from "./video-details";
import { VideoThumbnail } from "../video-thumbnail";
import { VideoCatalogHeader } from "../videos/video-catalog-header";
import { cn } from "@/lib/utils";

type CatalogEntry = { id: number; name: string; color?: string | null };
type Asset = CatalogEntry & { kind: string };
type Draft = { id: number; title: string; intent: string; profile_id: number | null; source_ids: number[] };

export function WorkspaceDrafts({ workspace, initialProfile, startInQueue = false, startComposing = false, startFromScratch = false, openDraft, onBack }: {
  workspace: CatalogEntry;
  openDraft?: number;
  startInQueue?: boolean;
  startComposing?: boolean;
  startFromScratch?: boolean;
  initialProfile: number | null;

  onBack: () => void;
}) {
  const router = useRouter();
  const [queueView, setQueueView] = useState<{ draftId: number | null } | null>(startInQueue ? { draftId: null } : null);
  const heading = useRef<HTMLHeadingElement>(null);
  const [tab, setTab] = useState<"compose" | "saved">(startComposing ? "compose" : "saved");
  const [mode, setMode] = useState<"recordings" | "scratch">(startFromScratch ? "scratch" : "recordings");
  const [step, setStep] = useState<"source" | "details">("source");
  const [dragging, setDragging] = useState(false);
  const [notice, setNotice] = useState<string>();
  const [drafts, setDrafts] = useState<Draft[]>();
  const [assets, setAssets] = useState<Asset[]>([]);
  const [profiles, setProfiles] = useState<CatalogEntry[]>([]);
  const [editing, setEditing] = useState<number | null>(null);
  const [title, setTitle] = useState("");
  const titleInput = useRef<HTMLInputElement>(null);
  const newDraft = useRef<Omit<Draft, "id"> | null>(null);
  const openedDraft = useRef(false);
  const [profile, setProfile] = useState(initialProfile);
  const [intent, setIntent] = useState("");
  const [sources, setSources] = useState<number[]>([]);
  const [attempt, setAttempt] = useState(0);
  const [loadError, setLoadError] = useState(false);
  const [error, setError] = useState<"saveError" | "importError" | null>(null);
  const [pending, setPending] = useState<"saving" | "importing" | null>(null);
  const workspaceId = workspace.id;
  const { locale, messages } = useLocale();
  const copy = messages.drafts;
  const catalog = messages.videoCatalog;
  const canSave = Boolean(title.trim() && (sources.length || intent.trim()));
  const composing = tab === "compose" && !startInQueue;
  const sourceStep = composing && step === "source";

  useEffect(() => { heading.current?.focus(); }, []);
  useEffect(() => {
    let active = true;
    Promise.all([
      invoke<Draft[]>("list_video_drafts", { workspaceId }),
      invoke<Asset[]>("list_footage", { workspaceId }),
      invoke<CatalogEntry[]>("list_profiles", { workspaceId }),
      invoke<{ draft_id: number }[]>("list_generation_runs", { workspaceId }),
    ]).then(([items, media, entries, runs]) => {
      if (!active) return;
      const submitted = new Set(runs.map((run) => run.draft_id));
      setDrafts(items.filter((draft) => !submitted.has(draft.id) || draft.id === openDraft));
      setAssets(media.filter((asset) => asset.kind === "video"));
      setProfiles(entries);
      const requested = items.find((draft) => draft.id === openDraft);
      if (requested && !openedDraft.current) {
        openedDraft.current = true;
        edit(requested);
      }
    }, () => { if (active) setLoadError(true); });
    return () => { active = false; };
  }, [workspaceId, attempt]);

  async function importClips(dropped?: string[]) {
    if (pending) return;
    setPending("importing");
    setError(null);
    try {
      const files = dropped ?? await open({ title: copy.import, multiple: true,
        filters: [{ name: copy.sources, extensions: ["mp4", "mov", "webm", "mkv", "avi"] }] });
      for (const source of files ?? []) {
        if (!/\.(mp4|mov|webm|mkv|avi)$/i.test(source)) {
          setError("importError");
          continue;
        }
        try {
          const asset = await invoke<Asset>("import_asset", { workspaceId, source, footage: true });
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

  const handleDrag = useEffectEvent((event: DragDropEvent) => {
    const ready = Boolean(drafts) && sourceStep && mode === "recordings" && !pending;
    setDragging(ready && (event.type === "enter" || event.type === "over"));
    if (ready && event.type === "drop") void importClips(event.paths);
  });

  useEffect(() => {
    if (!isTauri()) return;
    let active = true;
    const subscription = getCurrentWindow().onDragDropEvent(({ payload }) => {
      if (active) handleDrag(payload);
    });
    return () => {
      active = false;
      void subscription.then((unlisten) => unlisten(), () => {});
    };
  }, []);

  function edit(draft: Draft) {
    newDraft.current = { title, intent, profile_id: profile, source_ids: sources };
    setEditing(draft.id);
    setMode(draft.source_ids.length ? "recordings" : "scratch");
    setStep("details");
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
    if (pending || !canSave || sourceStep) return;
    const submitter = (event.nativeEvent as SubmitEvent).submitter;
    const prepareGeneration = submitter instanceof HTMLButtonElement && submitter.value === "generate";
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
      setNotice(copy.saved);
      setTab("saved");
      if (prepareGeneration) setQueueView({ draftId: draft.id });
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
      catalogView={startInQueue}
      onBack={startInQueue ? onBack : () => setQueueView(null)}
    />;
  }

  return (
    <WorkspaceShell>
      {composing ? (
        <>
          <div className="mb-5">
            <ol className="grid max-w-4xl grid-cols-3 gap-1.5 text-sm text-muted-foreground" aria-label={messages.videoDetails.step2}>
              <li aria-current={sourceStep ? "step" : undefined}
                className={cn("border-t-[6px] pt-1.5", sourceStep ? "border-primary font-semibold text-primary" : "border-foreground font-semibold text-foreground")}>
                {mode === "recordings" ? messages.videoDetails.step1 : catalog.scratch}
              </li>
              <li aria-current={sourceStep ? undefined : "step"}
                className={cn("border-t-[6px] pt-1.5", !sourceStep ? "border-primary font-semibold text-primary" : "border-border")}>{messages.videoDetails.step2}</li>
              <li className="border-t-[6px] border-border pt-1.5">{messages.videoDetails.step3}</li>
            </ol>
          </div>
          {sourceStep && <WorkspaceHeading>
            <h1 ref={heading} tabIndex={-1}>{sourceStep
              ? mode === "recordings" ? catalog.recordingsHeading : catalog.scratchHeading
              : editing === null ? catalog.details : copy.edit}</h1>
            {sourceStep && <p>{mode === "recordings" ? (editing === null ? messages.batch.intro : copy.sourcesHint) : copy.intentHint}</p>}
          </WorkspaceHeading>}
        </>
      ) : <VideoCatalogHeader workspaceId={workspaceId} active={startInQueue ? "generations" : "drafts"} count={startInQueue ? undefined : drafts?.length} />}
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
        composing && sourceStep && mode === "recordings" && editing === null ? (
          <RawFootageBatch workspaceId={workspaceId} clips={assets} pool={sources} profiles={profiles}
            initialProfile={initialProfile} dragging={dragging} importing={pending === "importing"}
            onImport={() => void importClips()} onPoolChange={setSources}
            onScratch={() => { setMode("scratch"); setError(null); }}
            onBack={() => { reset(); setTab("saved"); }}
            onCreated={(created) => {
              setDrafts((current) => [...(current ?? []), ...created]);
              setSources([]);
              edit(created[0]);
            }} />
        ) : composing && !sourceStep ? (
          <VideoDetails workspace={workspace} draftId={editing} profiles={profiles}
            draft={{ title, intent, profile_id: profile, source_ids: sources }}
            onChange={(change) => {
              if (change.title !== undefined) setTitle(change.title);
              if (change.intent !== undefined) setIntent(change.intent);
              if (change.profile_id !== undefined) setProfile(change.profile_id);
            }}
            onSaved={(saved) => {
              setDrafts((current) => current?.some((row) => row.id === saved.id)
                ? current.map((row) => row.id === saved.id ? saved : row) : [...(current ?? []), saved]);
              setEditing(saved.id);
            }}
            onGenerated={(next, runId) => {
              if (next) edit(next);
              else router.push(`/generation-run?workspace=${workspaceId}&run=${runId}`);
            }}
            onBack={() => setStep("source")} />
        ) : composing ? (
          <form onSubmit={save} aria-busy={pending !== null} className="grid max-w-5xl gap-5">
            {sourceStep ? (
              mode === "recordings" ? (
                <>
                  {sources.length === 0 && <Button type="button" variant="outline" className="justify-self-start" disabled={pending !== null}
                    onClick={() => { setMode("scratch"); setError(null); }}>
                    {catalog.scratch}
                  </Button>}
                  <DraftSourceClips assets={assets} sources={sources} disabled={pending !== null} dragging={dragging}
                    importing={pending === "importing"} onImport={() => void importClips()} onChange={setSources} />
                </>
              ) : (
                <Field>
                  <FieldLabel htmlFor="draft-scratch-intent">{copy.intent}</FieldLabel>
                  <MarkdownEditor id="draft-scratch-intent" label={copy.intent} value={intent}
                    disabled={pending !== null} onChange={setIntent} />
                  <Button type="button" variant="outline" className="self-start" disabled={pending !== null}
                    onClick={() => setMode("recordings")}>{catalog.source}</Button>
                </Field>
              )
            ) : null}
            {error && <Alert variant="destructive"><AlertDescription>{copy[error]}</AlertDescription></Alert>}
            <div className="flex flex-wrap items-center justify-between gap-2 border-t pt-4">
              <Button type="button" variant="ghost" disabled={pending !== null} onClick={() => {
                if (sourceStep) { reset(); setTab("saved"); }
                else setStep("source");
              }}>
                <ArrowLeft data-icon="inline-start" />{sourceStep ? catalog.back : mode === "recordings" ? catalog.source : catalog.scratch}
              </Button>
              {sourceStep ? <Button type="button" disabled={pending !== null || (mode === "recordings" ? !sources.length : !intent.trim())}
                onClick={() => {
                  setStep("details");
                  window.requestAnimationFrame(() => titleInput.current?.focus());
                }}>
                {catalog.detailsNext}<ArrowRight data-icon="inline-end" />
              </Button> : null}
            </div>
          </form>
        ) : (
          <section className="grid gap-3">
            {notice && <Alert><AlertDescription role="status">{notice}</AlertDescription></Alert>}
            {drafts.length === 0 ? (
              <Empty>
                <EmptyHeader>
                  <EmptyMedia variant="icon"><Clapperboard /></EmptyMedia>
                  <EmptyTitle>{copy.empty}</EmptyTitle>
                  <EmptyDescription>{copy.emptyHint}</EmptyDescription>
                </EmptyHeader>

              </Empty>
            ) : (
              <ul aria-label={copy.savedDrafts} className="divide-y border-y">
                {drafts.map((draft) => {
                  const draftProfile = profiles.find((entry) => entry.id === draft.profile_id);
                  return (
                    <li key={draft.id} className="grid grid-cols-[auto_minmax(0,1fr)] items-center gap-x-3 gap-y-2 px-1 py-3 lg:grid-cols-[auto_minmax(0,1fr)_auto]">
                      <VideoThumbnail title={draft.title} color={draftProfile?.color} small className="w-24 opacity-70 sm:w-36" />
                      <div className="min-w-0">
                        <h2 className="font-heading text-xl leading-tight font-[850] wrap-anywhere [font-variation-settings:'wdth'_112]">{draft.title}</h2>
                        <div className="mt-0.5 flex flex-wrap items-center gap-x-3 gap-y-0.5 text-sm text-muted-foreground">
                          {draftProfile && <span className="flex items-center gap-1.5 font-semibold text-foreground">
                            <span aria-hidden="true" className="size-2.5 rounded-[2px]" style={{ backgroundColor: draftProfile.color ?? "var(--border)" }} />{draftProfile.name}</span>}
                          <span>{draft.source_ids.length === 0 ? copy.intentOnly : draft.source_ids.length === 1
                            ? catalog.singleClip : catalog.clipCount.replace("{count}", draft.source_ids.length.toLocaleString(locale))}</span>
                        </div>
                      </div>
                      <div className="col-start-2 flex flex-wrap items-center gap-1.5 lg:col-start-3">
                        <Button type="button" size="sm" variant="outline" disabled={pending !== null || editing !== null}
                          aria-label={`${catalog.continue} ${draft.title}`} onClick={() => edit(draft)}>{catalog.continue}</Button>
                        <ConfirmTitleDelete title={draft.title} compact
                          copy={{ ...copy, deleteTitle: copy.deleteDraft, deleting: copy.deletingDraft }}
                          onConfirm={(confirmedTitle) => invoke<boolean>("delete_video_draft", {
                            workspaceId, draftId: draft.id, confirmedTitle,
                          })}
                          disabled={pending !== null || editing !== null}
                          onDelete={() => {
                            setDrafts((current) => current?.filter((entry) => entry.id !== draft.id));
                            setNotice(copy.draftDeleted);
                            setTab("saved");
                          }} />
                      </div>
                    </li>
                  );
                })}
              </ul>
            )}
          </section>
        )
      )}
    </WorkspaceShell>
  );
}


export function WorkspaceDraftsRoute({ queue = false }: { queue?: boolean }) {
  const { workspace, draftProfile, navigate } = useWorkspaceCatalog();
  const query = useSearchParams();
  const requestedCompose = query.get("compose");
  const openDraft = Number(query.get("draft")) || undefined;
  const compose = requestedCompose === "1" || requestedCompose === "scratch" || draftProfile !== null;
  return workspace ? <WorkspaceDrafts key={`${workspace.id}-${queue}-${draftProfile}-${requestedCompose}-${openDraft}`}
    workspace={workspace} initialProfile={draftProfile} startInQueue={queue} startComposing={compose} startFromScratch={requestedCompose === "scratch"}
    openDraft={openDraft}
    onBack={() => navigate("drafts")} /> : null;
}

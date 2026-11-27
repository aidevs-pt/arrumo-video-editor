"use client";

import { ActionSheet, PlacedPluginActions, usePlacedActionLabel, usePlacedActions } from "@/components/workspace/plugins/placed-actions";
import type { InstalledAction } from "@/components/workspace/plugins/action-form";
import { ItemContextMenu } from "@/components/workspace/item-context-menu";
import { completedSubtitleTracks } from "./completed-subtitle-tracks";
import { PluginSurfaces } from "@/components/workspace/plugins/plugin-surfaces";
import { invoke } from "@tauri-apps/api/core";
import { Archive, Captions, CheckCircle2, ChevronDown, ClipboardList, Download, Film, FileText, Image, Music, Puzzle } from "lucide-react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useState, type FormEvent } from "react";
import { Alert, AlertAction, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { WorkspaceHeading } from "@/components/workspace/layout";
import { useLocale } from "@/lib/locale-provider";
import { ConfirmTitleDelete } from "../confirm-title-delete";
import { completedVideoHref, workspaceHref, type CompletedFile } from "../home/workspace-links";
import { useStudioTrail } from "../home/studio-trail";
import { RunMotionResults, type MotionStageResult } from "../motion-library/run-motion-results";
import type { Stage } from "../generation/generation-plan";
import { AttachCompletedMedia } from "./attach-completed-media";
import { ImportVideoVariant } from "./import-video-variant";
import { DetachCompletedMedia } from "./detach-completed-media";
import { OutputDateField } from "./output-date-field";
import { SupplementalMediaPreview, type SupplementalMedia } from "./supplemental-media-preview";
import { CompletedOutputPreview } from "./completed-output-preview";
import { CompletedVideoFeedback, type FeedbackResult, type VideoFeedback } from "./completed-video-feedback";
import { CompletedVideoReplay } from "./completed-video-replay";

type Output = {
  format_id: number;
  video: string;
  /** Null when the video was delivered without subtitles. */
  subtitles: string | null;
  duration_seconds: number;
  subtitle_cues: number;
};
type Format = { id: number; name: string; width: number; height: number };
type Activity = {
  state: "queued" | "running" | "paused" | "failed" | "completed" | "cancelled";
  kind: "submitted" | "claimed" | "resumed" | "transition";
  attempt: number;
  created_at: number;
  detail: string | null;
};
type TextOutput = { kind: "title" | "description" | "tags"; format_id: number | null; text: string };

type CompletedVideo = {
  run: {
    id: number;
    snapshot: {
      draft: { title: string; source_ids?: number[] };
      profile?: { name: string; color?: string | null } | null;
      formats?: { format: Format; delivery?: { supplemental?: { burned_captions?: boolean } } }[];
    };
  };
  plan: { steps: { stage: Stage; review_required: boolean }[] };
  production: { stage_index: number; results: (FeedbackResult & MotionStageResult)[] };
  activity: Activity[];
  feedback: VideoFeedback[];
  title: string;
  delivery: { verified_outputs: Output[] };
  supplemental_text: TextOutput[];
  supplemental_media: SupplementalMedia[];
  output_dates: Record<number, string>;
  footage_removed?: boolean;
};

export function CompletedVideoDetail({ workspaceId, runId, initialFormatId, file }: { workspaceId: number; runId: number; initialFormatId: number; file?: CompletedFile }) {
  const { messages: { completedVideo: copy, queue } } = useLocale();
  const [video, setVideo] = useState<CompletedVideo>();
  const [loadError, setLoadError] = useState(false);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let active = true;
    invoke<CompletedVideo>("get_completed_video", { workspaceId, runId }).then(
      (result) => {
        if (!active) return;
        setVideo(result);
        void invoke("read_completion_notification", { workspaceId, runId }).catch(() => {
          // A failed acknowledgement leaves the notification unread.
        });
      },
      () => { if (active) setLoadError(true); },
    );
    return () => { active = false; };
  }, [attempt, runId, workspaceId]);

  if (!video) {
    return (
      <>
        <WorkspaceHeading><h1>{copy.details}</h1></WorkspaceHeading>
        {loadError ? (
          <Alert variant="destructive">
            <AlertDescription>{copy.loadError}</AlertDescription>
            <AlertAction>
              <Button size="sm" variant="outline" onClick={() => {
                setLoadError(false);
                setAttempt((current) => current + 1);
              }}>{queue.retry}</Button>
            </AlertAction>
          </Alert>
        ) : (
          <div className="flex flex-col gap-4" role="status" aria-label={copy.loading}>
            <Skeleton className="h-8 w-56 max-w-full" />
            <Skeleton className="aspect-video w-full rounded-xl" />
          </div>
        )}
      </>
    );
  }
  return <CompletedVideoContent workspaceId={workspaceId} video={video} initialFormatId={initialFormatId} file={file}
    onAttach={() => setAttempt((current) => current + 1)}
    onDetach={(id) => setVideo((current) => current && ({
      ...current,
      supplemental_media: current.supplemental_media.filter((item) => item.attachment_id !== id),
    }))} />;
}

function CompletedVideoContent({ workspaceId, video, initialFormatId, file, onAttach, onDetach }: { workspaceId: number; video: CompletedVideo; initialFormatId: number; file?: CompletedFile; onAttach: () => void; onDetach: (id: number) => void }) {
  const { locale, messages: { completedVideo: copy, queue, studio, subtitleRevisions } } = useLocale();
  const router = useRouter();
  const { id: runId, snapshot } = video.run;
  const [savedTitle, setSavedTitle] = useState(video.title);
  const [title, setTitle] = useState(video.title);
  const [action, setAction] = useState<"rename" | "export" | "captioned">();
  const [error, setError] = useState<"renameError" | "exportError" | "captionedError">();
  const [notice, setNotice] = useState<"saved" | "exported" | "captionedSaved">();
  const [outputDates, setOutputDates] = useState(video.output_dates);
  const finished = [...video.activity].reverse().find((entry) => entry.state === "completed");
  const completedAt = finished ? new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(new Date(finished.created_at * 1000)) : null;
  const crumbOutput = video.delivery.verified_outputs.find((output) => output.format_id === initialFormatId) ?? video.delivery.verified_outputs[0];
  const fileCrumb = file === "subtitles" ? crumbOutput?.subtitles ?? copy.subtitleText : undefined;
  useStudioTrail([
    { label: studio.videos, href: workspaceHref("videos", workspaceId) },
    { label: savedTitle, href: fileCrumb ? completedVideoHref(workspaceId, runId, crumbOutput?.format_id) : undefined },
    ...(fileCrumb ? [{ label: fileCrumb }] : []),
  ]);
  const [formatId, setFormatId] = useState(video.delivery.verified_outputs.find((output) => output.format_id === initialFormatId)?.format_id
    ?? video.delivery.verified_outputs[0]?.format_id);
  const selected = video.delivery.verified_outputs.find((output) => output.format_id === formatId);
  const formats = snapshot.formats ?? [];
  const format = formats.find(({ format }) => format.id === formatId)?.format;
  const burnsCaptions = formats.find(({ format }) => format.id === formatId)?.delivery?.supplemental?.burned_captions === true;
  const outputName = (output: Output) => formats.find(({ format }) => format.id === output.format_id)?.format.name ?? output.video;

  const placed = usePlacedActions(workspaceId, "completed-video");
  const actionLabel = usePlacedActionLabel();
  const [placedOpen, setPlacedOpen] = useState<InstalledAction>();
  const trackContext = { subtitleTracks: () => completedSubtitleTracks(workspaceId, runId, locale, subtitleRevisions.primary) };

  async function rename(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (action || !title.trim() || title.trim() === savedTitle) return;
    setAction("rename");
    setError(undefined);
    setNotice(undefined);
    try {
      const saved = await invoke<string>("rename_completed_video", { workspaceId, runId, title });
      setSavedTitle(saved);
      setTitle(saved);
      setNotice("saved");
    } catch {
      setError("renameError");
    } finally {
      setAction(undefined);
    }
  }

  async function exportZip() {
    if (action) return;
    setAction("export");
    setError(undefined);
    setNotice(undefined);
    try {
      if (await invoke<boolean>("export_completed_video", { workspaceId, runId })) setNotice("exported");
    } catch {
      setError("exportError");
    } finally {
      setAction(undefined);
    }
  }

  async function saveCaptioned(id: number) {
    if (action) return;
    setAction("captioned");
    setError(undefined);
    setNotice(undefined);
    try {
      if (await invoke<boolean>("export_captioned_output", { workspaceId, runId, formatId: id })) setNotice("captionedSaved");
    } catch {
      setError("captionedError");
    } finally {
      setAction(undefined);
    }
  }

  const textOutputs = video.supplemental_text;
  const mediaOutputs = video.supplemental_media;
  const mediaIndex = file?.startsWith("media-") ? Number(file.slice(6)) - 1 : undefined;
  const attachmentId = file?.startsWith("attachment-") ? Number(file.slice(11)) : undefined;
  const mediaOutput = attachmentId !== undefined
    ? mediaOutputs.find((item) => item.attachment_id === attachmentId)
    : mediaIndex === undefined ? undefined : mediaOutputs.filter((item) => item.attachment_id === undefined)[mediaIndex];
  const mediaLinks = mediaOutputs.map((item, index) => {
    const Icon = item.kind === "image" ? Image : item.kind === "audio" ? Music : Film;
    const mediaFile: CompletedFile = item.attachment_id !== undefined
      ? `attachment-${item.attachment_id}` : `media-${index + 1}`;
    return <Button key={mediaFile} asChild size="sm" variant="outline" className="max-w-full">
      <Link href={completedVideoHref(workspaceId, runId, selected?.format_id, mediaFile)}>
        <Icon data-icon="inline-start" /><span className="truncate" title={item.name}>{item.name}</span>
      </Link>
    </Button>;
  });
  const textIndex = file?.startsWith("text-") ? Number(file.slice(5)) - 1 : undefined;
  const textOutput = textIndex === undefined ? undefined : textOutputs[textIndex];
  const textFilename = (item: TextOutput, index: number) => `supplemental-${item.kind}-${index + 1}.txt`;
  const textLinks = textOutputs.map((item, index) => (
    <Button key={index} asChild size="sm" variant="outline">
      <Link href={completedVideoHref(workspaceId, runId, selected?.format_id, `text-${index + 1}`)}>
        <FileText data-icon="inline-start" />{copy[`supplemental_${item.kind}`]} {index + 1}
      </Link>
    </Button>
  ));
  if (textIndex !== undefined || mediaIndex !== undefined || attachmentId !== undefined) {
    const outputFormat = formats.find(({ format }) => format.id === (mediaOutput?.format_id ?? textOutput?.format_id))?.format;
    const filename = mediaOutput?.name ?? (textOutput && textIndex !== undefined ? textFilename(textOutput, textIndex) : copy.otherFiles);
    return <>
      <div className="mb-5 flex flex-wrap items-start justify-between gap-3">
        <WorkspaceHeading className="mb-0">
          <h1>{filename}</h1>
          <p>{savedTitle}{outputFormat && ` / ${outputFormat.name}`}</p>
        </WorkspaceHeading>
        <div className="flex flex-wrap gap-2">
          <Button asChild size="sm" variant="outline">
            <Link href={completedVideoHref(workspaceId, runId, selected?.format_id)}>{copy.videoTab}</Link>
          </Button>
          {attachmentId !== undefined && mediaOutput && <DetachCompletedMedia key={attachmentId}
            workspaceId={workspaceId} runId={runId} attachmentId={attachmentId} name={mediaOutput.name}
            onDetached={() => {
              onDetach(attachmentId);
              router.replace(completedVideoHref(workspaceId, runId, selected?.format_id));
            }} />}
        </div>
      </div>
      {mediaOutput ? <SupplementalMediaPreview key={file}
        workspaceId={workspaceId} runId={runId} index={mediaIndex} media={mediaOutput} />
        : textOutput ? <article className="max-w-3xl whitespace-pre-wrap wrap-anywhere rounded-lg border bg-card p-5">
        {textOutput.text}
      </article> : <Alert variant="destructive"><AlertDescription>{copy.loadError}</AlertDescription></Alert>}
      <div className="mt-6 flex flex-wrap gap-2">{textLinks}{mediaLinks}</div>
    </>;
  }

  if (file === "subtitles" && selected) {
    return <>
      <div className="mb-5 flex flex-wrap items-start justify-between gap-3">
        <WorkspaceHeading className="mb-0">
          <h1>{selected.subtitles ?? copy.subtitleText}</h1>
          <p>{copy.subtitlesIntro.replace("{video}", savedTitle).replace("{output}", outputName(selected))}</p>
        </WorkspaceHeading>
        <Button asChild variant="outline" size="sm">
          <Link href={completedVideoHref(workspaceId, runId, selected.format_id)}>{copy.videoTab}</Link>
        </Button>
      </div>
      <CompletedOutputPreview key={selected.format_id} workspaceId={workspaceId} runId={runId}
        formatId={selected.format_id} label={outputName(selected)} subtitleName={selected.subtitles ?? `${selected.format_id}.srt`} showTranscript />
      <section className="mt-6 border-t pt-4">
        <h2 className="mb-2 text-[1.3125rem]">{copy.otherFiles}</h2>
        <div className="flex flex-wrap gap-2">
          {video.delivery.verified_outputs.map((output) => (
            <Button key={output.format_id} asChild variant="outline" size="sm">
              <Link href={completedVideoHref(workspaceId, runId, output.format_id)}>
                <Film data-icon="inline-start" />{output.video}
              </Link>
            </Button>
          ))}
          {video.delivery.verified_outputs.filter((output) => output.format_id !== selected.format_id && output.subtitles).map((output) => (
            <Button key={output.format_id} asChild variant="outline" size="sm">
              <Link href={completedVideoHref(workspaceId, runId, output.format_id, "subtitles")}>
                <Captions data-icon="inline-start" />{output.subtitles}
              </Link>
            </Button>
          ))}
          {textLinks}{mediaLinks}
          <Button variant="outline" size="sm" disabled={Boolean(action)} onClick={() => void exportZip()}>
            <Archive data-icon="inline-start" />{action === "export" ? copy.exporting : copy.exportZip}
          </Button>
        </div>
        {error === "exportError" && <Alert className="mt-3" variant="destructive"><AlertDescription>{copy[error]}</AlertDescription></Alert>}
        {notice === "exported" && <p className="mt-3 text-sm text-muted-foreground" role="status">{copy[notice]}</p>}
      </section>
    </>;
  }

  return (
    <>
      <div className="mb-4 flex flex-wrap items-center justify-between gap-3 border-b pb-4">
        <div className="flex min-w-0 flex-1 flex-col gap-2">
          <p className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm font-semibold">
            {snapshot.profile && <span className="flex items-center gap-2">
              <span aria-hidden="true" className="size-2.5 rounded-[2px]" style={{ backgroundColor: snapshot.profile.color ?? "var(--border)" }} />
              {snapshot.profile.name}
            </span>}
            <span className="flex items-center gap-1.5 text-approval">
              <CheckCircle2 className="size-4" aria-hidden="true" />
              {completedAt ? copy.completedAt.replace("{date}", completedAt) : queue.completed}
            </span>
          </p>
          <WorkspaceHeading className="mb-0 border-0 pb-0">
            <h1>{savedTitle}</h1>
          </WorkspaceHeading>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button disabled={Boolean(action)} onClick={() => void exportZip()}>
            <Archive data-icon="inline-start" />{action === "export" ? copy.exporting : copy.exportZip}
          </Button>
          <ConfirmTitleDelete title={savedTitle} disabled={Boolean(action)} copy={copy}
            onConfirm={(confirmedTitle) => invoke<boolean>("delete_completed_video", {
              workspaceId, runId, confirmedTitle,
            })}
            onDelete={() => router.replace(workspaceHref("videos", workspaceId))} />
        </div>
      </div>
      {error === "exportError" && <Alert variant="destructive" className="mb-4"><AlertDescription>{copy[error]}</AlertDescription></Alert>}
      {notice === "exported" && <p role="status" className="mb-4 text-sm text-muted-foreground">{copy[notice]}</p>}
      <div className="@container">
          {selected ? (
            <div className="grid items-start gap-5 @3xl:grid-cols-[minmax(0,1fr)_13rem]">
              <ItemContextMenu label={savedTitle} items={[
                { label: copy.subtitleText, icon: Captions, href: completedVideoHref(workspaceId, runId, selected.format_id, "subtitles") },
                { label: copy.exportZip, icon: Archive, disabled: Boolean(action), onSelect: () => void exportZip() },
                ...placed.map((entry, index) => ({ label: actionLabel(entry), icon: Puzzle, group: index === 0, onSelect: () => setPlacedOpen(entry) })),
              ]}>
                <div className="min-w-0">
                  <CompletedOutputPreview key={selected.format_id} workspaceId={workspaceId} runId={runId}
                    formatId={selected.format_id} label={outputName(selected)} />
                </div>
              </ItemContextMenu>
              <Card>
                <CardHeader><CardTitle>{copy.outputFormat}</CardTitle></CardHeader>
                <CardContent className="flex flex-col gap-3">
                  {video.delivery.verified_outputs.length > 1 && (
                    <Select value={String(selected.format_id)} onValueChange={(value) => setFormatId(Number(value))}>
                      <SelectTrigger aria-label={copy.outputFormat} className="w-full"><SelectValue /></SelectTrigger>
                      <SelectContent><SelectGroup>
                        {video.delivery.verified_outputs.map((output) => (
                          <SelectItem key={output.format_id} value={String(output.format_id)}>{outputName(output)}</SelectItem>
                        ))}
                      </SelectGroup></SelectContent>
                    </Select>
                  )}
                  {video.delivery.verified_outputs.length === 1 && <p className="font-medium">{outputName(selected)}</p>}
                  <Separator />
                  <dl className="grid gap-2 text-sm">
                    {format && <Metadata name={copy.resolution} value={`${format.width} × ${format.height}`} />}
                    <Metadata name={copy.duration} value={new Intl.NumberFormat(locale, {
                      style: "unit", unit: "second", maximumFractionDigits: 1,
                    }).format(selected.duration_seconds)} />
                    <Metadata name={copy.videoFile} value={selected.video} />
                    <Metadata name={copy.subtitleFile} value={selected.subtitles ?? copy.noSubtitleFile} />
                  </dl>
                  <Button asChild size="sm" variant="outline">
                    <Link href={completedVideoHref(workspaceId, runId, selected.format_id, "subtitles")}>
                      <Captions data-icon="inline-start" />{selected.subtitles ? copy.subtitleText : copy.addSubtitles}
                    </Link>
                  </Button>
                  {burnsCaptions && <>
                    <Button size="sm" variant="outline" disabled={Boolean(action)} onClick={() => void saveCaptioned(selected.format_id)}>
                      <Download data-icon="inline-start" />{action === "captioned" ? copy.savingCaptioned : copy.saveCaptioned}
                    </Button>
                    {action === "captioned" && <p role="status" className="text-sm text-muted-foreground">{copy.savingCaptionedHint}</p>}
                    {error === "captionedError" && <Alert variant="destructive"><AlertDescription>{copy[error]}</AlertDescription></Alert>}
                    {notice === "captionedSaved" && <p role="status" className="text-sm text-muted-foreground">{copy[notice]}</p>}
                  </>}
                  <Separator />
                  <OutputDateField key={selected.format_id} workspaceId={workspaceId} runId={runId}
                    formatId={selected.format_id} savedDate={outputDates[selected.format_id] ?? ""}
                    onSaved={(date) => setOutputDates((current) => ({ ...current, [selected.format_id]: date }))} />
                </CardContent>
              </Card>
            </div>
          ) : (
            <Empty><EmptyHeader><EmptyTitle>{copy.noOutputs}</EmptyTitle><EmptyDescription>{copy.previewError}</EmptyDescription></EmptyHeader></Empty>
          )}
          {(selected || textLinks.length > 0 || mediaLinks.length > 0) && <section className="mt-6 border-t pt-4">
            <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
              <h2 className="text-[1.3125rem]">{copy.otherFiles}</h2>
              {selected && <div className="flex flex-wrap items-start gap-2">
                <AttachCompletedMedia key={selected.format_id} workspaceId={workspaceId} runId={runId}
                  formatId={selected.format_id} formatName={outputName(selected)} onAttached={onAttach}
                  attached={mediaOutputs.filter((item) => item.format_id === selected.format_id).map((item) => item.asset_id)} />
                <ImportVideoVariant key={`import-${selected.format_id}`} workspaceId={workspaceId} runId={runId}
                  formatId={selected.format_id} onImported={onAttach} />
              </div>}
            </div>
            <div className="flex flex-wrap gap-2">{textLinks}{mediaLinks}</div>
          </section>}
          <PlacedPluginActions className="mt-6" workspaceId={workspaceId} placement="completed-video" context={trackContext} />
          <ActionSheet workspaceId={workspaceId} entry={placedOpen} context={trackContext} onClose={() => setPlacedOpen(undefined)} />
      </div>
      <div className="mt-10 grid items-start gap-10 lg:grid-cols-[minmax(0,1fr)_20rem]">
        <section aria-labelledby="completed-story" className="flex min-w-0 flex-col gap-5">
          <div className="flex flex-wrap items-baseline justify-between gap-2">
            <h2 id="completed-story" className="text-[1.5rem]">{copy.story}</h2>
            <p className="text-sm text-muted-foreground">{copy.storyHint}</p>
          </div>
          <CompletedVideoFeedback workspaceId={workspaceId} runId={runId} initialFeedback={video.feedback}
            results={video.production.results} stages={video.plan.steps} />
          <RunMotionResults workspaceId={workspaceId} runId={runId} results={video.production.results}
            steps={video.plan.steps} stageIndex={video.production.stage_index} />
          <Collapsible>
            <CollapsibleTrigger asChild>
              <Button type="button" variant="ghost" size="sm" className="group self-start">
                <ClipboardList data-icon="inline-start" />{queue.activity}<ChevronDown data-icon="inline-end" className="group-data-[state=open]:rotate-180" />
              </Button>
            </CollapsibleTrigger>
            <CollapsibleContent className="mt-2"><CompletedVideoActivity events={video.activity} /></CollapsibleContent>
          </Collapsible>
        </section>
        <aside aria-labelledby="completed-settings" className="flex min-w-0 flex-col gap-5">
          <h2 id="completed-settings" className="text-[1.5rem]">{copy.settingsUsed}</h2>
          <Card>
            <CardHeader><CardTitle>{copy.replaySettings}</CardTitle></CardHeader>
            <CardContent><CompletedVideoReplay workspaceId={workspaceId} runId={runId} footageRemoved={video.footage_removed} /></CardContent>
          </Card>

          <Card>
            <CardHeader><CardTitle>{copy.detailsTab}</CardTitle></CardHeader>
            <CardContent className="flex flex-col gap-4">
              <dl className="grid gap-2">
                <Metadata name={copy.sourceTitle} value={snapshot.draft.title} />
                {snapshot.profile && <Metadata name={copy.sourceProfile} value={snapshot.profile.name} />}
                {(snapshot.draft.source_ids?.length ?? 0) > 0 && <Metadata name={copy.sourceClips} value={String(snapshot.draft.source_ids?.length)} />}
              </dl>
              <Separator />
              <form onSubmit={rename}>
                <FieldGroup>
                  <Field>
                    <FieldLabel htmlFor={`completed-title-${runId}`}>{copy.title}</FieldLabel>
                    <div className="flex flex-wrap items-center gap-2">
                      <Input className="min-w-32 flex-1" id={`completed-title-${runId}`} required
                        value={title} onChange={(event) => setTitle(event.target.value)} />
                      <Button type="submit" variant="outline"
                        disabled={Boolean(action) || !title.trim() || title.trim() === savedTitle}>{copy.saveTitle}</Button>
                    </div>
                  </Field>
                  {error === "renameError" && <Alert variant="destructive"><AlertDescription>{copy[error]}</AlertDescription></Alert>}
                  {notice === "saved" && <p role="status" className="text-sm text-muted-foreground">{copy[notice]}</p>}
                </FieldGroup>
              </form>
            </CardContent>
          </Card>
        </aside>
      </div>
      <PluginSurfaces placement="completed_video" workspaceId={workspaceId} />
    </>
  );
}

function Metadata({ name, value }: { name: string; value: string }) {
  return (
    <div className="flex min-w-0 flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
      <dt className="text-muted-foreground">{name}</dt>
      <dd className="min-w-0 font-medium wrap-anywhere tabular-nums">{value}</dd>
    </div>
  );
}

function CompletedVideoActivity({ events }: { events: Activity[] }) {
  const { locale, messages: { completedVideo: copy, queue } } = useLocale();
  const dates = new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" });
  if (!events.length) {
    return <Empty><EmptyHeader><EmptyTitle>{copy.noActivity}</EmptyTitle></EmptyHeader></Empty>;
  }
  return (
    <ol className="divide-y rounded-xl border bg-card px-4">
      {events.map((event, index) => {
        const date = new Date(event.created_at * 1000);
        return (
          <li key={index} className="flex flex-wrap items-start justify-between gap-2 py-3">
            <div className="min-w-0">
              <p className="font-medium">{queue[event.kind]} <span className="text-muted-foreground">· {queue[event.state]}</span></p>
              {event.detail && <p className="mt-1 text-muted-foreground wrap-anywhere whitespace-pre-wrap">{event.detail}</p>}
            </div>
            <time className="text-xs text-muted-foreground tabular-nums" dateTime={date.toISOString()}>{dates.format(date)}</time>
          </li>
        );
      })}
    </ol>
  );
}

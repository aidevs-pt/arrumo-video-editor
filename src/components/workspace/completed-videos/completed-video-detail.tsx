"use client";

import { PluginSurfaces } from "@/components/workspace/plugins/plugin-surfaces";
import { invoke } from "@tauri-apps/api/core";
import { Archive, CheckCircle2, ClipboardList, Film, FileText, MessageSquare } from "lucide-react";
import { useRouter } from "next/navigation";
import { useEffect, useState, type FormEvent } from "react";
import { Alert, AlertAction, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { WorkspaceHeading } from "@/components/workspace/layout";
import { useLocale } from "@/lib/locale-provider";
import { ConfirmTitleDelete } from "../confirm-title-delete";
import { workspaceHref } from "../home/workspace-links";
import type { Stage } from "../generation/generation-plan";
import { OutputDateField } from "./output-date-field";
import { CompletedOutputPreview } from "./completed-output-preview";
import { CompletedVideoFeedback, type FeedbackResult, type VideoFeedback } from "./completed-video-feedback";
import { CompletedVideoReplay } from "./completed-video-replay";

type Output = {
  format_id: number;
  video: string;
  subtitles: string;
  duration_seconds: number;
  subtitle_cues: number;
};
type Format = { id: number; name: string; width: number; height: number };
type Activity = {
  state: "queued" | "running" | "paused" | "failed" | "completed" | "cancelled";
  kind: "submitted" | "claimed" | "transition";
  attempt: number;
  created_at: number;
  detail: string | null;
};
type CompletedVideo = {
  run: {
    id: number;
    snapshot: {
      draft: { title: string; source_ids?: number[] };
      profile?: { name: string } | null;
      formats?: { format: Format }[];
    };
  };
  plan: { steps: { stage: Stage }[] };
  production: { results: FeedbackResult[] };
  activity: Activity[];
  feedback: VideoFeedback[];
  title: string;
  delivery: { verified_outputs: Output[] };
  output_dates: Record<number, string>;
};

export function CompletedVideoDetail({ workspaceId, runId }: { workspaceId: number; runId: number }) {
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
        <WorkspaceHeading icon={Film}><h1>{copy.details}</h1></WorkspaceHeading>
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
  return <CompletedVideoContent workspaceId={workspaceId} video={video} />;
}

function CompletedVideoContent({ workspaceId, video }: { workspaceId: number; video: CompletedVideo }) {
  const { locale, messages: { completedVideo: copy, queue } } = useLocale();
  const router = useRouter();
  const { id: runId, snapshot } = video.run;
  const [savedTitle, setSavedTitle] = useState(video.title);
  const [title, setTitle] = useState(video.title);
  const [action, setAction] = useState<"rename" | "export">();
  const [error, setError] = useState<"renameError" | "exportError">();
  const [notice, setNotice] = useState<"saved" | "exported">();
  const [outputDates, setOutputDates] = useState(video.output_dates);
  const [formatId, setFormatId] = useState(video.delivery.verified_outputs[0]?.format_id);
  const selected = video.delivery.verified_outputs.find((output) => output.format_id === formatId);
  const formats = snapshot.formats ?? [];
  const format = formats.find(({ format }) => format.id === formatId)?.format;
  const outputName = (output: Output) => formats.find(({ format }) => format.id === output.format_id)?.format.name ?? output.video;

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

  return (
    <>
      <div className="mb-4 flex flex-wrap items-start justify-between gap-3 border-b pb-4">
        <WorkspaceHeading icon={Film} className="mb-0 min-w-0 flex-1 border-0 pb-0">
          <h1>{savedTitle}</h1>
          <p>{copy.files}</p>
        </WorkspaceHeading>
        <div className="flex flex-wrap items-center gap-2">
          <Badge variant="secondary"><CheckCircle2 aria-hidden="true" />{queue.completed}</Badge>
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
      <Tabs defaultValue="video" className="gap-5">
        <TabsList variant="line" aria-label={copy.details} className="h-auto max-w-full flex-wrap justify-start gap-2">
          <TabsTrigger value="video"><Film data-icon="inline-start" />{copy.videoTab}</TabsTrigger>
          <TabsTrigger value="details"><FileText data-icon="inline-start" />{copy.detailsTab}</TabsTrigger>
          <TabsTrigger value="activity"><ClipboardList data-icon="inline-start" />{queue.activity}</TabsTrigger>
          <TabsTrigger value="feedback"><MessageSquare data-icon="inline-start" />{copy.feedback}</TabsTrigger>
        </TabsList>
        <TabsContent value="video" className="@container">
          {selected ? (
            <div className="grid items-start gap-5 @3xl:grid-cols-[minmax(0,1fr)_13rem]">
              <CompletedOutputPreview key={selected.format_id} workspaceId={workspaceId} runId={runId}
                formatId={selected.format_id} label={outputName(selected)} />
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
                    <Metadata name={copy.subtitleFile} value={selected.subtitles} />
                  </dl>
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
        </TabsContent>
        <TabsContent value="details" forceMount className="flex flex-col gap-5 data-[state=inactive]:hidden">
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
          <Card>
            <CardHeader><CardTitle>{copy.replaySettings}</CardTitle></CardHeader>
            <CardContent><CompletedVideoReplay workspaceId={workspaceId} runId={runId} /></CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="activity">
          <CompletedVideoActivity events={video.activity} />
        </TabsContent>
        <TabsContent value="feedback" forceMount className="data-[state=inactive]:hidden">
          <CompletedVideoFeedback workspaceId={workspaceId} runId={runId} initialFeedback={video.feedback}
            results={video.production.results} stages={video.plan.steps} />
        </TabsContent>
      </Tabs>
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

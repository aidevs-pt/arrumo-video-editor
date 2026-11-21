"use client";

import { useLocale } from "@/lib/locale-provider";
import { invoke } from "@tauri-apps/api/core";
import { ArrowUpRight, Check, Circle, Clock3, Pause, Play, RotateCcw, Square } from "lucide-react";
import Link from "next/link";
import { useEffect, useState, type ReactNode } from "react";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription,
  AlertDialogFooter, AlertDialogHeader, AlertDialogTitle, AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { completedVideoHref } from "@/components/workspace/home/workspace-router";
import { StepTrack } from "@/components/workspace/step-track";
import { VideoThumbnail } from "@/components/workspace/video-thumbnail";
import { AgentLogs } from "./agent-logs";
import { GenerationPlan } from "./generation-plan";
import type { DeliveryFormat } from "./delivery-subtitles";

type RunState = "queued" | "running" | "paused" | "failed" | "completed" | "cancelled";

export type Run = {
  id: number;
  draft_id: number;
  state: RunState;
  pending_review_id: number | null;
  display_title: string | null;
  progress?: { stage_index: number; stage_count: number; stage: string | null };
  snapshot: { draft: { id: number; title: string; source_ids?: number[] }; profile?: { id: number; name: string; color: string | null } | null; formats?: { format: DeliveryFormat }[]; options?: { save_motion_to_library?: boolean; autopilot?: boolean }; agent_settings?: { effective_provider?: string | null } };
};

export type RunAction = "pause" | "resume" | "retry" | "cancel" | "stop";
type Activity = { state: RunState; kind: "submitted" | "claimed" | "transition"; created_at: number; detail: string | null };

export function runNeedsReview(run: Run) {
  return run.state === "paused" && run.pending_review_id !== null;
}

export function runTitle(run: Run) {
  return run.display_title ?? run.snapshot.draft.title;
}

export function runHref(workspaceId: number, run: Run) {
  return run.state === "completed" ? completedVideoHref(workspaceId, run.id)
    : `/generation-run?workspace=${workspaceId}&run=${run.id}`;
}

type Messages = ReturnType<typeof useLocale>["messages"];
const stageKeys = ["script", "assets", "preview", "supplemental_outputs", "final_render", "verify_delivery"] as const;

/** One sentence that says where a run stands and whether it needs the user. */
export function runSummary(run: Run, messages: Messages) {
  const copy = messages.dashboard;
  const key = stageKeys.find((stage) => stage === run.progress?.stage);
  const stage = key ? messages.productionPlan[key].toLowerCase() : null;
  if (runNeedsReview(run)) return stage ? copy.summaryReview.replace("{stage}", stage) : messages.productionPlan.awaitingReview;
  if (run.state === "paused") return copy.summaryPaused;
  if (run.state === "running" && stage) return copy.summaryRunning.replace("{stage}", stage);
  if (run.state === "queued") return copy.summaryQueued;
  return messages.queue[run.state];
}

export function RunStatus({ run }: { run: Run }) {
  const { messages } = useLocale();
  const review = runNeedsReview(run);
  const variant = run.state === "failed" ? "destructive"
    : review ? "default"
    : run.state === "running" || run.state === "completed" ? "secondary" : "outline";
  return (
    <Badge variant={variant} className={review
      ? "h-auto min-h-5.5 shrink-0 whitespace-normal bg-attention text-[oklch(23%_.05_268)]"
      : "h-auto min-h-5.5 shrink-0 whitespace-normal"}>
      {run.state === "completed" ? <Check aria-hidden="true" /> : <Circle aria-hidden="true" />}
      {review ? messages.productionPlan.awaitingReview : messages.queue[run.state]}
    </Badge>
  );
}

export function GenerationRunRow({ run, workspaceId, actions }: { run: Run; workspaceId: number; actions?: ReactNode }) {
  const { messages } = useLocale();
  const completed = run.state === "completed";
  const title = runTitle(run);
  const href = completed ? completedVideoHref(workspaceId, run.id)
    : `/generation-run?workspace=${workspaceId}&run=${run.id}`;

  return (
    <li className="grid grid-cols-[auto_minmax(0,1fr)] items-center gap-x-4 gap-y-3 py-4 sm:grid-cols-[auto_minmax(0,1fr)_auto]">
      <VideoThumbnail title={title} color={run.snapshot.profile?.color} small className="row-span-2 w-24 sm:row-span-1 sm:w-36" />
      <div className="flex min-w-0 flex-col gap-1.5">
        {run.snapshot.profile && <span className="flex items-center gap-2 text-sm font-semibold">
          <span aria-hidden="true" className="size-2.5 rounded-[2px]" style={{ backgroundColor: run.snapshot.profile.color ?? "var(--border)" }} />
          {run.snapshot.profile.name}
        </span>}
        <Link href={href} className="min-w-0 font-heading text-lg leading-tight font-[800] wrap-anywhere [font-variation-settings:'wdth'_108] hover:text-primary focus-visible:ring-2 focus-visible:ring-ring">
          {title}
        </Link>
        {!completed && run.progress && <StepTrack count={run.progress.stage_count} current={run.progress.stage_index}
          label={messages.dashboard.progressLabel} className="max-w-sm" />}
        {!completed && <span className="text-sm text-muted-foreground">{runSummary(run, messages)}</span>}
      </div>
      <div className="col-start-2 flex flex-wrap items-center gap-2 sm:col-start-3">
        <RunStatus run={run} />
        <Button asChild size="sm" variant={runNeedsReview(run) ? "default" : "outline"}>
          <Link href={href}>
            {completed ? messages.completedVideo.details : runNeedsReview(run) ? messages.queue.reviewRun : messages.queue.openRun}
            <ArrowUpRight data-icon="inline-end" />
          </Link>
        </Button>
        {actions}
      </div>
    </li>
  );
}

export function GenerationRunControls({ run, workspaceId, disabled, onControl }: {
  run: Run;
  workspaceId: number;
  disabled: boolean;
  onControl: (runId: number, action: RunAction) => Promise<void>;
}) {
  const { messages } = useLocale();
  const copy = messages.queue;
  const canPause = run.state === "queued" || run.state === "running";
  const canResume = run.state === "paused" && run.pending_review_id === null;
  const canStop = ["queued", "running", "paused", "failed"].includes(run.state) && (run.progress?.stage_index ?? 0) > 0;

  return (
    <div className="flex flex-wrap items-center gap-2 [&_a[data-variant=outline]]:text-foreground [&_button[data-variant=outline]]:text-foreground">
      <RunStatus run={run} />
      {canPause && (
        <Button type="button" size="sm" variant="outline" disabled={disabled}
          onClick={() => void onControl(run.id, "pause")}><Pause data-icon="inline-start" />{copy.pause}</Button>
      )}
      {canResume && (
        <Button type="button" size="sm" variant="outline" disabled={disabled}
          onClick={() => void onControl(run.id, "resume")}><Play data-icon="inline-start" />{copy.resume}</Button>
      )}
      {run.state === "failed" && (
        <Button type="button" size="sm" variant="outline" disabled={disabled}
          onClick={() => void onControl(run.id, "retry")}><RotateCcw data-icon="inline-start" />{copy.retryRun}</Button>
      )}
      {canStop && (
        <AlertDialog>
          <AlertDialogTrigger asChild>
            <Button type="button" size="sm" variant="outline" disabled={disabled}><Square data-icon="inline-start" />{copy.stop}</Button>
          </AlertDialogTrigger>
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>{copy.stopTitle}</AlertDialogTitle>
              <AlertDialogDescription>{copy.stopDescription}</AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>{copy.keepGoing}</AlertDialogCancel>
              <AlertDialogAction onClick={() => void onControl(run.id, "stop")}>{copy.stop}</AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      )}
      {(canPause || run.state === "paused") && (
        <AlertDialog>
          <AlertDialogTrigger asChild>
            <Button type="button" size="sm" variant="ghost" disabled={disabled}>{copy.cancel}</Button>
          </AlertDialogTrigger>
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>{copy.confirmCancel}</AlertDialogTitle>
              <AlertDialogDescription>{runTitle(run)}</AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>{copy.keepRun}</AlertDialogCancel>
              <AlertDialogAction variant="destructive" onClick={() => void onControl(run.id, "cancel")}>{copy.cancel}</AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      )}
    </div>
  );
}

export function GenerationRunPanels({ run, workspaceId }: { run: Run; workspaceId: number }) {
  return (
    <GenerationPlan workspaceId={workspaceId} runId={run.id}
      pendingReviewId={runNeedsReview(run) ? run.pending_review_id : null}
      agentId={run.snapshot.agent_settings?.effective_provider}
      runState={run.state} hasSources={Boolean(run.snapshot.draft.source_ids?.length)}
      saveMotionDefault={run.snapshot.options?.save_motion_to_library ?? true}
      formats={run.snapshot.formats?.map((entry) => entry.format) ?? []}
      activity={<GenerationActivity run={run} workspaceId={workspaceId} />} />
  );
}

function sameDay(date: Date) {
  return date.toDateString() === new Date().toDateString();
}

function GenerationActivity({ run, workspaceId }: { run: Run; workspaceId: number }) {
  const [activity, setActivity] = useState<Activity[]>();
  const [historyError, setHistoryError] = useState(false);
  const { locale, messages: { queue: copy } } = useLocale();

  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      try {
        const items = await invoke<Activity[]>("generation_activity", { workspaceId, runId: run.id });
        if (!active) return;
        setActivity(items);
        setHistoryError(false);
      } catch {
        if (active) setHistoryError(true);
      }
      if (active && run.state !== "completed" && run.state !== "cancelled") timer = setTimeout(refresh, 2000);
    }
    void refresh();
    return () => { active = false; clearTimeout(timer); };
  }, [run.id, run.state, workspaceId]);

  function activityLabel(event: Activity, awaitingReview: boolean) {
    if (event.kind === "submitted") return copy.activitySubmitted;
    if (event.kind === "claimed") return copy.activityStarted;
    switch (event.state) {
      case "queued": return copy.activityQueued;
      case "running": return copy.activityRunning;
      case "paused": return awaitingReview ? copy.activityReview : copy.activityPaused;
      case "failed": return copy.activityFailed;
      case "completed": return copy.activityCompleted;
      case "cancelled": return copy.activityCancelled;
    }
  }

  return (
    <aside aria-label={copy.activity} className="flex min-w-0 flex-col gap-3 border-t border-white/20 pt-3 text-white">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="flex items-center gap-2 text-base"><Clock3 className="size-4" aria-hidden="true" />{copy.activity}</h2>
        <AgentLogs workspaceId={workspaceId} runId={run.id} className="border-white/40 bg-transparent text-white hover:bg-white/10 hover:text-white" />
      </div>
      <div>
        {historyError && <Alert variant="destructive"><AlertDescription>{copy.activityError}</AlertDescription></Alert>}
        {!activity && !historyError && <Skeleton className="h-14" aria-label={copy.loading} />}
        {activity?.length ? (
          <ol className="flex flex-col gap-0">
            {[...activity].reverse().map((event, index, newestFirst) => {
              const date = new Date(event.created_at * 1000);
              return (
                <li className="relative ms-1 border-s pb-4 ps-4 last:border-transparent last:pb-0" key={index}>
                  <span className="absolute inset-s-0 top-1.5 size-2 -translate-x-1/2 rounded-sm bg-white/60 ring-4 ring-agent rtl:translate-x-1/2" aria-hidden="true" />
                  <p className="font-medium">{activityLabel(event,
                    (index === 0 && run.pending_review_id !== null) || newestFirst[index + 1]?.kind === "claimed")}</p>
                  <p className="text-xs text-white/75"><time dateTime={date.toISOString()}>
                    {new Intl.DateTimeFormat(locale, sameDay(date) ? { timeStyle: "short" } : { dateStyle: "medium", timeStyle: "short" }).format(date)}
                  </time></p>
                  {event.detail && <p className="mt-1 wrap-anywhere whitespace-pre-wrap text-sm text-white/85">{event.detail}</p>}
                </li>
              );
            })}
          </ol>
        ) : activity && <p className="text-sm text-white/75">{copy.emptyActivity}</p>}
      </div>
    </aside>
  );
}

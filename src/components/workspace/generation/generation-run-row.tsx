"use client";

import { useLocale } from "@/lib/locale-provider";
import { invoke } from "@tauri-apps/api/core";
import { ArrowUpRight, Ban, Check, Circle, Clock3, Ellipsis, Pause, Play, RotateCcw, Square, Trash2 } from "lucide-react";
import Link from "next/link";
import { useEffect, useState, type ReactNode } from "react";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription,
  AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { ItemContextMenu, type ItemMenuEntry } from "../item-context-menu";
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
  snapshot: { draft: { id: number; title: string; source_ids?: number[] }; profile?: { id: number; name: string; color: string | null } | null; formats?: { format: DeliveryFormat; portrait_source?: { profile_id?: number }; delivery?: { supplemental?: { portrait_derivatives?: { count?: number } } } }[]; options?: { save_motion_to_library?: boolean; autopilot?: boolean }; agent_settings?: { effective_provider?: string | null } };
};

export type RunAction = "pause" | "resume" | "retry" | "cancel" | "stop";
type Activity = { state: RunState; kind: "submitted" | "claimed" | "resumed" | "transition"; created_at: number; detail: string | null; stage: string | null };

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

export function GenerationRunRow({ run, workspaceId, actions, menu = [] }: { run: Run; workspaceId: number; actions?: ReactNode; menu?: ItemMenuEntry[] }) {
  const { messages } = useLocale();
  const completed = run.state === "completed";
  const title = runTitle(run);
  const href = completed ? completedVideoHref(workspaceId, run.id)
    : `/generation-run?workspace=${workspaceId}&run=${run.id}`;

  return (
    <ItemContextMenu label={title} items={[
      { label: completed ? messages.completedVideo.details : runNeedsReview(run) ? messages.queue.reviewRun : messages.queue.openRun, icon: ArrowUpRight, href },
      ...menu,
    ]}>
    <li className="grid grid-cols-[auto_minmax(0,1fr)] items-center gap-x-4 gap-y-3 py-4 sm:grid-cols-[auto_minmax(0,1fr)_auto]">
      <VideoThumbnail title={title} color={run.snapshot.profile?.color} small className="row-span-2 w-24 sm:row-span-1 sm:w-36" />
      <div className="flex min-w-0 flex-col gap-1.5">
        {run.snapshot.profile && <span className="flex items-center gap-2 text-sm font-semibold">
          <span aria-hidden="true" className="size-2.5 rounded-[2px]" style={{ backgroundColor: run.snapshot.profile.color ?? "var(--border)" }} />
          {run.snapshot.profile.name}
        </span>}
        <Link href={href} className="min-w-0 font-semibold leading-tight wrap-anywhere hover:text-primary focus-visible:ring-2 focus-visible:ring-ring">
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
    </ItemContextMenu>
  );
}

export function GenerationRunControls({ run, disabled, onControl, deleteMenu }: {
  run: Run;
  disabled: boolean;
  onControl: (runId: number, action: RunAction) => Promise<void>;
  /** Renders the delete confirmation, opened from the menu. */
  deleteMenu?: (open: boolean, setOpen: (open: boolean) => void) => ReactNode;
}) {
  const { messages } = useLocale();
  const copy = messages.queue;
  const [dialog, setDialog] = useState<"stop" | "cancel" | "delete">();
  const canPause = run.state === "queued" || run.state === "running";
  const canResume = run.state === "paused" && run.pending_review_id === null;
  const canStop = ["queued", "running", "paused", "failed"].includes(run.state) && (run.progress?.stage_index ?? 0) > 0;
  const canCancel = canPause || run.state === "paused";
  const canDelete = run.state !== "completed" && deleteMenu !== undefined;

  return (
    <div className="flex flex-wrap items-center gap-2 [&_button[data-variant=outline]]:text-foreground">
      {canPause && (
        <Button type="button" variant="outline" disabled={disabled}
          onClick={() => void onControl(run.id, "pause")}><Pause data-icon="inline-start" />{copy.pause}</Button>
      )}
      {canResume && (
        <Button type="button" variant="outline" disabled={disabled}
          onClick={() => void onControl(run.id, "resume")}><Play data-icon="inline-start" />{copy.resume}</Button>
      )}
      {run.state === "failed" && (
        <Button type="button" variant="outline" disabled={disabled}
          onClick={() => void onControl(run.id, "retry")}><RotateCcw data-icon="inline-start" />{copy.retryRun}</Button>
      )}
      {(canStop || canCancel || canDelete) && <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button type="button" variant="outline" disabled={disabled}><Ellipsis data-icon="inline-start" />{copy.moreActions}</Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          {canStop && <DropdownMenuItem onSelect={() => setDialog("stop")}><Square />{copy.stop}</DropdownMenuItem>}
          {canCancel && <DropdownMenuItem onSelect={() => setDialog("cancel")}><Ban />{copy.cancel}</DropdownMenuItem>}
          {canDelete && <>
            {(canStop || canCancel) && <DropdownMenuSeparator />}
            <DropdownMenuItem variant="destructive" onSelect={() => setDialog("delete")}><Trash2 />{copy.deleteTitle}</DropdownMenuItem>
          </>}
        </DropdownMenuContent>
      </DropdownMenu>}
      <AlertDialog open={dialog === "stop"} onOpenChange={(open) => setDialog(open ? "stop" : undefined)}>
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
      <AlertDialog open={dialog === "cancel"} onOpenChange={(open) => setDialog(open ? "cancel" : undefined)}>
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
      {canDelete && deleteMenu(dialog === "delete", (open) => setDialog(open ? "delete" : undefined))}
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

/** Newest events shown before the creator asks for the whole history. */
const ACTIVITY_SHOWN = 6;

function GenerationActivity({ run, workspaceId }: { run: Run; workspaceId: number }) {
  const [activity, setActivity] = useState<Activity[]>();
  const [historyError, setHistoryError] = useState(false);
  const [showAll, setShowAll] = useState(false);
  const { locale, messages: { queue: copy, productionPlan } } = useLocale();

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
    if (event.kind === "resumed") return copy.activityResumed;
    if (event.kind === "claimed") {
      return event.stage && event.stage in productionPlan
        ? copy.activityStartedStage.replace("{stage}", productionPlan[event.stage as keyof typeof productionPlan] as string)
        : copy.activityStarted;
    }
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
        <h2 className="font-sans flex items-center gap-2 text-base"><Clock3 className="size-4" aria-hidden="true" />{copy.activity}</h2>
        <AgentLogs workspaceId={workspaceId} runId={run.id} className="border-white/40 bg-transparent text-white hover:bg-white/10 hover:text-white" />
      </div>
      <div>
        {historyError && <Alert variant="destructive"><AlertDescription>{copy.activityError}</AlertDescription></Alert>}
        {!activity && !historyError && <Skeleton className="h-14" aria-label={copy.loading} />}
        {activity?.length ? (
          <ol className={cn("flex flex-col gap-0", showAll && "max-h-[28rem] scroll-fade overflow-y-auto pe-2")}>
            {[...activity].reverse().slice(0, showAll ? undefined : ACTIVITY_SHOWN).map((event, index, newestFirst) => {
              const date = new Date(event.created_at * 1000);
              return (
                <li className="group/event relative ms-1 pb-4 ps-4 last:pb-0" key={index}>
                  <span className="absolute inset-s-0 top-2.5 -bottom-1.5 w-px -translate-x-1/2 bg-white/30 group-last/event:hidden rtl:translate-x-1/2" aria-hidden="true" />
                  <span className="absolute inset-s-0 top-1.5 size-2 -translate-x-1/2 rounded-sm bg-[color-mix(in_oklch,white_65%,var(--agent))] ring-4 ring-agent rtl:translate-x-1/2" aria-hidden="true" />
                  <p className="font-medium">{activityLabel(event,
                    (index === 0 && run.pending_review_id !== null) || ["claimed", "resumed"].includes(newestFirst[index + 1]?.kind ?? ""))}</p>
                  <p className="text-xs text-white/75"><time dateTime={date.toISOString()}>
                    {new Intl.DateTimeFormat(locale, sameDay(date) ? { timeStyle: "short" } : { dateStyle: "medium", timeStyle: "short" }).format(date)}
                  </time></p>
                  {event.detail && <p className="mt-1 wrap-anywhere whitespace-pre-wrap text-sm text-white/85">{event.detail}</p>}
                </li>
              );
            })}
          </ol>
        ) : activity && <p className="text-sm text-white/75">{copy.emptyActivity}</p>}
        {activity && activity.length > ACTIVITY_SHOWN && (
          <Button variant="link" size="sm" className="mt-2 h-auto px-0 text-white underline underline-offset-4"
            aria-expanded={showAll} onClick={() => setShowAll(!showAll)}>
            {showAll ? copy.activityShowLess : copy.activityShowAll.replace("{count}", String(activity.length))}
          </Button>
        )}
      </div>
    </aside>
  );
}

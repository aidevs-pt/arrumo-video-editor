"use client";

import { useLocale } from "@/lib/locale-provider";
import { invoke } from "@tauri-apps/api/core";
import { ArrowUpRight, Check, Circle, Clock3, ListVideo, Pause, Play, RotateCcw } from "lucide-react";
import Link from "next/link";
import { useEffect, useState } from "react";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription,
  AlertDialogFooter, AlertDialogHeader, AlertDialogTitle, AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { completedVideoHref } from "@/components/workspace/home/workspace-router";
import { GenerationPlan } from "./generation-plan";
import type { DeliveryFormat } from "./delivery-subtitles";

type RunState = "queued" | "running" | "paused" | "failed" | "completed" | "cancelled";

export type Run = {
  id: number;
  draft_id: number;
  state: RunState;
  pending_review_id: number | null;
  display_title: string | null;
  snapshot: { draft: { id: number; title: string; source_ids?: number[] }; formats?: { format: DeliveryFormat }[]; options?: { save_motion_to_library?: boolean } };
};

export type RunAction = "pause" | "resume" | "retry" | "cancel";
type Activity = { state: RunState; kind: "submitted" | "claimed" | "transition"; created_at: number; detail: string | null };

export function runNeedsReview(run: Run) {
  return run.state === "paused" && run.pending_review_id !== null;
}

export function runTitle(run: Run) {
  return run.display_title ?? run.snapshot.draft.title;
}

export function RunStatus({ run }: { run: Run }) {
  const { messages } = useLocale();
  const review = runNeedsReview(run);
  const variant = run.state === "failed" ? "destructive"
    : review ? "default"
    : run.state === "running" || run.state === "completed" ? "secondary" : "outline";
  return (
    <Badge variant={variant} className="h-auto min-h-5.5 shrink-0 whitespace-normal">
      {run.state === "completed" ? <Check aria-hidden="true" /> : <Circle aria-hidden="true" />}
      {review ? messages.productionPlan.awaitingReview : messages.queue[run.state]}
    </Badge>
  );
}

export function GenerationRunRow({ run, workspaceId }: { run: Run; workspaceId: number }) {
  const { messages } = useLocale();
  const completed = run.state === "completed";
  const title = runTitle(run);
  const href = completed ? completedVideoHref(workspaceId, run.id)
    : `/generation-run?workspace=${workspaceId}&run=${run.id}`;

  return (
    <li className="grid grid-cols-[auto_minmax(0,1fr)] items-center gap-x-3 gap-y-2 py-3 sm:grid-cols-[auto_minmax(0,1fr)_auto]">
      <span className="grid size-9 place-items-center rounded-lg bg-secondary text-secondary-foreground" aria-hidden="true">
        <ListVideo className="size-4" />
      </span>
      <Link href={href} className="min-w-0 wrap-anywhere rounded-sm text-sm font-medium hover:underline focus-visible:ring-2 focus-visible:ring-ring">
        {title}
      </Link>
      <div className="col-start-2 flex flex-wrap items-center gap-2 sm:col-start-3">
        <RunStatus run={run} />
        <Button asChild size="sm" variant={runNeedsReview(run) ? "default" : "outline"}>
          <Link href={href}>
            {completed ? messages.completedVideo.details : runNeedsReview(run) ? messages.queue.reviewRun : messages.queue.openRun}
            <ArrowUpRight data-icon="inline-end" />
          </Link>
        </Button>
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

  return (
    <div className="flex flex-wrap items-center gap-2">
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
      {run.state === "completed" && (
        <Button asChild size="sm" variant="outline">
          <Link href={completedVideoHref(workspaceId, run.id)}>{messages.completedVideo.details}<ArrowUpRight data-icon="inline-end" /></Link>
        </Button>
      )}
    </div>
  );
}

export function GenerationRunPanels({ run, workspaceId }: { run: Run; workspaceId: number }) {
  return (
    <div className="@container">
      <div className="grid grid-cols-1 items-start gap-5 @min-[54rem]:grid-cols-[minmax(0,1fr)_16rem]">
        <GenerationPlan workspaceId={workspaceId} runId={run.id}
          pendingReviewId={runNeedsReview(run) ? run.pending_review_id : null}
          runState={run.state} hasSources={Boolean(run.snapshot.draft.source_ids?.length)}
          saveMotionDefault={run.snapshot.options?.save_motion_to_library ?? true}
          formats={run.snapshot.formats?.map((entry) => entry.format) ?? []} />
        <GenerationActivity run={run} workspaceId={workspaceId} />
      </div>
    </div>
  );
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

  return (
    <Card aria-label={copy.activity}>
      <CardHeader><CardTitle className="flex items-center gap-2"><Clock3 className="size-4" aria-hidden="true" />{copy.activity}</CardTitle></CardHeader>
      <CardContent>
        {historyError && <Alert variant="destructive"><AlertDescription>{copy.activityError}</AlertDescription></Alert>}
        {!activity && !historyError && <Skeleton className="h-14" aria-label={copy.loading} />}
        {activity?.length ? (
          <ol className="flex flex-col gap-0">
            {[...activity].reverse().map((event, index) => {
              const date = new Date(event.created_at * 1000);
              return (
                <li className="relative ms-1 border-s pb-4 ps-4 last:border-transparent last:pb-0" key={index}>
                  <span className="absolute inset-s-0 top-1.5 size-2 -translate-x-1/2 rounded-full bg-muted-foreground ring-4 ring-card rtl:translate-x-1/2" aria-hidden="true" />
                  <p className="font-medium">{copy[event.kind]}</p>
                  <p className="text-xs text-muted-foreground">{copy[event.state]} · <time dateTime={date.toISOString()}>
                    {new Intl.DateTimeFormat(locale, { dateStyle: "short", timeStyle: "short" }).format(date)}
                  </time></p>
                  {event.detail && <p className="mt-1 wrap-anywhere whitespace-pre-wrap text-sm text-muted-foreground">{event.detail}</p>}
                </li>
              );
            })}
          </ol>
        ) : activity && <p className="text-sm text-muted-foreground">{copy.emptyActivity}</p>}
      </CardContent>
    </Card>
  );
}

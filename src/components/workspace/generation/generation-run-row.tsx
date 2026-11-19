"use client";

import { useLocale } from "@/lib/locale-provider";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import Link from "next/link";
import { completedVideoHref } from "@/components/workspace/home/workspace-router";
import { GenerationPlan } from "./generation-plan";

import type { DeliveryFormat } from "./delivery-subtitles";

type RunState = "queued" | "running" | "paused" | "failed" | "completed" | "cancelled";
export type Run = { id: number; draft_id: number; state: RunState; pending_review_id: number | null; display_title: string | null; snapshot: { draft: { id: number; title: string; source_ids?: number[] }; formats?: { format: DeliveryFormat }[] } };
export type RunAction = "pause" | "resume" | "retry" | "cancel";
type Activity = { state: RunState; kind: "submitted" | "claimed" | "transition"; created_at: number; detail: string | null };

export function GenerationRunRow({ run, workspaceId, disabled, onControl }: {
  run: Run;
  workspaceId: number;

  disabled: boolean;
  onControl: (runId: number, action: RunAction) => Promise<void>;
}) {
  const [confirm, setConfirm] = useState(false);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [activity, setActivity] = useState<Activity[]>();
  const [historyError, setHistoryError] = useState(false);
  const { locale, messages } = useLocale();
  const copy = messages.queue;
  const title = run.display_title ?? run.snapshot.draft.title;
  const canPause = run.state === "queued" || run.state === "running";
  const canResume = run.state === "paused" && run.pending_review_id === null;

  useEffect(() => {
    if (!historyOpen) return;
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
      if (active) timer = setTimeout(refresh, 2000);
    }
    void refresh();
    return () => { active = false; clearTimeout(timer); };
  }, [historyOpen, run.id, run.state, workspaceId]);

  return (
    <li>
      <h3 className="flex min-w-0 items-baseline gap-2.5 font-[680]">{title}</h3>
      <p className="mt-1.25 whitespace-pre-wrap text-muted-foreground">{run.state === "paused" && run.pending_review_id !== null ? messages.productionPlan.awaitingReview : copy[run.state]}</p>
      {(canPause || run.state === "paused") && (
        <div className="mt-2.5 [&>summary]:cursor-pointer">
          {confirm ? (
            <>
              <p>{copy.confirmCancel}</p>
              <div className="flex flex-wrap items-baseline gap-2.5">
                <Button type="button" size="sm" variant="destructive" disabled={disabled} onClick={() => {
                  setConfirm(false);
                  void onControl(run.id, "cancel");
                }}>{copy.cancel}</Button>
                <Button type="button" size="sm" variant="outline" onClick={() => setConfirm(false)}>{copy.keepRun}</Button>
              </div>
            </>
          ) : (
            <div className="flex flex-wrap items-baseline gap-2.5">
              {canPause && <Button type="button" size="sm" variant="outline" disabled={disabled}
                aria-label={`${copy.pause} · ${title}`} onClick={() => void onControl(run.id, "pause")}>{copy.pause}</Button>}
              {canResume && <Button type="button" size="sm" variant="outline" disabled={disabled}
                aria-label={`${copy.resume} · ${title}`} onClick={() => void onControl(run.id, "resume")}>{copy.resume}</Button>}
              <Button type="button" size="sm" variant="ghost" disabled={disabled}
                aria-label={`${copy.cancel} · ${title}`} onClick={() => setConfirm(true)}>{copy.cancel}</Button>
            </div>
          )}
        </div>
      )}
      {run.state === "failed" && (
        <Button type="button" size="sm" variant="outline" className="mt-2.5" disabled={disabled}
          aria-label={`${copy.retryRun} · ${title}`} onClick={() => void onControl(run.id, "retry")}>{copy.retryRun}</Button>
      )}
      {run.state === "completed" && (
        <Button asChild size="sm" variant="outline" className="mt-2.5">
          <Link href={completedVideoHref(workspaceId, run.id)}>{messages.completedVideo.details}</Link>
        </Button>
      )}
      <GenerationPlan workspaceId={workspaceId} runId={run.id}
        pendingReviewId={run.state === "paused" ? run.pending_review_id : null}
        runState={run.state} hasSources={Boolean(run.snapshot.draft.source_ids?.length)}
        formats={run.snapshot.formats?.map((entry) => entry.format) ?? []} />
      <details className="mt-2.5 [&>summary]:cursor-pointer" onToggle={(event) => setHistoryOpen(event.currentTarget.open)}>
        <summary>{copy.activity}</summary>
        {historyError ? <p role="alert">{copy.activityError}</p> : !activity ? <p role="status">{copy.loading}</p> : (
          <ol className="list-decimal ps-6 [&>li]:py-0.75">
            {activity.map((event, index) => (
              <li key={index}>
                {copy[event.kind]} · {copy[event.state]} · <time dateTime={new Date(event.created_at * 1000).toISOString()}>
                  {new Date(event.created_at * 1000).toLocaleString(locale)}
                </time>
                {event.detail && <p className="mt-1 whitespace-pre-wrap text-muted-foreground">{event.detail}</p>}
              </li>
            ))}
          </ol>
        )}
      </details>
    </li>
  );
}

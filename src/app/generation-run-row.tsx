"use client";

import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { queueCopy } from "./queue-copy";

type RunState = "queued" | "running" | "paused" | "failed" | "completed" | "cancelled";
export type Run = { id: number; draft_id: number; state: RunState; snapshot: { draft: { id: number; title: string } } };
export type RunAction = "pause" | "resume" | "cancel";
type Activity = { state: RunState; kind: "submitted" | "claimed" | "transition"; created_at: number };

export function GenerationRunRow({ run, workspaceId, locale, disabled, onControl }: {
  run: Run;
  workspaceId: number;
  locale: keyof typeof queueCopy;
  disabled: boolean;
  onControl: (runId: number, action: RunAction) => Promise<void>;
}) {
  const [confirm, setConfirm] = useState(false);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [activity, setActivity] = useState<Activity[]>();
  const [historyError, setHistoryError] = useState(false);
  const copy = queueCopy[locale];
  const title = run.snapshot.draft.title;
  const canPause = run.state === "queued" || run.state === "running";
  const canResume = run.state === "paused";

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
      <h3 className="profile-name">{title}</h3>
      <p className="profile-guidance">{copy[run.state]}</p>
      {(canPause || canResume) && (
        <div className="run-actions">
          {confirm ? (
            <>
              <p>{copy.confirmCancel}</p>
              <div className="profile-edit-actions">
                <Button type="button" size="sm" variant="destructive" disabled={disabled} onClick={() => {
                  setConfirm(false);
                  void onControl(run.id, "cancel");
                }}>{copy.cancel}</Button>
                <Button type="button" size="sm" variant="outline" onClick={() => setConfirm(false)}>{copy.keepRun}</Button>
              </div>
            </>
          ) : (
            <div className="profile-edit-actions">
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
      <details className="run-actions" onToggle={(event) => setHistoryOpen(event.currentTarget.open)}>
        <summary>{copy.activity}</summary>
        {historyError ? <p role="alert">{copy.activityError}</p> : !activity ? <p role="status">{copy.loading}</p> : (
          <ol className="draft-order">
            {activity.map((event, index) => (
              <li key={index}>
                {copy[event.kind]} · {copy[event.state]} · <time dateTime={new Date(event.created_at * 1000).toISOString()}>
                  {new Date(event.created_at * 1000).toLocaleString(locale)}
                </time>
              </li>
            ))}
          </ol>
        )}
      </details>
    </li>
  );
}

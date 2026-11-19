"use client";

import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { useLocale } from "@/lib/locale-provider";

import type { Stage } from "../generation/generation-plan";
import { CompletedVideoFeedback, type FeedbackResult, type VideoFeedback } from "./completed-video-feedback";
import { CompletedOutputPreview } from "./completed-output-preview";
import { CompletedVideoReplay } from "./completed-video-replay";

type CompletedVideo = {
  plan: { steps: { stage: Stage }[] };
  production: { results: FeedbackResult[] };
  feedback: VideoFeedback[];
  title: string;
  delivery: {
    verified_outputs: {
      format_id: number;
      video: string;
      subtitles: string;
      duration_seconds: number;
    }[];
  };
};

export function CompletedVideoDetail({ workspaceId, runId, onTitle }: {
  workspaceId: number;
  runId: number;
  onTitle: (title: string) => void;
}) {
  const { locale, messages: { completedVideo: copy } } = useLocale();
  const [open, setOpen] = useState(false);
  const [video, setVideo] = useState<CompletedVideo>();
  const [title, setTitle] = useState("");
  const [error, setError] = useState<"loadError" | "renameError" | "exportError">();
  const [pending, setPending] = useState(false);
  const [notice, setNotice] = useState<"saved" | "exported">();

  useEffect(() => {
    if (!open || video) return;
    let active = true;
    invoke<CompletedVideo>("get_completed_video", { workspaceId, runId }).then((result) => {
      if (!active) return;
      setVideo(result);
      setTitle(result.title);
      onTitle(result.title);
    }).catch(() => {
      if (active) setError("loadError");
    });
    return () => { active = false; };
  }, [open, onTitle, runId, video, workspaceId]);

  async function rename(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending) return;
    setPending(true);
    setError(undefined);
    setNotice(undefined);
    try {
      const saved = await invoke<string>("rename_completed_video", { workspaceId, runId, title });
      setVideo((current) => current && { ...current, title: saved });
      setTitle(saved);
      onTitle(saved);
      setNotice("saved");
    } catch {
      setError("renameError");
    } finally {
      setPending(false);
    }
  }

  async function exportZip() {
    setPending(true);
    setError(undefined);
    setNotice(undefined);
    try {
      const exported = await invoke<boolean>("export_completed_video", { workspaceId, runId });
      if (exported) setNotice("exported");
    } catch {
      setError("exportError");
    } finally {
      setPending(false);
    }
  }

  return (
    <details className="mt-2.5" onToggle={(event) => {
      setOpen(event.currentTarget.open);
      if (!video) setError(undefined);
    }}>
      <summary className="cursor-pointer">{copy.details}</summary>
      {error && <Alert variant="destructive" className="mt-3"><AlertDescription>{copy[error]}</AlertDescription></Alert>}
      {open && !video && !error && <p role="status" className="mt-3 text-sm text-muted-foreground">{copy.loading}</p>}
      {video && (
        <div className="mt-4 flex flex-col gap-5">
          <form onSubmit={rename}>
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor={`completed-title-${runId}`}>{copy.title}</FieldLabel>
                <div className="flex flex-wrap items-center gap-2">
                  <Input className="min-w-32 flex-1" id={`completed-title-${runId}`} required
                    value={title} onChange={(event) => setTitle(event.target.value)} />
                  <Button type="submit" size="sm" variant="outline"
                    disabled={pending || !title.trim() || title.trim() === video.title}>{copy.saveTitle}</Button>
                </div>
              </Field>
            </FieldGroup>
          </form>
          <section aria-labelledby={`delivery-files-${runId}`}>
            <h4 id={`delivery-files-${runId}`} className="font-medium">{copy.files}</h4>
            <ul className="mt-2 flex flex-col gap-2 text-sm text-muted-foreground">
              {video.delivery.verified_outputs.map((output) => (
                <li className="flex flex-col gap-1" key={output.format_id}>
                  <div className="flex flex-wrap gap-x-3 gap-y-1">
                    <span>{output.video}</span>
                    <span>{output.subtitles}</span>
                    <span className="tabular-nums">{new Intl.NumberFormat(locale, {
                      style: "unit", unit: "second", maximumFractionDigits: 1,
                    }).format(output.duration_seconds)}</span>
                  </div>
                  <CompletedOutputPreview workspaceId={workspaceId} runId={runId} formatId={output.format_id}
                    filename={output.video} />
                </li>
              ))}
            </ul>
          </section>
          <div className="flex flex-wrap items-center gap-3">
            <Button type="button" size="sm" disabled={pending} onClick={() => void exportZip()}>
              {copy.exportZip}
            </Button>
            {notice && <p role="status" className="text-sm text-muted-foreground">{copy[notice]}</p>}
          </div>
          <CompletedVideoReplay workspaceId={workspaceId} runId={runId} />
          <CompletedVideoFeedback workspaceId={workspaceId} runId={runId} initialFeedback={video.feedback}
            results={video.production.results} stages={video.plan.steps} />
        </div>
      )}
    </details>
  );
}

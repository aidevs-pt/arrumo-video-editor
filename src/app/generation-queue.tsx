"use client";

import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { queueCopy } from "./queue-copy";

type Draft = { id: number; title: string };
type RunState = "queued" | "running" | "paused" | "failed" | "completed" | "cancelled";
type Run = { id: number; draft_id: number; state: RunState; snapshot: { draft: Draft } };

export function GenerationQueue({ workspaceId, drafts, initialDraftId, locale, onBack }: {
  workspaceId: number;
  drafts: Draft[];
  initialDraftId: number | null;
  locale: keyof typeof queueCopy;
  onBack: () => void;
}) {
  const heading = useRef<HTMLHeadingElement>(null);
  const request = useRef<{ key: string; payload: string } | null>(null);
  const [selectedId, setSelectedId] = useState(initialDraftId);
  const [runs, setRuns] = useState<Run[]>();
  const [attempt, setAttempt] = useState(0);
  const [loadError, setLoadError] = useState(false);
  const [pending, setPending] = useState(false);
  const [submitError, setSubmitError] = useState(false);
  const copy = queueCopy[locale];
  const selected = drafts.find((draft) => draft.id === selectedId);

  useEffect(() => { heading.current?.focus(); }, []);
  useEffect(() => {
    let active = true;
    invoke<Run[]>("list_generation_runs", { workspaceId }).then(
      (items) => { if (active) setRuns(items); },
      () => { if (active) setLoadError(true); },
    );
    return () => { active = false; };
  }, [workspaceId, attempt]);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selected || !runs || pending) return;
    const fields = new FormData(event.currentTarget);
    const options = { autopilot: fields.has("autopilot"), platform: String(fields.get("platform") ?? "").trim() || null };
    const payload = JSON.stringify({ draftId: selected.id, options });
    if (request.current?.payload !== payload) {
      request.current = { key: crypto.randomUUID(), payload };
    }
    setPending(true);
    setSubmitError(false);
    try {
      const run = await invoke<Run>("enqueue_video_draft", {
        workspaceId, draftId: selected.id, requestKey: request.current.key, options,
      });
      const next = [...runs.filter((item) => item.id !== run.id), run].sort((a, b) => a.id - b.id);
      setRuns(next);
      const queued = new Set(next.map((item) => item.draft_id));
      setSelectedId(drafts.find((draft) => !queued.has(draft.id))?.id ?? null);
      request.current = null;
    } catch {
      setSubmitError(true);
    } finally {
      setPending(false);
    }
  }

  return (
    <main className="workspace-shell profile-shell">
      <Button type="button" variant="ghost" disabled={pending} onClick={onBack}>
        <ArrowLeft data-icon="inline-start" />{copy.back}
      </Button>
      <div className="workspace-heading"><h1 ref={heading} tabIndex={-1}>{copy.title}</h1></div>
      {loadError ? (
        <Alert variant="destructive">
          <AlertDescription>{copy.loadError}</AlertDescription>
          <Button type="button" variant="outline" onClick={() => {
            setLoadError(false);
            setAttempt((value) => value + 1);
          }}>{copy.retry}</Button>
        </Alert>
      ) : !runs ? <p role="status">{copy.loading}</p> : (
        <div className="workspace-layout">
          <section aria-labelledby="generation-runs-title">
            <h2 id="generation-runs-title">{copy.runs}</h2>
            {runs.length === 0 ? <p>{copy.empty}</p> : (
              <ul className="workspace-rows" aria-live="polite">
                {runs.map((run) => (
                  <li key={run.id}>
                    <h3 className="profile-name">{run.snapshot.draft.title}</h3>
                    <p className="profile-guidance">{copy[run.state]}</p>
                  </li>
                ))}
              </ul>
            )}
          </section>
          {selected && (
            <form key={selected.id} className="workspace-form" onSubmit={submit} aria-labelledby="run-options-title" aria-busy={pending}>
              <h2 id="run-options-title">{copy.options} · {selected.title}</h2>
              <fieldset disabled={pending}>
                <FieldGroup>
                  <Field>
                    <FieldLabel htmlFor="run-platform">{copy.platform}</FieldLabel>
                    <Input id="run-platform" name="platform" />
                  </Field>
                  <Field orientation="horizontal">
                    <input id="run-autopilot" name="autopilot" type="checkbox" />
                    <FieldLabel htmlFor="run-autopilot">{copy.autopilot}</FieldLabel>
                  </Field>
                  {submitError && <Alert variant="destructive"><AlertDescription>{copy.submitError}</AlertDescription></Alert>}
                  <Button type="submit">{pending ? copy.submitting : copy.submit}</Button>
                </FieldGroup>
              </fieldset>
            </form>
          )}
        </div>
      )}
    </main>
  );
}

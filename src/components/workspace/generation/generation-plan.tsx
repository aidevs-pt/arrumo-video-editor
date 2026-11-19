"use client";

import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState, type FormEvent } from "react";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Textarea } from "@/components/ui/textarea";
import { Badge } from "@/components/ui/badge";
import { useLocale } from "@/lib/locale-provider";
import { GuidedPreview, previewFiles } from "./guided-preview";
import { MediaStageAction } from "./media-stage-action";

export type Stage = "script" | "assets" | "preview" | "supplemental_outputs" | "final_render" | "verify_delivery";
type Plan = { steps: { stage: Stage; review_required: boolean }[] };

type StageResult = { id: number; result: Record<string, unknown> };
type Progress = { stage_index: number; pending_review_id: number | null; results: StageResult[] };

export function GenerationPlan({ workspaceId, runId, pendingReviewId, runState, hasSources }: {
  workspaceId: number;
  runId: number;
  pendingReviewId: number | null;
  runState: string;
  hasSources: boolean;
}) {
  const { messages: { productionPlan: copy } } = useLocale();
  const [open, setOpen] = useState(pendingReviewId !== null);
  const [progress, setProgress] = useState<Progress>();
  const [reviewing, setReviewing] = useState(false);
  const [reviewError, setReviewError] = useState(false);
  const awaiting = progress?.results.find((entry) => entry.id === progress.pending_review_id);

  useEffect(() => {
    if (pendingReviewId !== null) setOpen(true);
  }, [pendingReviewId]);
  const [plan, setPlan] = useState<Plan | null>();
  const [loadError, setLoadError] = useState(false);

  useEffect(() => {
    if (!open || reviewing) return;
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      try {
        const [current, status] = await Promise.all([
          invoke<Plan | null>("generation_plan", { workspaceId, runId }),
          invoke<Progress>("production_progress", { workspaceId, runId }),
        ]);
        if (!active) return;
        setPlan(current);
        setLoadError(false);
        setProgress(status);
      } catch {
        if (active) setLoadError(true);
      }
      if (active) timer = setTimeout(refresh, 2000);
    }
    void refresh();
    return () => { active = false; clearTimeout(timer); };
  }, [open, workspaceId, runId, pendingReviewId, reviewing]);

  const currentStage = progress && plan?.steps[progress.stage_index]?.stage;
  const previews = currentStage === "preview" ? previewFiles(awaiting?.result.preview_videos) : [];
  const mediaStage = currentStage === "verify_delivery"
    || (hasSources && (currentStage === "preview" || currentStage === "final_render"))
    ? currentStage : undefined;

  async function review(event: FormEvent<HTMLFormElement>, resultId: number) {
    event.preventDefault();
    if (reviewing) return;
    const submitter = (event.nativeEvent as SubmitEvent).submitter;
    if (!(submitter instanceof HTMLButtonElement)) return;
    const notes = String(new FormData(event.currentTarget).get("notes") ?? "");
    setReviewing(true);
    setReviewError(false);
    try {
      await invoke("review_production_result", {
        workspaceId, runId, resultId, decision: submitter.value, notes,
      });
      setProgress((current) => current && { ...current, pending_review_id: null });
    } catch {
      setReviewError(true);
    } finally {
      setReviewing(false);
    }
  }

  return (
    <details open={open} className="mt-2.5" onToggle={(event) => setOpen(event.currentTarget.open)}>
      <summary className="cursor-pointer">{copy.title}</summary>
      {loadError ? <p role="alert" className="mt-2">{copy.loadError}</p>
        : plan === undefined ? <p role="status" className="mt-2">{copy.loading}</p>
        : plan === null ? <p role="status" className="mt-2">{copy.waiting}</p>
        : (
          <>
            <ol className="mt-2 list-decimal ps-6">
              {plan.steps.map((step, index) => (
                <li key={step.stage} className="py-1">
                  <span>{copy[step.stage]}</span>
                  {progress && index < progress.stage_index && <Badge className="ms-2" variant="secondary">{copy.stageComplete}</Badge>}
                  {progress && index === progress.stage_index && <Badge className="ms-2">{copy.currentStage}</Badge>}
                  {step.review_required && <Badge className="ms-2" variant="outline">{copy.reviewRequired}</Badge>}
                </li>
              ))}
            </ol>
            {mediaStage && !awaiting && runState !== "completed" && runState !== "cancelled" && (
              <MediaStageAction key={mediaStage} workspaceId={workspaceId} runId={runId} stage={mediaStage}
                available={runState === "queued"} />
            )}
            {awaiting && (
              <form key={awaiting.id} className="mt-4 rounded-lg border p-4"
                onSubmit={(event) => void review(event, awaiting.id)}>
                <fieldset disabled={reviewing}>
                  <FieldGroup>
                    {previews.length > 0 ? (
                      <div className="flex flex-col gap-3">
                        {previews.map(({ filename, formatId }) => (
                          <GuidedPreview key={filename} workspaceId={workspaceId} runId={runId}
                            resultId={awaiting.id} filename={filename} formatId={formatId} />
                        ))}
                        {typeof awaiting.result.markdown === "string" && (
                          <p className="wrap-anywhere whitespace-pre-wrap text-sm">{awaiting.result.markdown}</p>
                        )}
                      </div>
                    ) : (
                      <pre className="wrap-anywhere whitespace-pre-wrap text-sm">{typeof awaiting.result.markdown === "string"
                        ? awaiting.result.markdown : JSON.stringify(awaiting.result, null, 2)}</pre>
                    )}
                    <Field>
                      <FieldLabel htmlFor={`review-notes-${runId}`}>{copy.reviewNotes}</FieldLabel>
                      <Textarea id={`review-notes-${runId}`} name="notes" />
                    </Field>
                    {reviewError && <p role="alert">{copy.reviewError}</p>}
                    <div className="flex flex-wrap gap-2">
                      <Button type="submit" value="approved">{copy.approve}</Button>
                      <Button type="submit" value="revise" variant="outline">{copy.revise}</Button>
                      <Button type="submit" value="rejected" variant="outline">{copy.reject}</Button>
                    </div>
                  </FieldGroup>
                </fieldset>
              </form>
            )}
          </>
        )}
    </details>
  );
}

"use client";

import { invoke } from "@tauri-apps/api/core";
import { Check, ChevronDown, Clapperboard, FileText, Image, ListChecks, PackageCheck, Video } from "lucide-react";
import { useEffect, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Checkbox } from "@/components/ui/checkbox";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Progress } from "@/components/ui/progress";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";
import { useLocale } from "@/lib/locale-provider";
import { GuidedPreview, previewFiles } from "./guided-preview";
import type { DeliveryFormat } from "./delivery-subtitles";
import { RunMotionResults, type MotionStageResult } from "../motion-library/run-motion-results";
import { MediaStageAction } from "./media-stage-action";

export type Stage = "script" | "assets" | "preview" | "supplemental_outputs" | "final_render" | "verify_delivery";
type Plan = { steps: { stage: Stage; review_required: boolean }[] };
type StageResult = MotionStageResult;
type StageProgress = { stage_index: number; pending_review_id: number | null; results: StageResult[] };
type Decision = "approved" | "revise" | "rejected";
type ReviewDraft = { decision: Decision | ""; notes: string; motionIndexes?: number[] };

const stages = {
  script: FileText,
  assets: Image,
  preview: Video,
  supplemental_outputs: PackageCheck,
  final_render: Clapperboard,
  verify_delivery: Check,
};
const decisionLabels = { approved: "approve", revise: "revise", rejected: "reject" } as const;
const emptyDraft: ReviewDraft = { decision: "", notes: "" };

function isDecision(value: unknown): value is Decision {
  return value === "approved" || value === "revise" || value === "rejected";
}

export function GenerationPlan({ workspaceId, runId, pendingReviewId, runState, hasSources, formats, saveMotionDefault }: {
  workspaceId: number;
  runId: number;
  pendingReviewId: number | null;
  runState: string;
  hasSources: boolean;
  formats: DeliveryFormat[];
  saveMotionDefault: boolean;
}) {
  const { messages: { productionPlan: copy } } = useLocale();
  const [progress, setProgress] = useState<StageProgress>();
  const [reviewing, setReviewing] = useState(false);
  const [reviewError, setReviewError] = useState(false);
  const [plan, setPlan] = useState<Plan | null>();
  const [loadError, setLoadError] = useState(false);
  const awaiting = progress?.results.find((entry) => entry.id === progress.pending_review_id);

  useEffect(() => {
    if (reviewing) return;
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      try {
        const [current, status] = await Promise.all([
          invoke<Plan | null>("generation_plan", { workspaceId, runId }),
          invoke<StageProgress>("production_progress", { workspaceId, runId }),
        ]);
        if (!active) return;
        setPlan(current);
        setLoadError(false);
        setProgress(status);
      } catch {
        if (active) setLoadError(true);
      }
      if (active && runState !== "completed" && runState !== "cancelled") timer = setTimeout(refresh, 2000);
    }
    void refresh();
    return () => { active = false; clearTimeout(timer); };
  }, [workspaceId, runId, pendingReviewId, reviewing, runState]);

  const currentStage = progress && plan?.steps[progress.stage_index]?.stage;
  const mediaStage = currentStage === "verify_delivery"
    || (hasSources && (currentStage === "preview" || currentStage === "final_render"))
    ? currentStage : undefined;

  async function review(resultId: number, decision: Decision, notes: string, saveMotionIndexes?: number[]) {
    if (reviewing) return false;
    setReviewing(true);
    setReviewError(false);
    try {
      await invoke("review_production_result", { workspaceId, runId, resultId, decision, notes, saveMotionIndexes });
      setProgress((current) => current && { ...current, pending_review_id: null });
      return true;
    } catch {
      setReviewError(true);
      return false;
    } finally {
      setReviewing(false);
    }
  }

  return (
    <div className="flex min-w-0 flex-col gap-4">
      <Card size="sm">
        <CardHeader><CardTitle className="flex items-center gap-2"><ListChecks className="size-4" aria-hidden="true" />{copy.title}</CardTitle></CardHeader>
        <CardContent className="@container flex flex-col gap-3">
          {loadError && <Alert variant="destructive"><AlertDescription>{copy.loadError}</AlertDescription></Alert>}
          {plan === undefined && !loadError && <Skeleton className="h-14" aria-label={copy.loading} />}
          {plan === null && <p role="status" className="text-sm text-muted-foreground">{copy.waiting}</p>}
          {plan && (
            <>
              <Progress value={plan.steps.length ? Math.min(100, (progress?.stage_index ?? 0) * 100 / plan.steps.length) : 0} aria-label={copy.title} className="h-1" />
              <ol className="grid grid-cols-2 items-stretch gap-1 @min-[30rem]:grid-cols-3 @min-[36rem]:grid-flow-col @min-[36rem]:auto-cols-fr @min-[36rem]:grid-cols-none @min-[36rem]:grid-rows-1">
                {plan.steps.map((step, index) => {
                  const complete = progress !== undefined && index < progress.stage_index;
                  const current = index === progress?.stage_index;
                  const Icon = stages[step.stage];
                  return (
                    <li key={step.stage} aria-current={current ? "step" : undefined} className={cn(
                      "flex min-w-0 items-center gap-2 rounded-md px-2 py-1.5 text-sm @min-[36rem]:flex-col @min-[36rem]:gap-1.5 @min-[36rem]:text-center",
                      current ? "bg-accent text-accent-foreground" : "text-muted-foreground",
                    )}>
                      <Badge variant={current ? "default" : complete ? "secondary" : "outline"} className="min-w-6 justify-center">
                        {complete ? <Check aria-hidden="true" /> : <Icon aria-hidden="true" />}
                      </Badge>
                      <span className={cn("wrap-anywhere", current && "font-semibold")}>{copy[step.stage]}</span>
                      {complete && <span className="sr-only">{copy.stageComplete}</span>}
                      {step.review_required && current && <span className="sr-only">{copy.reviewRequired}</span>}
                    </li>
                  );
                })}
              </ol>
            </>
          )}
        </CardContent>
      </Card>
      {mediaStage && !awaiting && runState !== "completed" && runState !== "cancelled" && (
        <Card size="sm">
          <CardHeader><CardTitle>{copy[mediaStage]}</CardTitle></CardHeader>
          <CardContent>
            <MediaStageAction key={mediaStage} workspaceId={workspaceId} runId={runId} stage={mediaStage}
              available={runState === "queued"} canImportSubtitles={runState === "queued" || runState === "failed"} formats={formats} />
          </CardContent>
        </Card>
      )}
      {progress && plan && <RunMotionResults workspaceId={workspaceId} runId={runId}
        results={progress.results} steps={plan.steps} stageIndex={progress.stage_index} />}
      {awaiting && currentStage && (
        <StageReview key={awaiting.id} workspaceId={workspaceId} runId={runId} stage={currentStage} result={awaiting}
          reviewing={reviewing} reviewError={reviewError} onReview={review} saveMotionDefault={saveMotionDefault} />
      )}
    </div>
  );
}

function StageReview({ workspaceId, runId, stage, result, reviewing, reviewError, onReview, saveMotionDefault }: {
  workspaceId: number;
  runId: number;
  stage: Stage;
  result: StageResult;
  reviewing: boolean;
  reviewError: boolean;
  onReview: (resultId: number, decision: Decision, notes: string, saveMotionIndexes?: number[]) => Promise<boolean>;
  saveMotionDefault: boolean;
}) {
  const { messages: { productionPlan: copy } } = useLocale();
  const motionItems = stage === "assets" && Array.isArray(result.result.motion_items)
    ? result.result.motion_items.flatMap((item: unknown, index: number) => (
      item && typeof item === "object" && "title" in item && typeof item.title === "string"
        ? [{ index, title: item.title }] : []
    )) : [];
  const draftKey = `arrumo:review:${workspaceId}:${runId}:${result.id}`;
  const [draft, setDraft] = useState<ReviewDraft>(() => {
    try {
      const saved = JSON.parse(sessionStorage.getItem(draftKey) ?? "null");
      if (saved && typeof saved.notes === "string") {
        return {
          notes: saved.notes,
          decision: isDecision(saved.decision) ? saved.decision : "",
          motionIndexes: Array.isArray(saved.motionIndexes)
            ? motionItems.map(({ index }) => index).filter((index) => saved.motionIndexes.includes(index))
            : undefined,
        };
      }
    } catch {}
    return emptyDraft;
  });
  const selectedMotion = draft.motionIndexes ?? (saveMotionDefault ? motionItems.map(({ index }) => index) : []);
  const previews = stage === "preview" ? previewFiles(result.result.preview_videos) : [];
  const markdown = typeof result.result.markdown === "string" ? result.result.markdown : undefined;
  const source = Object.fromEntries(Object.entries(result.result).filter(([key]) =>
    key !== "markdown" && (key !== "preview_videos" || previews.length === 0)));

  function updateDraft(next: ReviewDraft) {
    setDraft(next);
    try { sessionStorage.setItem(draftKey, JSON.stringify(next)); } catch {}
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!draft.decision || reviewing) return;
    if (await onReview(result.id, draft.decision, draft.notes,
      draft.decision === "approved" && motionItems.length ? selectedMotion : undefined)) {
      try { sessionStorage.removeItem(draftKey); } catch {}
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>{copy[stage]}</CardTitle>
        <CardDescription>{copy.awaitingReview}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        {markdown && <p className="max-w-[72ch] wrap-anywhere whitespace-pre-wrap text-sm leading-relaxed">{markdown}</p>}
        {previews.map(({ filename, formatId }) => (
          <GuidedPreview key={filename} workspaceId={workspaceId} runId={runId}
            resultId={result.id} filename={filename} formatId={formatId} />
        ))}
        {Object.keys(source).length > 0 && (
          <Collapsible defaultOpen={!markdown && previews.length === 0}>
            <CollapsibleTrigger asChild>
              <Button type="button" variant="ghost" size="sm" className="group">
                {copy.resultDetails}<ChevronDown data-icon="inline-end" className="group-data-[state=open]:rotate-180" />
              </Button>
            </CollapsibleTrigger>
            <CollapsibleContent>
              <pre className="mt-2 max-h-64 overflow-auto rounded-md bg-muted p-3 text-xs">{JSON.stringify(source, null, 2)}</pre>
            </CollapsibleContent>
          </Collapsible>
        )}
        <Separator />
        <form onSubmit={(event) => void submit(event)}>
          <fieldset disabled={reviewing}>
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor={`review-decision-${runId}`}>{copy.decision}</FieldLabel>
                <Select value={draft.decision} disabled={reviewing}
                  onValueChange={(value) => { if (isDecision(value)) updateDraft({ ...draft, decision: value }); }}>
                  <SelectTrigger id={`review-decision-${runId}`} className="max-w-full">
                    <SelectValue placeholder={copy.selectDecision} />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      {Object.entries(decisionLabels).map(([value, label]) => (
                        <SelectItem value={value} key={value}>{copy[label]}</SelectItem>
                      ))}
                    </SelectGroup>
                  </SelectContent>
                </Select>
              </Field>
              <Field>
                <FieldLabel htmlFor={`review-notes-${runId}`}>{copy.reviewNotes}</FieldLabel>
                <Textarea id={`review-notes-${runId}`} name="notes" value={draft.notes}
                  onChange={(event) => updateDraft({ ...draft, notes: event.target.value })} />
              </Field>
              {draft.decision === "approved" && motionItems.length > 0 && (
                <fieldset className="grid gap-2">
                  <legend className="mb-2 text-sm font-medium">{copy.keepMotionItems}</legend>
                  {motionItems.map(({ index, title }) => (
                    <label key={index} className="flex items-center gap-3 rounded-md border px-3 py-2 text-sm">
                      <Checkbox checked={selectedMotion.includes(index)} onCheckedChange={(checked) => {
                        updateDraft({ ...draft, motionIndexes: checked
                          ? [...selectedMotion, index]
                          : selectedMotion.filter((value) => value !== index) });
                      }} />
                      <span className="wrap-anywhere">{title}</span>
                    </label>
                  ))}
                </fieldset>
              )}
              {reviewError && <Alert variant="destructive"><AlertDescription>{copy.reviewError}</AlertDescription></Alert>}
              <Button type="submit" disabled={!draft.decision || reviewing}
                variant={draft.decision === "rejected" ? "destructive" : "default"} className="self-start">
                {copy.submitDecision}
              </Button>
            </FieldGroup>
          </fieldset>
        </form>
      </CardContent>
    </Card>
  );
}

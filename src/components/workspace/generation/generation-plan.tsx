"use client";

import { invoke } from "@tauri-apps/api/core";
import { Check, ChevronDown, Clapperboard, PencilLine, X } from "lucide-react";
import { useEffect, useRef, useState, type CSSProperties, type FormEvent, type ReactNode } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Checkbox } from "@/components/ui/checkbox";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";
import NextImage from "next/image";
import dynamic from "next/dynamic";
import { motion, useReducedMotion } from "motion/react";
import { useLocale } from "@/lib/locale-provider";
import { GuidedPreview, previewFiles } from "./guided-preview";
import type { DeliveryFormat } from "./delivery-subtitles";
import { RunMotionResults, type MotionStageResult } from "../motion-library/run-motion-results";
import { MotionRendererAction } from "./motion-renderer-action";
import { MotionArtifactAction } from "./motion-artifact-action";
import { SupplementalReview } from "./supplemental-review";
import { MediaStageAction } from "./media-stage-action";

export type Stage = "script" | "assets" | "preview" | "supplemental_outputs" | "final_render" | "verify_delivery";
type Plan = { steps: { stage: Stage; review_required: boolean }[] };
type StageResult = MotionStageResult & { supplemental_review?: SupplementalReview; preview_review?: PreviewChoice[] };
type StageProgress = { stage_index: number; pending_review_id: number | null; results: StageResult[] };
type Decision = "approved" | "revise" | "rejected";
type PreviewChoice = { format_id: number; decision: Decision; note?: string };
type SupplementalChoices<T = Decision> = { text: T[]; media: T[] };
type SupplementalReview = SupplementalChoices & { item_notes?: SupplementalChoices<string | null> };
type ReviewDraft = { decision: Decision | ""; notes: string; motionIndexes?: number[]; supplemental?: SupplementalChoices<Decision | "">; itemNotes?: SupplementalChoices<string> };

const ScriptReviewText = dynamic(() => import("./script-review-text"), { ssr: false });

const decisions = [
  { value: "approved", label: "approve", icon: Check, fill: "bg-approval text-white hover:bg-approval hover:text-white", marker: "bg-approval" },
  { value: "revise", label: "revise", icon: PencilLine, fill: "bg-attention text-ink hover:bg-attention hover:text-ink", marker: "bg-attention" },
  { value: "rejected", label: "reject", icon: X, fill: "bg-destructive text-white hover:bg-destructive hover:text-white", marker: "bg-destructive" },
] as const;
const emptyDraft: ReviewDraft = { decision: "", notes: "" };

function isDecision(value: unknown): value is Decision {
  return value === "approved" || value === "revise" || value === "rejected";
}

export function GenerationPlan({ workspaceId, runId, pendingReviewId, runState, hasSources, formats, saveMotionDefault, activity, agentId }: {
  workspaceId: number;
  runId: number;
  pendingReviewId: number | null;
  runState: "queued" | "running" | "paused" | "failed" | "completed" | "cancelled";
  hasSources: boolean;
  formats: DeliveryFormat[];
  saveMotionDefault: boolean;
  activity: ReactNode;
  agentId?: string | null;
}) {
  const { messages: { productionPlan: copy, queue } } = useLocale();
  const reduceMotion = useReducedMotion();
  const timeline = useRef<HTMLElement>(null);
  const [agent, setAgent] = useState<string>();
  const [progress, setProgress] = useState<StageProgress>();
  const [reviewing, setReviewing] = useState(false);
  const [reviewError, setReviewError] = useState(false);
  const [plan, setPlan] = useState<Plan | null>();
  const [loadError, setLoadError] = useState(false);
  const awaiting = progress?.results.find((entry) => entry.id === progress.pending_review_id);

  useEffect(() => {
    if (!agentId) return;
    let active = true;
    invoke<{ id: string; label: string }[]>("list_agent_plugins").then((providers) => {
      if (active) setAgent(providers.find((provider) => provider.id === agentId)?.label);
    }).catch(() => {});
    return () => { active = false; };
  }, [agentId]);

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
  const previousReview = awaiting && progress?.results.findLast((entry) => entry.stage_index === awaiting.stage_index
    && entry.id < awaiting.id);
  const mediaStage = currentStage === "verify_delivery"
    || (hasSources && (currentStage === "preview" || currentStage === "final_render"))
    ? currentStage : undefined;

  async function review(resultId: number, decision: Decision | "", notes: string, saveMotionIndexes?: number[], supplemental?: SupplementalReview, preview?: PreviewChoice[]) {
    if (reviewing) return false;
    setReviewing(true);
    setReviewError(false);
    try {
      if (preview) {
        await invoke("review_preview_formats", { workspaceId, runId, resultId, decisions: preview, notes });
      } else if (supplemental) {
        await invoke("review_supplemental_items", { workspaceId, runId, resultId, review: supplemental, notes });
      } else {
        await invoke("review_production_result", { workspaceId, runId, resultId, decision, notes, saveMotionIndexes });
      }
      setProgress((current) => current && { ...current, pending_review_id: null });
      timeline.current?.focus();
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
      <section ref={timeline} tabIndex={-1} aria-label={copy.title} className="flex min-w-0 flex-col gap-2 rounded-sm focus:outline-2 focus:outline-attention">
        {loadError && <Alert variant="destructive"><AlertDescription>{copy.loadError}</AlertDescription></Alert>}
        {plan === undefined && !loadError && <Skeleton className="h-14 bg-white/15" aria-label={copy.loading} />}
        {plan === null && <p role="status" className="border-y border-white/20 py-3 text-sm text-white/80">{copy.waiting}</p>}
        {plan && (
          <ol className="grid grid-cols-2 gap-x-1 gap-y-3 sm:grid-cols-3 xl:grid-cols-[repeat(var(--stage-count),minmax(0,1fr))]"
            style={{ "--stage-count": plan.steps.length } as CSSProperties}>
            {plan.steps.map((step, index) => {
              const complete = index < (progress?.stage_index ?? 0);
              const current = index === progress?.stage_index;
              const activeColor = runState === "paused" ? "bg-white" : "bg-attention";
              return (
                <li key={step.stage} aria-current={current ? "step" : undefined}
                  className="relative flex min-w-0 flex-col gap-1.5 pt-1.5 pb-2.5">
                  <span className={cn("h-[30px] rounded-xs border", complete
                    ? "border-white/85 bg-white/85" : current ? "border-transparent bg-attention" : "border-white/35")} aria-hidden="true" />
                  <span className={cn("min-w-0 px-1 text-xs font-bold wrap-anywhere",
                    current ? "text-attention" : "text-white/85")}>{copy[step.stage]}</span>
                  <span className="absolute inset-x-0 bottom-0 h-1.5 border-t border-white/30 bg-[repeating-linear-gradient(to_right,transparent_0,transparent_calc(25%_-_1px),rgb(255_255_255_/_20%)_calc(25%_-_1px),rgb(255_255_255_/_20%)_25%)]" aria-hidden="true" />
                  {complete && <span className="sr-only">{copy.stageComplete}</span>}
                  {step.review_required && current && <span className="sr-only">{copy.reviewRequired}</span>}
                  {current && (
                    <motion.span aria-hidden="true"
                      className={cn("pointer-events-none absolute -top-1 bottom-0 start-0 w-[3px]", activeColor)}
                      initial={reduceMotion ? false : { x: -16 }}
                      animate={{ x: 0 }}
                      transition={{ duration: reduceMotion ? 0 : 1.1, ease: [0.165, 0.84, 0.44, 1] }} />
                  )}
                </li>
              );
            })}
          </ol>
        )}
      </section>
      {currentStage && <div className="flex items-center gap-3 py-2 text-white">
        <span className="grid size-11 shrink-0 place-items-center rounded-md bg-attention" aria-hidden="true">
          <NextImage src="/arrumo-face.svg" width={26} height={26} alt="" />
        </span>
        <div className="flex min-w-0 flex-col items-start gap-1" role="status">
          <p className={cn("flex flex-wrap gap-x-2 text-sm font-bold", awaiting ? "text-attention" : "text-white/80")}>
            {agent && <span>{agent}</span>}
            <span>{awaiting ? copy.awaitingReview : queue[runState]}</span>
          </p>
          <p className="rounded-[4px] bg-[#0d1229d1] px-2.5 py-0.5 text-[23px] leading-[31px] font-bold wrap-anywhere">
            {copy[currentStage]}
          </p>
        </div>
      </div>}
      <div className="@container">
        <div className="grid items-start gap-5 @min-[52rem]:grid-cols-[minmax(0,1fr)_17rem]">
          <div className="flex min-w-0 flex-col gap-4">
            {currentStage === "assets" && !awaiting && runState !== "completed" && runState !== "cancelled" && (
              <Card size="sm">
                <CardHeader><CardTitle>{copy.assets}</CardTitle></CardHeader>
                <CardContent className="flex flex-col items-start gap-3">
                  <MotionArtifactAction workspaceId={workspaceId} runId={runId} available={runState === "queued"} />
                  <MotionRendererAction workspaceId={workspaceId} runId={runId} available={runState === "queued"} />
                </CardContent>
              </Card>
            )}
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
              <StageReview key={awaiting.id} workspaceId={workspaceId} runId={runId} stage={currentStage} result={awaiting} formats={formats}
                reviewing={reviewing} reviewError={reviewError} onReview={review} saveMotionDefault={saveMotionDefault}
                previousSupplementalReview={previousReview?.supplemental_review} previousPreviewReview={previousReview?.preview_review} />
            )}
          </div>
          {activity}
        </div>
      </div>
    </div>
  );
}

function StageReview({ workspaceId, runId, stage, result, formats, reviewing, reviewError, onReview, saveMotionDefault, previousSupplementalReview, previousPreviewReview }: {
  workspaceId: number;
  runId: number;
  stage: Stage;
  result: StageResult;
  formats: DeliveryFormat[];
  reviewing: boolean;
  reviewError: boolean;
  onReview: (resultId: number, decision: Decision | "", notes: string, saveMotionIndexes?: number[], supplemental?: SupplementalReview, preview?: PreviewChoice[]) => Promise<boolean>;
  saveMotionDefault: boolean;
  previousSupplementalReview?: SupplementalChoices;
  previousPreviewReview?: PreviewChoice[];
}) {
  const { messages: { productionPlan: copy, motionLibrary } } = useLocale();
  const motionItems = stage === "assets" && Array.isArray(result.result.motion_items)
    ? result.result.motion_items.flatMap((item: unknown, index: number) => (
      item && typeof item === "object" && "title" in item && typeof item.title === "string"
        ? [{
          index,
          title: item.title,
          usage: "usage_markdown" in item && typeof item.usage_markdown === "string" ? item.usage_markdown : "",
          kind: "kind" in item && item.kind === "transition" ? "transition" as const : "animation" as const,
        }] : []
    )) : [];
  const previews = stage === "preview" ? previewFiles(result.result.preview_videos) : [];
  const hasPreviewReview = previews.length > 0 && previews.length === formats.length
    && formats.every((format) => previews.some((preview) => preview.formatId === format.id));
  const textCount = stage === "supplemental_outputs" && Array.isArray(result.result.supplemental_text)
    ? result.result.supplemental_text.length : 0;
  const mediaCount = stage === "supplemental_outputs" && Array.isArray(result.result.supplemental_media)
    ? result.result.supplemental_media.length : hasPreviewReview ? previews.length : 0;
  const hasSupplemental = stage === "supplemental_outputs" && textCount + mediaCount > 0;
  const hasItemReview = hasSupplemental || hasPreviewReview;
  const itemChoices = (saved?: SupplementalChoices<unknown>): SupplementalChoices<Decision | ""> => ({
    text: Array.from({ length: textCount }, (_, index) => {
      const value = saved?.text?.[index];
      return value === "" || isDecision(value) ? value : previousSupplementalReview?.text[index] === "approved" ? "approved" : "";
    }),
    media: Array.from({ length: mediaCount }, (_, index) => {
      const value = saved?.media?.[index];
      return value === "" || isDecision(value) ? value : (hasPreviewReview
        ? previousPreviewReview?.find((entry) => entry.format_id === previews[index].formatId)?.decision
        : previousSupplementalReview?.media[index]) === "approved" ? "approved" : "";
    }),
  });
  const itemNotes = (saved?: SupplementalChoices<unknown>): SupplementalChoices<string> => ({
    text: Array.from({ length: textCount }, (_, index) => {
      const note = saved?.text?.[index];
      return typeof note === "string" ? note : "";
    }),
    media: Array.from({ length: mediaCount }, (_, index) => {
      const note = saved?.media?.[index];
      return typeof note === "string" ? note : "";
    }),
  });
  const reviewFormId = `stage-review-${runId}-${result.id}`;
  const draftKey = `arrumo:review:${workspaceId}:${runId}:${result.id}`;
  const [draft, setDraft] = useState<ReviewDraft>(() => {
    try {
      const saved = JSON.parse(sessionStorage.getItem(draftKey) ?? "null");
      if (saved && typeof saved.notes === "string") {
        return {
          notes: saved.notes,
          supplemental: hasItemReview ? itemChoices(saved.supplemental) : undefined,
          itemNotes: hasItemReview ? itemNotes(saved.itemNotes) : undefined,
          decision: isDecision(saved.decision) ? saved.decision : "",
          motionIndexes: Array.isArray(saved.motionIndexes)
            ? motionItems.map(({ index }) => index).filter((index) => saved.motionIndexes.includes(index))
            : undefined,
        };
      }
    } catch {}
    return hasItemReview ? { ...emptyDraft, supplemental: itemChoices(), itemNotes: itemNotes() } : emptyDraft;
  });
  const selectedMotion = draft.motionIndexes ?? (saveMotionDefault ? motionItems.map(({ index }) => index) : []);
  const markdown = typeof result.result.markdown === "string" ? result.result.markdown : undefined;
  const itemDraft = draft.supplemental;
  const [batchSelection, setBatchSelection] = useState<Set<string>>(() => new Set());
  const [openNotes, setOpenNotes] = useState<Set<string>>(() => new Set());
  const hasItemNotes = Boolean(draft.itemNotes && [...draft.itemNotes.text, ...draft.itemNotes.media].some((note) => note.trim()));
  const selectedCount = batchSelection.size;
  function selectItem(key: string, selected: boolean) {
    setBatchSelection((current) => {
      const next = new Set(current);
      if (selected) next.add(key);
      else next.delete(key);
      return next;
    });
  }
  const textDecisions = itemDraft?.text.filter(isDecision) ?? [];
  const mediaDecisions = itemDraft?.media.filter(isDecision) ?? [];
  const staged = textDecisions.length + mediaDecisions.length;
  const canSend = hasItemReview ? staged === textCount + mediaCount : Boolean(draft.decision);
  const source = Object.fromEntries(Object.entries(result.result).filter(([key]) =>
    key !== "markdown" && (key !== "preview_videos" || previews.length === 0)
    && (key !== "motion_items" || motionItems.length === 0)
    && (!hasSupplemental || (key !== "supplemental_text" && key !== "supplemental_media"))));

  function updateDraft(next: ReviewDraft) {
    setDraft(next);
    try { sessionStorage.setItem(draftKey, JSON.stringify(next)); } catch {}
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!canSend || reviewing) return;
    if (await onReview(result.id, draft.decision, draft.notes,
      draft.decision === "approved" && motionItems.length ? selectedMotion : undefined,
      hasSupplemental ? {
        text: textDecisions, media: mediaDecisions,
        item_notes: hasItemNotes && draft.itemNotes ? {
          text: draft.itemNotes.text.map((note) => note.trim() ? note : null),
          media: draft.itemNotes.media.map((note) => note.trim() ? note : null),
        } : undefined,
      } : undefined,
      hasPreviewReview ? previews.map(({ formatId }, index) => ({
        format_id: formatId, decision: mediaDecisions[index],
        note: draft.itemNotes?.media[index]?.trim() || undefined,
      })) : undefined)) {
      try { sessionStorage.removeItem(draftKey); } catch {}
    }
  }

  function reviewControls(kind: "text" | "media", index: number, label: string) {
    if (!itemDraft) return null;
    const key = `${kind}-${index}`;
    const note = draft.itemNotes?.[kind][index] ?? "";
    return <div className="grid gap-2">
      <div className="flex flex-wrap items-center gap-2">
        <Checkbox disabled={reviewing} checked={batchSelection.has(`${kind}-${index}`)}
          aria-label={copy.selectOutput.replace("{label}", label)}
          onCheckedChange={(checked) => selectItem(`${kind}-${index}`, checked === true)} />
        {decisions.map(({ value, icon: Icon, fill }) => {
          const active = itemDraft[kind][index] === value;
          return <Button key={value} type="button" variant="outline" size="sm" disabled={reviewing}
            aria-pressed={active} className={cn(active && fill)} onClick={() => {
              const items = itemDraft[kind].map((decision, position) => position === index ? (active ? "" : value) : decision);
              updateDraft({ ...draft, supplemental: { ...itemDraft, [kind]: items } });
            }}><Icon data-icon="inline-start" />{copy[`item_${value}`]}</Button>;
        })}
        <Button type="button" size="sm" variant="ghost" disabled={reviewing} aria-expanded={openNotes.has(key)}
          aria-controls={`${reviewFormId}-${key}`} onClick={() => {
            setOpenNotes((current) => {
              const next = new Set(current);
              if (next.has(key)) next.delete(key);
              else next.add(key);
              return next;
            });
          }}>{note ? copy.editItemNote : copy.addItemNote}</Button>
      </div>
      {openNotes.has(key) && <Field>
        <FieldLabel htmlFor={`${reviewFormId}-${key}`}>{copy.itemNoteFor.replace("{label}", label)}</FieldLabel>
        <Textarea id={`${reviewFormId}-${key}`} disabled={reviewing} value={note} maxLength={4096}
          onChange={(event) => {
            const notes = draft.itemNotes ?? itemNotes();
            updateDraft({ ...draft, itemNotes: {
              ...notes, [kind]: notes[kind].map((value, position) => position === index ? event.target.value : value),
            } });
          }} />
        </Field>}
      </div>;
  }

  return (
    <Card className="overflow-visible rounded-[14px]">
      <CardHeader className="border-b">
        <CardTitle className="text-xl font-bold">{copy[stage]}</CardTitle>
        <CardDescription>{copy.awaitingReview}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        {markdown && <ScriptReviewText markdown={markdown} />}
        {itemDraft && <>
          <div className="flex flex-wrap items-center gap-2 rounded-lg border bg-secondary p-2" role="group" aria-label={selectedCount ? copy.selectedOutputCount.replace("{count}", String(selectedCount)) : copy.batchDecisions}>
            {selectedCount > 0 && <span className="px-1 text-sm">{copy.selectedOutputCount.replace("{count}", String(selectedCount))}</span>}
            {decisions.map(({ value, icon: Icon }) => <Button key={value} type="button" variant="outline" size="sm" disabled={reviewing}
              onClick={() => updateDraft({ ...draft, supplemental: {
                text: itemDraft.text.map((current, index) => !selectedCount || batchSelection.has(`text-${index}`) ? value : current),
                media: itemDraft.media.map((current, index) => !selectedCount || batchSelection.has(`media-${index}`) ? value : current),
              } })}><Icon data-icon="inline-start" />{selectedCount ? copy[`selected_${value}`] : copy[`batch_${value}`]}</Button>)}
            {selectedCount > 0 && <Button type="button" size="sm" variant="ghost" disabled={reviewing}
              onClick={() => setBatchSelection(new Set())}>{copy.clearOutputSelection}</Button>}
          </div>
          {hasSupplemental && <SupplementalReview workspaceId={workspaceId} runId={runId} resultId={result.id} formats={formats}
            controls={reviewControls} />}
        </>}
        {motionItems.length > 0 && (
          <ul className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            {motionItems.map(({ index, title, usage, kind }) => (
              <li key={index} className="min-w-0 rounded-lg border">
                <div className="flex min-h-20 items-center gap-2 rounded-t-lg bg-secondary px-3">
                  <Clapperboard className="size-4 shrink-0 text-primary" aria-hidden="true" />
                  <span className="text-sm">{motionLibrary[kind]}</span>
                </div>
                <div className="px-3 py-2">
                  <p className="font-semibold wrap-anywhere">{title}</p>
                  {usage && <p className="text-sm wrap-anywhere whitespace-pre-wrap text-muted-foreground">{usage}</p>}
                </div>
              </li>
            ))}
          </ul>
        )}
        {previews.length > 0 && <div className="grid items-start gap-4 md:grid-cols-2">{previews.map(({ filename, formatId }, index) => (
          <div key={filename} className="grid min-w-0 gap-2">
            <GuidedPreview workspaceId={workspaceId} runId={runId}
              resultId={result.id} filename={filename} formatId={formatId} initiallyOpen={index === 0}
              label={formats.find((format) => format.id === formatId)?.name ?? filename} />
            {hasPreviewReview && reviewControls("media", index, formats.find((format) => format.id === formatId)?.name ?? filename)}
          </div>
        ))}</div>}
        {Object.keys(source).length > 0 && (
          <Collapsible defaultOpen={!markdown && !hasSupplemental && previews.length === 0}>
            <CollapsibleTrigger asChild>
              <Button type="button" variant="ghost" size="sm" className="group">
                {copy.resultDetails}<ChevronDown data-icon="inline-end" className="group-data-[state=open]:rotate-180" />
              </Button>
            </CollapsibleTrigger>
            <CollapsibleContent>
              <pre className="mt-2 max-h-64 overflow-auto rounded-md bg-muted p-3 font-sans text-sm wrap-anywhere whitespace-pre-wrap">{JSON.stringify(source, null, 2)}</pre>
            </CollapsibleContent>
          </Collapsible>
        )}
        <Separator />
        <form id={reviewFormId} onSubmit={(event) => void submit(event)}>
          <fieldset disabled={reviewing}>
            <FieldGroup>
              {!hasItemReview && <Field>
                <div role="group" aria-label={copy.decision} className="flex flex-wrap items-center gap-2">
                  {decisions.map(({ value, label, icon: Icon, fill }) => {
                    const selected = draft.decision === value;
                    return <Button key={value} type="button" size="sm" variant="outline" aria-pressed={selected}
                      onClick={() => updateDraft({ ...draft, decision: selected ? "" : value })}
                      className={cn(selected && fill)}>
                      <Icon data-icon="inline-start" />{copy[label]}
                    </Button>;
                  })}
                </div>
              </Field>}
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
            </FieldGroup>
          </fieldset>
        </form>
      </CardContent>
      <div className="sticky bottom-4 z-[1] flex flex-wrap mx-4 items-center justify-between gap-3 rounded-lg bg-ink px-3 py-2 text-white">
        <p className={cn("flex flex-wrap items-center gap-2 text-sm", (draft.decision || staged > 0 || draft.notes || hasItemNotes) && "font-bold text-attention")} role="status">
          {draft.decision && <span aria-hidden="true"
            className={cn("size-2 rounded-xs", decisions.find((option) => option.value === draft.decision)?.marker)} />}
          {draft.decision || staged > 0 || draft.notes || hasItemNotes ? copy.unsentReview : hasItemReview ? copy.selectEveryItem : copy.selectDecision}
        </p>
        <Button type="submit" form={reviewFormId} disabled={!canSend || reviewing}
          className="ms-auto bg-attention text-ink hover:bg-[oklch(90%_.13_86)]">
          {hasItemReview && staged !== 1 ? copy.submitDecisions.replace("{count}", String(staged)) : copy.submitDecision}
        </Button>
      </div>
    </Card>
  );
}

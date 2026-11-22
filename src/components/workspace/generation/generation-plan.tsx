"use client";

import { PlacedPluginActions } from "@/components/workspace/plugins/placed-actions";
import { PluginIcon } from "@/lib/plugin-text";
import { invoke } from "@tauri-apps/api/core";
import { ArrowUpRight, Check, ChevronDown, Clapperboard, Film, Image as ImageIcon, Music, PencilLine, Sparkles, TriangleAlert, X } from "lucide-react";
import Link from "next/link";
import { isScriptSections, ScriptSectionReview } from "./script-section-review";
import { AgentProgress } from "./agent-progress";
import { useEffect, useRef, useState, type CSSProperties, type FormEvent, type ReactNode } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Checkbox } from "@/components/ui/checkbox";
import { FieldDescription, Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";
import dynamic from "next/dynamic";
import { motion, useReducedMotionConfig } from "motion/react";
import { useLocale } from "@/lib/locale-provider";
import { AssetThumbnail } from "@/components/workspace/assets/asset-thumbnail";
import { MotionPreviewDialog } from "@/components/workspace/motion-library/motion-preview-dialog";
import { GuidedPreview, previewFiles } from "./guided-preview";
import type { DeliveryFormat } from "./delivery-subtitles";
import { RunMotionResults, type MotionStageResult } from "../motion-library/run-motion-results";
import { MotionRendererAction } from "./motion-renderer-action";
import { MotionArtifactAction } from "./motion-artifact-action";
import { SupplementalReview } from "./supplemental-review";
import type { AutomaticSubtitles } from "./delivery-subtitles";
import { MediaStageAction } from "./media-stage-action";
import { completedVideoHref } from "@/components/workspace/home/workspace-router";

export type Stage = "script" | "assets" | "preview" | "supplemental_outputs" | "final_render" | "verify_delivery";
type Plan = { steps: { stage: Stage; review_required: boolean }[] };
type StageResult = MotionStageResult & { supplemental_review?: SupplementalReview; preview_review?: PreviewChoice[] };
type StageProgress = { stage_index: number; pending_review_id: number | null; results: StageResult[]; automatic_subtitles?: AutomaticSubtitles };
type Decision = "approved" | "revise" | "rejected";
type PreviewChoice = { format_id: number; decision: Decision; note?: string };
type SupplementalChoices<T = Decision> = { text: T[]; media: T[] };
type SupplementalReview = SupplementalChoices & { item_notes?: SupplementalChoices<string | null> };
type ReviewDraft = { decision: Decision | ""; notes: string; motionIndexes?: number[]; motion?: Record<number, Decision>; motionNotes?: Record<number, string>; supplemental?: SupplementalChoices<Decision | "">; itemNotes?: SupplementalChoices<string> };

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
  const reduceMotion = useReducedMotionConfig();
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
  const stageName = (stage: Stage) => copy[stage].toLowerCase();
  function agentCaption(stage: Stage) {
    const summary = awaiting?.result.summary;
    if (awaiting) return typeof summary === "string" && summary.trim() ? summary : copy.captionReview.replace("{stage}", stageName(stage));
    if (runState === "completed") return copy.captionDone;
    if (stage === "verify_delivery" && runState === "queued") {
      const automatic = Object.values(progress?.automatic_subtitles ?? {});
      return automatic.includes("started") ? copy.captionSubtitling : copy.captionNeedsSubtitles;
    }
    if (runState === "paused") return copy.captionPaused.replace("{stage}", stageName(stage));
    return (runState === "running" ? copy.captionRunning : copy.captionQueued).replace("{stage}", stageName(stage));
  }
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
                    ? "border-white/85 bg-white/85" : current ? (runState === "paused"
                      ? "border-white/85 bg-[repeating-linear-gradient(135deg,rgb(255_255_255_/_30%)_0,rgb(255_255_255_/_30%)_4px,transparent_4px,transparent_9px)]"
                      : "border-transparent bg-attention") : "border-white/35")} aria-hidden="true" />
                  <span className={cn("min-w-0 px-1 text-xs font-bold wrap-anywhere",
                    current && runState !== "paused" ? "text-attention" : "text-white/85")}>{copy[step.stage]}</span>
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
      {(currentStage || runState === "completed") && <div className="flex items-center gap-3 py-2 text-white">
        <PluginIcon pluginId={agentId} tone="dark" className="size-11" />
        <div className="flex min-w-0 flex-col items-start gap-1" role="status">
          <p className={cn("flex flex-wrap gap-x-2 text-sm font-bold", awaiting ? "text-attention" : "text-white/80")}>
            {awaiting ? copy.waitingFor.replace("{agent}", agent ?? copy.agentFallback) : <>{agent && <span>{agent}</span>}<span>{queue[runState]}</span></>}
          </p>
          <p className="max-w-[60ch] rounded-thumb bg-caption px-2.5 py-0.5 text-[clamp(1.125rem,2vw,1.4375rem)] leading-[1.35] font-bold text-balance wrap-anywhere">
            {currentStage ? agentCaption(currentStage) : copy.captionDone}
          </p>
        </div>
      </div>}
      <div className="@container">
        <div className="grid items-start gap-5 @min-[52rem]:grid-cols-[minmax(0,1fr)_17rem]">
          <div className="flex min-w-0 flex-col gap-4">
            {runState === "running" && currentStage && !awaiting && (
              <AgentProgress workspaceId={workspaceId} runId={runId} agent={agent ?? copy.agentFallback} stage={stageName(currentStage)} />
            )}
            {runState === "completed" && (
              <Card size="sm">
                <CardHeader>
                  <CardTitle className="font-heading text-lg font-[850]">{copy.finishedTitle}</CardTitle>
                  <CardDescription>{copy.finishedDescription}</CardDescription>
                </CardHeader>
                <CardContent>
                  <Button asChild><Link href={completedVideoHref(workspaceId, runId)}>{copy.openFinished}<ArrowUpRight data-icon="inline-end" /></Link></Button>
                </CardContent>
              </Card>
            )}
            {currentStage === "assets" && !awaiting && runState !== "completed" && runState !== "cancelled" && runState !== "running" && (
              <Card size="sm">
                <CardHeader><CardTitle className="font-heading text-lg font-[850]">{copy.assets}</CardTitle></CardHeader>
                <CardContent className="flex flex-col items-start gap-3">
                  <MotionArtifactAction workspaceId={workspaceId} runId={runId} available={runState === "queued"} />
                  <MotionRendererAction workspaceId={workspaceId} runId={runId} available={runState === "queued"} />
                </CardContent>
              </Card>
            )}
            {mediaStage && !awaiting && runState !== "completed" && runState !== "cancelled" && runState !== "running" && (
              <Card size="sm">
                <CardHeader><CardTitle className="font-heading text-lg font-[850]">{copy[mediaStage]}</CardTitle></CardHeader>
                <CardContent>
                  <MediaStageAction key={mediaStage} workspaceId={workspaceId} runId={runId} stage={mediaStage}
                    available={runState === "queued"} canImportSubtitles={runState === "queued" || runState === "failed"} formats={formats}
                    automaticSubtitles={progress?.automatic_subtitles} />
                </CardContent>
              </Card>
            )}
            {progress && plan && <RunMotionResults workspaceId={workspaceId} runId={runId}
              results={progress.results} steps={plan.steps} stageIndex={progress.stage_index} />}
            {awaiting && currentStage && (
              <StageReview key={awaiting.id} workspaceId={workspaceId} runId={runId} stage={currentStage} result={awaiting} formats={formats}
                agent={agent} version={new Set(progress?.results.filter((entry) => entry.stage_index === awaiting.stage_index && entry.id <= awaiting.id)
                  .map((entry) => JSON.stringify(entry.result))).size || 1}
                reviewing={reviewing} reviewError={reviewError} onReview={review} saveMotionDefault={saveMotionDefault}
                previousSupplementalReview={previousReview?.supplemental_review} previousPreviewReview={previousReview?.preview_review} />
            )}
            <PlacedPluginActions workspaceId={workspaceId} placement="generation" />
          </div>
          {activity}
        </div>
      </div>
    </div>
  );
}

function StageReview({ workspaceId, runId, stage, result, formats, agent, version, reviewing, reviewError, onReview, saveMotionDefault, previousSupplementalReview, previousPreviewReview }: {
  workspaceId: number;
  agent?: string;
  version: number;
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
          thumbnail: "thumbnail_asset_id" in item && typeof item.thumbnail_asset_id === "number" ? item.thumbnail_asset_id : null,
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
          motion: saved.motion && typeof saved.motion === "object"
            ? Object.fromEntries(Object.entries(saved.motion).filter(([, value]) => isDecision(value))) as Record<number, Decision> : undefined,
          motionNotes: saved.motionNotes && typeof saved.motionNotes === "object"
            ? Object.fromEntries(Object.entries(saved.motionNotes).filter(([, value]) => typeof value === "string")) as Record<number, string> : undefined,
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
  const motionReview = !hasItemReview && motionItems.length > 0;
  const motionStaged = motionItems.filter(({ index }) => draft.motion?.[index]).length;
  const footerItems = hasItemReview || (motionReview && !draft.decision);
  const stagedCount = motionReview ? motionStaged : staged;
  const undecided = hasItemReview ? textCount + mediaCount - staged : motionReview ? motionItems.length - motionStaged : 0;
  function approveRest() {
    if (motionReview) {
      updateDraft({ ...draft, motion: Object.fromEntries(motionItems.map(({ index }) => [index, draft.motion?.[index] ?? "approved"])) });
      return;
    }
    setDraft((current) => current.supplemental ? {
      ...current,
      supplemental: {
        text: current.supplemental.text.map((value) => value || "approved"),
        media: current.supplemental.media.map((value) => value || "approved"),
      },
    } : current);
  }
  const canSend = hasItemReview ? staged === textCount + mediaCount : motionReview ? Boolean(draft.decision) || motionStaged === motionItems.length : Boolean(draft.decision);
  const source = Object.fromEntries(Object.entries(result.result).filter(([key]) =>
    key !== "markdown" && key !== "summary" && key !== "sections" && key !== "agent" && key !== "footage_without_transcript" && key !== "thumbnail_prompts"
      && !(stage === "assets" && key === "asset_plan" && isAssetPlan(result.result.asset_plan))
      && !(previews.length > 0 && isCutList(result.result.cuts) && (key === "cuts" || key === "cut_notes")) && (key !== "preview_videos" || previews.length === 0)
    && (key !== "motion_items" || motionItems.length === 0)
    && (!hasSupplemental || (key !== "supplemental_text" && key !== "supplemental_media"))));

  function updateDraft(next: ReviewDraft) {
    setDraft(next);
    try { sessionStorage.setItem(draftKey, JSON.stringify(next)); } catch {}
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!canSend || reviewing) return;
    let decision = draft.decision;
    let notes = draft.notes;
    if (motionReview && !draft.decision) {
      const labels = { approved: copy.item_approved, revise: copy.item_revise, rejected: copy.item_rejected };
      const lines = motionItems.map(({ index, title }) => {
        const note = draft.motionNotes?.[index]?.trim();
        return `- ${title}: ${labels[draft.motion![index]]}${note ? `. ${note}` : ""}`;
      });
      decision = motionItems.every(({ index }) => draft.motion?.[index] === "approved") && !draft.notes.trim() ? "approved" : "revise";
      notes = [decision === "revise" || draft.motionNotes && Object.values(draft.motionNotes).some((note) => note.trim())
        ? `${copy.motionDecisions}\n${lines.join("\n")}` : "", draft.notes.trim()].filter(Boolean).join("\n\n");
    }
    if (!decision) return;
    if (await onReview(result.id, decision, notes,
      decision === "approved" && motionItems.length ? selectedMotion : undefined,
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

  if (stage === "script" && isScriptSections(result.result.sections)) {
    const sections = result.result.sections;
    return (
      <Card className="overflow-visible rounded-[14px]">
        <CardHeader className="border-b">
          <CardTitle className="font-heading text-[1.5rem] font-[850] [font-variation-settings:'wdth'_112]">
            {copy.draftTitle.replace("{stage}", copy[stage]).replace("{version}", String(version))}
          </CardTitle>
          <CardDescription>{copy.reviewHint}</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4 pb-4">
          <NoTranscriptNote workspaceId={workspaceId} clips={result.result.footage_without_transcript} />
          <ScriptSectionReview storageKey={draftKey} sections={sections} agent={agent} reviewing={reviewing} reviewError={reviewError}
            onSend={(decision, notes) => onReview(result.id, decision, notes)} />
        </CardContent>
      </Card>
    );
  }

  return (
    <Card className="overflow-visible rounded-[14px]">
      <CardHeader className="border-b">
        <CardTitle className="font-heading text-[1.5rem] font-[850] [font-variation-settings:'wdth'_112]">
          {copy.draftTitle.replace("{stage}", copy[stage]).replace("{version}", String(version))}
        </CardTitle>
        <CardDescription>{copy.reviewHint}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <NoTranscriptNote workspaceId={workspaceId} clips={result.result.footage_without_transcript} />
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
            prompts={result.result.thumbnail_prompts}
            controls={reviewControls} />}
        </>}
        {motionItems.length > 0 && (
          <ul className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            {motionItems.map(({ index, title, usage, kind, thumbnail }) => (
              <li key={index} className={cn("min-w-0 overflow-hidden rounded-lg border", draft.motion?.[index] && "border-line-strong")}>
                {thumbnail !== null
                  ? <AssetThumbnail workspaceId={workspaceId} assetId={thumbnail} kind="image" className="w-full rounded-none bg-ink/90" />
                  : <div className="flex aspect-video items-center justify-center bg-secondary">
                    <Clapperboard className="size-6 text-primary" aria-hidden="true" />
                  </div>}
                <div className="flex flex-col items-start gap-2 px-3 py-3">
                  <p className="font-semibold wrap-anywhere">{title}</p>
                  <span className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
                    <span className="rounded-md border px-2 py-0.5 text-foreground">{motionLibrary[kind]}</span>
                    {usage && <span className="wrap-anywhere">{usage}</span>}
                  </span>
                  <MotionPreviewDialog workspaceId={workspaceId} runId={runId} resultId={result.id} index={index} title={title} />
                  <div className="flex w-full flex-wrap items-center justify-between gap-2 border-t pt-2">
                    <label className="flex items-center gap-2 text-sm">
                      <Checkbox checked={selectedMotion.includes(index)} disabled={reviewing} onCheckedChange={(checked) => {
                        updateDraft({ ...draft, motionIndexes: checked
                          ? [...selectedMotion, index]
                          : selectedMotion.filter((value) => value !== index) });
                      }} />
                      {copy.saveToLibrary}
                    </label>
                    {motionReview && <div className="ms-auto flex items-center gap-1.5" role="group" aria-label={copy.decisionFor.replace("{label}", title)}>
                      {decisions.map(({ value, icon: Icon, fill }) => {
                        const active = draft.motion?.[index] === value;
                        return <Button key={value} type="button" size="icon" variant="outline" disabled={reviewing} aria-pressed={active}
                          aria-label={copy[`item_${value}`]} title={copy[`item_${value}`]} className={cn("size-8", active && fill)}
                          onClick={() => {
                            const motion = { ...draft.motion };
                            if (active) delete motion[index];
                            else motion[index] = value;
                            updateDraft({ ...draft, motion });
                          }}><Icon /></Button>;
                      })}
                    </div>}
                  </div>
                  {motionReview && draft.motion?.[index] && draft.motion[index] !== "approved" && (
                    <Textarea aria-label={copy.itemNoteFor.replace("{label}", title)} placeholder={copy.motionNotePlaceholder} maxLength={2000}
                      disabled={reviewing} value={draft.motionNotes?.[index] ?? ""} className="min-h-16 text-sm"
                      onChange={(event) => updateDraft({ ...draft, motionNotes: { ...draft.motionNotes, [index]: event.target.value } })} />
                  )}
                </div>
              </li>
            ))}
          </ul>
        )}
        {previews.length > 0 && <div className={cn("grid items-start gap-4", previews.length > 1 && "md:grid-cols-2")}>{previews.map(({ filename, formatId }, index) => (
          <div key={filename} className="grid min-w-0 gap-2">
            <GuidedPreview workspaceId={workspaceId} runId={runId}
              resultId={result.id} filename={filename} formatId={formatId} initiallyOpen={index === 0}
              label={formats.find((format) => format.id === formatId)?.name ?? filename} />
            {hasPreviewReview && reviewControls("media", index, formats.find((format) => format.id === formatId)?.name ?? filename)}
          </div>
        ))}</div>}
        {stage === "assets" && <AssetPlan plan={result.result.asset_plan} agent={agent} />}
        {previews.length > 0 && <CutList cuts={result.result.cuts} notes={result.result.cut_notes} agent={agent} />}
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
                {motionReview && <FieldDescription>{copy.wholeStepHint}</FieldDescription>}
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
              {reviewError && <Alert variant="destructive"><AlertDescription>{copy.reviewError}</AlertDescription></Alert>}
            </FieldGroup>
          </fieldset>
        </form>
      </CardContent>
      <div className="sticky bottom-4 z-[1] flex flex-wrap mx-4 items-center justify-between gap-3 rounded-lg bg-ink px-3 py-2 text-white">
        <p className={cn("flex flex-wrap items-center gap-2 text-sm", (draft.decision || stagedCount > 0 || draft.notes || hasItemNotes) && "font-bold text-attention")} role="status">
          {draft.decision && <span aria-hidden="true"
            className={cn("size-2 rounded-xs", decisions.find((option) => option.value === draft.decision)?.marker)} />}
          {!footerItems && draft.decision ? copy.stagedOne.replace("{agent}", agent ?? copy.agentFallback)
            : footerItems && stagedCount > 0 && undecided > 0 ? <>{(stagedCount === 1 ? copy.stagedItemsOne : copy.stagedItems).replace("{count}", String(stagedCount))}
              <span className="font-normal text-white">{(undecided === 1 ? copy.stillNeedsOne : copy.stillNeeds).replace("{count}", String(undecided))}</span></>
            : draft.decision || stagedCount > 0 || draft.notes || hasItemNotes ? copy.unsentReview : footerItems ? (motionReview ? copy.selectEveryAnimation : copy.selectEveryItem) : copy.selectDecision}
        </p>
        {footerItems && stagedCount > 0 && undecided > 0 && <Button type="button" variant="outline" disabled={reviewing} onClick={approveRest}
          className="ms-auto border-white/40 bg-transparent text-white hover:bg-white/10 hover:text-white">{copy.approveRest}</Button>}
        <Button type="submit" form={reviewFormId} disabled={!canSend || reviewing}
          className={cn("bg-attention text-ink hover:bg-[oklch(90%_.13_86)] disabled:bg-white/15 disabled:text-white/70 disabled:opacity-100",
            !(footerItems && stagedCount > 0 && undecided > 0) && "ms-auto")}>
          {footerItems ? (stagedCount === 0 ? copy.sendNothing : stagedCount === 1 ? copy.submitDecision : copy.submitDecisions.replace("{count}", String(stagedCount)))
            : draft.decision ? copy.submitDecision : copy.sendNothing}
        </Button>
      </div>
    </Card>
  );
}

type Placement = { section: string; name: string; kind: string; use: string };

function isAssetPlan(value: unknown): value is Placement[] {
  return Array.isArray(value) && value.every((entry) => entry && typeof entry.name === "string" && typeof entry.kind === "string");
}

const placementIcons: Record<string, typeof Film> = { image: ImageIcon, video: Film, audio: Music };

function AssetPlan({ plan, agent }: { plan: unknown; agent?: string }) {
  const { messages: { productionPlan: copy } } = useLocale();
  if (!isAssetPlan(plan)) return null;
  if (plan.length === 0) return <p className="rounded-lg bg-secondary px-3 py-2.5 text-sm">{copy.assetPlanEmpty}</p>;
  const sections = [...new Set(plan.map((entry) => entry.section || copy.assetPlanAnywhere))];
  return (
    <section aria-labelledby="asset-plan" className="flex flex-col gap-3">
      <h3 id="asset-plan" className="font-bold">{agent ? copy.assetPlanTitle.replace("{agent}", agent) : copy.assetPlanTitleFallback}</h3>
      {sections.map((section) => (
        <div key={section} className="flex flex-col gap-1.5">
          <p className="text-sm font-semibold text-muted-foreground">{section}</p>
          <ul className="flex flex-col divide-y rounded-lg border">
            {plan.filter((entry) => (entry.section || copy.assetPlanAnywhere) === section).map((entry, index) => {
              const Icon = placementIcons[entry.kind] ?? Sparkles;
              return (
                <li key={index} className="flex items-start gap-3 px-3 py-2 text-sm">
                  <Icon className="mt-0.5 size-4 shrink-0 text-primary" aria-hidden="true" />
                  <span className="min-w-0 flex-1"><span className="font-semibold wrap-anywhere">{entry.name}</span>
                    {entry.use && <span className="block text-muted-foreground wrap-anywhere">{entry.use}</span>}</span>
                </li>
              );
            })}
          </ul>
        </div>
      ))}
    </section>
  );
}

type Cut = { asset_id: number; start_ms: number; end_ms: number };

function isCutList(value: unknown): value is Cut[] {
  return Array.isArray(value) && value.length > 0 && value.every((cut) =>
    cut && typeof cut.start_ms === "number" && typeof cut.end_ms === "number");
}

function clock(ms: number) {
  const seconds = ms / 1000;
  return `${Math.floor(seconds / 60)}:${(seconds % 60).toFixed(1).padStart(4, "0")}`;
}

function CutList({ cuts, notes, agent }: { cuts: unknown; notes: unknown; agent?: string }) {
  const { messages: { productionPlan: copy } } = useLocale();
  if (!isCutList(cuts)) return null;
  const details = Array.isArray(notes) ? notes : [];
  const total = cuts.reduce((sum, cut) => sum + cut.end_ms - cut.start_ms, 0);
  return (
    <section aria-labelledby="cut-list" className="flex flex-col gap-2">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h3 id="cut-list" className="font-bold">{agent ? copy.cutsTitle.replace("{agent}", agent) : copy.cutsTitleFallback}</h3>
        <p className="text-sm text-muted-foreground tabular-nums">
          {copy.cutsSummary.replace("{count}", String(cuts.length)).replace("{duration}", clock(total))}</p>
      </div>
      <ol className="flex flex-col divide-y rounded-lg border">
        {cuts.map((cut, index) => {
          const detail = details[index] && typeof details[index] === "object" ? details[index] : {};
          return (
            <li key={index} className="grid gap-x-4 gap-y-0.5 px-3 py-2 text-sm sm:grid-cols-[2rem_minmax(0,12rem)_8rem_minmax(0,1fr)]">
              <span className="font-bold tabular-nums text-muted-foreground">{index + 1}</span>
              <span className="truncate font-semibold" title={detail.clip}>{detail.clip ?? `#${cut.asset_id}`}</span>
              <span className="tabular-nums">{clock(cut.start_ms)}–{clock(cut.end_ms)}</span>
              <span className="min-w-0 wrap-anywhere text-muted-foreground">{detail.purpose}</span>
            </li>
          );
        })}
      </ol>
    </section>
  );
}

function NoTranscriptNote({ workspaceId, clips }: { workspaceId: number; clips: unknown }) {
  const { messages: { productionPlan: copy } } = useLocale();
  if (!Array.isArray(clips) || clips.length === 0) return null;
  return (
    <div role="note" className="flex flex-wrap items-start gap-x-3 gap-y-1 rounded-lg bg-attention/25 px-3 py-2.5 text-sm">
      <TriangleAlert className="mt-0.5 size-4 shrink-0" aria-hidden="true" />
      <p className="min-w-0 flex-1">{copy.noTranscript.replace("{clips}", clips.join(", "))}</p>
      <Link href={`/settings?workspace=${workspaceId}&section=services`} className="font-semibold underline underline-offset-4">{copy.addKey}</Link>
    </div>
  );
}

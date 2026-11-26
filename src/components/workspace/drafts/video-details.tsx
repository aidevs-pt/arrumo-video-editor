"use client";

import { PlacedPluginActions } from "@/components/workspace/plugins/placed-actions";
import { invoke } from "@tauri-apps/api/core";
import { ChevronDown, Zap } from "lucide-react";
import Link from "next/link";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Field, FieldDescription, FieldGroup, FieldLabel, FieldContent } from "@/components/ui/field";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Switch } from "@/components/ui/switch";
import { TagInput } from "@/components/ui/tag-input";
import { outputOptions, VideoOutputs, type OutputChoice, type OutputPlan } from "./video-outputs";
import { MarkdownEditor } from "@/components/markdown/editor";
import { RunHooks } from "@/components/workspace/generation/run-hooks";
import { RunProviders } from "@/components/workspace/generation/run-providers";
import { UsageNotice } from "@/components/workspace/usage/provider-usage";
import { profileHref, workspaceHref } from "@/components/workspace/home/workspace-links";
import { LayerGauge } from "@/components/workspace/layer-gauge";
import { useLocale } from "@/lib/locale-provider";
import { useAgentChanges } from "@/lib/agent-changes";

export type DraftSettings = { title: string; intent: string; profile_id: number | null; source_ids: number[] };
export type SavedDraft = DraftSettings & { id: number };
type Profile = { id: number; name: string; color?: string | null };
type Format = { id: number; name: string; width: number; height: number };
type Supplemental = OutputPlan["supplemental"];
type Plan = OutputPlan;

/** Loads what a run would deliver with the draft's current profile, resolved the same way the queue snapshots it. */
function useDeliveryPlan(workspaceId: number, profileId: number | null) {
  const [plan, setPlan] = useState<Plan[]>();
  useEffect(() => {
    let active = true;
    (async () => {
      const formats = profileId === null
        ? (await invoke<(Format & { enabled: boolean })[]>("list_output_formats", { workspaceId })).filter((format) => format.enabled)
        : await invoke<Format[]>("list_profile_formats", { workspaceId, profileId });
      const deliveries = await Promise.all(formats.map((format) => profileId === null
        ? invoke<{ supplemental: Supplemental }>("get_format_delivery", { workspaceId, formatId: format.id })
        : invoke<{ effective: { supplemental: Supplemental } }>("get_profile_delivery", { workspaceId, profileId, formatId: format.id })
          .then((delivery) => delivery.effective)));
      if (active) setPlan(formats.map((format, index) => ({ format, supplemental: deliveries[index].supplemental })));
    })().catch(() => { if (active) setPlan([]); });
    return () => { active = false; };
  }, [profileId, workspaceId]);
  return plan;
}

export function VideoDetails({ workspace, draftId, draft, profiles, onChange, onSaved, onGenerated, onBack }: {
  workspace: { id: number; name: string };
  draftId: number | null;
  draft: DraftSettings;
  profiles: Profile[];
  onChange: (change: Partial<DraftSettings>) => void;
  onSaved: (draft: SavedDraft) => void;
  onGenerated: (next: SavedDraft | null, runId: number) => void;
  onBack: () => void;
}) {
  const { messages } = useLocale();
  const copy = messages.videoDetails;
  const workspaceId = workspace.id;
  const request = useRef<{ key: string; payload: string } | null>(null);
  const [pending, setPending] = useState<"saving" | "generating" | null>(null);
  const [message, setMessage] = useState<"saved" | "saveError" | "generateError" | "needsTitle" | "needsContent" | null>(null);
  const [agent, setAgent] = useState<string | null>(null);
  const [platforms, setPlatforms] = useState<string[]>([]);
  const [outputChoice, setOutputChoice] = useState<OutputChoice>({ skipped: [], overrides: {} });
  const [queue, setQueue] = useState<SavedDraft[]>([]);
  const plan = useDeliveryPlan(workspaceId, draft.profile_id);
  const [images, setImages] = useState<{ id: number; name: string }[]>([]);
  useEffect(() => {
    invoke<{ id: number; name: string; kind: string }[]>("list_assets", { workspaceId })
      .then((assets) => setImages(assets.filter((asset) => asset.kind === "image")), () => setImages([]));
  }, [workspaceId]);
  const profile = profiles.find((entry) => entry.id === draft.profile_id);
  const position = draftId === null ? -1 : queue.findIndex((entry) => entry.id === draftId);
  const next = queue.find((entry, index) => entry.id !== draftId && (position === -1 || index > position)) ?? null;

  const agentChanges = useAgentChanges();
  useEffect(() => {
    let active = true;
    Promise.all([
      invoke<SavedDraft[]>("list_video_drafts", { workspaceId }),
      invoke<{ draft_id: number }[]>("list_generation_runs", { workspaceId }),
      Promise.all([
        invoke<{ effective_provider: string | null }>("get_agent_settings", { workspaceId }),
        invoke<{ id: string; label: string }[]>("list_agent_plugins"),
      ]).then(([settings, plugins]) => plugins.find(({ id }) => id === settings.effective_provider)?.label ?? null, () => null),
    ]).then(([drafts, runs, label]) => {
      if (!active) return;
      const submitted = new Set(runs.map((run) => run.draft_id));
      setQueue(drafts.filter((entry) => !submitted.has(entry.id)));
      setAgent(label);
    }, () => {});
    return () => { active = false; };
  }, [workspaceId, draftId, agentChanges]);

  async function save(): Promise<SavedDraft | null> {
    if (!draft.title.trim()) { setMessage("needsTitle"); return null; }
    if (!draft.source_ids.length && !draft.intent.trim()) { setMessage("needsContent"); return null; }
    try {
      const saved = await invoke<SavedDraft>(draftId === null ? "create_video_draft" : "update_video_draft", {
        workspaceId, ...(draftId !== null && { draftId }), settings: { ...draft, title: draft.title.trim() },
      });
      onSaved(saved);
      return saved;
    } catch {
      setMessage("saveError");
      return null;
    }
  }

  async function generate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending) return;
    const fields = new FormData(event.currentTarget);
    setPending("generating");
    setMessage(null);
    const saved = await save();
    if (!saved) { setPending(null); return; }
    const options = {
      autopilot: fields.has("autopilot"),
      generate_delivery_subtitles: fields.has("generate-subtitles"),
      save_motion_to_library: fields.has("save-motion"),
      platform: String(fields.get("platform") ?? "").trim() || null,
      provider_overrides: JSON.parse(String(fields.get("provider-overrides") ?? "{}")) as Record<string, string[]>,
      skip_hooks: JSON.parse(String(fields.get("skip-hooks") ?? "[]")) as string[],
      ...outputOptions(outputChoice, plan ?? []),
    };
    const payload = JSON.stringify({ draftId: saved.id, draft, options });
    if (request.current?.payload !== payload) request.current = { key: crypto.randomUUID(), payload };
    try {
      const run = await invoke<{ id: number }>("enqueue_video_draft", { workspaceId, draftId: saved.id, requestKey: request.current.key, options });
      request.current = null;
      onGenerated(next, run.id);
    } catch {
      setMessage("generateError");
    } finally {
      setPending(null);
    }
  }

  async function saveAndStop() {
    if (pending) return;
    setPending("saving");
    setMessage(null);
    if (await save()) setMessage("saved");
    setPending(null);
  }

  const agentName = agent ?? copy.willMakeFallback;
  const clips = draft.source_ids.length;

  return (
    <form onSubmit={generate} aria-busy={pending !== null}>
      <fieldset disabled={pending !== null} className="min-w-0">
        <div className="mb-8 flex flex-wrap items-end justify-between gap-4">
          <div className="min-w-0 flex-1">
            {position >= 0 && queue.length > 1 && <p className="mb-1 text-sm text-muted-foreground">
              {copy.position.replace("{index}", String(position + 1)).replace("{total}", String(queue.length))}
            </p>}
            <label htmlFor="video-title" className="sr-only">{copy.title}</label>
            <input id="video-title" value={draft.title} maxLength={200} required placeholder={copy.title}
              onChange={(event) => onChange({ title: event.target.value })}
              className="w-full rounded-md bg-transparent font-heading text-[clamp(2rem,4.5vw,3rem)] leading-[1.05] font-[850] outline-none [font-variation-settings:'wdth'_130] placeholder:text-muted-foreground/50 focus-visible:bg-card focus-visible:ring-2 focus-visible:ring-ring" />
            <div className="mt-2 flex flex-wrap items-center gap-3 text-sm">
              <span aria-hidden="true" className="size-3 rounded-[3px]" style={{ backgroundColor: profile?.color ?? "var(--border)" }} />
              <NativeSelect aria-label={messages.drafts.profile} value={draft.profile_id ?? ""} size="sm" className="font-semibold"
                onChange={(event) => onChange({ profile_id: event.target.value ? Number(event.target.value) : null })}>
                <NativeSelectOption value="">{messages.drafts.noProfile}</NativeSelectOption>
                {profiles.map((entry) => <NativeSelectOption key={entry.id} value={entry.id}>{entry.name}</NativeSelectOption>)}
              </NativeSelect>
              <span className="text-muted-foreground">
                {clips === 0 ? messages.drafts.intentOnly : clips === 1 ? messages.videoCatalog.singleClip : messages.videoCatalog.clipCount.replace("{count}", String(clips))}
              </span>
            </div>
          </div>
          {next && <p className="flex items-center gap-2 text-sm text-muted-foreground">{copy.next}
            <span className="rounded-md bg-secondary px-2.5 py-1 font-semibold text-foreground">{next.title}</span></p>}
        </div>
        <div className="grid items-start gap-8 lg:grid-cols-[minmax(0,1fr)_20rem]">
          <FieldGroup className="min-w-0">
            <Field>
              <FieldLabel htmlFor="draft-intent" className="text-base">{copy.intent}</FieldLabel>
              <MarkdownEditor id="draft-intent" label={copy.intent} value={draft.intent} onChange={(intent) => onChange({ intent })} />
              <FieldDescription>{profile
                ? copy.intentHint.replace("{profile}", profile.name).replace("{workspace}", workspace.name)
                : copy.intentHintWorkspace.replace("{workspace}", workspace.name)}</FieldDescription>
            </Field>
            <Field className="max-w-md">
              <div className="flex items-center justify-between gap-2">
                <FieldLabel htmlFor="run-platform" className="text-base">{copy.publishTo}</FieldLabel>
                <LayerGauge layer="video" />
              </div>
              <TagInput id="run-platform" name="platform" value={platforms} onChange={setPlatforms}
                placeholder={copy.publishPlaceholder} removeLabel={copy.removePlatform}
                suggestions={["YouTube", "YouTube Shorts", "TikTok", "Instagram Reels", "LinkedIn", "X", "Facebook"]} />
              <FieldDescription>{copy.publishHint}</FieldDescription>
            </Field>
            <Collapsible className="rounded-xl border bg-card">
              <CollapsibleTrigger className="group flex w-full items-center justify-between gap-3 px-5 py-4 text-start font-semibold">
                {copy.moreOptions}
                <ChevronDown aria-hidden="true" className="size-4 transition-transform duration-150 group-data-[state=open]:rotate-180" />
              </CollapsibleTrigger>
              <CollapsibleContent forceMount className="flex flex-col gap-5 border-t px-5 py-4 data-[state=closed]:hidden">
                <Field orientation="horizontal">
                  <Switch id="run-generate-subtitles" name="generate-subtitles" defaultChecked aria-describedby="run-generate-subtitles-description" />
                  <FieldContent>
                    <FieldLabel htmlFor="run-generate-subtitles">{messages.queue.automaticSubtitles}</FieldLabel>
                    <FieldDescription id="run-generate-subtitles-description">{messages.queue.automaticSubtitlesHint}</FieldDescription>
                  </FieldContent>
                </Field>
                <Field orientation="horizontal">
                  <Switch id="run-save-motion" name="save-motion" defaultChecked aria-describedby="run-save-motion-description" />
                  <FieldContent>
                    <FieldLabel htmlFor="run-save-motion">{messages.queue.saveMotion}</FieldLabel>
                    <FieldDescription id="run-save-motion-description">{messages.queue.saveMotionHint}</FieldDescription>
                  </FieldContent>
                </Field>
                <RunHooks workspaceId={workspaceId} disabled={pending !== null} />
                <RunProviders workspaceId={workspaceId} disabled={pending !== null} />
              </CollapsibleContent>
            </Collapsible>
          </FieldGroup>
          <aside className="flex flex-col gap-4 lg:sticky lg:top-20">
            <section className="rounded-xl border bg-card p-5" aria-labelledby="video-makes">
              <h2 id="video-makes" className="mb-3 text-[1.0625rem] font-bold [font-family:var(--font-sans)] [font-variation-settings:normal]">
                {agent ? copy.willMake.replace("{agent}", agent) : copy.willMakeFallback}
              </h2>
              <VideoOutputs workspaceId={workspaceId} images={images} plan={plan ?? []} choice={outputChoice} onChange={setOutputChoice} disabled={pending !== null} />
              <Link href={profile ? `${profileHref(workspaceId, profile.id)}&tab=outputs` : workspaceHref("outputs", workspaceId)}
                className="mt-4 inline-block font-semibold text-primary underline underline-offset-4">
                {copy.changeOutputs.replace("{name}", profile?.name ?? workspace.name)}
              </Link>
            </section>
            <section className="rounded-xl border bg-card p-5">
              <div className="flex items-center justify-between gap-3">
                <label htmlFor="run-autopilot" className="flex items-center gap-2 font-bold">
                  <Zap className="size-4 text-primary" aria-hidden="true" />{messages.queue.autopilot}
                </label>
                <Switch id="run-autopilot" name="autopilot" aria-describedby="run-autopilot-description" />
              </div>
              <p id="run-autopilot-description" className="mt-2 text-sm text-muted-foreground">{copy.autopilotHint.replace("{agent}", agentName)}</p>
            </section>
            <PlacedPluginActions workspaceId={workspaceId} placement="draft" />
            <UsageNotice workspaceId={workspaceId} />
            {message && <Alert variant={message === "saved" ? "default" : "destructive"}><AlertDescription role="status">{copy[message]}</AlertDescription></Alert>}
            <Button type="submit" size="lg" className="h-12 text-base">{pending === "generating" ? copy.generating : copy.generate}</Button>
            {next && <p className="text-center text-sm text-muted-foreground">{copy.thenNext.replace("{title}", next.title)}</p>}
            <Button type="button" variant="ghost" onClick={() => void saveAndStop()}>{copy.saveStop}</Button>
            <Button type="button" variant="link" className="self-center" onClick={onBack}>{clips ? copy.backToFootage : copy.backToBrief}</Button>
          </aside>
        </div>
      </fieldset>
    </form>
  );
}

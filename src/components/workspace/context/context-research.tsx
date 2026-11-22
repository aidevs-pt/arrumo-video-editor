"use client";

import { invoke } from "@tauri-apps/api/core";
import { Check, ChevronDown, Image as ImageIcon, Link2, LoaderCircle, Plus, Sparkles, TriangleAlert, X } from "lucide-react";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { useLocale } from "@/lib/locale-provider";

const MAX_SOURCES = 8;

type Problem = "invalid_address" | "not_public" | "not_found" | "unreachable" | "timed_out" | "http_error" | "not_a_page" | "no_text" | "too_many_redirects";
type Profile = { name: string; description: string; guidance: string; color: string | null };
type FoundImage = { url: string; name: string; description: string; group: string | null; labels: string[]; purpose: "brand" | "thumbnail_reference" | "asset" };
type Draft = {
  markdown: string;
  summary: string;
  profiles: Profile[];
  colors: string[];
  images: FoundImage[];
  unread: { source: string; reason: Problem }[];
};
type Outcome = { colors: number; images: number; profiles: number; failedImages: string[] };
type Failure = "agent_unavailable" | "agent_timed_out" | "agent_failed" | "context_draft_failed";

export function ContextResearch({ workspaceId, startOpen, onDraft }: {
  workspaceId: number;
  startOpen: boolean;
  onDraft: (markdown: string) => void;
}) {
  const { messages: { contextResearch: copy } } = useLocale();
  const [open, setOpen] = useState(startOpen);
  const [sources, setSources] = useState([""]);
  const [notes, setNotes] = useState("");
  const [pending, setPending] = useState(false);
  const [failure, setFailure] = useState<Failure>();
  const [draft, setDraft] = useState<Draft>();
  const [agent, setAgent] = useState<string>();

  useEffect(() => {
    let active = true;
    Promise.all([
      invoke<{ effective_provider: string | null }>("get_agent_settings", { workspaceId }),
      invoke<{ id: string; label: string }[]>("list_agent_plugins"),
    ]).then(([settings, plugins]) => {
      if (active) setAgent(plugins.find(({ id }) => id === settings.effective_provider)?.label);
    }, () => {});
    return () => { active = false; };
  }, [workspaceId]);

  const filled = sources.map((source) => source.trim()).filter(Boolean);
  const agentName = agent ?? copy.agentFallback;

  async function research() {
    if (pending || (!filled.length && !notes.trim())) return;
    setPending(true);
    setFailure(undefined);
    setDraft(undefined);
    try {
      const result = await invoke<Draft>("draft_workspace_context", { workspaceId, sources: filled, notes });
      setDraft(result);
      if (result.markdown) onDraft(result.markdown);
    } catch (error) {
      setFailure(typeof error === "string" && error in copy.failures ? error as Failure : "context_draft_failed");
    } finally {
      setPending(false);
    }
  }

  return (
    <Collapsible open={open} onOpenChange={setOpen} className="rounded-xl border bg-card">
      <CollapsibleTrigger asChild>
        <button type="button" className="group flex w-full items-center gap-3 p-4 text-start sm:p-5">
          <span className="grid size-9 shrink-0 place-items-center rounded-md bg-attention text-ink" aria-hidden="true"><Sparkles className="size-4" /></span>
          <span className="min-w-0 flex-1">
            <span className="block font-bold">{copy.title.replace("{agent}", agentName)}</span>
            <span className="block text-sm text-muted-foreground">{copy.intro.replace("{agent}", agentName)}</span>
          </span>
          <ChevronDown aria-hidden="true" className="size-4 shrink-0 transition-transform group-data-[state=open]:rotate-180 motion-reduce:transition-none" />
        </button>
      </CollapsibleTrigger>
      <CollapsibleContent className="flex flex-col gap-4 border-t px-4 py-4 sm:px-5">
        <fieldset className="flex flex-col gap-2" disabled={pending}>
          <legend className="mb-1 text-sm font-semibold">{copy.links}</legend>
          {sources.map((source, index) => (
            <div key={index} className="flex items-center gap-2">
              <Link2 aria-hidden="true" className="size-4 shrink-0 text-muted-foreground" />
              <Input type="url" inputMode="url" value={source} placeholder={copy.linkPlaceholder} aria-label={copy.linkLabel.replace("{number}", String(index + 1))}
                onChange={(event) => setSources((current) => current.map((value, position) => position === index ? event.target.value : value))} />
              {sources.length > 1 && <Button type="button" variant="ghost" size="icon" aria-label={copy.removeLink}
                onClick={() => setSources((current) => current.filter((_, position) => position !== index))}><X /></Button>}
            </div>
          ))}
          {sources.length < MAX_SOURCES && <Button type="button" variant="ghost" size="sm" className="self-start"
            onClick={() => setSources((current) => [...current, ""])}><Plus data-icon="inline-start" />{copy.addLink}</Button>}
          <label className="mt-2 flex flex-col gap-1.5 text-sm font-semibold">{copy.notes}
            <Textarea value={notes} maxLength={8000} placeholder={copy.notesPlaceholder} onChange={(event) => setNotes(event.target.value)} className="min-h-20 font-normal" />
          </label>
        </fieldset>
        <p className="text-sm text-muted-foreground">{copy.privacy}</p>
        <div className="flex flex-wrap items-center gap-3">
          <Button type="button" disabled={pending || (!filled.length && !notes.trim())} onClick={() => void research()}>
            {pending ? <><LoaderCircle data-icon="inline-start" className="animate-spin motion-reduce:animate-none" />{copy.reading}</> : copy.draft}
          </Button>
          {pending && <p role="status" className="text-sm text-muted-foreground">{copy.readingHint.replace("{agent}", agentName)}</p>}
        </div>
        {failure && <Alert variant="destructive"><AlertDescription>{copy.failures[failure].replace("{agent}", agentName)}</AlertDescription></Alert>}
        {draft && (
          <div className="flex flex-col gap-3" role="status">
            {draft.markdown ? <p className="flex items-start gap-2 rounded-lg bg-secondary px-3 py-2.5 text-sm">
              <Check aria-hidden="true" className="mt-0.5 size-4 shrink-0 text-approval" />
              <span>{draft.summary || copy.drafted} {copy.reviewBelow}</span>
            </p> : <p className="text-sm">{copy.nothingRead}</p>}
            {draft.unread.length > 0 && (
              <div className="flex items-start gap-2 rounded-lg bg-attention/25 px-3 py-2.5 text-sm">
                <TriangleAlert aria-hidden="true" className="mt-0.5 size-4 shrink-0" />
                <ul className="min-w-0">{draft.unread.map(({ source, reason }) => (
                  <li key={source} className="wrap-anywhere"><span className="font-semibold">{source}</span>: {copy.problems[reason] ?? copy.problems.unreachable}</li>
                ))}</ul>
              </div>
            )}
            {(draft.profiles.length + draft.colors.length + draft.images.length) > 0 && (
              <FoundItems key={draft.summary + draft.images.length} workspaceId={workspaceId} draft={draft} />
            )}
          </div>
        )}
      </CollapsibleContent>
    </Collapsible>
  );
}

/** Everything else the agent found on the creator's pages, ticked by default, added in one go. */
function FoundItems({ workspaceId, draft }: { workspaceId: number; draft: Draft }) {
  const { messages: { contextResearch: copy } } = useLocale();
  const [colors, setColors] = useState(() => new Set(draft.colors));
  const [images, setImages] = useState(() => new Set(draft.images.map((image) => image.url)));
  const [profiles, setProfiles] = useState(() => new Set(draft.profiles.map((profile) => profile.name)));
  const [pending, setPending] = useState(false);
  const [outcome, setOutcome] = useState<Outcome>();
  const [failed, setFailed] = useState(false);
  const toggle = <T,>(set: Set<T>, value: T) => { const next = new Set(set); if (next.has(value)) next.delete(value); else next.add(value); return next; };
  const chosen = colors.size + images.size + profiles.size;

  async function apply() {
    setPending(true);
    setFailed(false);
    try {
      const result = await invoke<Outcome>("apply_context_import", { workspaceId, import: {
        colors: draft.colors.filter((color) => colors.has(color)),
        images: draft.images.filter((image) => images.has(image.url)),
        profiles: draft.profiles.filter((profile) => profiles.has(profile.name)),
      } });
      setOutcome(result);
      if (result.profiles) window.dispatchEvent(new Event("arrumo:profiles-changed"));
    } catch {
      setFailed(true);
    } finally {
      setPending(false);
    }
  }

  return (
    <section aria-labelledby="found-items" className="flex flex-col gap-4 rounded-lg border p-3">
      <div>
        <h3 id="found-items" className="text-sm font-semibold">{copy.foundTitle}</h3>
        <p className="text-sm text-muted-foreground">{copy.foundIntro}</p>
      </div>
      {draft.colors.length > 0 && (
        <fieldset className="flex flex-col gap-2" disabled={pending || !!outcome}>
          <legend className="text-sm font-semibold">{copy.foundColors}</legend>
          <div className="flex flex-wrap gap-2">
            {draft.colors.map((color) => (
              <label key={color} className="flex cursor-pointer items-center gap-2 rounded-md border px-2 py-1 text-sm has-checked:border-primary">
                <Checkbox checked={colors.has(color)} onCheckedChange={() => setColors((current) => toggle(current, color))} />
                <span className="size-4 rounded-sm border" style={{ background: color }} aria-hidden="true" />
                <span className="font-mono text-xs">{color}</span>
              </label>
            ))}
          </div>
        </fieldset>
      )}
      {draft.images.length > 0 && (
        <fieldset className="flex flex-col gap-2" disabled={pending || !!outcome}>
          <legend className="text-sm font-semibold">{copy.foundImages}</legend>
          <ul className="flex flex-col divide-y rounded-md border">
            {draft.images.map((image) => (
              <li key={image.url}>
                <label className="flex cursor-pointer items-start gap-3 px-3 py-2.5 text-sm">
                  <Checkbox className="mt-0.5" checked={images.has(image.url)} onCheckedChange={() => setImages((current) => toggle(current, image.url))} />
                  <ImageIcon className="mt-0.5 size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
                  <span className="min-w-0 flex-1">
                    <span className="flex flex-wrap items-center gap-2 font-semibold">{image.name}
                      <span className="rounded-md bg-secondary px-1.5 py-0.5 text-xs font-medium">{copy.purposes[image.purpose]}</span>
                      {image.group && <span className="text-xs font-normal text-muted-foreground">{image.group}</span>}
                    </span>
                    {image.description && <span className="block text-muted-foreground">{image.description}</span>}
                  </span>
                </label>
              </li>
            ))}
          </ul>
        </fieldset>
      )}
      {draft.profiles.length > 0 && (
        <fieldset className="flex flex-col gap-2" disabled={pending || !!outcome}>
          <legend className="text-sm font-semibold">{copy.profiles}</legend>
          <ul className="flex flex-col divide-y rounded-md border">
            {draft.profiles.map((profile) => (
              <li key={profile.name}>
                <label className="flex cursor-pointer items-start gap-3 px-3 py-2.5 text-sm">
                  <Checkbox className="mt-0.5" checked={profiles.has(profile.name)} onCheckedChange={() => setProfiles((current) => toggle(current, profile.name))} />
                  {profile.color && <span className="mt-1 size-3 shrink-0 rounded-sm" style={{ background: profile.color }} aria-hidden="true" />}
                  <span className="min-w-0 flex-1"><span className="block font-semibold">{profile.name}</span>
                    {profile.description && <span className="block text-muted-foreground">{profile.description}</span>}</span>
                </label>
              </li>
            ))}
          </ul>
        </fieldset>
      )}
      {outcome ? (
        <p role="status" className="flex items-start gap-2 text-sm">
          <Check aria-hidden="true" className="mt-0.5 size-4 shrink-0 text-approval" />
          <span>{copy.imported.replace("{images}", String(outcome.images)).replace("{colors}", String(outcome.colors)).replace("{profiles}", String(outcome.profiles))}
            {outcome.failedImages.length > 0 && <> {copy.importFailedImages.replace("{count}", String(outcome.failedImages.length))}</>}</span>
        </p>
      ) : (
        <div className="flex flex-wrap items-center gap-3">
          <Button type="button" disabled={pending || chosen === 0} onClick={() => void apply()}>
            {pending ? <><LoaderCircle data-icon="inline-start" className="animate-spin motion-reduce:animate-none" />{copy.importing}</> : copy.importSelected.replace("{count}", String(chosen))}
          </Button>
          {failed && <p role="alert" className="text-sm text-destructive">{copy.importError}</p>}
        </div>
      )}
    </section>
  );
}


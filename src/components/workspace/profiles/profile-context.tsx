"use client";

import { invoke } from "@tauri-apps/api/core";
import { ChevronDown, RectangleHorizontal, RectangleVertical } from "lucide-react";
import dynamic from "next/dynamic";
import { useEffect, useState, type FormEvent } from "react";
import { Button } from "@/components/ui/button";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { MarkdownEditor } from "@/components/markdown/editor";
import { LayerGauge } from "@/components/workspace/layer-gauge";
import type { Workspace } from "@/components/workspace/home/workspace-links";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";
import type { Profile } from "./profile-route";

const MarkdownText = dynamic(() => import("@/components/workspace/generation/script-review-text"), { ssr: false });

type ProfileFormat = { id: number; name: string; width: number; height: number; context: string };
type SaveState = "idle" | "saving" | "saved" | "error";

export function ProfileContext({ workspace, profile, onUpdate }: {
  workspace: Workspace;
  profile: Profile;
  onUpdate: (profile: Profile) => void;
}) {
  const { messages } = useLocale();
  const copy = messages.profileHub;
  const [workspaceContext, setWorkspaceContext] = useState<string | null>();
  const [formats, setFormats] = useState<ProfileFormat[]>();
  const [failed, setFailed] = useState(false);
  const workspaceId = workspace.id;

  useEffect(() => {
    let active = true;
    Promise.all([
      invoke<string | null>("get_workspace_context", { workspaceId }),
      invoke<ProfileFormat[]>("list_profile_formats", { workspaceId, profileId: profile.id }),
    ]).then(([context, entries]) => {
      if (!active) return;
      setWorkspaceContext(context);
      setFormats(entries);
    }, () => { if (active) setFailed(true); });
    return () => { active = false; };
  }, [profile.id, workspaceId]);

  return (
    <div className="grid items-start gap-10 lg:grid-cols-[minmax(0,1fr)_18rem]">
      <div className="flex min-w-0 flex-col gap-10">
        {failed && <p role="alert" className="text-destructive">{copy.loadError}</p>}
        <Collapsible className="rounded-xl bg-secondary">
          <CollapsibleTrigger className="group flex w-full items-center gap-3 p-4 text-start">
            <span className="min-w-0 flex-1">
              <LayerGauge layer="workspace" />
              <span className="mt-1 block font-semibold">{copy.inherited.replace("{workspace}", workspace.name)}</span>
            </span>
            <ChevronDown aria-hidden="true" className="size-4 transition-transform duration-150 group-data-[state=open]:rotate-180" />
          </CollapsibleTrigger>
          <CollapsibleContent className="border-t border-border/70 px-4 pt-3 pb-4">
            {workspaceContext?.trim() ? <MarkdownText markdown={workspaceContext} />
              : <p className="text-muted-foreground">{copy.noWorkspaceContext}</p>}
          </CollapsibleContent>
        </Collapsible>
        <ProfileWideContext workspaceId={workspaceId} profile={profile} onUpdate={onUpdate} />
        {formats && formats.length > 0 && <FormatContext workspaceId={workspaceId} profileId={profile.id} formats={formats} onSaved={setFormats} />}
      </div>
      <CascadeExplainer workspace={workspace} profile={profile} />
    </div>
  );
}

function SectionHeading({ title, state }: { title: string; state: SaveState | "dirty" }) {
  const { messages: { profileHub: copy } } = useLocale();
  return (
    <div className="mb-3 flex flex-wrap items-baseline justify-between gap-2">
      <h2 className="text-[1.5rem]">{title}</h2>
      <span role="status" className={cn("text-sm text-muted-foreground", state === "dirty" && "font-semibold text-foreground", state === "error" && "text-destructive")}>
        {state === "saved" ? copy.saved : state === "dirty" ? copy.unsaved : state === "error" ? copy.saveError : ""}
      </span>
    </div>
  );
}

function ProfileWideContext({ workspaceId, profile, onUpdate }: {
  workspaceId: number;
  profile: Profile;
  onUpdate: (profile: Profile) => void;
}) {
  const { messages: { profileHub: copy } } = useLocale();
  const [guidance, setGuidance] = useState(profile.guidance);
  const [state, setState] = useState<SaveState>("idle");
  const dirty = guidance !== profile.guidance;

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!dirty || state === "saving") return;
    setState("saving");
    try {
      onUpdate(await invoke<Profile>("update_profile", {
        workspaceId, profileId: profile.id, name: profile.name, color: profile.color, guidance,
      }));
      setState("saved");
    } catch {
      setState("error");
    }
  }

  return (
    <form onSubmit={save}>
      <SectionHeading title={copy.everyVideo.replace("{profile}", profile.name)} state={dirty && state !== "error" ? "dirty" : state} />
      <MarkdownEditor id="profile-context" label={copy.everyVideo.replace("{profile}", profile.name)} value={guidance}
        disabled={state === "saving"} onChange={(value) => { setGuidance(value); if (state === "saved") setState("idle"); }} />
      <Button type="submit" className="mt-3" disabled={!dirty || state === "saving"}>{copy.save}</Button>
    </form>
  );
}

function FormatContext({ workspaceId, profileId, formats, onSaved }: {
  workspaceId: number;
  profileId: number;
  formats: ProfileFormat[];
  onSaved: (formats: ProfileFormat[]) => void;
}) {
  const { messages: { profileHub: copy } } = useLocale();
  const [selected, setSelected] = useState(formats[0].id);
  const format = formats.find(({ id }) => id === selected) ?? formats[0];
  const [drafts, setDrafts] = useState<Record<number, string>>({});
  const [state, setState] = useState<SaveState>("idle");
  const value = drafts[format.id] ?? format.context;
  const dirty = value !== format.context;

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!dirty || state === "saving") return;
    setState("saving");
    try {
      await invoke("set_profile_format_context", { workspaceId, profileId, formatId: format.id, markdown: value });
      onSaved(formats.map((entry) => entry.id === format.id ? { ...entry, context: value.trim() ? value : "" } : entry));
      setDrafts(({ [format.id]: _, ...rest }) => rest);
      setState("saved");
    } catch {
      setState("error");
    }
  }

  return (
    <form onSubmit={save}>
      <SectionHeading title={copy.oneFormat} state={dirty && state !== "error" ? "dirty" : state} />
      <div role="tablist" aria-label={copy.formatPicker} className="mb-3 inline-flex flex-wrap rounded-lg bg-secondary p-1">
        {formats.map((entry) => {
          const Icon = entry.height > entry.width ? RectangleVertical : RectangleHorizontal;
          const changed = (drafts[entry.id] ?? entry.context) !== entry.context;
          return (
            <button key={entry.id} type="button" role="tab" aria-selected={entry.id === format.id}
              onClick={() => { setSelected(entry.id); setState("idle"); }}
              className={cn("flex items-center gap-2 rounded-md px-3 py-1.5 text-sm font-semibold text-muted-foreground hover:text-foreground",
                entry.id === format.id && "bg-card text-foreground shadow-xs")}>
              <Icon className="size-4" aria-hidden="true" />{entry.name}
              {changed && <span aria-hidden="true" className="size-1.5 rounded-[2px] bg-attention" />}
            </button>
          );
        })}
      </div>
      <MarkdownEditor key={format.id} id={`format-context-${format.id}`} label={`${copy.oneFormat}: ${format.name}`} value={value}
        disabled={state === "saving"} onChange={(next) => { setDrafts((current) => ({ ...current, [format.id]: next })); if (state === "saved") setState("idle"); }} />
      <Button type="submit" className="mt-3" disabled={!dirty || state === "saving"}>{copy.save}</Button>
    </form>
  );
}

function CascadeExplainer({ workspace, profile }: { workspace: Workspace; profile: Profile }) {
  const { messages: { profileHub: copy } } = useLocale();
  const layers = [
    { layer: "app", detail: copy.readsApp },
    { layer: "workspace", detail: copy.readsWorkspace.replace("{workspace}", workspace.name) },
    { layer: "profile", detail: copy.readsProfile },
    { layer: "video", detail: copy.readsVideo },
  ] as const;
  return (
    <aside className="lg:sticky lg:top-20">
      <h2 className="mb-3 text-[1.3125rem] leading-tight">{copy.reads.replace("{profile}", profile.name)}</h2>
      <ol className="flex flex-col gap-1.5">
        {layers.map(({ layer, detail }, index) => (
          <li key={layer} style={{ marginInlineStart: `${index * 0.5}rem` }}
            className={cn("flex flex-wrap items-center justify-between gap-2 rounded-lg border bg-card px-3 py-2.5 text-sm",
              layer === "profile" && "border-primary/40 bg-secondary")}>
            <LayerGauge layer={layer} />
            <span className="text-muted-foreground">{detail}</span>
          </li>
        ))}
      </ol>
      <p className="mt-3 text-sm text-muted-foreground">{copy.readsHint}</p>
    </aside>
  );
}

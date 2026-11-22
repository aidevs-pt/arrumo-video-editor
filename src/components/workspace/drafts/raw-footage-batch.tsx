"use client";

import { invoke } from "@tauri-apps/api/core";
import { ChevronRight, Layers, Pencil, Plus, TriangleAlert, Upload, X } from "lucide-react";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { AssetThumbnail } from "@/components/workspace/assets/asset-thumbnail";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";

type Clip = { id: number; name: string };
type Profile = { id: number; name: string };
type Group = { key: number; title: string; profileId: number | null; sourceIds: number[] };
export type CreatedDraft = { id: number; title: string; intent: string; profile_id: number | null; source_ids: number[] };

function titleFromFile(name: string) {
  return name.replace(/\.[^.]+$/, "").replace(/[_-]+/g, " ").trim();
}

/** Restores the batch a user was building, so leaving the page does not lose groups. */
function useStoredGroups(workspaceId: number, initialProfile: number | null) {
  const key = `arrumo:batch:${workspaceId}`;
  const [groups, setGroups] = useState<Group[]>(() => {
    try {
      const saved = JSON.parse(sessionStorage.getItem(key) ?? "null");
      if (Array.isArray(saved)) return saved.filter((group): group is Group => Array.isArray(group?.sourceIds));
    } catch {}
    return [];
  });
  useEffect(() => {
    try {
      if (groups.length) sessionStorage.setItem(key, JSON.stringify(groups));
      else sessionStorage.removeItem(key);
    } catch {}
  }, [groups, key]);
  return [groups.map((group) => ({ ...group, profileId: group.profileId === undefined ? initialProfile : group.profileId })), setGroups] as const;
}

export function RawFootageBatch({ workspaceId, clips, pool, profiles, initialProfile, dragging, importing, onImport, onPoolChange, onScratch, onBack, onCreated }: {
  workspaceId: number;
  clips: Clip[];
  pool: number[];
  profiles: Profile[];
  initialProfile: number | null;
  dragging: boolean;
  importing: boolean;
  onImport: () => void;
  onPoolChange: (pool: number[]) => void;
  onScratch: () => void;
  onBack: () => void;
  onCreated: (drafts: CreatedDraft[]) => void;
}) {
  const { messages } = useLocale();
  const copy = messages.batch;
  const [groups, setGroups] = useStoredGroups(workspaceId, initialProfile);
  const [selected, setSelected] = useState<number[]>([]);
  const [saving, setSaving] = useState(false);
  const [problem, setProblem] = useState<"missingTitle" | "saveError" | null>(null);
  const names = new Map(clips.map((clip) => [clip.id, clip.name]));
  const grouped = new Map(groups.flatMap((group, index) => group.sourceIds.map((id) => [id, index + 1] as const)));
  const batchClips = [...new Set([...pool, ...groups.flatMap((group) => group.sourceIds)])].filter((id) => names.has(id));
  const earlier = clips.filter((clip) => !batchClips.includes(clip.id));
  const loose = batchClips.filter((id) => !grouped.has(id)).length;

  function toggle(id: number, checked: boolean) {
    setSelected((current) => checked ? [...current, id] : current.filter((value) => value !== id));
  }

  function addGroups(next: number[][]) {
    const taken = new Set(next.flat());
    setGroups((current) => [
      ...current.map((group) => ({ ...group, sourceIds: group.sourceIds.filter((id) => !taken.has(id)) }))
        .filter((group) => group.sourceIds.length),
      ...next.map((sourceIds, index) => ({
        key: Date.now() + index,
        title: titleFromFile(names.get(sourceIds[0]) ?? ""),
        profileId: initialProfile,
        sourceIds,
      })),
    ]);
    setSelected([]);
    setProblem(null);
  }

  function update(key: number, change: Partial<Group>) {
    setGroups((current) => current.map((group) => group.key === key ? { ...group, ...change } : group));
  }

  async function create() {
    if (groups.some((group) => !group.title.trim())) {
      setProblem("missingTitle");
      return;
    }
    setSaving(true);
    setProblem(null);
    const created: CreatedDraft[] = [];
    const remaining: Group[] = [];
    for (const group of groups) {
      try {
        created.push(await invoke<CreatedDraft>("create_video_draft", {
          workspaceId,
          settings: { title: group.title.trim(), intent: "", profile_id: group.profileId, source_ids: group.sourceIds },
        }));
      } catch {
        remaining.push(group);
      }
    }
    setSaving(false);
    setGroups(remaining);
    if (remaining.length) setProblem("saveError");
    else onPoolChange([]);
    if (created.length) onCreated(created);
  }

  return (
    <div className="grid items-start gap-8 xl:grid-cols-[minmax(0,1fr)_22rem]">
      <section className="flex min-w-0 flex-col gap-5" aria-label={copy.clips}>
        <Button type="button" variant="ghost" className="-ms-3 self-start" onClick={onScratch}><Pencil data-icon="inline-start" />{copy.scratchInstead}</Button>
        <div className={cn("flex flex-wrap items-center gap-4 rounded-xl border border-dashed bg-secondary p-5 transition-colors duration-150",
          dragging && "border-primary bg-accent")}>
          <Upload className="size-6 shrink-0 text-primary" aria-hidden="true" />
          <div className="min-w-0 flex-1">
            <p className="font-semibold">{messages.videoCatalog.drop}</p>
            <p className="text-sm text-muted-foreground">{copy.dropHint}</p>
          </div>
          <Button type="button" variant="outline" disabled={importing || saving} onClick={onImport}>
            {importing ? messages.drafts.importing : copy.choose}
          </Button>
        </div>
        <div className="flex min-h-14 flex-wrap items-center gap-2 rounded-xl bg-secondary px-4 py-2" role="group" aria-label={copy.clips}>
          <p className="me-auto font-semibold" role="status">
            {selected.length === 0 ? <span className="font-normal text-muted-foreground">{copy.selectHint}</span>
              : selected.length === 1 ? copy.selectedOne : copy.selected.replace("{count}", String(selected.length))}
          </p>
          {selected.length > 0 && <>
            <Button type="button" variant="ghost" size="sm" onClick={() => setSelected([])}>{copy.clear}</Button>
            <Button type="button" variant="outline" size="sm" onClick={() => addGroups(selected.map((id) => [id]))}>{copy.each}</Button>
            <Button type="button" variant="outline" size="sm" className="border-primary text-primary" onClick={() => addGroups([selected])}>
              <Layers data-icon="inline-start" />{copy.group}
            </Button>
          </>}
        </div>
        {batchClips.length > 0 && <ul className="grid grid-cols-[repeat(auto-fill,minmax(min(100%,11rem),1fr))] gap-3">
          {batchClips.map((id) => {
            const video = grouped.get(id);
            const checked = selected.includes(id);
            return (
              <li key={id} className="min-w-0">
                <label className={cn("flex cursor-pointer flex-col gap-2 rounded-lg border-2 border-transparent p-1 transition-colors duration-150 hover:border-border",
                  checked && "border-primary hover:border-primary")}>
                  <span className="relative block">
                    <AssetThumbnail workspaceId={workspaceId} assetId={id} kind="video" className="aspect-video w-full" />
                    {video && <span className="absolute start-1.5 top-1.5 rounded-[5px] bg-attention px-1.5 py-0.5 text-xs font-bold text-ink">
                      {copy.videoLabel.replace("{number}", String(video))}
                    </span>}
                    <Checkbox className="absolute end-1.5 top-1.5 bg-card" checked={checked}
                      aria-label={names.get(id)} onCheckedChange={(value) => toggle(id, value === true)} />
                  </span>
                  <span className="min-w-0 truncate px-1 text-sm" title={names.get(id)}>{names.get(id)}</span>
                </label>
              </li>
            );
          })}
        </ul>}
        {earlier.length > 0 && <details className="group">
          <summary className="flex cursor-pointer list-none items-center gap-1.5 font-semibold text-muted-foreground hover:text-foreground [&::-webkit-details-marker]:hidden">
            <ChevronRight className="size-4 transition-transform duration-150 group-open:rotate-90 motion-reduce:transition-none" aria-hidden="true" />{copy.earlier}
          </summary>
          <ul className="mt-3 flex flex-wrap gap-2">
            {earlier.map((clip) => (
              <li key={clip.id}>
                <Button type="button" variant="outline" size="sm" aria-label={copy.addEarlier.replace("{name}", clip.name)}
                  onClick={() => onPoolChange([...pool, clip.id])}><Plus data-icon="inline-start" />{clip.name}</Button>
              </li>
            ))}
          </ul>
        </details>}
      </section>
      <aside className="flex flex-col gap-4 xl:sticky xl:top-20" aria-labelledby="batch-videos">
        <div className="flex items-center justify-between">
          <h2 id="batch-videos" className="text-[1.5rem]">{copy.videos}</h2>
          <span className="rounded-[5px] bg-secondary px-2 text-sm font-bold tabular-nums">{groups.length}</span>
        </div>
        {groups.length === 0 ? <p className="rounded-xl border border-dashed p-4 text-muted-foreground">{copy.noVideos}</p> : (
          <ol className="divide-y rounded-xl border bg-card">
            {groups.map((group, index) => (
              <li key={group.key} className="flex flex-col gap-3 p-4">
                <div className="flex items-center justify-between text-sm text-muted-foreground">
                  {copy.videoLabel.replace("{number}", String(index + 1))}
                  <Button type="button" variant="ghost" size="icon-sm" aria-label={copy.removeVideo.replace("{number}", String(index + 1))}
                    onClick={() => setGroups((current) => current.filter(({ key }) => key !== group.key))}><X /></Button>
                </div>
                <Input aria-label={copy.title.replace("{number}", String(index + 1))} value={group.title} maxLength={200}
                  onChange={(event) => update(group.key, { title: event.target.value })} className="font-semibold" />
                <div className="flex items-center gap-3">
                  <span className="shrink-0 text-sm text-muted-foreground tabular-nums">
                    {group.sourceIds.length === 1 ? copy.clipsOne : copy.clipsMany.replace("{count}", String(group.sourceIds.length))}
                  </span>
                  <NativeSelect aria-label={messages.drafts.profile} value={group.profileId ?? ""} className="min-w-0 flex-1"
                    onChange={(event) => update(group.key, { profileId: event.target.value ? Number(event.target.value) : null })}>
                    <NativeSelectOption value="">{messages.drafts.noProfile}</NativeSelectOption>
                    {profiles.map((profile) => <NativeSelectOption key={profile.id} value={profile.id}>{profile.name}</NativeSelectOption>)}
                  </NativeSelect>
                </div>
              </li>
            ))}
            {loose > 0 && <li className="flex gap-2 p-4 text-sm text-muted-foreground">
              <TriangleAlert className="mt-0.5 size-4 shrink-0 text-[oklch(55%_.14_70)]" aria-hidden="true" />
              {loose === 1 ? copy.looseOne : copy.loose.replace("{count}", String(loose))}
            </li>}
          </ol>
        )}
        {problem && <Alert variant="destructive"><AlertDescription>{copy[problem]}</AlertDescription></Alert>}
        {groups.length > 0 && <p className="text-sm text-muted-foreground">{copy.kept}</p>}
        <Button type="button" size="lg" disabled={groups.length === 0 || saving} onClick={() => void create()}>
          {saving ? copy.saving : groups.length === 0 ? copy.continue : groups.length === 1 ? copy.continueOne
            : copy.continueMany.replace("{count}", String(groups.length))}
          <ChevronRight data-icon="inline-end" />
        </Button>
        <Button type="button" variant="ghost" className="self-start" disabled={saving} onClick={onBack}>{messages.videoCatalog.back}</Button>
      </aside>
    </div>
  );
}

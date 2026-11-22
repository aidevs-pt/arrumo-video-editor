"use client";

import { WorkspaceSettingsHeading } from "@/components/workspace/settings/workspace-settings-heading";
import { useLocale } from "@/lib/locale-provider";
import { WorkspaceShell } from "@/components/workspace/layout";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Check, Plus, X } from "lucide-react";
import { cn } from "@/lib/utils";
import { WorkspaceBrandImages } from "./workspace-brand-images";

type BrandColor = { hex: string; name: string; forVideos: boolean };

export function WorkspaceBrand({ workspace }: {
  workspace: { id: number; name: string };

}) {
  const heading = useRef<HTMLHeadingElement>(null);
  const [palette, setPalette] = useState<BrandColor[]>();
  const colors = palette?.map((color) => color.hex);
  const [candidate, setCandidate] = useState("#000000");
  const [loadError, setLoadError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [saving, setSaving] = useState(false);
  const [result, setResult] = useState<"saveError" | null>(null);
  const { messages } = useLocale();
  const copy = messages.brand;

  useEffect(() => { heading.current?.focus(); }, []);
  useEffect(() => {
    let active = true;
    invoke<BrandColor[]>("get_brand_palette", { workspaceId: workspace.id }).then(
      (value) => { if (active) setPalette(value); },
      () => { if (active) setLoadError(true); },
    );
    return () => { active = false; };
  }, [workspace.id, attempt]);

  async function save(next: string[]) {
    if (saving) return;
    setSaving(true);
    setResult(null);
    try {
      await invoke<string[]>("set_brand_colors", { workspaceId: workspace.id, colors: next });
      setPalette(await invoke<BrandColor[]>("get_brand_palette", { workspaceId: workspace.id }));
    } catch {
      setResult("saveError");
    } finally {
      setSaving(false);
    }
  }

  async function saveDetails(color: BrandColor, name: string, forVideos: boolean) {
    if (name.trim() === color.name && forVideos === color.forVideos) return;
    setPalette((current) => current?.map((item) => item.hex === color.hex ? { ...item, name: name.trim(), forVideos } : item));
    setResult(null);
    try {
      setPalette(await invoke<BrandColor[]>("set_brand_color_details", { workspaceId: workspace.id, color: color.hex, name, forVideos }));
    } catch {
      setPalette((current) => current?.map((item) => item.hex === color.hex ? color : item));
      setResult("saveError");
    }
  }

  return (
    <WorkspaceShell className="max-w-6xl">
      <WorkspaceSettingsHeading workspace={workspace} current="brand" headingRef={heading} />
      {loadError ? (
        <Alert variant="destructive">
          <AlertDescription>{copy.loadError}</AlertDescription>
          <Button type="button" variant="outline" onClick={() => {
            setLoadError(false);
            setAttempt((current) => current + 1);
          }}>{copy.retry}</Button>
        </Alert>
      ) : !palette || !colors ? <p role="status">{copy.loading}</p> : (
        <section aria-labelledby="brand-colors">
          <div className="mb-4 flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
            <h2 id="brand-colors" className="text-[1.75rem]">{copy.colors}</h2>
            {palette.length > 0 && <p className="text-sm text-muted-foreground" role="status">
              {copy.forVideosCount.replace("{count}", String(palette.filter((color) => color.forVideos).length)).replace("{total}", String(palette.length))}
            </p>}
          </div>
          <ul className="flex flex-wrap items-start gap-4">
            {palette.map((color) => (
              <li key={color.hex} className="group relative flex w-32 flex-col gap-1.5">
                <span className="h-20 rounded-lg border" style={{ backgroundColor: color.hex }} aria-hidden="true" />
                <Button type="button" size="icon-sm" aria-pressed={color.forVideos}
                  aria-label={copy.forVideos.replace("{color}", color.name || color.hex)} title={copy.forVideos.replace("{color}", color.name || color.hex)}
                  className={cn("absolute end-1.5 top-1.5 size-7 border shadow-xs",
                    color.forVideos ? "border-transparent bg-approval text-white hover:bg-approval/90" : "border-line-strong bg-card text-transparent hover:bg-card hover:text-muted-foreground")}
                  onClick={() => void saveDetails(color, color.name, !color.forVideos)}><Check /></Button>
                <Button type="button" variant="secondary" size="icon-sm" disabled={saving} aria-label={`${copy.remove} ${color.name || color.hex}`}
                  className="absolute start-1.5 top-1.5 size-7 opacity-0 transition-opacity duration-150 group-hover:opacity-100 focus-visible:opacity-100"
                  onClick={() => void save(colors.filter((item) => item !== color.hex))}><X /></Button>
                <input key={color.name} defaultValue={color.name} maxLength={40} placeholder={copy.namePlaceholder}
                  aria-label={copy.nameLabel.replace("{color}", color.hex)}
                  className="-mx-1 rounded-sm px-1 py-0.5 font-semibold outline-none placeholder:font-normal placeholder:text-muted-foreground hover:bg-secondary focus-visible:bg-card focus-visible:outline-2 focus-visible:outline-ring"
                  onBlur={(event) => void saveDetails(color, event.currentTarget.value, color.forVideos)}
                  onKeyDown={(event) => { if (event.key === "Enter") event.currentTarget.blur(); }} />
                <span className={cn("text-sm tabular-nums", color.forVideos ? "text-muted-foreground" : "text-muted-foreground/70 line-through")}>{color.hex}</span>
              </li>
            ))}
            <li className="w-32">
              <label className="flex h-20 cursor-pointer flex-col items-center justify-center gap-1 rounded-lg border border-dashed border-line-strong text-sm text-muted-foreground hover:border-primary hover:text-foreground focus-within:outline-2 focus-within:outline-ring">
                <Plus className="size-4" aria-hidden="true" />{copy.add}
                <input type="color" className="sr-only" value={candidate} disabled={saving}
                  onChange={(event) => setCandidate(event.target.value)}
                  onBlur={() => { const next = candidate.toUpperCase(); if (!colors.includes(next)) void save([...colors, next]); }} />
              </label>
            </li>
          </ul>
          {result === "saveError" && <p role="alert" className="mt-3 text-destructive">{copy.saveError}</p>}
        </section>
      )}
      <WorkspaceBrandImages workspaceId={workspace.id} />
    </WorkspaceShell>
  );
}

"use client";

import { WorkspaceSettingsHeading } from "@/components/workspace/settings/workspace-settings-heading";
import { useLocale } from "@/lib/locale-provider";
import { WorkspaceShell } from "@/components/workspace/layout";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Plus, X } from "lucide-react";
import { WorkspaceBrandImages } from "./workspace-brand-images";

export function WorkspaceBrand({ workspace }: {
  workspace: { id: number; name: string };

}) {
  const heading = useRef<HTMLHeadingElement>(null);
  const [colors, setColors] = useState<string[]>();
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
    invoke<string[]>("get_brand_colors", { workspaceId: workspace.id }).then(
      (value) => { if (active) setColors(value); },
      () => { if (active) setLoadError(true); },
    );
    return () => { active = false; };
  }, [workspace.id, attempt]);

  async function save(next: string[]) {
    if (saving) return;
    setSaving(true);
    setResult(null);
    try {
      setColors(await invoke<string[]>("set_brand_colors", { workspaceId: workspace.id, colors: next }));
    } catch {
      setResult("saveError");
    } finally {
      setSaving(false);
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
      ) : !colors ? <p role="status">{copy.loading}</p> : (
        <section aria-labelledby="brand-colors">
          <h2 id="brand-colors" className="mb-4 text-[1.75rem]">{copy.colors}</h2>
          <ul className="flex flex-wrap items-start gap-4">
            {colors.map((color) => (
              <li key={color} className="group relative flex w-32 flex-col gap-2">
                <span className="h-20 rounded-lg border" style={{ backgroundColor: color }} aria-hidden="true" />
                <span className="text-sm text-muted-foreground tabular-nums">{color}</span>
                <Button type="button" variant="secondary" size="icon-sm" disabled={saving} aria-label={`${copy.remove} ${color}`}
                  className="absolute end-1.5 top-1.5 opacity-0 transition-opacity duration-150 group-hover:opacity-100 focus-visible:opacity-100"
                  onClick={() => void save(colors.filter((item) => item !== color))}><X /></Button>
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

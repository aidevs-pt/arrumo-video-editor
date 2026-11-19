"use client";

import { useLocale } from "@/lib/locale-provider";
import { WorkspaceHeading, WorkspaceShell } from "@/components/workspace/layout";
import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { WorkspaceBrandImages } from "./workspace-brand-images";

export function WorkspaceBrand({ workspace, onBack }: {
  workspace: { id: number; name: string };

  onBack: () => void;
}) {
  const heading = useRef<HTMLHeadingElement>(null);
  const [colors, setColors] = useState<string[]>();
  const [candidate, setCandidate] = useState("#000000");
  const [loadError, setLoadError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [saving, setSaving] = useState(false);
  const [savingImages, setSavingImages] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [result, setResult] = useState<"saved" | "saveError" | null>(null);
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

  function update(next: string[]) {
    setColors(next);
    setDirty(true);
    setResult(null);
  }

  async function save() {
    if (!dirty || saving || !colors) return;
    setSaving(true);
    setResult(null);
    try {
      const stored = await invoke<string[]>("set_brand_colors", { workspaceId: workspace.id, colors });
      setColors(stored);
      setDirty(false);
      setResult("saved");
    } catch {
      setResult("saveError");
    } finally {
      setSaving(false);
    }
  }

  return (
    <WorkspaceShell>
      <Button type="button" variant="ghost" onClick={onBack} disabled={saving || savingImages}>
        <ArrowLeft data-icon="inline-start" />
        {copy.back}
      </Button>
      <WorkspaceHeading>
        <h1 ref={heading} tabIndex={-1}>{copy.title}</h1>
        <p>{workspace.name}</p>
      </WorkspaceHeading>
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
          <h2 id="brand-colors" className="flex min-w-0 items-baseline gap-2.5 font-[680]">{copy.colors}</h2>
          {colors.length ? (
            <ul className="my-3.5 flex flex-wrap items-center gap-3 [&>li]:flex [&>li]:items-center [&>li]:gap-2 [&>li]:rounded-lg [&>li]:border [&>li]:p-1.5">
              {colors.map((color) => (
                <li key={color}>
                  <span className="size-6 rounded-sm border" style={{ backgroundColor: color }} aria-hidden="true" />
                  <span>{color}</span>
                  <Button type="button" variant="ghost" size="sm" disabled={saving}
                    aria-label={`${copy.remove} ${color}`} onClick={() => update(colors.filter((item) => item !== color))}>{copy.remove}</Button>
                </li>
              ))}
            </ul>
          ) : <p className="mt-1.25 whitespace-pre-wrap text-muted-foreground">{copy.empty}</p>}
          <div className="my-3.5 flex flex-wrap items-center gap-3">
            <Field orientation="horizontal" className="w-auto [&_input]:w-13">
              <FieldLabel htmlFor="brand-candidate">{copy.picker}</FieldLabel>
              <Input id="brand-candidate" type="color" value={candidate} disabled={saving}
                onChange={(event) => setCandidate(event.target.value)} />
            </Field>
            <Button type="button" variant="outline" disabled={saving || colors.includes(candidate.toUpperCase())}
              onClick={() => update([...colors, candidate.toUpperCase()])}>{copy.add}</Button>
            <Button type="button" disabled={!dirty || saving} onClick={() => void save()}>{copy.save}</Button>
          </div>
          {result === "saved" && <p role="status">{copy.saved}</p>}
          {result === "saveError" && <p role="alert">{copy.saveError}</p>}
        </section>
      )}
      <WorkspaceBrandImages workspaceId={workspace.id} onSaving={setSavingImages} />
    </WorkspaceShell>
  );
}

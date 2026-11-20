"use client";

import { PluginSurfaces } from "@/components/workspace/plugins/plugin-surfaces";
import { WorkspaceSettingsHeading } from "@/components/workspace/settings/workspace-settings-heading";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";
import { LayerGauge } from "@/components/workspace/layer-gauge";
import { WorkspaceShell } from "@/components/workspace/layout";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { MarkdownEditor } from "@/components/markdown/editor";
import { WorkspaceFeedback } from "./workspace-feedback";

export function WorkspaceContext({ workspace }: {
  workspace: { id: number; name: string };

}) {
  const heading = useRef<HTMLHeadingElement>(null);
  const [markdown, setMarkdown] = useState<string>();
  const [loadError, setLoadError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [saving, setSaving] = useState(false);
  const [result, setResult] = useState<"saved" | "saveError" | null>(null);
  const [dirty, setDirty] = useState(false);
  const { messages } = useLocale();
  const copy = messages.context;

  useEffect(() => { heading.current?.focus(); }, []);
  useEffect(() => {
    let active = true;
    invoke<string | null>("get_workspace_context", { workspaceId: workspace.id }).then(
      (value) => { if (active) setMarkdown(value ?? copy.template); },
      () => { if (active) setLoadError(true); },
    );
    return () => { active = false; };
  }, [workspace.id, copy.template, attempt]);

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (markdown === undefined || saving) return;
    setSaving(true);
    setResult(null);
    try {
      await invoke("set_workspace_context", { workspaceId: workspace.id, markdown });
      setResult("saved");
      setDirty(false);
    } catch {
      setResult("saveError");
    } finally {
      setSaving(false);
    }
  }

  return (
    <WorkspaceShell className="max-w-7xl">
      <WorkspaceSettingsHeading workspace={workspace} current="context" headingRef={heading} />
      <div className="grid items-start gap-10 lg:grid-cols-[minmax(0,1fr)_19rem]">
        <div className="flex min-w-0 flex-col gap-12">
          {loadError ? (
            <Alert variant="destructive">
              <AlertDescription>{copy.loadError}</AlertDescription>
              <Button type="button" variant="outline" onClick={() => {
                setLoadError(false);
                setAttempt((current) => current + 1);
              }}>{copy.retry}</Button>
            </Alert>
          ) : markdown === undefined ? <p role="status">{copy.loading}</p> : (
            <form onSubmit={save} aria-busy={saving} aria-labelledby="context-about">
              <div className="mb-3 flex flex-wrap items-baseline justify-between gap-2">
                <h2 id="context-about" className="text-[1.75rem]">{copy.aboutYou}</h2>
                <span role="status" className={cn("text-sm text-muted-foreground", dirty && "font-semibold text-foreground", result === "saveError" && "text-destructive")}>
                  {result === "saveError" ? copy.saveError : dirty ? copy.unsaved : result === "saved" ? copy.saved : ""}
                </span>
              </div>
              <MarkdownEditor id="workspace-context" label={copy.label} value={markdown} disabled={saving} onChange={(value) => {
                setMarkdown(value);
                setDirty(true);
                setResult(null);
              }} />
              <Button type="submit" className="mt-3" disabled={saving || !dirty}>{copy.save}</Button>
            </form>
          )}
          <WorkspaceFeedback workspaceId={workspace.id} />
          <PluginSurfaces placement="workspace" workspaceId={workspace.id} />
        </div>
        <SettingsCascade workspace={workspace} />
      </div>
    </WorkspaceShell>
  );
}

function SettingsCascade({ workspace }: { workspace: { name: string } }) {
  const { messages: { context: copy } } = useLocale();
  const layers = [
    { layer: "app", detail: copy.cascadeApp },
    { layer: "workspace", detail: copy.cascadeWorkspace.replace("{workspace}", workspace.name) },
    { layer: "profile", detail: copy.cascadeProfile },
    { layer: "video", detail: copy.cascadeVideo },
  ] as const;
  return (
    <aside className="rounded-xl border bg-card p-5 lg:sticky lg:top-20">
      <h2 className="mb-3 text-[1.0625rem] font-bold [font-family:var(--font-sans)] [font-variation-settings:normal]">{copy.cascadeTitle}</h2>
      <ol className="flex flex-col gap-1.5">
        {layers.map(({ layer, detail }, index) => (
          <li key={layer} style={{ marginInlineStart: `${index * 0.5}rem` }}
            className={cn("flex flex-wrap items-center justify-between gap-2 rounded-lg border bg-background px-3 py-2.5 text-sm",
              layer === "workspace" && "border-primary/40 bg-secondary")}>
            <LayerGauge layer={layer} />
            <span className="text-muted-foreground">{detail}</span>
          </li>
        ))}
      </ol>
      <p className="mt-3 text-sm text-muted-foreground">{copy.cascadeHint}</p>
    </aside>
  );
}

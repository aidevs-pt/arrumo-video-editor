"use client";

import { useLocale } from "@/lib/locale-provider";
import { WorkspaceForm, WorkspaceHeading, WorkspaceShell } from "@/components/workspace/layout";
import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Textarea } from "@/components/ui/textarea";
import { WorkspaceFeedback } from "./workspace-feedback";

export function WorkspaceContext({ workspace, onBack }: {
  workspace: { id: number; name: string };

  onBack: () => void;
}) {
  const heading = useRef<HTMLHeadingElement>(null);
  const [markdown, setMarkdown] = useState<string>();
  const [loadError, setLoadError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [saving, setSaving] = useState(false);
  const [result, setResult] = useState<"saved" | "saveError" | null>(null);
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
    } catch {
      setResult("saveError");
    } finally {
      setSaving(false);
    }
  }

  return (
    <WorkspaceShell>
      <Button type="button" variant="ghost" onClick={onBack} disabled={saving}>
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
      ) : markdown === undefined ? <p role="status">{copy.loading}</p> : (
        <WorkspaceForm  onSubmit={save} aria-busy={saving}>
          <FieldGroup>
            <Field data-disabled={saving}>
              <FieldLabel htmlFor="workspace-context">{copy.label}</FieldLabel>
              <Textarea id="workspace-context" rows={10} value={markdown} disabled={saving} onChange={(event) => {
                setMarkdown(event.target.value);
                setResult(null);
              }} />
            </Field>
            {result === "saved" && <p role="status">{copy.saved}</p>}
            {result === "saveError" && <Alert variant="destructive"><AlertDescription>{copy.saveError}</AlertDescription></Alert>}
            <Button type="submit" className="self-start" disabled={saving}>{copy.save}</Button>
          </FieldGroup>
        </WorkspaceForm>
      )}
      <WorkspaceFeedback workspaceId={workspace.id} />
    </WorkspaceShell>
  );
}

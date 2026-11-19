"use client";

import { Bot, FolderOpen } from "lucide-react";
import { useLocale } from "@/lib/locale-provider";
import { WorkspaceColumns, WorkspaceForm, WorkspaceHeading, WorkspaceRows, WorkspaceShell } from "@/components/workspace/layout";
import { invoke } from "@tauri-apps/api/core";
import { motion, useReducedMotion } from "motion/react";
import Link from "next/link";
import { useEffect, useState, type FormEvent } from "react";
import { Alert, AlertAction, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { useRouter } from "next/navigation";
import { useWorkspaceCatalog, workspaceHref } from "./workspace-router";
import type { Workspace } from "./workspace-frame";

export function WorkspaceHome() {
  const { status, workspaces, load, add } = useWorkspaceCatalog();
  const router = useRouter();
  const [name, setName] = useState("");
  const [needsProvider, setNeedsProvider] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState(false);
  const reducedMotion = useReducedMotion();
  const { messages } = useLocale();
  const copy = messages.workspaces;

  useEffect(() => {
    if (status !== "ready") return;
    let active = true;
    invoke<{ effective_provider: string | null }>("get_agent_settings", { workspaceId: null }).then(
      (settings) => { if (active) setNeedsProvider(settings.effective_provider === null); },
      () => {},
    );
    return () => { active = false; };
  }, [status]);

  async function createWorkspace(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!name.trim() || saving || status !== "ready") return;
    setSaving(true);
    setSaveError(false);
    try {
      const workspace = await invoke<Workspace>("create_workspace", { name });
      add(workspace);
      setName("");
      if (workspaces.length === 0 && needsProvider) router.push(`/settings?setup=${workspace.id}`);
    } catch {
      setSaveError(true);
    } finally {
      setSaving(false);
    }
  }

  return (
    <WorkspaceShell>
      <WorkspaceHeading icon={FolderOpen}>
        <h1>{copy.title}</h1>
        <p>{copy.intro}</p>
      </WorkspaceHeading>
      {status === "ready" && needsProvider && (
        <Alert className="mb-5">
          <Bot />
          <AlertTitle>{messages.agentSettings.setupTitle}</AlertTitle>
          <AlertDescription>{messages.agentSettings.setupIntro}</AlertDescription>
          <AlertAction><Button asChild size="sm" variant="outline">
            <Link href="/settings">{messages.agentSettings.setupAction}</Link>
          </Button></AlertAction>
        </Alert>
      )}
      <WorkspaceColumns>
        <section className="min-w-0 [&>p]:mt-3 [&>p]:max-w-[60ch] [&>p]:text-muted-foreground" aria-labelledby="workspace-list-title">
          <h2 id="workspace-list-title">{copy.list}</h2>
          {status === "loading" && <p role="status">{copy.loading}</p>}
          {status === "browser" && <p role="status">{copy.desktop}</p>}
          {status === "error" && (
            <Alert variant="destructive">
              <AlertDescription>{copy.loadError}</AlertDescription>
              <Button type="button" variant="outline" onClick={load}>{copy.retry}</Button>
            </Alert>
          )}
          {status === "ready" && (workspaces.length ? (
            <WorkspaceRows compact aria-live="polite">
              {workspaces.map((workspace) => (
                <motion.li
                  key={workspace.id}
                  initial={reducedMotion ? false : { opacity: 0.75, y: 5 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ duration: 0.22, ease: "easeOut" }}
                >
                  <Button className="h-auto min-h-10 w-full justify-start text-start whitespace-normal wrap-anywhere" variant="ghost" onClick={() => router.push(workspaceHref("profiles", workspace.id))}>
                    {workspace.name}
                  </Button>
                </motion.li>
              ))}
            </WorkspaceRows>
          ) : (
            <Empty className="min-h-44 border border-dashed">
              <EmptyHeader>
                <EmptyTitle>{copy.empty}</EmptyTitle>
                <EmptyDescription>{copy.emptyHint}</EmptyDescription>
              </EmptyHeader>
            </Empty>
          ))}
        </section>
        <WorkspaceForm  onSubmit={createWorkspace} aria-labelledby="new-workspace-title">
          <h2 id="new-workspace-title">{copy.form}</h2>
          <FieldGroup>
            <Field data-disabled={status !== "ready" || saving}>
              <FieldLabel htmlFor="workspace-name">{copy.name}</FieldLabel>
              <Input
                id="workspace-name"
                value={name}
                onChange={(event) => setName(event.target.value)}
                disabled={status !== "ready" || saving}
                required
              />
            </Field>
            {saveError && <Alert variant="destructive" role="alert"><AlertDescription>{copy.createError}</AlertDescription></Alert>}
            <Button type="submit" disabled={status !== "ready" || saving || !name.trim()}>
              {saving ? copy.creating : copy.create}
            </Button>
          </FieldGroup>
        </WorkspaceForm>
      </WorkspaceColumns>
    </WorkspaceShell>
  );
}

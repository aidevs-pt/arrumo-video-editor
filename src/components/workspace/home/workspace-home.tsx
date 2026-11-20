"use client";

import { invoke } from "@tauri-apps/api/core";
import { ArrowRight, FolderOpen, Plus } from "lucide-react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState, type FormEvent } from "react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { WorkspaceHeading, WorkspaceShell } from "@/components/workspace/layout";
import { useLocale } from "@/lib/locale-provider";
import { WorkspaceOnboarding } from "./workspace-onboarding";
import { useWorkspaceCatalog, workspaceHref } from "./workspace-router";
import type { Workspace } from "./workspace-links";

export function WorkspaceHome() {
  const { status, workspaces, load, add } = useWorkspaceCatalog();
  const router = useRouter();
  const { messages: { workspaces: copy } } = useLocale();
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [pending, setPending] = useState(false);
  const [saveError, setSaveError] = useState(false);

  async function createWorkspace(workspaceName: string) {
    const workspace = await invoke<Workspace>("create_workspace", { name: workspaceName });
    add(workspace);
    router.push(workspaceHref("context", workspace.id));
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending || !name.trim()) return;
    setPending(true);
    setSaveError(false);
    try {
      await createWorkspace(name.trim());
      setOpen(false);
      setName("");
    } catch {
      setSaveError(true);
    } finally {
      setPending(false);
    }
  }

  if (!workspaces.length && status !== "error") {
    return <WorkspaceOnboarding desktop={status === "ready"} loading={status === "loading"} onCreate={createWorkspace} />;
  }

  return (
    <WorkspaceShell>
      <div className="flex flex-wrap items-start justify-between gap-x-5 gap-y-2">
        <WorkspaceHeading><h1>{copy.title}</h1><p>{copy.intro}</p></WorkspaceHeading>
        <Dialog open={open} onOpenChange={(next) => {
          if (pending) return;
          setOpen(next);
          if (!next) { setName(""); setSaveError(false); }
        }}>
          <DialogTrigger asChild><Button disabled={status !== "ready"}><Plus data-icon="inline-start" />{copy.form}</Button></DialogTrigger>
          <DialogContent>
            <DialogHeader><DialogTitle>{copy.form}</DialogTitle><DialogDescription>{copy.intro}</DialogDescription></DialogHeader>
            <form className="flex flex-col gap-4" onSubmit={submit}>
              <Field data-disabled={pending}>
                <FieldLabel htmlFor="workspace-name">{copy.name}</FieldLabel>
                <Input id="workspace-name" autoComplete="organization" value={name} onChange={(event) => setName(event.target.value)} disabled={pending} required />
              </Field>
              {saveError && <Alert variant="destructive" role="alert"><AlertDescription>{copy.createError}</AlertDescription></Alert>}
              <DialogFooter><Button disabled={pending || !name.trim()}>{pending ? copy.creating : copy.create}</Button></DialogFooter>
            </form>
          </DialogContent>
        </Dialog>
      </div>
      {status === "error" && <Alert variant="destructive">
        <AlertDescription>{copy.loadError}</AlertDescription>
        <Button variant="outline" onClick={load}>{copy.retry}</Button>
      </Alert>}
      <ul className="divide-y border-y" aria-label={copy.list}>
        {workspaces.map((workspace) => <li key={workspace.id}>
          <Button className="h-auto min-h-14 w-full justify-start whitespace-normal py-3 text-start font-semibold" variant="ghost" asChild>
            <Link href={`/?workspace=${workspace.id}`}>
              <span className="grid size-8 shrink-0 place-items-center rounded-md bg-accent text-accent-foreground"><FolderOpen className="size-4" aria-hidden="true" /></span>
              <span className="min-w-0 flex-1 wrap-anywhere">{workspace.name}</span>
              <ArrowRight data-icon="inline-end" aria-hidden="true" />
            </Link>
          </Button>
        </li>)}
      </ul>
    </WorkspaceShell>
  );
}

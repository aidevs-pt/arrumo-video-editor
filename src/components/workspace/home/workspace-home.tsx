"use client";

import { handOffContextSources } from "@/components/workspace/context/context-research";
import { invoke } from "@tauri-apps/api/core";
import { Paintbrush, Plus } from "lucide-react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useState, type FormEvent } from "react";

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
import { WorkspaceMark } from "../workspaces/workspace-mark";

export function WorkspaceHome() {
  const { status, workspaces, load, add } = useWorkspaceCatalog();
  const router = useRouter();
  const { messages: { workspaces: copy } } = useLocale();
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [pending, setPending] = useState(false);
  const [saveError, setSaveError] = useState(false);

  async function createWorkspace(workspaceName: string, sources: string[] = []) {
    const workspace = await invoke<Workspace>("create_workspace", { name: workspaceName });
    add(workspace);
    if (sources.length) handOffContextSources(workspace.id, sources);
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

  // Onboarding only once the catalog is known to be empty, so it never flashes before workspaces load.
  if (status === "loading" && !workspaces.length) return null;
  if (!workspaces.length && status !== "error") {
    return <WorkspaceOnboarding desktop={status === "ready"} onCreate={createWorkspace} />;
  }

  return (
    <WorkspaceShell>
      <div className="mb-6 flex flex-wrap items-center justify-between gap-x-5 gap-y-2">
        <WorkspaceHeading className="mb-0 flex-1"><h1>{copy.title}</h1><p>{copy.intro}</p></WorkspaceHeading>
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
      <ul className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3" aria-label={copy.list}>
        {workspaces.map((workspace) => <WorkspaceCard key={workspace.id} workspace={workspace} />)}
        <li>
          <button type="button" disabled={status !== "ready"} onClick={() => setOpen(true)}
            className="flex size-full min-h-40 flex-col items-center justify-center gap-2 rounded-xl border border-dashed p-5 text-center text-muted-foreground transition-colors hover:border-primary hover:text-primary focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none disabled:opacity-50 motion-reduce:transition-none">
            <Plus className="size-5" aria-hidden="true" />
            <span className="font-semibold">{copy.form}</span>
            <span className="max-w-56 text-sm">{copy.intro}</span>
          </button>
        </li>
      </ul>
    </WorkspaceShell>
  );
}

function WorkspaceCard({ workspace }: { workspace: Workspace }) {
  const { messages: { workspaces: copy, studio } } = useLocale();
  const [counts, setCounts] = useState<{ profiles: number; videos: number }>();
  useEffect(() => {
    let active = true;
    Promise.all([
      invoke<unknown[]>("list_profiles", { workspaceId: workspace.id }),
      invoke<unknown[]>("list_generation_runs", { workspaceId: workspace.id }),
    ]).then(([profiles, runs]) => { if (active) setCounts({ profiles: profiles.length, videos: runs.length }); }, () => {});
    return () => { active = false; };
  }, [workspace.id]);

  return <li className="relative flex min-h-40 flex-col rounded-xl border bg-card p-5 transition-colors hover:border-foreground/40 motion-reduce:transition-none">
    <div className="flex items-start justify-between gap-3">
      <WorkspaceMark workspace={workspace} className="size-12" />
      <Button asChild size="icon-sm" variant="ghost" className="relative z-[1]">
        <Link href={workspaceHref("appearance", workspace.id)} aria-label={`${copy.customize} · ${workspace.name}`} title={copy.customize}><Paintbrush /></Link>
      </Button>
    </div>
    <h2 className="mt-4 text-[1.375rem] leading-tight font-[850] wrap-anywhere [font-variation-settings:'wdth'_125]">
      <Link href={`/?workspace=${workspace.id}`}
        className="rounded-sm outline-none after:absolute after:inset-0 after:rounded-xl focus-visible:after:ring-2 focus-visible:after:ring-ring">
        {workspace.name}
      </Link>
    </h2>
    <p className="mt-auto pt-2 text-sm text-muted-foreground">
      {counts ? (counts.profiles === 1 ? studio.workspaceCountsOne : studio.workspaceCounts)
        .replace("{profiles}", String(counts.profiles)).replace("{videos}", String(counts.videos)) : "\u00a0"}
    </p>
  </li>;
}

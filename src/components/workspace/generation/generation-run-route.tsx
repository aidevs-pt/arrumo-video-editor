"use client";

import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft } from "lucide-react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { useStudioTrail } from "@/components/workspace/home/studio-trail";
import { profileHref } from "@/components/workspace/home/workspace-links";
import { useWorkspaceCatalog, workspaceHref } from "@/components/workspace/home/workspace-router";
import { WorkspaceShell } from "@/components/workspace/layout";
import { Skeleton } from "@/components/ui/skeleton";
import { useLocale } from "@/lib/locale-provider";
import { ConfirmTitleDelete } from "../confirm-title-delete";
import { GenerationRunControls, GenerationRunPanels, RunStatus, runTitle, type Run, type RunAction } from "./generation-run-row";

export function GenerationRunRoute() {
  const { workspace } = useWorkspaceCatalog();
  const query = useSearchParams();
  const rawRun = query.get("run") ?? "";
  const runId = Number(rawRun);
  if (!workspace) return null;
  return <GenerationRunDetail key={`${workspace.id}-${rawRun}`} workspaceId={workspace.id}
    runId={/^[1-9]\d*$/.test(rawRun) && Number.isSafeInteger(runId) ? runId : null} />;
}

function GenerationRunDetail({ workspaceId, runId }: { workspaceId: number; runId: number | null }) {
  const { messages } = useLocale();
  const copy = messages.queue;
  const router = useRouter();
  const [run, setRun] = useState<Run | null>();
  const [loadError, setLoadError] = useState(false);
  const [controlError, setControlError] = useState(false);
  const [pending, setPending] = useState(false);

  useEffect(() => {
    if (runId === null || pending) return;
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      try {
        const next = await invoke<Run | null>("get_generation_run", { workspaceId, runId });
        if (!active) return;
        setRun(next);
        setLoadError(false);
      } catch {
        if (active) setLoadError(true);
      }
      if (active) timer = setTimeout(refresh, 2000);
    }
    void refresh();
    return () => { active = false; clearTimeout(timer); };
  }, [pending, runId, workspaceId]);

  async function control(id: number, action: RunAction) {
    if (pending) return;
    setPending(true);
    setControlError(false);
    try {
      setRun(await invoke<Run>("control_generation", { workspaceId, runId: id, action }));
    } catch {
      setControlError(true);
    } finally {
      setPending(false);
    }
  }

  const profile = run?.snapshot.profile;
  useStudioTrail([
    { label: messages.studio.videos, href: workspaceHref("generations", workspaceId) },
    ...(profile ? [{ label: profile.name, href: profileHref(workspaceId, profile.id) }] : []),
    { label: run ? runTitle(run) : copy.runDetail },
  ]);
  const problem = runId === null || run === null ? copy.missingRun : loadError ? copy.loadError : undefined;
  return (
    <div className="min-h-[calc(100svh-3.5rem)] bg-agent text-white">
      <WorkspaceShell className="flex max-w-[94rem] flex-col gap-5">
        <header className="grid items-end gap-4 lg:grid-cols-[minmax(0,1fr)_auto]">
          <div className="flex min-w-0 flex-col items-start gap-3">
            <Button asChild variant="ghost" size="sm" className="-ms-2 text-white/80 hover:bg-white/10 hover:text-white">
              <Link href={workspaceHref("generations", workspaceId)}><ArrowLeft data-icon="inline-start" />{messages.studio.videos}</Link>
            </Button>
            {profile && <span className="flex items-center gap-2 text-sm font-semibold text-white/90">
              <span aria-hidden="true" className="size-3 rounded-[3px] ring-1 ring-white/60" style={{ backgroundColor: profile.color ?? "transparent" }} />
              {profile.name}
            </span>}
            {run && <span className="[&_[data-slot=badge][data-variant=outline]]:border-white/50 [&_[data-slot=badge][data-variant=outline]]:text-white [&_[data-slot=badge][data-variant=secondary]]:bg-white/15 [&_[data-slot=badge][data-variant=secondary]]:text-white"><RunStatus run={run} /></span>}
            <h1 className="max-w-[24ch] text-[clamp(2rem,4.5vw,3rem)] leading-[1] font-[850] tracking-[-.01em] text-balance wrap-anywhere [font-variation-settings:'wdth'_130]">
              {run ? runTitle(run) : copy.runDetail}
            </h1>
          </div>
          {run && (
            <div className="flex flex-wrap items-center gap-2 self-end [&_[data-slot=badge][data-variant=outline]]:border-white/50 [&_[data-slot=badge][data-variant=outline]]:text-white [&_[data-slot=badge][data-variant=secondary]]:bg-white/15 [&_[data-slot=badge][data-variant=secondary]]:text-white">
              <GenerationRunControls run={run} disabled={pending} onControl={control}
                deleteMenu={(open, setOpen) => <ConfirmTitleDelete title={runTitle(run)} disabled={pending} copy={copy} hideTrigger
                  open={open} onOpenChange={setOpen}
                  onConfirm={(confirmedTitle) => invoke<boolean>("delete_generation", {
                    workspaceId, runId: run.id, confirmedTitle,
                  })}
                  onDelete={() => router.replace(workspaceHref("generations", workspaceId))} />} />
            </div>
          )}
        </header>
        {problem && <Alert variant="destructive"><AlertDescription>{problem}</AlertDescription></Alert>}
        {controlError && <Alert variant="destructive"><AlertDescription>{copy.controlError}</AlertDescription></Alert>}
        {run ? <GenerationRunPanels run={run} workspaceId={workspaceId} />
          : !problem && <Skeleton className="min-h-40 bg-white/15" aria-label={copy.loading} />}
      </WorkspaceShell>
    </div>
  );
}

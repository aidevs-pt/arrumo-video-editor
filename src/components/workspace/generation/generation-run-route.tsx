"use client";

import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft } from "lucide-react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { useWorkspaceCatalog, workspaceHref } from "@/components/workspace/home/workspace-router";
import { WorkspaceShell } from "@/components/workspace/layout";
import { Skeleton } from "@/components/ui/skeleton";
import { useLocale } from "@/lib/locale-provider";
import { ConfirmTitleDelete } from "../confirm-title-delete";
import { GenerationRunControls, GenerationRunPanels, runTitle, type Run, type RunAction } from "./generation-run-row";

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
  const { messages: { queue: copy } } = useLocale();
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

  const problem = runId === null || run === null ? copy.missingRun : loadError ? copy.loadError : undefined;
  return (
    <div className="min-h-[calc(100svh-3.5rem)] bg-agent text-white">
      <WorkspaceShell className="flex max-w-[94rem] flex-col gap-5">
        <header className="grid items-end gap-4 lg:grid-cols-[minmax(0,1fr)_auto]">
          <div className="flex min-w-0 flex-col items-start gap-3">
            <Button asChild variant="ghost" size="sm" className="-ms-2 text-white/80 hover:bg-white/10 hover:text-white">
              <Link href={workspaceHref("generations", workspaceId)}><ArrowLeft data-icon="inline-start" />{copy.title}</Link>
            </Button>
            <h1 className="max-w-[28ch] text-3xl leading-tight font-[850] wrap-anywhere sm:text-[2.125rem]">
              {run ? runTitle(run) : copy.runDetail}
            </h1>
          </div>
          {run && (
            <div className="flex flex-wrap items-center gap-2 self-end">
              <GenerationRunControls run={run} workspaceId={workspaceId} disabled={pending} onControl={control} />
              {run.state !== "completed" && <ConfirmTitleDelete title={runTitle(run)} disabled={pending} copy={copy}
                onConfirm={(confirmedTitle) => invoke<boolean>("delete_generation", {
                  workspaceId, runId: run.id, confirmedTitle,
                })}
                onDelete={() => router.replace(workspaceHref("generations", workspaceId))} />}
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

"use client";

import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft, ListVideo } from "lucide-react";
import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { useWorkspaceCatalog, workspaceHref } from "@/components/workspace/home/workspace-router";
import { WorkspaceHeading, WorkspaceRows, WorkspaceShell } from "@/components/workspace/layout";
import { useLocale } from "@/lib/locale-provider";
import { GenerationRunRow, type Run, type RunAction } from "./generation-run-row";

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
    <WorkspaceShell>
      <Button asChild variant="ghost">
        <Link href={workspaceHref("generations", workspaceId)}>
          <ArrowLeft data-icon="inline-start" />{copy.title}
        </Link>
      </Button>
      <WorkspaceHeading icon={ListVideo}><h1>{copy.runDetail}</h1></WorkspaceHeading>
      {problem && <Alert variant="destructive"><AlertDescription>{problem}</AlertDescription></Alert>}
      {controlError && <Alert variant="destructive"><AlertDescription>{copy.controlError}</AlertDescription></Alert>}
      {run ? (
        <WorkspaceRows><GenerationRunRow run={run} workspaceId={workspaceId} disabled={pending}
          onControl={control} detail /></WorkspaceRows>
      ) : !problem && <p role="status">{copy.loading}</p>}
    </WorkspaceShell>
  );
}

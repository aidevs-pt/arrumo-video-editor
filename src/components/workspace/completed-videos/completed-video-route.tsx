"use client";

import { ArrowLeft, Film } from "lucide-react";
import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { useWorkspaceCatalog, workspaceHref } from "@/components/workspace/home/workspace-router";
import { WorkspaceHeading, WorkspaceShell } from "@/components/workspace/layout";
import { useLocale } from "@/lib/locale-provider";
import { CompletedVideoDetail } from "./completed-video-detail";

export function CompletedVideoRoute() {
  const { workspace } = useWorkspaceCatalog();
  const query = useSearchParams();
  const { messages } = useLocale();
  const rawRun = query.get("run") ?? "";
  const runId = Number(rawRun);
  if (!workspace) return null;

  return (
    <WorkspaceShell>
      <Button asChild variant="ghost" className="mb-4">
        <Link href={workspaceHref("videos", workspace.id)}>
          <ArrowLeft data-icon="inline-start" />{messages.studio.completedVideos}
        </Link>
      </Button>
      {/^[1-9]\d*$/.test(rawRun) && Number.isSafeInteger(runId) ? (
        <CompletedVideoDetail key={`${workspace.id}-${runId}`} workspaceId={workspace.id} runId={runId} />
      ) : (
        <>
          <WorkspaceHeading icon={Film}><h1>{messages.completedVideo.details}</h1></WorkspaceHeading>
          <Alert variant="destructive"><AlertDescription>{messages.completedVideo.loadError}</AlertDescription></Alert>
        </>
      )}
    </WorkspaceShell>
  );
}

"use client";

import { invoke } from "@tauri-apps/api/core";
import { ArrowUpRight, Captions, Film, Puzzle, Search } from "lucide-react";
import Link from "next/link";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { useWorkspaceCatalog } from "@/components/workspace/home/workspace-router";
import { completedVideoHref, workspaceHref } from "@/components/workspace/home/workspace-links";
import { WorkspaceShell } from "@/components/workspace/layout";
import { useLocale } from "@/lib/locale-provider";
import { ItemContextMenu } from "@/components/workspace/item-context-menu";
import type { InstalledAction } from "@/components/workspace/plugins/action-form";
import { ActionSheet, usePlacedActionLabel, usePlacedActions } from "@/components/workspace/plugins/placed-actions";
import { completedSubtitleTracks } from "@/components/workspace/completed-videos/completed-subtitle-tracks";
import { VideoCatalogHeader } from "./video-catalog-header";

type Run = {
  id: number;
  state: string;
  display_title: string | null;
  snapshot: { draft: { title: string }; profile?: { name: string } | null };
};

export function CompletedVideoLibrary() {
  const { workspace } = useWorkspaceCatalog();
  return workspace ? <VideoLibrary key={workspace.id} workspaceId={workspace.id} /> : null;
}

function VideoLibrary({ workspaceId }: { workspaceId: number }) {
  const { locale, messages } = useLocale();
  const copy = messages.studio;
  const [runs, setRuns] = useState<Run[]>();
  const [failed, setFailed] = useState(false);
  const [load, setLoad] = useState(0);
  const [search, setSearch] = useState("");
  const actions = usePlacedActions(workspaceId, "completed-video");
  const actionLabel = usePlacedActionLabel();
  const [action, setAction] = useState<{ entry: InstalledAction; runId: number }>();
  const normalized = search.trim().toLocaleLowerCase(locale);
  const filtered = runs?.filter((run) => title(run).toLocaleLowerCase(locale).includes(normalized));

  useEffect(() => {
    let active = true;
    invoke<Run[]>("list_generation_runs", { workspaceId }).then(
      (results) => {
        if (!active) return;
        setRuns(results.filter((run) => run.state === "completed").reverse());
        setFailed(false);
      },
      () => { if (active) setFailed(true); },
    );
    return () => { active = false; };
  }, [load, workspaceId]);

  return (
    <WorkspaceShell>
      <VideoCatalogHeader workspaceId={workspaceId} active="videos" count={runs?.length} />
      {failed ? (
        <Alert variant="destructive">
          <AlertDescription>{messages.queue.loadError}</AlertDescription>
          <Button size="sm" variant="outline" onClick={() => { setFailed(false); setLoad((value) => value + 1); }}>
            {messages.queue.retry}
          </Button>
        </Alert>
      ) : !runs || !filtered ? (
        <div className="flex flex-col gap-3" role="status" aria-label={messages.queue.loading}>
          <Skeleton className="h-8 w-52" /><Skeleton className="h-32 w-full" />
        </div>
      ) : runs.length === 0 ? (
        <Empty className="rounded-xl border">
          <EmptyHeader>
            <EmptyMedia variant="icon"><Film /></EmptyMedia>
            <EmptyTitle>{copy.noCompleted}</EmptyTitle>
            <EmptyDescription>{copy.completedHint}</EmptyDescription>
          </EmptyHeader>
          <Button asChild variant="outline">
            <Link href={workspaceHref("generations", workspaceId)}>{messages.queue.title}</Link>
          </Button>
        </Empty>
      ) : (
        <div className="flex flex-col gap-4">
          <div className="relative w-full max-w-xs">
            <Search className="pointer-events-none absolute start-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
            <Input type="search" className="ps-9" aria-label={copy.searchCompleted} placeholder={copy.searchCompleted}
              value={search} onChange={(event) => setSearch(event.target.value)} />
          </div>
          <ul className="divide-y border-y" aria-label={copy.completedVideos}>
            {filtered.map((run) => (
              <ItemContextMenu key={run.id} label={title(run)} items={[
                { label: messages.completedVideo.details, icon: ArrowUpRight, href: completedVideoHref(workspaceId, run.id) },
                { label: messages.completedVideo.subtitleText, icon: Captions, href: completedVideoHref(workspaceId, run.id, undefined, "subtitles") },
                ...actions.map((entry, index) => ({ label: actionLabel(entry), icon: Puzzle, group: index === 0, onSelect: () => setAction({ entry, runId: run.id }) })),
              ]}>
              <li>
                <Link className="group flex items-center gap-3 px-1 py-3 hover:bg-muted/60 sm:gap-4"
                  href={completedVideoHref(workspaceId, run.id)}>
                  <VideoPoster workspaceId={workspaceId} runId={run.id} />
                  <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                    <span className="wrap-anywhere font-semibold">{title(run)}</span>
                    <span className="text-sm text-muted-foreground">{run.snapshot.profile?.name ?? messages.drafts.noProfile}</span>
                  </span>
                  <ArrowUpRight className="me-2 size-4 shrink-0 text-muted-foreground group-hover:text-primary" aria-hidden="true" />
                </Link>
              </li>
              </ItemContextMenu>
            ))}
            {filtered.length === 0 && <li className="p-4 text-muted-foreground">{copy.noSearchMatches}</li>}
          </ul>
        </div>
      )}
      <ActionSheet workspaceId={workspaceId} entry={action?.entry} onClose={() => setAction(undefined)}
        context={action && { subtitleTracks: () => completedSubtitleTracks(workspaceId, action.runId, locale, messages.subtitleRevisions.primary) }} />
    </WorkspaceShell>
  );
}

function title(run: Run) {
  return run.display_title ?? run.snapshot.draft.title;
}

/** The video's thumbnail or a frame of it, with the film icon while it loads or when there is none. */
function VideoPoster({ workspaceId, runId }: { workspaceId: number; runId: number }) {
  const [url, setUrl] = useState<string>();
  useEffect(() => {
    let active = true;
    invoke<string>("completed_video_poster", { workspaceId, runId })
      .then((value) => { if (active) setUrl(value); }, () => {});
    return () => { active = false; };
  }, [workspaceId, runId]);
  return (
    <span className="grid aspect-video w-24 shrink-0 place-items-center overflow-hidden rounded-thumb bg-muted text-muted-foreground sm:w-36" aria-hidden="true">
      {url
        // Managed local still served by the desktop host.
        // eslint-disable-next-line @next/next/no-img-element
        ? <img src={url} alt="" className="size-full object-cover" onError={() => setUrl(undefined)} />
        : <Film className="size-4" />}
    </span>
  );
}

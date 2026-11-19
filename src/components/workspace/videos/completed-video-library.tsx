"use client";

import { invoke } from "@tauri-apps/api/core";
import { ArrowUpRight, Film, Search } from "lucide-react";
import Link from "next/link";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { useWorkspaceCatalog } from "@/components/workspace/home/workspace-router";
import { completedVideoHref, workspaceHref } from "@/components/workspace/home/workspace-links";
import { WorkspaceHeading, WorkspaceShell } from "@/components/workspace/layout";
import { useLocale } from "@/lib/locale-provider";

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
      <WorkspaceHeading icon={Film}>
        <h1>{copy.completedVideos}</h1>
        <p>{copy.completedHint}</p>
      </WorkspaceHeading>
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
            <Search className="pointer-events-none absolute start-2.5 top-2 size-4 text-muted-foreground" aria-hidden="true" />
            <Input type="search" className="ps-8" aria-label={copy.searchCompleted} placeholder={copy.searchCompleted}
              value={search} onChange={(event) => setSearch(event.target.value)} />
          </div>
          <div className="overflow-hidden rounded-xl border">
            <Table>
              <TableHeader className="bg-muted/50"><TableRow>
                <TableHead className="ps-4">{messages.completedVideo.title}</TableHead>
                <TableHead>{messages.drafts.profile}</TableHead>
                <TableHead className="text-end pe-4">{messages.queue.completed}</TableHead>
              </TableRow></TableHeader>
              <TableBody>
                {filtered.map((run) => (
                  <TableRow key={run.id}>
                    <TableCell className="min-w-32 ps-4 whitespace-normal">
                      <Link className="inline-flex items-center gap-1.5 font-medium underline-offset-4 hover:underline"
                        href={completedVideoHref(workspaceId, run.id)}>
                        {title(run)}<ArrowUpRight className="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
                      </Link>
                    </TableCell>
                    <TableCell className="whitespace-normal text-muted-foreground">{run.snapshot.profile?.name ?? messages.drafts.noProfile}</TableCell>
                    <TableCell className="text-end pe-4"><Badge variant="secondary">{messages.queue.completed}</Badge></TableCell>
                  </TableRow>
                ))}
                {filtered.length === 0 && <TableRow><TableCell colSpan={3} className="p-4 text-muted-foreground">{copy.noSearchMatches}</TableCell></TableRow>}
              </TableBody>
            </Table>
          </div>
        </div>
      )}
    </WorkspaceShell>
  );
}

function title(run: Run) {
  return run.display_title ?? run.snapshot.draft.title;
}

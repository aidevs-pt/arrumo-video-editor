"use client";

import { invoke } from "@tauri-apps/api/core";
import { Upload } from "lucide-react";
import Link from "next/link";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { workspaceHref } from "@/components/workspace/home/workspace-links";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";

type CatalogSection = "drafts" | "generations" | "videos";
type Counts = Record<CatalogSection, number> & { reviews: number };
type RunState = { draft_id: number; state: string; pending_review_id: number | null };

/** Counts every section from the same data so the tabs agree with each other, whatever page is open. */
function useCatalogCounts(workspaceId: number, refreshKey?: number) {
  const [counts, setCounts] = useState<Counts>();
  useEffect(() => {
    let active = true;
    Promise.all([
      invoke<{ id: number }[]>("list_video_drafts", { workspaceId }),
      invoke<RunState[]>("list_generation_runs", { workspaceId }),
    ]).then(([drafts, runs]) => {
      if (!active) return;
      const submitted = new Set(runs.map((run) => run.draft_id));
      setCounts({
        drafts: drafts.filter((draft) => !submitted.has(draft.id)).length,
        generations: runs.filter((run) => !["completed", "cancelled"].includes(run.state)).length,
        videos: runs.filter((run) => run.state === "completed").length,
        reviews: runs.filter((run) => run.state === "paused" && run.pending_review_id !== null).length,
      });
    }, () => {});
    return () => { active = false; };
  }, [workspaceId, refreshKey]);
  return counts;
}

export function VideoCatalogHeader({ workspaceId, active, count }: {
  workspaceId: number;
  active: CatalogSection;
  count?: number;
}) {
  const { messages } = useLocale();
  const copy = messages.videoCatalog;
  const counts = useCatalogCounts(workspaceId, count);
  const sections = [
    { id: "drafts", label: copy.drafts },
    { id: "generations", label: copy.generating },
    { id: "videos", label: copy.completed },
  ] as const;

  return (
    <header className="mb-5">
      <div className="flex flex-wrap items-start justify-between gap-3 pb-4">
        <div className="min-w-0">
          <h1 className="wrap-anywhere font-heading text-[clamp(2.25rem,5vw,3.25rem)] leading-[1] font-[850] [font-variation-settings:'wdth'_135]">{copy.title}</h1>
          <p className="mt-2 text-lg text-muted-foreground">{copy.description}</p>
        </div>
        <Button size="lg" asChild>
          <Link href={`${workspaceHref("drafts", workspaceId)}&compose=1`}>
            <Upload data-icon="inline-start" />{copy.generate}
          </Link>
        </Button>
      </div>
      <nav className="flex flex-wrap gap-x-5 border-b" aria-label={copy.title}>
        {sections.map(({ id, label }) => (
          <Link
            key={id}
            href={workspaceHref(id, workspaceId)}
            aria-current={id === active ? "page" : undefined}
            className={cn("-mb-px inline-flex min-h-11 items-center gap-2 border-b-[3px] border-transparent px-1 font-semibold text-muted-foreground transition-colors hover:text-foreground",
              id === active && "border-primary text-foreground")}
          >
            {label}
            {counts && (id === "generations" && counts.reviews > 0
              ? <span className="rounded-[5px] bg-attention px-1.5 text-xs font-bold text-ink tabular-nums"
                aria-label={`${counts.generations}, ${counts.reviews} ${messages.productionPlan.reviewRequired}`}>{counts.generations}</span>
              : <span className="rounded-[5px] bg-secondary px-1.5 text-xs tabular-nums text-muted-foreground">{counts[id]}</span>)}
          </Link>
        ))}
      </nav>
    </header>
  );
}

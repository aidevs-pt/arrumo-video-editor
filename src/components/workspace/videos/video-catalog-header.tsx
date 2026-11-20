"use client";

import { Plus } from "lucide-react";
import Link from "next/link";
import { Button } from "@/components/ui/button";
import { workspaceHref } from "@/components/workspace/home/workspace-links";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";

type CatalogSection = "drafts" | "generations" | "videos";

export function VideoCatalogHeader({ workspaceId, active, count }: {
  workspaceId: number;
  active: CatalogSection;
  count?: number;
}) {
  const { messages: { videoCatalog: copy } } = useLocale();
  const sections = [
    { id: "drafts", label: copy.drafts },
    { id: "generations", label: copy.generating },
    { id: "videos", label: copy.completed },
  ] as const;

  return (
    <header className="mb-5">
      <div className="flex flex-wrap items-start justify-between gap-3 pb-4">
        <div className="min-w-0">
          <h1 className="wrap-anywhere font-heading text-[2.125rem] leading-tight font-[850] [font-stretch:135%]">{copy.title}</h1>
          <p className="mt-1 text-muted-foreground">{copy.description}</p>
        </div>
        <Button asChild>
          <Link href={`${workspaceHref("drafts", workspaceId)}&compose=1`}>
            <Plus data-icon="inline-start" />{copy.generate}
          </Link>
        </Button>
      </div>
      <nav className="flex flex-wrap gap-x-5 border-b" aria-label={copy.title}>
        {sections.map(({ id, label }) => (
          <Link
            key={id}
            href={workspaceHref(id, workspaceId)}
            aria-current={id === active ? "page" : undefined}
            className={cn("inline-flex min-h-10 items-center gap-2 border-b-2 border-transparent px-1 text-sm font-semibold text-muted-foreground transition-colors hover:text-foreground",
              id === active && "border-primary text-foreground")}
          >
            {label}
            {id === active && count !== undefined && (
              <span className="text-xs tabular-nums text-muted-foreground">{count}</span>
            )}
          </Link>
        ))}
      </nav>
    </header>
  );
}

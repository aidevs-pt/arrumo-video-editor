"use client";

import Link from "next/link";
import type { Ref } from "react";
import { workspaceHref, type Workspace, type WorkspaceView } from "@/components/workspace/home/workspace-links";
import { WorkspaceHeading } from "@/components/workspace/layout";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";

export function WorkspaceSettingsHeading({ workspace, current, headingRef }: {
  workspace: Workspace;
  current: WorkspaceView;
  headingRef?: Ref<HTMLHeadingElement>;
}) {
  const { messages } = useLocale();
  const tabs = [
    ["context", messages.context.title],
    ["brand", messages.brand.title],
    ["assets", messages.assets.title],
    ["outputs", messages.outputs.title],
    ["motion-library", messages.motionLibrary.title],
  ] as const;
  return <>
    <WorkspaceHeading className="mb-3">
      <h1 tabIndex={-1} ref={headingRef}>{messages.studio.workspaceSettings}</h1>
      <p>{messages.studio.workspaceSettingsHint.replace("{workspace}", workspace.name)}</p>
    </WorkspaceHeading>
    <nav aria-label={messages.studio.workspaceSettings} className="mb-6 flex flex-wrap gap-x-5 gap-y-1 border-b">
      {tabs.map(([view, label]) => (
        <Link key={view} href={workspaceHref(view, workspace.id)} aria-current={view === current ? "page" : undefined}
          className={cn("border-b-2 border-transparent py-2 text-sm font-semibold text-muted-foreground hover:text-foreground",
            view === current && "border-primary text-foreground")}>{label}</Link>
      ))}
    </nav>
  </>;
}

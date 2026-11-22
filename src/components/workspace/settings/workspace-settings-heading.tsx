"use client";

import Link from "next/link";
import type { Ref } from "react";
import { workspaceHref, type Workspace, type WorkspaceView } from "@/components/workspace/home/workspace-links";
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
    ["motion-library", messages.profileHub.animations],
    ["appearance", messages.workspaces.appearance],
  ] as const;
  return <header className="mb-8">
    <h1 tabIndex={-1} ref={headingRef} className="text-[clamp(2.25rem,5vw,3.25rem)] leading-[1] font-[850] outline-none [font-variation-settings:'wdth'_135]">{messages.studio.workspaceSettings}</h1>
    <p className="mt-2 text-lg text-muted-foreground">{messages.studio.workspaceSettingsHint.replace("{workspace}", workspace.name)}</p>
    <nav aria-label={messages.studio.workspaceSettings} className="mt-8 flex gap-x-1 scroll-fade-x overflow-x-auto border-b [scrollbar-width:none]">
      {tabs.map(([view, label]) => (
        <Link key={view} href={workspaceHref(view, workspace.id)} aria-current={view === current ? "page" : undefined}
          className={cn("-mb-px inline-flex min-h-11 shrink-0 items-center border-b-[3px] border-transparent px-3 font-semibold text-muted-foreground transition-colors duration-150 hover:text-foreground",
            view === current && "border-primary text-foreground")}>{label}</Link>
      ))}
    </nav>
  </header>;
}

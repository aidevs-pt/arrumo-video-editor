import { ChevronRight, Clapperboard, FileText, FolderOpen, Images, Layers, ListVideo, Palette, Plus, SlidersHorizontal } from "lucide-react";
import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { useLocale } from "@/lib/locale-provider";
import { WorkspaceOverview } from "./workspace-overview";

export type Workspace = { id: number; name: string };
export type WorkspaceView = "profiles" | "context" | "brand" | "assets" | "outputs" | "drafts" | "generations";

export function WorkspaceFrame({ workspaces, workspace, view, onSelect, onNavigate, children }: {
  workspaces: Workspace[];
  workspace: Workspace | null;
  view: WorkspaceView;
  onSelect: (workspace: Workspace | null) => void;
  onNavigate: (view: WorkspaceView) => void;
  children: ReactNode;
}) {
  const { messages } = useLocale();
  const copy = messages.studio;
  const sections = [
    { view: "profiles", label: messages.profiles.title, icon: Layers },
    { view: "context", label: messages.profiles.context, icon: FileText },
    { view: "brand", label: messages.brand.title, icon: Palette },
    { view: "assets", label: messages.assets.title, icon: Images },
    { view: "outputs", label: messages.outputs.title, icon: SlidersHorizontal },
    { view: "drafts", label: messages.drafts.title, icon: Clapperboard },
    { view: "generations", label: messages.queue.title, icon: ListVideo },
  ] as const;
  const workspaceTabs = sections.filter(({ view: target }) =>
    target === "context" || target === "brand" || target === "assets" || target === "outputs");
  const workspaceSection = workspaceTabs.some(({ view: target }) => target === view);
  const primarySections = sections.filter(({ view: target }) =>
    target === "profiles" || target === "drafts" || target === "generations");
  const current = sections.find((section) => section.view === view);
  const Icon = current?.icon ?? FolderOpen;

  return (
    <div className="min-h-dvh min-[800px]:grid min-[800px]:grid-cols-[12rem_minmax(0,1fr)]">
      <aside className="border-b bg-sidebar text-sidebar-foreground min-[800px]:sticky min-[800px]:top-0 min-[800px]:h-dvh min-[800px]:overflow-y-auto min-[800px]:border-r min-[800px]:border-b-0">
        <div className="flex flex-col gap-4 px-4 py-3 min-[800px]:gap-6 min-[800px]:py-5">
          <div className="flex flex-wrap items-center justify-between gap-3 min-[800px]:flex-col min-[800px]:items-stretch min-[800px]:gap-6">
            <Button variant="ghost" className="-mx-1 h-auto justify-start gap-2.5 p-1 text-start" onClick={() => onSelect(null)}>
              {/* eslint-disable-next-line @next/next/no-img-element */}
              <img src="/arrumo-icon.svg" alt="" width={32} height={32} />
              <span className="flex flex-col">
                <span className="text-lg leading-none font-bold tracking-tight">Arrumo</span>
                <span className="mt-1 text-xs font-medium text-muted-foreground">{copy.videoStudio}</span>
              </span>
            </Button>
            {workspaces.length > 0 && (
              <NativeSelect aria-label={copy.workspace} value={workspace?.id ?? ""}
                className="max-w-48 min-[800px]:w-full"
                onChange={(event) => onSelect(workspaces.find(({ id }) => String(id) === event.target.value) ?? null)}>
                <NativeSelectOption value="">{messages.workspaces.title}</NativeSelectOption>
                {workspaces.map(({ id, name }) => <NativeSelectOption key={id} value={id}>{name}</NativeSelectOption>)}
              </NativeSelect>
            )}
          </div>
          {workspace && (
            <nav aria-label={copy.navigation} className="flex flex-wrap gap-1 min-[800px]:flex-col">
              <Button type="button" variant={workspaceSection ? "default" : "ghost"}
                aria-current={workspaceSection ? "true" : undefined}
                className="min-h-9 justify-start gap-2.5" onClick={() => onNavigate("context")}>
                <FolderOpen data-icon="inline-start" />{copy.workspaceSection}
              </Button>
              {primarySections.map(({ view: target, label, icon: SectionIcon }) => (
                <Button key={target} type="button" variant={target === view ? "default" : "ghost"}
                  aria-current={target === view ? "page" : undefined}
                  className="h-auto min-h-9 justify-start gap-2.5 whitespace-normal text-start"
                  onClick={() => onNavigate(target)}><SectionIcon data-icon="inline-start" />{target === "generations" ? copy.generations : label}</Button>
              ))}
            </nav>
          )}
        </div>
      </aside>

      <div className="min-w-0">
        <header className="flex min-h-14 items-center justify-between gap-3 border-b px-4 sm:px-7">
          <div className="flex min-w-0 items-center gap-2 text-sm">
            <Icon className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
            {workspace ? (
              <>
                <span className="truncate font-medium">{workspace.name}</span>
                <ChevronRight className="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
                <span className="truncate text-muted-foreground">{current?.label}</span>
              </>
            ) : (
              <span className="font-medium">{messages.workspaces.title}</span>
            )}
          </div>
          {workspace && (
            <Button onClick={() => onNavigate("drafts")} size="sm">
              <Plus data-icon="inline-start" />{copy.newVideo}
            </Button>
          )}
        </header>
        <div className={workspace ? "grid min-h-[calc(100dvh-3.5rem)] min-[1200px]:grid-cols-[minmax(0,1fr)_15.5rem]" : undefined}>
          <div className="min-w-0">
            {workspace && workspaceSection && (
              <nav aria-label={copy.workspaceSettings} className="flex flex-wrap gap-1 border-b px-4 py-2.5 sm:px-7">
                {workspaceTabs.map(({ view: target, label, icon: TabIcon }) => (
                  <Button key={target} type="button" variant={target === view ? "secondary" : "ghost"}
                    aria-current={target === view ? "page" : undefined}
                    onClick={() => onNavigate(target)}><TabIcon data-icon="inline-start" />{label}</Button>
                ))}
              </nav>
            )}
            {children}
          </div>
          {workspace && (
            <aside className="hidden border-l bg-sidebar px-4 pt-7 min-[1200px]:block" aria-label={copy.overview}>
              <div className="sticky top-7 flex flex-col gap-5">
                <WorkspaceOverview key={workspace.id} workspaceId={workspace.id} view={view} onNavigate={onNavigate} />
              </div>
            </aside>
          )}
        </div>
      </div>
    </div>
  );
}

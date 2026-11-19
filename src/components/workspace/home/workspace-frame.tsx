import { ChevronDown, Clapperboard, FileText, Film, FolderOpen, Images, Layers, ListVideo, Palette, Plus, Settings2, SlidersHorizontal, X } from "lucide-react";
import Link from "next/link";
import { usePathname, useSearchParams } from "next/navigation";
import type { CSSProperties, ReactNode } from "react";
import { Breadcrumb, BreadcrumbItem, BreadcrumbLink, BreadcrumbList, BreadcrumbPage, BreadcrumbSeparator } from "@/components/ui/breadcrumb";
import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuGroup, DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import {
  Sidebar, SidebarContent, SidebarGroup, SidebarGroupContent, SidebarGroupLabel, SidebarHeader, SidebarFooter,
  SidebarMenu, SidebarMenuButton, SidebarMenuItem, SidebarProvider, SidebarSeparator, SidebarTrigger, useSidebar,
} from "@/components/ui/sidebar";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";
import { ReviewInbox, useGenerationInbox } from "./review-inbox";
import { WorkspaceOverview } from "./workspace-overview";
import { WorkspaceProfileNavigation } from "./workspace-profile-navigation";
import { workspaceHref, type Workspace, type WorkspaceView } from "./workspace-links";

export type { Workspace, WorkspaceView } from "./workspace-links";

export function WorkspaceFrame(props: {
  workspaces: Workspace[];
  workspace: Workspace | null;
  view: WorkspaceView;
  onSelect: (workspace: Workspace | null) => void;
  onNavigate: (view: WorkspaceView) => void;
  children: ReactNode;
}) {
  return (
    <TooltipProvider delayDuration={250}>
      <SidebarProvider style={{ "--sidebar-width": "13rem" } as CSSProperties}><StudioLayout {...props} /></SidebarProvider>
    </TooltipProvider>
  );
}

function StudioLayout({ workspaces, workspace, view, onSelect, onNavigate, children }: Parameters<typeof WorkspaceFrame>[0]) {
  const { messages } = useLocale();
  const { setOpenMobile } = useSidebar();
  const pathname = usePathname().replace(/\/$/, "");
  const query = useSearchParams();
  const copy = messages.studio;
  const inbox = useGenerationInbox(workspace?.id);
  const hasPendingReviews = (inbox?.inbox?.reviews.length ?? 0) > 0;
  const workspaceTabs = [
    { view: "context", label: messages.profiles.context, icon: FileText },
    { view: "brand", label: messages.brand.title, icon: Palette },
    { view: "assets", label: messages.assets.title, icon: Images },
    { view: "outputs", label: messages.outputs.title, icon: SlidersHorizontal },
  ] as const;
  const production = [
    { view: "drafts", label: messages.drafts.title, icon: Clapperboard },
    { view: "generations", label: copy.generations, icon: ListVideo },
    { view: "videos", label: copy.completedVideos, icon: Film },
  ] as const;
  const workspaceSection = workspaceTabs.some((item) => item.view === view);
  const sectionLabel = pathname === "/settings" ? messages.agentSettings.title
    : pathname === "/generation-run" ? messages.queue.runDetail
    : pathname === "/completed-video" ? messages.completedVideo.details
    : pathname === "/profile" ? messages.profileFormats.title
    : [...workspaceTabs, ...production].find((item) => item.view === view)?.label ?? messages.profiles.title;

  function choose(entry: Workspace | null) {
    setOpenMobile(false);
    onSelect(entry);
  }

  return (
    <>
      <Sidebar collapsible="icon" mobileTitle={copy.navigation}>
        <SidebarHeader className="relative gap-3 px-3 pt-3 pb-2">
          <Button variant="ghost" size="icon" className="absolute end-2 top-3 md:hidden"
            onClick={() => setOpenMobile(false)} aria-label={copy.closeNavigation}><X /></Button>
          <SidebarMenu>
            <SidebarMenuItem>
              <SidebarMenuButton size="lg" tooltip="Arrumo" asChild>
                <Link href="/" onClick={() => setOpenMobile(false)} aria-label="Arrumo">
                  {/* eslint-disable-next-line @next/next/no-img-element */}
                  <img className="size-8 shrink-0 rounded-lg" src="/arrumo-icon.svg" alt="" width={32} height={32} />
                  <span className="flex min-w-0 flex-col group-data-[collapsible=icon]:hidden">
                    <span className="text-base leading-none font-bold tracking-tight">Arrumo</span>
                    <span className="mt-1 text-xs text-muted-foreground">{copy.videoStudio}</span>
                  </span>
                </Link>
              </SidebarMenuButton>
            </SidebarMenuItem>
            {workspaces.length > 0 && (
              <SidebarMenuItem>
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <SidebarMenuButton variant="outline" tooltip={copy.workspace} aria-label={copy.workspace}>
                      <Layers /><span className="flex-1 truncate">{workspace?.name ?? messages.workspaces.title}</span>
                      <ChevronDown className="ms-auto group-data-[collapsible=icon]:hidden" />
                    </SidebarMenuButton>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent side="bottom" align="start" className="w-56">
                    <DropdownMenuGroup>
                      <DropdownMenuLabel>{messages.workspaces.list}</DropdownMenuLabel>
                      {workspaces.map((entry) => (
                        <DropdownMenuItem key={entry.id} onSelect={() => choose(entry)}>{entry.name}</DropdownMenuItem>
                      ))}
                    </DropdownMenuGroup>
                    <DropdownMenuSeparator />
                    <DropdownMenuItem onSelect={() => choose(null)}>{messages.workspaces.title}</DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
              </SidebarMenuItem>
            )}
          </SidebarMenu>
        </SidebarHeader>
        {workspace && (
          <SidebarContent>
            <SidebarGroup className="px-3">
              <SidebarGroupLabel>{copy.workspaceSection}</SidebarGroupLabel>
              <SidebarGroupContent>
                <SidebarMenu aria-label={copy.workspaceSection}>
                  <SidebarMenuItem>
                    <NavLink href={workspaceHref("context", workspace.id)} active={workspaceSection} label={copy.workspaceSettings} icon={FolderOpen} />
                  </SidebarMenuItem>
                  <SidebarMenuItem>
                    <NavLink href={workspaceHref("profiles", workspace.id)} active={view === "profiles"} label={messages.profiles.title} icon={Layers} />
                    <WorkspaceProfileNavigation key={workspace.id} workspaceId={workspace.id}
                      selected={pathname === "/profile" ? Number(query.get("profile")) : null} />
                  </SidebarMenuItem>
                </SidebarMenu>
              </SidebarGroupContent>
            </SidebarGroup>
            <SidebarSeparator />
            <SidebarGroup className="px-3">
              <SidebarGroupLabel>{copy.production}</SidebarGroupLabel>
              <SidebarGroupContent>
                <SidebarMenu aria-label={copy.production}>
                  {production.map(({ view: section, label, icon }) => (
                    <SidebarMenuItem key={section}>
                      <NavLink href={workspaceHref(section, workspace.id)} active={view === section} label={label} icon={icon}
                        attention={section === "generations" && hasPendingReviews ? messages.productionPlan.reviewRequired : undefined} />
                    </SidebarMenuItem>
                  ))}
                </SidebarMenu>
              </SidebarGroupContent>
            </SidebarGroup>
          </SidebarContent>
        )}
        <SidebarFooter className="mt-auto px-3 pb-3">
          <SidebarMenu><SidebarMenuItem>
            <NavLink href={workspace ? `/settings?workspace=${workspace.id}` : "/settings"}
              active={pathname === "/settings"} label={messages.agentSettings.title} icon={Settings2} />
          </SidebarMenuItem></SidebarMenu>
        </SidebarFooter>
      </Sidebar>

      <div className="flex min-w-0 flex-1 flex-col">
        <header className="sticky top-0 z-10 flex min-h-14 items-center gap-3 border-b bg-background px-3 sm:px-5">
          <SidebarTrigger aria-label={copy.navigation} />
          <Breadcrumb aria-label={copy.location} className="min-w-0 flex-1">
            <BreadcrumbList className="flex-nowrap">
              <BreadcrumbItem className="min-w-0">
                {workspace ? <BreadcrumbLink asChild className="truncate font-medium">
                  <Link href={workspaceHref("profiles", workspace.id)}>{workspace.name}</Link>
                </BreadcrumbLink> : <BreadcrumbPage>{pathname === "/settings" ? sectionLabel : messages.workspaces.title}</BreadcrumbPage>}
              </BreadcrumbItem>
              {workspace && <>
                <BreadcrumbSeparator />
                <BreadcrumbItem className="min-w-0"><BreadcrumbPage className="truncate">{sectionLabel}</BreadcrumbPage></BreadcrumbItem>
              </>}
            </BreadcrumbList>
          </Breadcrumb>
          {workspace && <ReviewInbox key={workspace.id} workspaceId={workspace.id} state={inbox} />}
          {workspace && !(view === "drafts" && query.get("compose") === "1") && <Button asChild size="sm">
            <Link href={`${workspaceHref("drafts", workspace.id)}&compose=1`}><Plus data-icon="inline-start" />{copy.newVideo}</Link>
          </Button>}
        </header>
        {workspace && workspaceSection && (
          <nav aria-label={copy.workspaceSettings} className="flex flex-wrap gap-1 border-b bg-sidebar/45 px-3 py-2 sm:px-5">
            {workspaceTabs.map(({ view: section, label, icon: Icon }) => (
              <Button asChild key={section} size="sm" variant={section === view ? "secondary" : "ghost"}>
                <Link href={workspaceHref(section, workspace.id)} aria-current={section === view ? "page" : undefined}>
                  <Icon data-icon="inline-start" />{label}
                </Link>
              </Button>
            ))}
          </nav>
        )}
        <div className={workspace && workspaceSection ? "grid flex-1 min-[1280px]:grid-cols-[minmax(0,1fr)_15rem]" : "flex-1"}>
          <div className="min-w-0">{children}</div>
          {workspace && workspaceSection && (
            <aside className="hidden border-l bg-sidebar px-4 py-6 min-[1280px]:block" aria-label={copy.overview}>
              <div className="sticky top-20 flex flex-col gap-4">
                <WorkspaceOverview key={workspace.id} workspaceId={workspace.id} view={view} onNavigate={onNavigate} />
              </div>
            </aside>
          )}
        </div>
      </div>
    </>
  );
}

function NavLink({ href, label, active, icon: Icon, attention }: {
  href: string;
  label: string;
  active: boolean;
  icon: typeof Layers;
  attention?: string;
}) {
  const { setOpenMobile } = useSidebar();
  return (
    <SidebarMenuButton asChild isActive={active} tooltip={label} className="h-9 data-active:bg-sidebar-primary data-active:text-sidebar-primary-foreground">
      <Link href={href} aria-current={active ? "page" : undefined}
        aria-label={attention ? `${label} · ${attention}` : label}
        onClick={() => setOpenMobile(false)}>
        <Icon /><span className="truncate">{label}</span>
        {attention && <span aria-hidden="true"
          className={cn("pointer-events-none absolute end-2 size-1.5 rounded-full group-data-[collapsible=icon]:end-1 group-data-[collapsible=icon]:top-1", active ? "bg-current" : "bg-primary")} />}
      </Link>
    </SidebarMenuButton>
  );
}

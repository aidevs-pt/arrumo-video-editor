import { CalendarDays, ChevronDown, Film, FolderOpen, House, Images, Layers, Package, Settings2, X } from "lucide-react";
import Link from "next/link";
import { usePathname, useSearchParams } from "next/navigation";
import type { CSSProperties, ReactNode } from "react";
import { Breadcrumb, BreadcrumbItem, BreadcrumbLink, BreadcrumbList, BreadcrumbPage, BreadcrumbSeparator } from "@/components/ui/breadcrumb";
import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuGroup, DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import {
  Sidebar, SidebarContent, SidebarGroup, SidebarGroupContent, SidebarGroupLabel, SidebarHeader, SidebarFooter,
  SidebarMenu, SidebarMenuButton, SidebarMenuItem, SidebarProvider, SidebarTrigger, useSidebar,
} from "@/components/ui/sidebar";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useLocale } from "@/lib/locale-provider";
import { ReviewInbox, useGenerationInbox } from "./review-inbox";
import { WorkspaceSearch } from "./workspace-search";
import { WorkspaceProfileNavigation } from "./workspace-profile-navigation";
import { workspaceHref, type Workspace, type WorkspaceView } from "./workspace-links";

export type { Workspace, WorkspaceView } from "./workspace-links";

export function WorkspaceFrame(props: {
  workspaces: Workspace[];
  workspace: Workspace | null;
  view: WorkspaceView;
  onSelect: (workspace: Workspace | null) => void;
  children: ReactNode;
}) {
  return (
    <TooltipProvider delayDuration={250}>
      <SidebarProvider style={{ "--sidebar-width": "15.5rem" } as CSSProperties}><StudioLayout {...props} /></SidebarProvider>
    </TooltipProvider>
  );
}

function StudioLayout({ workspaces, workspace, view, onSelect, children }: Parameters<typeof WorkspaceFrame>[0]) {
  const { messages } = useLocale();
  const { setOpenMobile } = useSidebar();
  const pathname = usePathname().replace(/\/$/, "");
  const query = useSearchParams();
  const copy = messages.studio;
  const inbox = useGenerationInbox(workspace?.id);
  const hasPendingReviews = (inbox?.inbox?.reviews.length ?? 0) > 0;
  const videoSection = view === "drafts" || view === "generations" || view === "videos";
  const workspaceSection = view === "context" || view === "brand" || view === "outputs";
  const sectionLabel = pathname === "" ? copy.home
    : pathname === "/settings" ? messages.agentSettings.title
    : pathname === "/generation-run" ? messages.queue.runDetail
    : pathname === "/completed-video" ? messages.completedVideo.details
    : pathname === "/profile" ? messages.profileFormats.title
    : videoSection ? copy.videos
    : view === "calendar" ? messages.calendar.title
    : workspaceSection ? copy.workspaceSettings
    : view === "assets" ? messages.assets.title
    : view === "motion-library" ? messages.motionLibrary.title
    : messages.profiles.title;

  function choose(entry: Workspace | null) {
    setOpenMobile(false);
    onSelect(entry);
  }

  return (
    <>
      <Sidebar collapsible="icon" mobileTitle={copy.navigation}>
        <SidebarHeader className="relative gap-2 px-2 pt-3 pb-2">
          <Button variant="ghost" size="icon" className="absolute end-2 top-0 md:hidden"
            onClick={() => setOpenMobile(false)} aria-label={copy.closeNavigation}><X /></Button>
          <SidebarMenu>
            <SidebarMenuItem>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <SidebarMenuButton size="lg" tooltip={copy.workspace} className="h-12 px-1.5" aria-label={copy.workspace}>
                    <span aria-hidden="true" className="grid size-8 shrink-0 place-items-center rounded-sm bg-attention">
                      {/* eslint-disable-next-line @next/next/no-img-element */}
                      <img className="size-[1.125rem]" src="/arrumo-face.svg" alt="" width={18} height={18} />
                    </span>
                    <span className="min-w-0 flex-1 group-data-[collapsible=icon]:hidden">
                      <span className="block truncate font-heading text-sm font-[850]">{workspace?.name ?? "Arrumo"}</span>
                      <span className="block truncate text-xs text-sidebar-foreground/80">{copy.workspace}</span>
                    </span>
                    <ChevronDown className="group-data-[collapsible=icon]:hidden" />
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
          </SidebarMenu>
        </SidebarHeader>
        {workspace && (
          <SidebarContent className="gap-0">
            <SidebarGroup className="pe-0 pb-2">
              <SidebarGroupContent>
                <SidebarMenu>
                  <SidebarMenuItem><NavLink href={`/?workspace=${workspace.id}`} active={!pathname} label={copy.home} icon={House} /></SidebarMenuItem>
                  <SidebarMenuItem>
                    <NavLink href={workspaceHref("drafts", workspace.id)} active={videoSection} label={copy.videos} icon={Film}
                      attention={hasPendingReviews ? messages.productionPlan.reviewRequired : undefined} />
                  </SidebarMenuItem>
                  <SidebarMenuItem><NavLink href={workspaceHref("calendar", workspace.id)} active={view === "calendar"} label={messages.calendar.title} icon={CalendarDays} /></SidebarMenuItem>
                </SidebarMenu>
              </SidebarGroupContent>
            </SidebarGroup>
            <SidebarGroup className="pe-0 pt-0">
              <SidebarGroupLabel className="h-6 text-sidebar-foreground/75">{copy.library}</SidebarGroupLabel>
              <SidebarGroupContent>
                <SidebarMenu>
                  <SidebarMenuItem><NavLink href={workspaceHref("assets", workspace.id)} active={view === "assets"} label={messages.assets.title} icon={Images} /></SidebarMenuItem>
                  <SidebarMenuItem><NavLink href={workspaceHref("motion-library", workspace.id)} active={view === "motion-library"} label={messages.motionLibrary.title} icon={Layers} /></SidebarMenuItem>
                </SidebarMenu>
              </SidebarGroupContent>
            </SidebarGroup>
            <SidebarGroup className="pe-0 pt-0">
              <SidebarGroupContent>
                <SidebarMenu>
                  <SidebarMenuItem>
                    <NavLink href={workspaceHref("profiles", workspace.id)} active={view === "profiles" && Boolean(pathname) && pathname !== "/profile"} label={messages.profiles.title} icon={Layers} />
                    <WorkspaceProfileNavigation key={workspace.id} workspaceId={workspace.id}
                      selected={pathname === "/profile" ? Number(query.get("profile")) : null} />
                  </SidebarMenuItem>
                </SidebarMenu>
              </SidebarGroupContent>
            </SidebarGroup>
          </SidebarContent>
        )}
        <SidebarFooter className="mt-auto ps-2 pe-0 pb-3">
          <SidebarMenu>
            {workspace && <SidebarMenuItem>
              <NavLink href={workspaceHref("context", workspace.id)} active={workspaceSection} label={copy.workspaceSettings} icon={FolderOpen} />
            </SidebarMenuItem>}
            <SidebarMenuItem>
              <NavLink href={workspace ? `/settings?workspace=${workspace.id}&scope=app&section=plugins` : "/settings?section=plugins"} active={pathname === "/settings" && query.get("section") === "plugins"}
                label={messages.pluginCatalog.title} icon={Package} />
            </SidebarMenuItem>
            <SidebarMenuItem>
              <NavLink href={workspace ? `/settings?workspace=${workspace.id}` : "/settings"}
                active={pathname === "/settings" && query.get("section") !== "plugins"} label={messages.agentSettings.title} icon={Settings2} />
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarFooter>
      </Sidebar>

      <div className="flex min-w-0 flex-1 flex-col">
        <header className="sticky top-0 z-10 flex min-h-14 items-center gap-3 border-b bg-background px-3 sm:px-5">
          <SidebarTrigger aria-label={copy.navigation} />
          <Breadcrumb aria-label={copy.location} className="min-w-0 flex-1">
            <BreadcrumbList className="flex-nowrap">
              <BreadcrumbItem className="min-w-0">
                {workspace ? <BreadcrumbLink asChild className="truncate font-medium">
                  <Link href={`/?workspace=${workspace.id}`}>{workspace.name}</Link>
                </BreadcrumbLink> : <BreadcrumbPage>{pathname === "/settings" ? sectionLabel : messages.workspaces.title}</BreadcrumbPage>}
              </BreadcrumbItem>
              {workspace && <>
                <BreadcrumbSeparator />
                <BreadcrumbItem className="min-w-0"><BreadcrumbPage className="truncate">{sectionLabel}</BreadcrumbPage></BreadcrumbItem>
              </>}
            </BreadcrumbList>
          </Breadcrumb>
          {workspace && <>
            <WorkspaceSearch key={workspace.id} workspaceId={workspace.id} />
            <ReviewInbox key={workspace.id} workspaceId={workspace.id} state={inbox} />
          </>}
        </header>
        <div className="min-w-0 flex-1">{children}</div>
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
    <SidebarMenuButton asChild isActive={active} tooltip={label} className="h-9 rounded-e-none pe-3 data-active:bg-sidebar-primary data-active:font-bold data-active:text-sidebar-primary-foreground">
      <Link href={href} aria-current={active ? "page" : undefined}
        aria-label={attention ? `${label}, ${attention}` : label}
        onClick={() => setOpenMobile(false)}>
        <Icon /><span className="truncate">{label}</span>
        {attention && <span aria-hidden="true"
          className="pointer-events-none absolute end-2 size-1.5 rounded-[2px] bg-attention group-data-[collapsible=icon]:end-1 group-data-[collapsible=icon]:top-1" />}
      </Link>
    </SidebarMenuButton>
  );
}

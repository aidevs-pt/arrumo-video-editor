import { invoke } from "@tauri-apps/api/core";
import { CalendarDays, Check, ChevronDown, Film, FolderCog, House, Images, Plus, Puzzle, Settings2, WandSparkles, X } from "lucide-react";
import Link from "next/link";
import { usePathname, useSearchParams } from "next/navigation";
import { Fragment, useEffect, useState, type CSSProperties, type ReactNode } from "react";
import { Breadcrumb, BreadcrumbItem, BreadcrumbLink, BreadcrumbList, BreadcrumbPage, BreadcrumbSeparator } from "@/components/ui/breadcrumb";
import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuGroup, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import {
  Sidebar, SidebarContent, SidebarGroup, SidebarGroupContent, SidebarGroupLabel, SidebarHeader, SidebarFooter,
  SidebarMenu, SidebarMenuButton, SidebarMenuItem, SidebarProvider, SidebarTrigger, useSidebar,
} from "@/components/ui/sidebar";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useLocale } from "@/lib/locale-provider";
import { ReviewInbox, useGenerationInbox } from "./review-inbox";
import { StudioTrailProvider, useStudioTrailItems } from "./studio-trail";
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
      <SidebarProvider style={{ "--sidebar-width": "15.5rem" } as CSSProperties}>
        <StudioTrailProvider><StudioLayout {...props} /></StudioTrailProvider>
      </SidebarProvider>
    </TooltipProvider>
  );
}

function FaceMark({ className = "size-8" }: { className?: string }) {
  return (
    <span aria-hidden="true" className={`grid shrink-0 place-items-center rounded-sm bg-attention ${className}`}>
      {/* eslint-disable-next-line @next/next/no-img-element */}
      <img className="size-[56%]" src="/arrumo-face.svg" alt="" width={18} height={18} />
    </span>
  );
}

function StudioLayout({ workspaces, workspace, view, onSelect, children }: Parameters<typeof WorkspaceFrame>[0]) {
  const { messages } = useLocale();
  const { setOpenMobile } = useSidebar();
  const pathname = usePathname().replace(/\/$/, "");
  const query = useSearchParams();
  const copy = messages.studio;
  const inbox = useGenerationInbox(workspace?.id);
  const trail = useStudioTrailItems();
  const reviewCount = inbox?.inbox?.reviews.length ?? 0;
  const videoSection = view === "drafts" || view === "generations" || view === "videos";
  const workspaceSection = view === "context" || view === "brand" || view === "outputs";
  const onProfile = pathname === "/profile";
  const sectionLabel = pathname === "" ? copy.home
    : pathname === "/settings" ? (query.get("section") === "plugins" ? messages.pluginCatalog.title : messages.agentSettings.title)
    : videoSection || pathname === "/generation-run" || pathname === "/completed-video" ? copy.videos
    : view === "calendar" ? messages.calendar.title
    : workspaceSection ? copy.workspaceSettings
    : view === "assets" ? messages.assets.title
    : view === "motion-library" ? messages.profileHub.animations
    : messages.profiles.title;

  const [counts, setCounts] = useState<Record<number, { profiles: number; videos: number }>>({});

  function loadCounts(open: boolean) {
    if (!open) return;
    for (const entry of workspaces) {
      Promise.all([
        invoke<unknown[]>("list_profiles", { workspaceId: entry.id }),
        invoke<unknown[]>("list_generation_runs", { workspaceId: entry.id }),
      ]).then(([profiles, runs]) => setCounts((current) => ({ ...current, [entry.id]: { profiles: profiles.length, videos: runs.length } })), () => {});
    }
  }

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
              <DropdownMenu onOpenChange={loadCounts}>
                <DropdownMenuTrigger asChild>
                  <SidebarMenuButton size="lg" tooltip={copy.workspace} className="h-12 px-1.5" aria-label={copy.workspace}>
                    <FaceMark />
                    <span className="min-w-0 flex-1 group-data-[collapsible=icon]:hidden">
                      <span className="block truncate font-heading text-[0.95rem] leading-tight font-[850] [font-variation-settings:'wdth'_120]">{workspace?.name ?? "Arrumo"}</span>
                      <span className="block truncate text-xs text-sidebar-foreground/80">{copy.workspace}</span>
                    </span>
                    <ChevronDown className="group-data-[collapsible=icon]:hidden" />
                  </SidebarMenuButton>
                </DropdownMenuTrigger>
                <DropdownMenuContent side="bottom" align="start" className="w-64">
                  <DropdownMenuGroup aria-label={messages.workspaces.list}>
                    {workspaces.map((entry) => (
                      <DropdownMenuItem key={entry.id} onSelect={() => choose(entry)} className="gap-3 py-2">
                        <FaceMark className="size-7" />
                        <span className="min-w-0 flex-1">
                          <span className="block truncate font-semibold">{entry.name}</span>
                          {counts[entry.id] && <span className="block truncate text-xs text-muted-foreground">
                            {(counts[entry.id].profiles === 1 ? copy.workspaceCountsOne : copy.workspaceCounts)
                              .replace("{profiles}", String(counts[entry.id].profiles)).replace("{videos}", String(counts[entry.id].videos))}
                          </span>}
                        </span>
                        {entry.id === workspace?.id && <Check className="text-primary" aria-label={copy.currentWorkspace} />}
                      </DropdownMenuItem>
                    ))}
                  </DropdownMenuGroup>
                  <DropdownMenuSeparator />
                  {workspace && <DropdownMenuItem asChild>
                    <Link href={workspaceHref("context", workspace.id)} onClick={() => setOpenMobile(false)}>
                      <FolderCog />{copy.workspaceSettings}
                    </Link>
                  </DropdownMenuItem>}
                  <DropdownMenuItem onSelect={() => choose(null)}><Plus />{copy.newWorkspace}</DropdownMenuItem>
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
                    <NavLink href={workspaceHref(reviewCount ? "generations" : "drafts", workspace.id)}
                      active={videoSection || pathname === "/generation-run" || pathname === "/completed-video"}
                      label={copy.videos} icon={Film} count={reviewCount}
                      countLabel={messages.productionPlan.reviewRequired} />
                  </SidebarMenuItem>
                  <SidebarMenuItem>
                    <NavLink href={workspaceHref("calendar", workspace.id)} active={view === "calendar" && !query.get("profile")}
                      label={messages.calendar.title} icon={CalendarDays} />
                  </SidebarMenuItem>
                </SidebarMenu>
              </SidebarGroupContent>
            </SidebarGroup>
            <SidebarGroup className="pe-0 pt-0">
              <SidebarGroupLabel className="h-6 text-sidebar-foreground/75">{copy.library}</SidebarGroupLabel>
              <SidebarGroupContent>
                <SidebarMenu>
                  <SidebarMenuItem><NavLink href={workspaceHref("assets", workspace.id)} active={view === "assets" && !onProfile} label={messages.assets.title} icon={Images} /></SidebarMenuItem>
                  <SidebarMenuItem>
                    <NavLink href={workspaceHref("motion-library", workspace.id)}
                      active={view === "motion-library" && !onProfile && !query.get("profile")} label={messages.profileHub.animations} icon={WandSparkles} />
                  </SidebarMenuItem>
                </SidebarMenu>
              </SidebarGroupContent>
            </SidebarGroup>
            <SidebarGroup className="pe-0 pt-0">
              <SidebarGroupLabel asChild className="h-6 text-sidebar-foreground/75 hover:text-sidebar-foreground">
                <Link href={workspaceHref("profiles", workspace.id)}>{messages.profiles.title}</Link>
              </SidebarGroupLabel>
              <SidebarGroupContent>
                <WorkspaceProfileNavigation key={workspace.id} workspaceId={workspace.id}
                  selected={onProfile ? Number(query.get("profile")) : null} />
              </SidebarGroupContent>
            </SidebarGroup>
          </SidebarContent>
        )}
        <SidebarFooter className="mt-auto ps-2 pe-0 pb-3">
          <SidebarMenu>
            <SidebarMenuItem>
              <NavLink href={workspace ? `/settings?workspace=${workspace.id}&scope=app&section=plugins` : "/settings?section=plugins"} active={pathname === "/settings" && query.get("section") === "plugins"}
                label={messages.pluginCatalog.title} icon={Puzzle} />
            </SidebarMenuItem>
            <SidebarMenuItem>
              <NavLink href={workspace ? `/settings?workspace=${workspace.id}` : "/settings"}
                active={pathname === "/settings" && query.get("section") !== "plugins"} label={messages.agentSettings.title} icon={Settings2} />
            </SidebarMenuItem>
          </SidebarMenu>
          {workspace && <AgentStatus workspaceId={workspace.id} working={inbox?.working ?? 0} />}
        </SidebarFooter>
      </Sidebar>

      <div className="flex min-w-0 flex-1 flex-col">
        <header className="sticky top-0 z-10 flex min-h-14 items-center gap-3 border-b bg-background px-3 sm:px-5">
          <SidebarTrigger aria-label={copy.navigation} />
          <Breadcrumb aria-label={copy.location} className="min-w-0 flex-1">
            <BreadcrumbList className="flex-nowrap">
              <BreadcrumbItem className="min-w-0">
                {workspace ? <BreadcrumbLink asChild className="truncate">
                  <Link href={`/?workspace=${workspace.id}`}>{workspace.name}</Link>
                </BreadcrumbLink> : <BreadcrumbPage>{pathname === "/settings" ? sectionLabel : messages.workspaces.title}</BreadcrumbPage>}
              </BreadcrumbItem>
              {workspace && (trail ?? [{ label: sectionLabel }]).map((item, index, items) => (
                <Fragment key={`${index}-${item.label}`}>
                  <BreadcrumbSeparator />
                  <BreadcrumbItem className="min-w-0">
                    {item.href && index < items.length - 1
                      ? <BreadcrumbLink asChild className="truncate"><Link href={item.href}>{item.label}</Link></BreadcrumbLink>
                      : <BreadcrumbPage className="truncate font-semibold">{item.label}</BreadcrumbPage>}
                  </BreadcrumbItem>
                </Fragment>
              ))}
            </BreadcrumbList>
          </Breadcrumb>
          {workspace && <Fragment key={workspace.id}>
            <WorkspaceSearch workspaceId={workspace.id} />
            <ReviewInbox workspaceId={workspace.id} state={inbox} />
          </Fragment>}
        </header>
        <div className="min-w-0 flex-1">{children}</div>
      </div>
    </>
  );
}

type AgentSettings = { effective_provider: string | null };
type AgentPlugin = { id: string; label: string };

function AgentStatus({ workspaceId, working }: { workspaceId: number; working: number }) {
  const { messages: { studio: copy } } = useLocale();
  const { setOpenMobile } = useSidebar();
  const [agent, setAgent] = useState<string | null>();

  useEffect(() => {
    let active = true;
    Promise.all([
      invoke<AgentSettings>("get_agent_settings", { workspaceId }),
      invoke<AgentPlugin[]>("list_agent_plugins"),
    ]).then(([settings, plugins]) => {
      if (active) setAgent(plugins.find(({ id }) => id === settings.effective_provider)?.label ?? null);
    }, () => { if (active) setAgent(null); });
    return () => { active = false; };
  }, [workspaceId]);

  if (agent === undefined) return null;
  const title = !agent ? copy.agentMissing
    : working ? copy.agentWorking.replace("{agent}", agent) : copy.agentIdle.replace("{agent}", agent);
  const detail = !agent ? copy.agentChoose
    : working === 1 ? copy.agentOneVideo : working ? copy.agentVideos.replace("{count}", String(working)) : copy.agentReady;
  const href = agent ? workspaceHref("generations", workspaceId) : `/settings?workspace=${workspaceId}`;

  return (
    <Link href={href} onClick={() => setOpenMobile(false)}
      className="mt-2 flex items-center gap-3 rounded-lg bg-[color-mix(in_oklch,var(--sidebar)_55%,var(--ink))] p-2.5 text-sidebar-foreground outline-none hover:bg-[color-mix(in_oklch,var(--sidebar)_40%,var(--ink))] focus-visible:ring-2 focus-visible:ring-attention group-data-[collapsible=icon]:bg-transparent group-data-[collapsible=icon]:p-0">
      <FaceMark className="size-9" />
      <span className="min-w-0 group-data-[collapsible=icon]:hidden">
        <span className="block truncate text-sm font-bold">{title}</span>
        <span className="block truncate text-xs text-sidebar-foreground/80">{detail}</span>
      </span>
    </Link>
  );
}

function NavLink({ href, label, active, icon: Icon, count = 0, countLabel }: {
  href: string;
  label: string;
  active: boolean;
  icon: typeof Film;
  count?: number;
  countLabel?: string;
}) {
  const { setOpenMobile } = useSidebar();
  return (
    <SidebarMenuButton asChild isActive={active} tooltip={label} className="h-9 rounded-e-none pe-3 data-active:bg-sidebar-primary data-active:font-bold data-active:text-sidebar-primary-foreground data-active:hover:bg-sidebar-primary data-active:hover:text-sidebar-primary-foreground">
      <Link href={href} aria-current={active ? "page" : undefined}
        aria-label={count && countLabel ? `${label}, ${count} ${countLabel}` : label}
        onClick={() => setOpenMobile(false)}>
        <Icon /><span className="truncate">{label}</span>
        {count > 0 && <span aria-hidden="true"
          className="pointer-events-none ms-auto grid h-5 min-w-5 place-items-center rounded-[4px] bg-attention px-1 text-xs font-bold text-ink tabular-nums group-data-[collapsible=icon]:absolute group-data-[collapsible=icon]:end-0.5 group-data-[collapsible=icon]:top-0.5 group-data-[collapsible=icon]:h-2 group-data-[collapsible=icon]:min-w-2 group-data-[collapsible=icon]:p-0 group-data-[collapsible=icon]:text-[0]">{count}</span>}
      </Link>
    </SidebarMenuButton>
  );
}

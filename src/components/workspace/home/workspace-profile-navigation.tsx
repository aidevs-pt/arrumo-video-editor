import { invoke } from "@tauri-apps/api/core";
import { CalendarDays, FileText, Film, ImageIcon, MonitorPlay, Palette, Plus, Upload, WandSparkles } from "lucide-react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useState } from "react";
import { SidebarMenu, SidebarMenuButton, SidebarMenuItem, useSidebar } from "@/components/ui/sidebar";
import { useLocale } from "@/lib/locale-provider";
import { ItemContextMenu } from "../item-context-menu";
import { profileHref, workspaceHref } from "./workspace-links";

type Profile = { id: number; name: string; color: string | null };

export function WorkspaceProfileNavigation({ workspaceId, selected }: { workspaceId: number; selected: number | null }) {
  const { setOpenMobile } = useSidebar();
  const { messages } = useLocale();
  const pathname = usePathname();
  const [profiles, setProfiles] = useState<Profile[]>([]);

  useEffect(() => {
    let active = true;
    function refresh() {
      invoke<Profile[]>("list_profiles", { workspaceId }).then(
        (items) => { if (active) setProfiles(items); },
        () => { if (active) setProfiles([]); },
      );
    }
    refresh();
    window.addEventListener("arrumo:profiles-changed", refresh);
    return () => {
      active = false;
      window.removeEventListener("arrumo:profiles-changed", refresh);
    };
  }, [pathname, workspaceId]);

  const itemClass = "h-9 rounded-e-none pe-3 data-active:bg-sidebar-primary data-active:font-bold data-active:text-sidebar-primary-foreground data-active:hover:bg-sidebar-primary data-active:hover:text-sidebar-primary-foreground";
  return (
    <SidebarMenu>
      {profiles.map((profile) => (
        <SidebarMenuItem key={profile.id}>
          <ItemContextMenu label={profile.name} items={[
            { label: messages.profileHub.videos, icon: Film, href: profileHref(workspaceId, profile.id) },
            { label: messages.profiles.context, icon: FileText, href: `${profileHref(workspaceId, profile.id)}&tab=context` },
            { label: messages.profiles.brand, icon: Palette, href: `${profileHref(workspaceId, profile.id)}&tab=brand` },
            { label: messages.profiles.assets, icon: ImageIcon, href: `${profileHref(workspaceId, profile.id)}&tab=assets` },
            { label: messages.profiles.outputs, icon: MonitorPlay, href: `${profileHref(workspaceId, profile.id)}&tab=outputs` },
            { label: messages.profileHub.animations, icon: WandSparkles, href: `${profileHref(workspaceId, profile.id)}&tab=animations` },
            { label: messages.profileHub.calendar, icon: CalendarDays, href: `${profileHref(workspaceId, profile.id)}&tab=calendar` },
            { label: messages.profileHub.generate.replace("{profile}", profile.name), icon: Upload, group: true, href: `${workspaceHref("drafts", workspaceId, profile.id)}&compose=1` },
          ]}>
          <div>
          <SidebarMenuButton asChild isActive={profile.id === selected} tooltip={profile.name} className={itemClass}>
            <Link href={profileHref(workspaceId, profile.id)} aria-current={profile.id === selected ? "page" : undefined}
              onClick={() => setOpenMobile(false)}>
              <span aria-hidden="true" className="ms-0.5 size-3 shrink-0 rounded-[3px] ring-1 ring-white/70"
                style={{ backgroundColor: profile.color ?? "var(--muted-foreground)" }} />
              <span className="truncate">{profile.name}</span>
            </Link>
          </SidebarMenuButton>
          </div>
          </ItemContextMenu>
        </SidebarMenuItem>
      ))}
      <SidebarMenuItem>
        <SidebarMenuButton asChild tooltip={messages.profiles.newProfile} className={itemClass}>
          <Link href={`${workspaceHref("profiles", workspaceId)}&new=1`} onClick={() => setOpenMobile(false)}>
            <Plus /><span className="truncate">{messages.profiles.newProfile}</span>
          </Link>
        </SidebarMenuButton>
      </SidebarMenuItem>
    </SidebarMenu>
  );
}

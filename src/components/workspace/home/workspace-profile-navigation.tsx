import { invoke } from "@tauri-apps/api/core";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useState } from "react";
import { SidebarMenuSub, SidebarMenuSubButton, SidebarMenuSubItem, useSidebar } from "@/components/ui/sidebar";
import { profileHref } from "./workspace-links";

type Profile = { id: number; name: string; color: string | null };

export function WorkspaceProfileNavigation({ workspaceId, selected }: { workspaceId: number; selected: number | null }) {
  const { setOpenMobile } = useSidebar();
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

  if (!profiles.length) return null;
  return (
    <SidebarMenuSub>
      {profiles.map((profile) => (
        <SidebarMenuSubItem key={profile.id}>
          <SidebarMenuSubButton asChild isActive={profile.id === selected} size="sm" className="data-active:bg-sidebar-primary data-active:text-sidebar-primary-foreground">
            <Link href={profileHref(workspaceId, profile.id)} aria-current={profile.id === selected ? "page" : undefined}
              onClick={() => setOpenMobile(false)}>
              <span aria-hidden="true" className="size-2 shrink-0 rounded-[2px] border border-current/20"
                style={{ backgroundColor: profile.color ?? "var(--muted-foreground)" }} />
              <span className="truncate">{profile.name}</span>
            </Link>
          </SidebarMenuSubButton>
        </SidebarMenuSubItem>
      ))}
    </SidebarMenuSub>
  );
}

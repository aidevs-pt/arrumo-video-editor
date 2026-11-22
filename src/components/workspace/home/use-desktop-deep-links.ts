import { isTauri } from "@tauri-apps/api/core";
import { useEffect, useEffectEvent } from "react";
import { calendarHref, completedFile, completedVideoHref, profileHref, workspaceHref, type WorkspaceView } from "./workspace-links";

const workspaceSections: readonly WorkspaceView[] = [
  "profiles", "context", "brand", "assets", "motion-library", "outputs", "drafts", "generations", "videos", "calendar",
];

const settingsSections = ["general", "agent", "services", "plugins", "quality", "backup"] as const;
const profileTabs = ["context", "brand", "assets", "outputs", "animations", "calendar"] as const;

function numericId(query: URLSearchParams, name: string) {
  const values = query.getAll(name);
  const id = Number(values[0]);
  return values.length === 1 && /^[1-9]\d*$/.test(values[0]) && Number.isSafeInteger(id) ? id : undefined;
}

function internalRoute(value: string) {
  let url: URL;
  try { url = new URL(value); } catch { return; }
  if (url.protocol !== "arrumo:" || url.host !== "open" || url.username || url.password || url.hash) return;
  const query = url.searchParams;
  if (url.pathname === "/workspaces" && !url.search) return "/?manage=1";
  const workspace = numericId(query, "workspace");
  if (!workspace) return;
  if (url.pathname === "/profile") {
    const profile = numericId(query, "profile");
    const tab = profileTabs.find((id) => id === query.get("tab"));
    if (profile) return tab ? `${profileHref(workspace, profile)}&tab=${tab}` : profileHref(workspace, profile);
  } else if (url.pathname === "/generation-run" || url.pathname === "/completed-video") {
    const run = numericId(query, "run");
    if (run) return url.pathname === "/completed-video"
      ? completedVideoHref(workspace, run, numericId(query, "format"), completedFile(query.get("file")))
      : `/generation-run?workspace=${workspace}&run=${run}`;
  } else if (url.pathname === "/settings") {
    const section = settingsSections.find((id) => id === query.get("section")) ?? "general";
    const scope = query.get("scope") === "workspace" ? "workspace" : "app";
    return `/settings?workspace=${workspace}&scope=${scope}&section=${section}`;
  } else if (url.pathname === "/calendar") {
    return calendarHref(workspace, numericId(query, "profile"), query.get("month"));
  } else {
    const section = workspaceSections.find((view) => url.pathname === `/${view}`);
    if (!section) return;
    const href = workspaceHref(section, workspace, section === "drafts" || section === "motion-library" ? numericId(query, "profile") : undefined);
    const draft = section === "drafts" ? numericId(query, "draft") : undefined;
    const compose = section === "drafts" ? ["1", "scratch"].find((value) => value === query.get("compose")) : undefined;
    return draft ? `${href}&draft=${draft}` : compose ? `${href}&compose=${compose}` : href;
  }
}

export function useDesktopDeepLinks(router: { push: (href: string) => void }) {
  const navigate = useEffectEvent((href: string) => router.push(href));
  useEffect(() => {
    if (!isTauri()) return;
    let active = true;
    let unsubscribe: (() => void) | undefined;
    let receivedLive = false;
    function open(urls: string[]) {
      const route = urls.map(internalRoute).find((href) => href !== undefined);
      if (active && route) navigate(route);
    }
    void (async () => {
      const { getCurrent, onOpenUrl } = await import("@tauri-apps/plugin-deep-link");
      if (!active) return;
      const unlisten = await onOpenUrl((urls) => {
        receivedLive = true;
        open(urls);
      });
      if (!active) {
        unlisten();
        return;
      }
      unsubscribe = unlisten;
      const initial = await getCurrent();
      if (initial && !receivedLive) open(initial);
    })().catch((error: unknown) => console.error("Arrumo deep-link listener failed", error));
    return () => { active = false; unsubscribe?.(); };
  }, []);
}

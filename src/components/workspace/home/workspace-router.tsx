"use client";

import { markAppReady } from "@/components/launch-screen";

import { invoke, isTauri } from "@tauri-apps/api/core";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { WorkspaceShell } from "@/components/workspace/layout";
import { useLocale } from "@/lib/locale-provider";
import { useDesktopDeepLinks } from "./use-desktop-deep-links";
import { WorkspaceFrame } from "./workspace-frame";
import { workspaceHref, type Workspace, type WorkspaceView } from "./workspace-links";
export { workspaceHref, completedVideoHref } from "./workspace-links";

type Status = "loading" | "ready" | "error" | "browser";
const views: WorkspaceView[] = ["profiles", "context", "brand", "assets", "motion-library", "outputs", "appearance", "drafts", "generations", "videos", "calendar"];

type WorkspaceCatalog = {
  workspaces: Workspace[];
  status: Status;
  load: () => Promise<void>;
  add: (workspace: Workspace) => void;
  update: (workspace: Workspace) => void;
  workspace: Workspace | undefined;
  navigate: (view: WorkspaceView, profile?: number) => void;
  draftProfile: number | null;
};
const Catalog = createContext<WorkspaceCatalog | null>(null);

export function useWorkspaceCatalog() {
  const catalog = useContext(Catalog);
  if (!catalog) throw new Error("Workspace layout is missing");
  return catalog;
}

export function WorkspaceRouter({ children }: { children: ReactNode }) {
  const router = useRouter();
  useDesktopDeepLinks(router);
  const pathname = usePathname();
  const query = useSearchParams();
  const { messages: { workspaces: copy } } = useLocale();
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [status, setStatus] = useState<Status>("loading");
  const section = pathname.replace(/\/$/, "");
  const view = section === "/completed-video" ? "videos"
    : section === "/generation-run" ? "generations"
    : section === "/profile" ? "profiles"
    : views.find((view) => section === `/${view}`);
  const workspaceId = query.get("workspace");
  const workspace = workspaceId
    ? workspaces.find((entry) => String(entry.id) === workspaceId)
    : !section && query.get("manage") !== "1" ? workspaces[0] : undefined;
  const profile = Number(query.get("profile"));
  const draftProfile = Number.isSafeInteger(profile) && profile > 0 ? profile : null;

  const load = useCallback(async () => {
    if (!isTauri()) {
      setStatus("browser");
      return;
    }
    setStatus("loading");
    try {
      setWorkspaces(await invoke<Workspace[]>("list_workspaces"));
      setStatus("ready");
    } catch {
      setStatus("error");
    }
  }, []);

  useEffect(() => { void load(); }, [load]);
  useEffect(() => { if (status !== "loading") markAppReady(); }, [status]);

  function navigate(target: WorkspaceView, profileId?: number) {
    if (workspace) router.push(workspaceHref(target, workspace.id, profileId));
  }

  return (
    <Catalog.Provider value={{
      workspaces, status, load, workspace, navigate, draftProfile,
      add: (entry) => setWorkspaces((current) => [...current, entry]),
      update: (entry) => setWorkspaces((current) => current.map((item) => item.id === entry.id ? entry : item)),
    }}>
      {!section && !workspaces.length ? children : <WorkspaceFrame workspaces={workspaces} workspace={workspace ?? null} view={view ?? "profiles"}
        onSelect={(entry) => router.push(entry ? `/?workspace=${entry.id}` : "/?manage=1")}
        onUpdate={(entry) => setWorkspaces((current) => current.map((item) => item.id === entry.id ? entry : item))}>
        {!view || workspace ? children : (
          <WorkspaceShell>
            {status === "loading" ? <p role="status">{copy.loading}</p> : (
              <Alert>
                <AlertDescription>{status === "browser" ? copy.desktop : status === "error" ? copy.loadError : copy.missing}</AlertDescription>
                {status === "error" ? <Button variant="outline" onClick={load}>{copy.retry}</Button>
                  : <Button variant="outline" onClick={() => router.push("/")}>{copy.title}</Button>}
              </Alert>
            )}
          </WorkspaceShell>
        )}
      </WorkspaceFrame>}
    </Catalog.Provider>
  );
}

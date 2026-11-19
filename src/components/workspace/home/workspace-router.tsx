"use client";

import { invoke, isTauri } from "@tauri-apps/api/core";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { WorkspaceShell } from "@/components/workspace/layout";
import { useLocale } from "@/lib/locale-provider";
import { WorkspaceFrame, type Workspace, type WorkspaceView } from "./workspace-frame";

type Status = "loading" | "ready" | "error" | "browser";
const views: WorkspaceView[] = ["profiles", "context", "brand", "assets", "outputs", "drafts", "generations"];

type WorkspaceCatalog = {
  workspaces: Workspace[];
  status: Status;
  load: () => Promise<void>;
  add: (workspace: Workspace) => void;
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

export function workspaceHref(view: WorkspaceView, workspaceId: number, profile?: number) {
  const query = new URLSearchParams({ workspace: String(workspaceId) });
  if (profile !== undefined) query.set("profile", String(profile));
  return `/${view}?${query}`;
}

export function WorkspaceRouter({ children }: { children: ReactNode }) {
  const router = useRouter();
  const pathname = usePathname();
  const query = useSearchParams();
  const { messages: { workspaces: copy } } = useLocale();
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [status, setStatus] = useState<Status>("loading");
  const view = views.find((section) => pathname === `/${section}` || pathname === `/${section}/`);
  const workspace = view ? workspaces.find((entry) => String(entry.id) === query.get("workspace")) : undefined;
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

  function navigate(target: WorkspaceView, profileId?: number) {
    if (workspace) router.push(workspaceHref(target, workspace.id, profileId));
  }

  return (
    <Catalog.Provider value={{
      workspaces, status, load, workspace, navigate, draftProfile,
      add: (entry) => setWorkspaces((current) => [...current, entry]),
    }}>
      <WorkspaceFrame workspaces={workspaces} workspace={workspace ?? null} view={view ?? "profiles"}
        onSelect={(entry) => router.push(entry ? workspaceHref("profiles", entry.id) : "/")}
        onNavigate={navigate}>
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
      </WorkspaceFrame>
    </Catalog.Provider>
  );
}

export type Workspace = { id: number; name: string };
export type WorkspaceView = "profiles" | "context" | "brand" | "assets" | "outputs"
  | "drafts" | "generations" | "videos";

export function workspaceHref(view: WorkspaceView, workspaceId: number, profile?: number) {
  const query = new URLSearchParams({ workspace: String(workspaceId) });
  if (profile !== undefined) query.set("profile", String(profile));
  return `/${view}?${query}`;
}

export function profileHref(workspaceId: number, profileId: number) {
  return `/profile?workspace=${workspaceId}&profile=${profileId}`;
}

export function completedVideoHref(workspaceId: number, runId: number) {
  return `/completed-video?workspace=${workspaceId}&run=${runId}`;
}

export type Workspace = { id: number; name: string };
export type WorkspaceView = "profiles" | "context" | "brand" | "assets" | "outputs"
  | "drafts" | "generations" | "videos" | "calendar";

export function workspaceHref(view: WorkspaceView, workspaceId: number, profile?: number) {
  const query = new URLSearchParams({ workspace: String(workspaceId) });
  if (profile !== undefined) query.set("profile", String(profile));
  return `/${view}?${query}`;
}

export function profileHref(workspaceId: number, profileId: number) {
  return `/profile?workspace=${workspaceId}&profile=${profileId}`;
}

export function completedVideoHref(workspaceId: number, runId: number, formatId?: number) {
  const query = new URLSearchParams({ workspace: String(workspaceId), run: String(runId) });
  if (formatId !== undefined) query.set("format", String(formatId));
  return `/completed-video?${query}`;
}

export function calendarHref(workspaceId: number, profileId?: number, month?: string | null) {
  const href = workspaceHref("calendar", workspaceId, profileId);
  return month && /^[1-9]\d{3}-(0[1-9]|1[0-2])$/.test(month) ? `${href}&month=${month}` : href;
}

export type Workspace = { id: number; name: string };
export type WorkspaceView = "profiles" | "context" | "brand" | "assets" | "motion-library" | "outputs"
  | "drafts" | "generations" | "videos" | "calendar";

export function workspaceHref(view: WorkspaceView, workspaceId: number, profile?: number) {
  const query = new URLSearchParams({ workspace: String(workspaceId) });
  if (profile !== undefined) query.set("profile", String(profile));
  return `/${view}?${query}`;
}

export function profileHref(workspaceId: number, profileId: number) {
  return `/profile?workspace=${workspaceId}&profile=${profileId}`;
}

export function completedVideoHref(workspaceId: number, runId: number, formatId?: number, file?: "subtitles" | `text-${number}`) {
  const query = new URLSearchParams({ workspace: String(workspaceId), run: String(runId) });
  if (formatId !== undefined) query.set("format", String(formatId));
  if (file) query.set("file", file);
  return `/completed-video?${query}`;
}

export function isCalendarMonth(value?: string | null): value is string {
  return Boolean(value && /^(?!0000)\d{4}-(0[1-9]|1[0-2])$/.test(value));
}

export function calendarHref(workspaceId: number, profileId?: number, month?: string | null) {
  const href = workspaceHref("calendar", workspaceId, profileId);
  return isCalendarMonth(month) ? `${href}&month=${month}` : href;
}

export function completedFile(value: string | null): "subtitles" | `text-${number}` | undefined {
  if (value === "subtitles") return value;
  if (value?.startsWith("text-") && /^[1-9]\d*$/.test(value.slice(5))) {
    const index = Number(value.slice(5));
    if (Number.isSafeInteger(index)) return `text-${index}`;
  }
}

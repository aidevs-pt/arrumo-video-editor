export type Workspace = { id: number; name: string; color?: string | null; icon?: string | null; imageAssetId?: number | null };
export type WorkspaceView = "profiles" | "context" | "brand" | "assets" | "motion-library" | "outputs" | "appearance"
  | "drafts" | "generations" | "videos" | "calendar";

export function workspaceHref(view: WorkspaceView, workspaceId: number, profile?: number) {
  const query = new URLSearchParams({ workspace: String(workspaceId) });
  if (profile !== undefined) query.set("profile", String(profile));
  return `/${view}?${query}`;
}

export function profileHref(workspaceId: number, profileId: number) {
  return `/profile?workspace=${workspaceId}&profile=${profileId}`;
}

export type CompletedFile = "subtitles" | `text-${number}` | `media-${number}` | `attachment-${number}`;

export function completedVideoHref(workspaceId: number, runId: number, formatId?: number, file?: CompletedFile) {
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

export function completedFile(value: string | null): CompletedFile | undefined {
  if (value === "subtitles") return value;
  const match = value?.match(/^(text|media|attachment)-([1-9]\d*)$/);
  if (!match) return;
  const index = Number(match[2]);
  if (!Number.isSafeInteger(index)) return;
  if (match[1] === "attachment") return `attachment-${index}`;
  return match[1] === "text" ? `text-${index}` : `media-${index}`;
}

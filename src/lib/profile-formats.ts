import { invoke } from "@tauri-apps/api/core";

/** The formats a profile makes its videos in: the workspace's, less any the profile switched off. */
export async function madeProfileFormats<T>(workspaceId: number, profileId: number): Promise<T[]> {
  const formats = await invoke<(T & { enabled?: boolean })[]>("list_profile_formats", { workspaceId, profileId });
  return formats.filter((format) => format.enabled !== false);
}

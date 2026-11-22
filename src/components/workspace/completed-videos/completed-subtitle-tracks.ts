import { invoke } from "@tauri-apps/api/core";

import type { SubtitleTrack } from "@/components/workspace/plugins/action-form";

type Output = { format_id: number; video: string };
type Video = {
  run: { snapshot: { formats?: { format: { id: number; name: string } }[] } };
  delivery: { verified_outputs: Output[] };
};

/** Every subtitle track of a finished video: each output's primary track, then its added languages. */
export async function completedSubtitleTracks(workspaceId: number, runId: number, locale: string, primary: string): Promise<SubtitleTrack[]> {
  const video = await invoke<Video>("get_completed_video", { workspaceId, runId });
  const names = new Intl.DisplayNames([locale], { type: "language" });
  const formatName = (output: Output) =>
    video.run.snapshot.formats?.find(({ format }) => format.id === output.format_id)?.format.name ?? output.video;
  const outputs = await Promise.all(video.delivery.verified_outputs.map(async (output) => {
    const preview = await invoke<{ languages: string[] }>("preview_completed_output", { workspaceId, runId, formatId: output.format_id });
    return ["", ...preview.languages].map((language) => ({
      id: `${output.format_id}:${language}`,
      label: `${formatName(output)} · ${language ? (names.of(language) ?? language) : primary}`,
      load: async () => (await invoke<{ subtitles: string }>("read_completed_subtitle_revision", {
        workspaceId, runId, formatId: output.format_id, language: language || null,
      })).subtitles,
    }));
  }));
  return outputs.flat();
}

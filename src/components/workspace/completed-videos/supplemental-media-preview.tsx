import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { Alert, AlertAction, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useLocale } from "@/lib/locale-provider";

export type SupplementalMedia = { name: string; kind: "image" | "audio" | "video"; format_id: number | null };

export function SupplementalMediaPreview({ workspaceId, runId, index, media }: {
  workspaceId: number;
  runId: number;
  index: number;
  media: SupplementalMedia;
}) {
  const { messages } = useLocale();
  const [url, setUrl] = useState<string>();
  const [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let active = true;
    invoke<string>("preview_completed_supplemental_media", { workspaceId, runId, index }).then(
      (result) => { if (active) setUrl(result); },
      () => { if (active) setFailed(true); },
    );
    return () => { active = false; };
  }, [workspaceId, runId, index, attempt]);

  if (failed) return <Alert variant="destructive">
    <AlertDescription>{messages.completedVideo.previewError}</AlertDescription>
    <AlertAction><Button variant="outline" size="sm" onClick={() => {
      setUrl(undefined);
      setFailed(false);
      setAttempt((current) => current + 1);
    }}>{messages.queue.retry}</Button></AlertAction>
  </Alert>;

  if (!url) return <Skeleton className="h-36 max-w-xl" role="status" aria-label={messages.completedVideo.loadingPreview} />;

  const unavailable = () => setFailed(true);
  if (media.kind === "image") return (
    // eslint-disable-next-line @next/next/no-img-element
    <img src={url} alt={media.name} className="max-h-[65dvh] max-w-full rounded-[4px] object-contain" onError={unavailable} />
  );
  if (media.kind === "audio") return <audio src={url} aria-label={media.name} controls preload="metadata"
    className="w-full max-w-xl" onError={unavailable} />;
  return <video src={url} aria-label={media.name} controls playsInline preload="metadata"
    className="aspect-video max-h-[65dvh] w-full rounded-[4px] bg-ink object-contain" onError={unavailable} />;
}

import { invoke } from "@tauri-apps/api/core";
import { AudioLines, Film, ImageIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { cn } from "@/lib/utils";

type MediaKind = "image" | "audio" | "video";

const icons = { image: ImageIcon, audio: AudioLines, video: Film };
const TRANSPARENCY =
  "repeating-conic-gradient(color-mix(in oklab, var(--muted), var(--foreground) 38%) 0% 25%, color-mix(in oklab, var(--muted), var(--foreground) 50%) 0% 50%) 50% / 16px 16px";

export function AssetThumbnail({ workspaceId, assetId, kind, className, fit = "cover", transparency = false }: {
  workspaceId: number;
  assetId: number;
  kind: MediaKind;
  className?: string;
  /** `contain` shows the whole image, for logos and marks that must never be cropped. */
  fit?: "cover" | "contain";
  /** A checkerboard behind the image, so white marks on transparent backgrounds stay visible. */
  transparency?: boolean;
}) {
  const element = useRef<HTMLDivElement>(null);
  const [url, setUrl] = useState<string>();
  const Icon = icons[kind];

  useEffect(() => {
    if (kind === "audio" || !element.current) return;
    let active = true;
    const observer = new IntersectionObserver(([entry]) => {
      if (!entry.isIntersecting) return;
      observer.disconnect();
      invoke<string | null>(kind === "image" ? "preview_workspace_image" : "preview_workspace_video_frame", { workspaceId, assetId }).then(
        (value) => { if (active && value) setUrl(value); },
        () => {},
      );
    }, { rootMargin: "120px" });
    observer.observe(element.current);
    return () => { active = false; observer.disconnect(); };
  }, [assetId, kind, workspaceId]);

  return (
    <div ref={element} aria-hidden="true" className={cn("relative grid aspect-video place-items-center overflow-hidden rounded-thumb bg-muted", className)}
      style={url && transparency ? { background: TRANSPARENCY } : undefined}>
      {url ? (
        // Managed local preview provided by the desktop host.
        // eslint-disable-next-line @next/next/no-img-element
        <img src={url} alt="" className={cn("absolute inset-0 size-full", fit === "contain" ? "object-contain" : "object-cover")} onError={() => setUrl(undefined)} />
      ) : <Icon className="size-6 text-muted-foreground" />}
    </div>
  );
}

/** Plays a video or audio asset, with the video's still as its poster until it plays. */
export function AssetPlayer({ workspaceId, assetId, kind, label }: {
  workspaceId: number;
  assetId: number;
  kind: "audio" | "video";
  label: string;
}) {
  const [url, setUrl] = useState<string>();
  const [poster, setPoster] = useState<string>();
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let active = true;
    invoke<string>("preview_workspace_media", { workspaceId, assetId, kind })
      .then((value) => { if (active) setUrl(value); }, () => { if (active) setFailed(true); });
    if (kind === "video") {
      invoke<string>("preview_workspace_video_frame", { workspaceId, assetId })
        .then((value) => { if (active) setPoster(value); }, () => {});
    }
    return () => { active = false; };
  }, [assetId, kind, workspaceId]);

  if (failed || !url) return <AssetThumbnail workspaceId={workspaceId} assetId={assetId} kind={kind} className="aspect-video w-full" />;
  return kind === "video"
    ? <video src={url} poster={poster} aria-label={label} controls playsInline preload="metadata"
        className="aspect-video w-full rounded-thumb bg-ink object-contain" onError={() => setFailed(true)} />
    : <audio src={url} aria-label={label} controls preload="metadata" className="w-full" onError={() => setFailed(true)} />;
}

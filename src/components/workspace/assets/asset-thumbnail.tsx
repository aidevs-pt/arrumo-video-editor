import { invoke } from "@tauri-apps/api/core";
import { AudioLines, Film, ImageIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { cn } from "@/lib/utils";

type MediaKind = "image" | "audio" | "video";

const icons = { image: ImageIcon, audio: AudioLines, video: Film };
const TRANSPARENCY =
  "repeating-conic-gradient(color-mix(in oklab, var(--muted), var(--foreground) 38%) 0% 25%, color-mix(in oklab, var(--muted), var(--foreground) 50%) 0% 50%) 50% / 16px 16px";

export function AssetThumbnail({ workspaceId, assetId, kind, className, fit = "cover" }: {
  workspaceId: number;
  assetId: number;
  kind: MediaKind;
  className?: string;
  /** `contain` shows the whole image, for logos and marks that must never be cropped. */
  fit?: "cover" | "contain";
}) {
  const element = useRef<HTMLDivElement>(null);
  const [url, setUrl] = useState<string>();
  const Icon = icons[kind];

  useEffect(() => {
    if (kind !== "image" || !element.current) return;
    let active = true;
    const observer = new IntersectionObserver(([entry]) => {
      if (!entry.isIntersecting) return;
      observer.disconnect();
      invoke<string | null>("preview_workspace_image", { workspaceId, assetId }).then(
        (value) => { if (active && value) setUrl(value); },
        () => {},
      );
    }, { rootMargin: "120px" });
    observer.observe(element.current);
    return () => { active = false; observer.disconnect(); };
  }, [assetId, kind, workspaceId]);

  return (
    <div ref={element} aria-hidden="true" className={cn("relative grid aspect-video place-items-center overflow-hidden rounded-thumb bg-muted", className)}
      // A checkerboard behind whole images shows transparency, so white marks stay visible.
      style={url && fit === "contain" ? { background: TRANSPARENCY } : undefined}>
      {url ? (
        // Managed local preview provided by the desktop host.
        // eslint-disable-next-line @next/next/no-img-element
        <img src={url} alt="" className={cn("absolute inset-0 size-full", fit === "contain" ? "object-contain" : "object-cover")} onError={() => setUrl(undefined)} />
      ) : <Icon className="size-6 text-muted-foreground" />}
    </div>
  );
}

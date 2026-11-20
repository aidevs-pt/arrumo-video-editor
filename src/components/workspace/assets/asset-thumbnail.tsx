import { invoke } from "@tauri-apps/api/core";
import { AudioLines, Film, ImageIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { cn } from "@/lib/utils";

type MediaKind = "image" | "audio" | "video";

const icons = { image: ImageIcon, audio: AudioLines, video: Film };

export function AssetThumbnail({ workspaceId, assetId, kind, className }: {
  workspaceId: number;
  assetId: number;
  kind: MediaKind;
  className?: string;
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
    <div ref={element} aria-hidden="true" className={cn("grid aspect-video place-items-center overflow-hidden rounded-[4px] bg-muted", className)}>
      {url ? (
        // Managed local preview provided by the desktop host.
        // eslint-disable-next-line @next/next/no-img-element
        <img src={url} alt="" className="size-full object-cover" onError={() => setUrl(undefined)} />
      ) : <Icon className="size-6 text-muted-foreground" />}
    </div>
  );
}

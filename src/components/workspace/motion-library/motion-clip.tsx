"use client";

import { invoke } from "@tauri-apps/api/core";
import { useReducedMotion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import { AssetThumbnail } from "@/components/workspace/assets/asset-thumbnail";
import { cn } from "@/lib/utils";

/** Where a still is taken: late enough that the animation has arrived, before it leaves. */
const SETTLED = 0.7;

/**
 * A library item's rendered preview, muted. `still` shows its settled moment and plays while
 * hovered; `loop` plays on repeat. With reduced motion it stays still and offers
 * controls instead. Falls back to the item's thumbnail when the video can't load.
 */
export function MotionClip({ workspaceId, profileId, itemId, title, revision, thumbnail, mode, className }: {
  workspaceId: number;
  profileId: number | null;
  itemId: number;
  title: string;
  /** Changes when the item does, so the clip reloads. */
  revision: unknown;
  thumbnail: { kind: "image" | "video"; assetId: number };
  mode: "still" | "loop";
  className?: string;
}) {
  const reduceMotion = useReducedMotion();
  const video = useRef<HTMLVideoElement>(null);
  const [url, setUrl] = useState<string>();
  const [failed, setFailed] = useState(false);
  // WebKit paints the first frames at a default 2:1 size, so the video fills the box until its
  // real shape is known, and is fitted inside only when that shape isn't the box's.
  const [fitted, setFitted] = useState(false);
  const revisionKey = JSON.stringify(revision);

  useEffect(() => {
    let active = true;
    setFailed(false);
    setFitted(false);
    invoke<{ video_url: string }>("preview_motion_library_item", { workspaceId, profileId, itemId })
      .then((preview) => { if (active) setUrl(preview.video_url); }, () => { if (active) setFailed(true); });
    return () => { active = false; };
  }, [workspaceId, profileId, itemId, revisionKey]);

  function measure() {
    const element = video.current;
    if (element?.videoHeight) setFitted(Math.abs(element.videoWidth / element.videoHeight - 16 / 9) > 0.02);
    if (mode === "still" || reduceMotion) settle();
  }

  function settle() {
    const element = video.current;
    if (element && Number.isFinite(element.duration)) element.currentTime = element.duration * SETTLED;
  }

  const box = cn("relative aspect-video w-full overflow-hidden rounded-lg bg-ink/90", className);
  if (failed || !url) {
    return failed
      ? <AssetThumbnail workspaceId={workspaceId} kind={thumbnail.kind} assetId={thumbnail.assetId} fit="contain" className={box} />
      : <div className={box} aria-hidden />;
  }
  const hover = mode === "still" && !reduceMotion
    ? {
      onMouseEnter: () => void video.current?.play().catch(() => {}),
      onMouseLeave: () => { video.current?.pause(); settle(); },
    }
    : {};
  return (
    <div className={box} {...hover}>
      <video ref={video} key={url} src={url} aria-label={title} muted playsInline
        loop={mode === "loop" || !reduceMotion} preload="metadata"
        autoPlay={mode === "loop" && !reduceMotion} controls={mode === "loop" && !!reduceMotion}
        onLoadedMetadata={measure}
        onError={() => setFailed(true)}
        className={cn("absolute inset-0 size-full", fitted ? "object-contain" : "object-cover")} />
    </div>
  );
}

"use client";

import { invoke } from "@tauri-apps/api/core";
import {
  Book, Briefcase, Camera, Clapperboard, Code, Dumbbell, Gamepad2, GraduationCap, Heart, Leaf, Mic, Music,
  Palette, Rocket, Sparkles, Utensils, type LucideIcon,
} from "lucide-react";
import { useEffect, useState } from "react";

import { cn } from "@/lib/utils";
import type { Workspace } from "../home/workspace-links";

/** The preset icons, keyed as the workspace store saves them. */
export const WORKSPACE_ICONS: Record<string, LucideIcon> = {
  clapperboard: Clapperboard, camera: Camera, mic: Mic, sparkles: Sparkles, rocket: Rocket, gamepad: Gamepad2,
  book: Book, palette: Palette, music: Music, code: Code, heart: Heart, leaf: Leaf, briefcase: Briefcase,
  "graduation-cap": GraduationCap, utensils: Utensils, dumbbell: Dumbbell,
};

const previews = new Map<string, Promise<string | null>>();

/** A workspace image's local preview address, fetched once per image. */
export function useWorkspaceImage(workspaceId: number, assetId: number | null | undefined) {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    if (!assetId) { setUrl(null); return; }
    const key = `${workspaceId}/${assetId}`;
    if (!previews.has(key)) {
      previews.set(key, invoke<string | null>("preview_workspace_image", { workspaceId, assetId }).catch(() => null));
    }
    let active = true;
    void previews.get(key)!.then((value) => { if (active) setUrl(value); });
    return () => { active = false; };
  }, [workspaceId, assetId]);
  return url;
}

/** How a workspace is recognised: its image, or its preset icon on its color. */
export function WorkspaceMark({ workspace, className = "size-8" }: {
  workspace: Pick<Workspace, "id" | "color" | "icon" | "imageAssetId">;
  className?: string;
}) {
  const image = useWorkspaceImage(workspace.id, workspace.imageAssetId);
  const Icon = WORKSPACE_ICONS[workspace.icon ?? ""] ?? Clapperboard;
  return (
    <span aria-hidden="true" style={image ? undefined : { backgroundColor: workspace.color ?? "var(--attention)" }}
      className={cn("grid shrink-0 place-items-center overflow-hidden rounded-[25%] text-white", className)}>
      {image
        // Managed local preview provided by the desktop host.
        // eslint-disable-next-line @next/next/no-img-element
        ? <img src={image} alt="" className="size-full object-cover" />
        : <Icon className="size-[55%]" strokeWidth={2.25} />}
    </span>
  );
}

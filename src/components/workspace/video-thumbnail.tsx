import type { CSSProperties } from "react";
import { cn } from "@/lib/utils";

const fallbackHue = "#2337b4";

/** Stand-in frame for a video without real media: a field in the profile hue, a creator silhouette and the title. */
export function VideoThumbnail({ title, color, portrait = false, small = false, className }: {
  title: string;
  color?: string | null;
  portrait?: boolean;
  small?: boolean;
  className?: string;
}) {
  const hue = color ?? fallbackHue;
  const style = {
    "--thumb-field": `oklch(from ${hue} 30% 0.09 h)`,
    "--thumb-figure": `oklch(from ${hue} 70% 0.15 h)`,
  } as CSSProperties;
  return (
    <span aria-hidden="true" style={style}
      className={cn("relative block shrink-0 overflow-hidden rounded-[4px] bg-[var(--thumb-field,var(--ink))]",
        portrait ? "aspect-[9/16]" : "aspect-video", className)}>
      <svg viewBox="0 0 160 90" preserveAspectRatio="xMaxYMax meet" className="absolute inset-0 size-full">
        <circle cx="122" cy="40" r="15" fill="var(--thumb-figure, #4ba3f2)" />
        <path d="M86 90c0-22 16-34 36-34s36 12 36 34z" fill="var(--thumb-figure, #4ba3f2)" />
      </svg>
      <span className={`absolute start-[7%] bottom-[9%] line-clamp-2 max-w-[62%] font-heading ${small ? "text-[.62rem]" : "text-[1.05rem]"} leading-[.95] font-black text-white [font-stretch:72%] [font-variation-settings:'wdth'_72] wrap-anywhere`}>
        {title}
      </span>
    </span>
  );
}

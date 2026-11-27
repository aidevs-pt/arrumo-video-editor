"use client";

import { useEffect, useRef, useState, type ComponentProps, type ReactNode } from "react";
import { cn } from "@/lib/utils";

/** Where the panel sticks: `top-20`. */
const STUCK_TOP = 80;
const BOTTOM_GAP = 16;

/**
 * A details panel beside a list. On wide windows it stays in view with its title and actions
 * pinned, and only its body scrolls. Its height follows where it starts, so on a short page it
 * still ends inside the window instead of below it.
 */
export function InspectorPanel({ header, footer, children, className, bodyClassName, ...props }: ComponentProps<"aside"> & {
  header: ReactNode;
  footer?: ReactNode;
  bodyClassName?: string;
}) {
  const panel = useRef<HTMLElement>(null);
  const [maxHeight, setMaxHeight] = useState<number>();

  useEffect(() => {
    const wide = window.matchMedia("(min-width: 80rem)");
    let frame = 0;
    const fit = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const element = panel.current;
        if (!element || !wide.matches) { setMaxHeight(undefined); return; }
        const top = Math.max(element.getBoundingClientRect().top, STUCK_TOP);
        setMaxHeight(Math.max(240, window.innerHeight - top - BOTTOM_GAP));
      });
    };
    fit();
    window.addEventListener("scroll", fit, { passive: true });
    window.addEventListener("resize", fit);
    wide.addEventListener("change", fit);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("scroll", fit);
      window.removeEventListener("resize", fit);
      wide.removeEventListener("change", fit);
    };
  }, []);

  return (
    <aside ref={panel} style={{ maxHeight }}
      className={cn("flex flex-col overflow-hidden rounded-xl border bg-card xl:sticky xl:top-20", className)} {...props}>
      <div className="flex items-start justify-between gap-3 border-b px-5 py-4">{header}</div>
      <div className={cn("flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto overscroll-contain p-5 *:shrink-0", bodyClassName)}>{children}</div>
      {footer && <div className="border-t px-5 py-4">{footer}</div>}
    </aside>
  );
}

import type { ComponentProps } from "react";
import type { LucideIcon } from "lucide-react";
import { cn } from "@/lib/utils";

export function WorkspaceShell({ className, ...props }: ComponentProps<"main">) {
  return <main className={cn(
    "mx-auto max-w-6xl px-4 py-6 sm:px-7 sm:py-7 [&>button]:mb-4",
    className,
  )} {...props} />;
}

export function WorkspaceHeading({ className, icon: Icon, children, ...props }: ComponentProps<"div"> & { icon?: LucideIcon }) {
  return (
    <div className={cn("mb-6 flex items-start gap-3 border-b pb-5", className)} {...props}>
      {Icon && <span className="grid size-11 shrink-0 place-items-center rounded-lg bg-foreground text-background">
        <Icon className="size-5" aria-hidden="true" />
      </span>}
      <div className="min-w-0 [&>h1]:wrap-anywhere [&>h1]:text-[clamp(1.65rem,3.5vw,2.15rem)] [&>h1]:leading-[1.1] [&>h1]:font-bold [&>h1]:tracking-[-0.035em] [&>h1]:outline-none [&>p]:mt-1.5 [&>p]:max-w-[65ch] [&>p]:text-sm [&>p]:text-muted-foreground">
        {children}
      </div>
    </div>
  );
}

export function WorkspaceColumns({ className, wideForm, ...props }: ComponentProps<"div"> & {
  wideForm?: boolean;
}) {
  return <div className={cn(
    "grid grid-cols-1 items-start gap-6 [&_h2]:mb-4 [&_h2]:text-base [&_h2]:font-semibold",
    wideForm ? "min-[1080px]:grid-cols-[minmax(180px,1fr)_minmax(0,2fr)]"
      : "min-[1080px]:grid-cols-[minmax(0,1.5fr)_minmax(16rem,1fr)]",
    className,
  )} {...props} />;
}

export function WorkspaceForm({ className, ...props }: ComponentProps<"form">) {
  return <form className={cn(
    "rounded-xl border bg-workspace-panel p-4 sm:p-5",
    className,
  )} {...props} />;
}

export function WorkspaceRows({ className, compact, ...props }: ComponentProps<"ul"> & {
  compact?: boolean;
}) {
  return <ul className={cn(
    "m-0 list-none divide-y rounded-xl border bg-card p-0 [&>li]:wrap-anywhere [&>li]:px-4",
    compact ? "[&>li]:py-1.5" : "[&>li]:py-[15px]",
    className,
  )} {...props} />;
}

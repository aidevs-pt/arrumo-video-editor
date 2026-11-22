import type { ComponentProps } from "react";
import { cn } from "@/lib/utils";

export function WorkspaceShell({ className, ...props }: ComponentProps<"main">) {
  return <main className={cn(
    "mx-auto max-w-6xl px-4 py-6 sm:px-9 sm:py-9 [&>button]:mb-4",
    className,
  )} {...props} />;
}

export function WorkspaceHeading({ className, children, ...props }: ComponentProps<"div">) {
  return (
    <div className={cn("mb-6", className)} {...props}>
      <div className="min-w-0 [&>h1]:wrap-anywhere [&>h1]:text-[clamp(2.25rem,5vw,3.25rem)] [&>h1]:leading-[1] [&>h1]:font-[850] [&>h1]:outline-none [&>h1]:[font-variation-settings:'wdth'_135] [&>p]:mt-2 [&>p]:max-w-[65ch] [&>p]:text-lg [&>p]:text-muted-foreground">
        {children}
      </div>
    </div>
  );
}

export function WorkspaceColumns({ className, wideForm, ...props }: ComponentProps<"div"> & {
  wideForm?: boolean;
}) {
  return <div className={cn(
    "grid grid-cols-1 items-start gap-6 [&_h2]:mb-4 [&_h2]:text-[1.3125rem]",
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
    "m-0 list-none divide-y border-y bg-transparent p-0 [&>li]:wrap-anywhere",
    compact ? "[&>li]:py-1.5" : "[&>li]:py-[15px]",
    className,
  )} {...props} />;
}

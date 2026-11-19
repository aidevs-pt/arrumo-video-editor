import type { ComponentProps } from "react";
import { cn } from "@/lib/utils";

export function WorkspaceShell({ className, ...props }: ComponentProps<"main">) {
  return <main className={cn(
    "mx-auto max-w-7xl px-[clamp(24px,5vw,64px)] py-[clamp(38px,8vw,100px)] [&>button]:mb-[18px]",
    className,
  )} {...props} />;
}

export function WorkspaceHeading({ className, ...props }: ComponentProps<"div">) {
  return <div className={cn(
    "mb-[clamp(30px,6vw,62px)] [&>h1]:text-[clamp(2.4rem,6vw,4.5rem)] [&>h1]:leading-[1.05] [&>h1]:tracking-[-0.04em] [&>h1]:font-[720] [&>h1]:wrap-anywhere [&>p]:mt-3 [&>p]:max-w-[60ch] [&>p]:text-muted-foreground",
    className,
  )} {...props} />;
}

export function WorkspaceColumns({ className, wideForm, ...props }: ComponentProps<"div"> & {
  wideForm?: boolean;
}) {
  return <div className={cn(
    "grid grid-cols-1 items-start gap-[clamp(20px,5vw,54px)] [&_h2]:mb-4 [&_h2]:text-[1.05rem] [&_h2]:font-[680]",
    wideForm ? "min-[681px]:grid-cols-[minmax(180px,1fr)_minmax(0,2fr)]"
      : "min-[681px]:grid-cols-[minmax(0,1.5fr)_minmax(250px,1fr)]",
    className,
  )} {...props} />;
}

export function WorkspaceForm({ className, ...props }: ComponentProps<"form">) {
  return <form className={cn(
    "rounded-[14px] bg-workspace-panel p-6 max-[680px]:row-start-1",
    className,
  )} {...props} />;
}

export function WorkspaceRows({ className, compact, ...props }: ComponentProps<"ul"> & {
  compact?: boolean;
}) {
  return <ul className={cn(
    "m-0 list-none border-t p-0 [&>li]:wrap-anywhere [&>li]:border-b",
    compact ? "[&>li]:py-1.5" : "[&>li]:py-[15px]",
    className,
  )} {...props} />;
}

"use client";

import { forwardRef, type ComponentProps, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

/**
 * A square icon button named by its tooltip, for rows of secondary actions that would wrap as
 * labelled buttons. It forwards its ref, so it can open a dialog or menu as their trigger.
 */
export const IconAction = forwardRef<HTMLButtonElement, Omit<ComponentProps<typeof Button>, "size" | "children"> & {
  label: string;
  children: ReactNode;
  destructive?: boolean;
}>(function IconAction({ label, children, destructive, className, variant = "outline", ...props }, ref) {
  return (
    <TooltipProvider delayDuration={250}>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button ref={ref} type="button" size="icon" variant={variant} aria-label={label}
            className={cn(destructive && "text-destructive hover:text-destructive", className)} {...props}>
            {children}
          </Button>
        </TooltipTrigger>
        <TooltipContent>{label}</TooltipContent>
      </Tooltip>
    </TooltipProvider>
  );
});

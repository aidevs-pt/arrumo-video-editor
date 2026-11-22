"use client";

import type { LucideIcon } from "lucide-react";
import { useRouter } from "next/navigation";
import type { ReactElement } from "react";

import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuLabel, ContextMenuSeparator, ContextMenuTrigger } from "@/components/ui/context-menu";

export type ItemMenuEntry = {
  label: string;
  icon?: LucideIcon;
  /** A page to open, or a handler to run. */
  href?: string;
  onSelect?: () => void;
  destructive?: boolean;
  disabled?: boolean;
  /** Starts a new group with a separator above it. */
  group?: boolean;
} | false | null | undefined;

/** The right-click menu of a list item. It repeats actions the item already shows, so nothing is only
 * reachable here; the item stays usable by keyboard (Shift+F10 or the context-menu key) and touch. */
export function ItemContextMenu({ label, items, children }: {
  /** The item's name, shown at the top of the menu. */
  label?: string;
  items: ItemMenuEntry[];
  children: ReactElement;
}) {
  const router = useRouter();
  const entries = items.filter((item): item is Exclude<ItemMenuEntry, false | null | undefined> => !!item);
  if (!entries.length) return children;
  return <ContextMenu>
    <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
    <ContextMenuContent className="max-w-72">
      {label && <>
        <ContextMenuLabel className="truncate">{label}</ContextMenuLabel>
        <ContextMenuSeparator />
      </>}
      {entries.map((item, index) => {
        const Icon = item.icon;
        return <div key={`${item.label}-${index}`} className="contents">
          {item.group && index > 0 && <ContextMenuSeparator />}
          <ContextMenuItem variant={item.destructive ? "destructive" : "default"} disabled={item.disabled}
            onSelect={() => {
              if (item.href) router.push(item.href);
              // After the menu has closed and handed focus back, so a dialog it opens keeps focus.
              else if (item.onSelect) setTimeout(item.onSelect, 0);
            }}>
            {Icon && <Icon aria-hidden="true" />}{item.label}
          </ContextMenuItem>
        </div>;
      })}
    </ContextMenuContent>
  </ContextMenu>;
}

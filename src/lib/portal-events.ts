import type { MouseEvent } from "react";

/**
 * React sends events from portalled content up through the component that rendered it, so a
 * right-click in a dialog opened from a list item would also open that item's context menu.
 * Overlays stop it there and keep the system menu (copy, paste) for their own fields.
 */
export function keepContextMenuInside<T extends Element>(handler?: (event: MouseEvent<T>) => void) {
  return (event: MouseEvent<T>) => {
    handler?.(event);
    event.stopPropagation();
  };
}

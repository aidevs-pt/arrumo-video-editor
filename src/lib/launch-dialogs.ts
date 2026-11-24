import { useEffect, useSyncExternalStore } from "react";

/** Dialogs that can open on their own at launch, most important first. */
const order = ["media-tools", "update", "announcement"] as const;
type LaunchDialog = (typeof order)[number];

const waiting = new Set<LaunchDialog>();
const listeners = new Set<() => void>();
const current = () => order.find((dialog) => waiting.has(dialog));

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Whether this dialog may open now: only one launch dialog shows at a time, the rest wait. */
export function useLaunchDialog(dialog: LaunchDialog, wants: boolean): boolean {
  useEffect(() => {
    if (!wants) return;
    waiting.add(dialog);
    for (const listener of listeners) listener();
    return () => {
      waiting.delete(dialog);
      for (const listener of listeners) listener();
    };
  }, [dialog, wants]);
  return useSyncExternalStore(subscribe, current, () => undefined) === dialog && wants;
}

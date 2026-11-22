"use client";

import { invoke } from "@tauri-apps/api/core";
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import { useEffect } from "react";

import { useLocale } from "@/lib/locale-provider";

type Notice = { id: number; kind: "review_required" | "completed" | "failed"; workspace: string; title: string };
type Poll = { latest: number; enabled: boolean; notices: Notice[] };

const POLL_MS = 5000;
// Shared by every mounted watcher, so a remount never sends an event twice.
let sentThrough = 0;

/** Sends finished, failed and waiting-for-review videos from every workspace to the operating
 * system's notifications while Arrumo is in the background, unless turned off in Settings. */
export function useSystemNotifications(enabled: boolean) {
  const { messages: { systemNotifications: copy } } = useLocale();

  useEffect(() => {
    if (!enabled) return;
    let active = true;
    let after: number | null = null;
    let timer: ReturnType<typeof setTimeout>;
    let allowed: boolean | null = null;

    async function permitted() {
      if (allowed === null) {
        allowed = await isPermissionGranted().catch(() => false);
        if (!allowed) allowed = (await requestPermission().catch(() => "denied")) === "granted";
      }
      return allowed;
    }

    async function poll() {
      try {
        const result = await invoke<Poll>("system_notices", { after });
        if (after !== null && result.enabled && result.notices.length && !document.hasFocus() && await permitted()) {
          for (const notice of result.notices) {
            if (notice.id <= sentThrough) continue;
            sentThrough = notice.id;
            sendNotification({
              title: copy[notice.kind].replace("{title}", notice.title),
              body: copy.body.replace("{workspace}", notice.workspace),
            });
          }
        }
        after = result.notices.at(-1)?.id ?? Math.max(after ?? 0, result.latest);
      } catch {}
      if (active) timer = setTimeout(poll, POLL_MS);
    }
    void poll();
    return () => { active = false; clearTimeout(timer); };
  }, [enabled, copy]);
}

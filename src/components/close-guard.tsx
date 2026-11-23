"use client";

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from "@/components/ui/alert-dialog";
import { useLocale } from "@/lib/locale-provider";

/** Asks before the window closes while videos are generating; the desktop host holds the close. */
export function CloseGuard() {
  const { messages: { closeGuard: copy } } = useLocale();
  const [generating, setGenerating] = useState(0);

  useEffect(() => {
    let active = true;
    let unsubscribe: (() => void) | undefined;
    void listen<number>("close-requested", (event) => { if (active) setGenerating(event.payload); })
      .then((unlisten) => { if (active) unsubscribe = unlisten; else unlisten(); })
      .catch(() => {
        // Without the listener the host never holds the close.
      });
    return () => { active = false; unsubscribe?.(); };
  }, []);

  function answer(close: boolean) {
    setGenerating(0);
    void invoke("answer_close_request", { close }).catch(() => {});
  }

  return (
    <AlertDialog open={generating > 0} onOpenChange={(open) => { if (!open) answer(false); }}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{copy.title}</AlertDialogTitle>
          <AlertDialogDescription>
            {(generating === 1 ? copy.descriptionOne : copy.description).replace("{count}", String(generating))}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>{copy.keepOpen}</AlertDialogCancel>
          <AlertDialogAction onClick={() => answer(true)}>{copy.close}</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}

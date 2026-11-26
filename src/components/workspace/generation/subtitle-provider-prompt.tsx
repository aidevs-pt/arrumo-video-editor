import { invoke } from "@tauri-apps/api/core";
import { useRouter } from "next/navigation";
import { useCallback, useRef, useState } from "react";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription,
  AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { useLocale } from "@/lib/locale-provider";

/** What to do with a video that asks for automatic subtitles. */
export type SubtitleChoice = "continue" | "without" | "cancel";

/**
 * Before a video is queued with automatic subtitles, checks that the workspace has a
 * transcription provider ready, and otherwise asks whether to set one up or go on without them,
 * so the creator doesn't find out at delivery.
 */
export function useSubtitleProviderPrompt(workspaceId: number) {
  const { messages: { queue: copy } } = useLocale();
  const router = useRouter();
  const [open, setOpen] = useState(false);
  const resolver = useRef<(choice: SubtitleChoice) => void>(undefined);

  const confirm = useCallback(async (wanted: boolean): Promise<SubtitleChoice> => {
    if (!wanted) return "continue";
    try {
      if (await invoke<boolean>("subtitle_provider_ready", { workspaceId })) return "continue";
    } catch {
      // The check is advice; a video can still be queued and its subtitles added later.
      return "continue";
    }
    return new Promise((resolve) => {
      resolver.current = resolve;
      setOpen(true);
    });
  }, [workspaceId]);

  function finish(choice: SubtitleChoice) {
    setOpen(false);
    resolver.current?.(choice);
    resolver.current = undefined;
  }

  function setUp() {
    finish("cancel");
    router.push(`/settings?workspace=${workspaceId}&scope=app&section=services`);
  }

  const dialog = (
    <AlertDialog open={open} onOpenChange={(next) => { if (!next) finish("cancel"); }}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{copy.subtitleProviderTitle}</AlertDialogTitle>
          <AlertDialogDescription>{copy.subtitleProviderDescription}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter className="sm:flex-wrap">
          <AlertDialogCancel className="sm:me-auto">{copy.subtitleProviderCancel}</AlertDialogCancel>
          <Button type="button" variant="outline" onClick={() => finish("without")}>{copy.subtitleProviderWithout}</Button>
          <AlertDialogAction onClick={setUp}>{copy.subtitleProviderSetUp}</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );

  return { confirm, dialog };
}

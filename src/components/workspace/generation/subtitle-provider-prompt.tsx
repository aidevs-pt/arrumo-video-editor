import { invoke } from "@tauri-apps/api/core";
import { Captions } from "lucide-react";
import { useRouter } from "next/navigation";
import { useCallback, useRef, useState } from "react";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription,
  AlertDialogFooter, AlertDialogHeader, AlertDialogMedia, AlertDialogTitle,
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

  const confirm = useCallback(async (wanted: boolean, transcriptionOrder?: string[]): Promise<SubtitleChoice> => {
    if (!wanted) return "continue";
    try {
      if (await invoke<boolean>("subtitle_provider_ready", { workspaceId, transcriptionOrder: transcriptionOrder ?? null })) return "continue";
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
      <AlertDialogContent className="gap-5 p-6 data-[size=default]:max-w-[calc(100vw-2rem)] data-[size=default]:sm:max-w-lg">
        <AlertDialogHeader className="gap-2">
          <AlertDialogMedia className="bg-primary/10 text-primary"><Captions aria-hidden="true" /></AlertDialogMedia>
          <AlertDialogTitle className="text-lg">{copy.subtitleProviderTitle}</AlertDialogTitle>
          <AlertDialogDescription className="text-pretty">{copy.subtitleProviderDescription}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter className="-mx-6 -mb-6 px-6 py-4 sm:items-center">
          <AlertDialogCancel variant="ghost" className="sm:me-auto">{copy.subtitleProviderCancel}</AlertDialogCancel>
          <Button type="button" variant="outline" onClick={() => finish("without")}>{copy.subtitleProviderWithout}</Button>
          <AlertDialogAction onClick={setUp}>{copy.subtitleProviderSetUp}</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );

  return { confirm, dialog };
}

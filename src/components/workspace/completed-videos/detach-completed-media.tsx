import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent,
  AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle, AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { useLocale } from "@/lib/locale-provider";

export function DetachCompletedMedia({ workspaceId, runId, attachmentId, name, onDetached }: {
  workspaceId: number;
  runId: number;
  attachmentId: number;
  name: string;
  onDetached: () => void;
}) {
  const { messages: { completedVideo: copy } } = useLocale();
  const [open, setOpen] = useState(false);
  const [pending, setPending] = useState(false);
  const [failed, setFailed] = useState(false);

  async function detach() {
    if (pending) return;
    setPending(true);
    setFailed(false);
    try {
      await invoke<boolean>("detach_completed_media", { workspaceId, runId, attachmentId });
      setOpen(false);
      onDetached();
    } catch {
      setFailed(true);
    } finally {
      setPending(false);
    }
  }

  return <AlertDialog open={open} onOpenChange={(next) => {
    if (pending) return;
    setFailed(false);
    setOpen(next);
  }}>
    <AlertDialogTrigger asChild>
      <Button size="sm" variant="outline">{copy.detachAttachment}</Button>
    </AlertDialogTrigger>
    <AlertDialogContent>
      <AlertDialogHeader>
        <AlertDialogTitle className="wrap-anywhere">{copy.detachAttachmentTitle.replace("{name}", () => name)}</AlertDialogTitle>
        <AlertDialogDescription>{copy.detachAttachmentDescription}</AlertDialogDescription>
      </AlertDialogHeader>
      {failed && <Alert variant="destructive"><AlertDescription>{copy.detachAttachmentError}</AlertDescription></Alert>}
      <AlertDialogFooter>
        <AlertDialogCancel disabled={pending}>{copy.cancelDelete}</AlertDialogCancel>
        <AlertDialogAction variant="destructive" disabled={pending} onClick={(event) => {
          event.preventDefault();
          void detach();
        }}>{copy.detachAttachment}</AlertDialogAction>
      </AlertDialogFooter>
    </AlertDialogContent>
  </AlertDialog>;
}

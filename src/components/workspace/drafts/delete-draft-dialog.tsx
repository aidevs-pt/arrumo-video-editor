import { invoke } from "@tauri-apps/api/core";
import { Trash2 } from "lucide-react";
import { useId, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import {
  AlertDialog, AlertDialogCancel, AlertDialogContent, AlertDialogDescription,
  AlertDialogFooter, AlertDialogHeader, AlertDialogTitle, AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { useLocale } from "@/lib/locale-provider";

export function DeleteDraftDialog({ workspaceId, draftId, title, disabled, onDelete }: {
  workspaceId: number;
  draftId: number;
  title: string;
  disabled: boolean;
  onDelete: () => void;
}) {
  const { messages: { drafts: copy } } = useLocale();
  const inputId = useId();
  const [open, setOpen] = useState(false);
  const [confirmation, setConfirmation] = useState("");
  const [pending, setPending] = useState(false);
  const [failed, setFailed] = useState(false);

  async function remove(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending || confirmation !== title) return;
    setPending(true);
    setFailed(false);
    try {
      if (await invoke<boolean>("delete_video_draft", { workspaceId, draftId, confirmedTitle: confirmation })) {
        setOpen(false);
        onDelete();
      } else {
        setFailed(true);
      }
    } catch {
      setFailed(true);
    } finally {
      setPending(false);
    }
  }

  return (
    <AlertDialog open={open} onOpenChange={(next) => {
      if (pending) return;
      setOpen(next);
      if (!next) {
        setConfirmation("");
        setFailed(false);
      }
    }}>
      <AlertDialogTrigger asChild>
        <Button type="button" size="sm" variant="ghost" disabled={disabled} aria-label={`${copy.deleteDraft} · ${title}`}>
          <Trash2 data-icon="inline-start" />{copy.deleteDraft}
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{copy.deleteDraft}</AlertDialogTitle>
          <AlertDialogDescription>{copy.deleteDescription}</AlertDialogDescription>
        </AlertDialogHeader>
        <form className="flex flex-col gap-4" onSubmit={remove}>
          <Field>
            <FieldLabel htmlFor={inputId}>{copy.confirmTitle}</FieldLabel>
            <p className="select-text wrap-anywhere text-sm font-medium">{title}</p>
            <Input id={inputId} value={confirmation} onChange={(event) => setConfirmation(event.target.value)}
              disabled={pending} autoComplete="off" spellCheck={false} />
          </Field>
          {failed && <Alert variant="destructive"><AlertDescription>{copy.deleteError}</AlertDescription></Alert>}
          <AlertDialogFooter>
            <AlertDialogCancel disabled={pending}>{copy.cancelDelete}</AlertDialogCancel>
            <Button type="submit" variant="destructive" disabled={pending || confirmation !== title}>
              {pending ? copy.deletingDraft : copy.deleteDraft}
            </Button>
          </AlertDialogFooter>
        </form>
      </AlertDialogContent>
    </AlertDialog>
  );
}

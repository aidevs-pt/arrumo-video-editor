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

export function ConfirmTitleDelete({ title, disabled, copy, onConfirm, onDelete }: {
  title: string;
  disabled: boolean;
  copy: {
    deleteTitle: string;
    deleteDescription: string;
    confirmTitle: string;
    deleteError: string;
    cancelDelete: string;
    deleting: string;
  };
  onConfirm: (title: string) => Promise<boolean>;
  onDelete: () => void;
}) {
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
      if (await onConfirm(confirmation)) {
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
        <Button type="button" size="sm" variant="ghost" disabled={disabled} aria-label={`${copy.deleteTitle} · ${title}`}>
          <Trash2 data-icon="inline-start" />{copy.deleteTitle}
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{copy.deleteTitle}</AlertDialogTitle>
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
              {pending ? copy.deleting : copy.deleteTitle}
            </Button>
          </AlertDialogFooter>
        </form>
      </AlertDialogContent>
    </AlertDialog>
  );
}

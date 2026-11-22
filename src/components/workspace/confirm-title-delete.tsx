import { Check, Copy, Trash2 } from "lucide-react";
import { useLocale } from "@/lib/locale-provider";
import { useId, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import {
  AlertDialog, AlertDialogCancel, AlertDialogContent, AlertDialogDescription,
  AlertDialogFooter, AlertDialogHeader, AlertDialogTitle, AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";

export function ConfirmTitleDelete({ title, disabled, copy, onConfirm, onDelete, compact = false, size = "default", open: controlled, onOpenChange, hideTrigger = false }: {
  /** Opened only by another control, such as a menu item. */
  hideTrigger?: boolean;
  title: string;
  disabled: boolean;
  compact?: boolean;
  /** Matches the buttons beside it. */
  size?: "default" | "sm";
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
  /** Lets another control, such as a context menu, open the dialog. */
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}) {
  const inputId = useId();
  const [own, setOwn] = useState(false);
  const open = controlled ?? own;
  const setOpen = (next: boolean) => { setOwn(next); onOpenChange?.(next); };
  const [confirmation, setConfirmation] = useState("");
  const [pending, setPending] = useState(false);
  const [failed, setFailed] = useState(false);
  const [copied, setCopied] = useState(false);
  const { messages } = useLocale();
  const shared = messages.pluginActions;

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
      {!hideTrigger && <AlertDialogTrigger asChild>
        <Button type="button" size={compact ? "icon-sm" : size} variant="ghost" disabled={disabled} title={compact ? copy.deleteTitle : undefined}
          aria-label={`${copy.deleteTitle} · ${title}`}>
          <Trash2 data-icon="inline-start" />{!compact && copy.deleteTitle}
        </Button>
      </AlertDialogTrigger>}
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{copy.deleteTitle}</AlertDialogTitle>
          <AlertDialogDescription>{copy.deleteDescription}</AlertDialogDescription>
        </AlertDialogHeader>
        <form className="flex flex-col gap-4" onSubmit={remove}>
          <Field>
            <FieldLabel htmlFor={inputId}>{copy.confirmTitle}</FieldLabel>
            <div className="flex items-start gap-2 rounded-md border bg-muted/60 py-1.5 ps-3 pe-1.5">
              <code className="min-w-0 flex-1 select-text py-1 font-mono text-sm wrap-anywhere">{title}</code>
              <Button type="button" variant="ghost" size="icon-sm" title={copied ? shared.copied : shared.copy} aria-label={copied ? shared.copied : shared.copy}
                onClick={() => void navigator.clipboard.writeText(title).then(() => { setCopied(true); setTimeout(() => setCopied(false), 1500); }, () => {})}>
                {copied ? <Check /> : <Copy />}
              </Button>
            </div>
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

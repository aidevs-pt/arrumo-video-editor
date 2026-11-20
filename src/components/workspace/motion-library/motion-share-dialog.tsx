import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { useLocale } from "@/lib/locale-provider";

export function MotionShareDialog({ workspaceId, itemId, title, ownerId, profiles, disabled, onSave }: {
  workspaceId: number;
  itemId: number;
  title: string;
  ownerId: number;
  profiles: { id: number; name: string }[];
  disabled: boolean;
  onSave: () => void;
}) {
  const { messages } = useLocale();
  const copy = messages.motionLibrary;
  const [open, setOpen] = useState(false);
  const [recipients, setRecipients] = useState<number[]>();
  const [error, setError] = useState<"load" | "save">();
  const [attempt, setAttempt] = useState(0);
  const [saving, setSaving] = useState(false);
  const available = profiles.filter(({ id }) => id !== ownerId);

  useEffect(() => {
    if (!open) return;
    let active = true;
    invoke<number[]>("get_motion_library_shares", { workspaceId, itemId }).then(
      (ids) => { if (active) setRecipients(ids); },
      () => { if (active) setError("load"); },
    );
    return () => { active = false; };
  }, [open, workspaceId, itemId, attempt]);

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!recipients || saving) return;
    setSaving(true);
    setError(undefined);
    try {
      await invoke("set_motion_library_shares", { workspaceId, itemId, profileIds: recipients });
      setOpen(false);
      onSave();
    } catch {
      setError("save");
    } finally {
      setSaving(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={(next) => {
      if (saving) return;
      setRecipients(undefined);
      setError(undefined);
      setOpen(next);
    }}>
      <DialogTrigger asChild>
        <Button type="button" size="sm" variant="ghost" disabled={disabled} aria-label={`${copy.share} · ${title}`}>
          {copy.share}
        </Button>
      </DialogTrigger>
      <DialogContent className="max-h-[85dvh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>{copy.share}</DialogTitle>
          <DialogDescription>{copy.shareDescription}</DialogDescription>
        </DialogHeader>
        <form className="flex flex-col gap-4" onSubmit={save}>
          <p className="wrap-anywhere font-medium">{title}</p>
          {error === "load" ? <Alert variant="destructive">
            <AlertDescription>{copy.shareLoadError}</AlertDescription>
            <Button type="button" variant="outline" onClick={() => {
              setError(undefined);
              setAttempt((value) => value + 1);
            }}>{copy.retry}</Button>
          </Alert> : !recipients ? <p role="status" className="text-muted-foreground">{copy.loadingShares}</p>
            : available.length === 0 ? <p className="text-muted-foreground">{copy.noRecipients}</p>
            : <fieldset className="flex flex-col gap-2" disabled={saving}>
              <legend className="sr-only">{copy.share}</legend>
              {available.map(({ id, name }) => (
                <label key={id} className="flex items-center gap-3 rounded-md border px-3 py-2">
                  <Checkbox checked={recipients.includes(id)} onCheckedChange={(checked) => {
                    setRecipients((current) => checked === true ? [...(current ?? []), id] : current?.filter((value) => value !== id));
                  }} />
                  <span className="wrap-anywhere">{name}</span>
                </label>
              ))}
            </fieldset>}
          {error === "save" && <Alert variant="destructive"><AlertDescription>{copy.shareSaveError}</AlertDescription></Alert>}
          <DialogFooter>
            <Button type="button" variant="outline" disabled={saving} onClick={() => setOpen(false)}>{copy.cancel}</Button>
            <Button type="submit" disabled={!recipients || !available.length || saving}>{saving ? copy.saving : copy.saveShares}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

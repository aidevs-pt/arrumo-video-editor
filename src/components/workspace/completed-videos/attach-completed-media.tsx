import { invoke } from "@tauri-apps/api/core";
import { Paperclip } from "lucide-react";
import { useEffect, useId, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Field, FieldLabel } from "@/components/ui/field";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Skeleton } from "@/components/ui/skeleton";
import { useLocale } from "@/lib/locale-provider";

type Asset = { id: number; name: string };

export function AttachCompletedMedia({ workspaceId, runId, formatId, formatName, attached, onAttached }: {
  workspaceId: number;
  runId: number;
  formatId: number;
  formatName: string;
  attached: readonly number[];
  onAttached: () => void;
}) {
  const { messages: { completedVideo: copy, queue } } = useLocale();
  const inputId = useId();
  const [open, setOpen] = useState(false);
  const [assets, setAssets] = useState<Asset[]>();
  const [assetId, setAssetId] = useState("");
  const [error, setError] = useState<"attachLoadError" | "attachSaveError">();
  const [attempt, setAttempt] = useState(0);
  const [saving, setSaving] = useState(false);
  const options = assets?.filter((asset) => !attached.includes(asset.id));

  useEffect(() => {
    if (!open) return;
    let active = true;
    invoke<Asset[]>("list_assets", { workspaceId }).then(
      (items) => { if (active) setAssets(items); },
      () => { if (active) setError("attachLoadError"); },
    );
    return () => { active = false; };
  }, [open, attempt, workspaceId]);

  async function attach(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (saving || !options?.some((asset) => String(asset.id) === assetId)) return;
    setSaving(true);
    setError(undefined);
    try {
      await invoke<boolean>("attach_completed_media", { workspaceId, runId, formatId, assetId: Number(assetId) });
      setOpen(false);
      onAttached();
    } catch {
      setError("attachSaveError");
    } finally {
      setSaving(false);
    }
  }

  return <Dialog open={open} onOpenChange={(next) => {
    if (saving) return;
    if (next) {
      setAssets(undefined);
      setError(undefined);
      setAssetId("");
    }
    setOpen(next);
  }}>
    <DialogTrigger asChild>
      <Button size="sm" variant="outline"><Paperclip data-icon="inline-start" />{copy.attachFile}</Button>
    </DialogTrigger>
    <DialogContent showCloseButton={!saving}>
      <DialogHeader>
        <DialogTitle>{copy.attachFile}</DialogTitle>
        <DialogDescription>{copy.attachFileDescription.replace("{format}", () => formatName)}</DialogDescription>
      </DialogHeader>
      <form className="flex flex-col gap-4" onSubmit={(event) => void attach(event)}>
        {error === "attachLoadError" ? <>
          <Alert variant="destructive"><AlertDescription>{copy[error]}</AlertDescription></Alert>
          <Button type="button" variant="outline" className="self-start" onClick={() => {
            setError(undefined);
            setAttempt((current) => current + 1);
          }}>{queue.retry}</Button>
        </> : options === undefined ? <Skeleton className="h-8 w-full" role="status" aria-label={copy.loading} />
          : options.length === 0 ? <p className="text-sm text-muted-foreground">{copy.attachNoAssets}</p>
          : <Field>
            <FieldLabel htmlFor={inputId}>{copy.attachAssetLabel}</FieldLabel>
            <NativeSelect id={inputId} className="w-full min-w-0" required value={assetId} disabled={saving}
              onChange={(event) => setAssetId(event.target.value)}>
              <NativeSelectOption value="">{copy.attachAssetPlaceholder}</NativeSelectOption>
              {options.map((asset) => <NativeSelectOption key={asset.id} value={asset.id}>{asset.name}</NativeSelectOption>)}
            </NativeSelect>
          </Field>}
        {error === "attachSaveError" && <Alert variant="destructive"><AlertDescription>{copy[error]}</AlertDescription></Alert>}
        <DialogFooter>
          <Button type="button" variant="outline" disabled={saving} onClick={() => setOpen(false)}>{copy.cancelDelete}</Button>
          <Button type="submit" disabled={saving || !assetId || !options?.length}>{copy.attachFile}</Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>;
}

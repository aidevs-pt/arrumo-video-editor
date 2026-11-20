import { invoke } from "@tauri-apps/api/core";
import { useId, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { useLocale } from "@/lib/locale-provider";

export type AssetOrganization = { group: string | null; labels: string[] };

export function AssetOrganizationDialog({ workspaceId, assetId, name, organization, disabled, onSave }: {
  workspaceId: number;
  assetId: number;
  name: string;
  organization?: AssetOrganization;
  disabled: boolean;
  onSave: (organization: AssetOrganization) => void;
}) {
  const { messages: { assets: copy } } = useLocale();
  const id = useId();
  const [open, setOpen] = useState(false);
  const [pending, setPending] = useState(false);
  const [failed, setFailed] = useState(false);

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending) return;
    const fields = new FormData(event.currentTarget);
    const group = String(fields.get("group") ?? "").trim() || null;
    const labels = String(fields.get("labels") ?? "").split("\n").map((label) => label.trim()).filter(Boolean);
    setPending(true);
    setFailed(false);
    try {
      onSave(await invoke<AssetOrganization>("set_asset_organization", { workspaceId, assetId, settings: { group, labels } }));
      setOpen(false);
    } catch {
      setFailed(true);
    } finally {
      setPending(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={(next) => {
      if (pending) return;
      setOpen(next);
      setFailed(false);
    }}>
      <DialogTrigger asChild>
        <Button size="sm" variant="ghost" disabled={disabled} aria-label={`${copy.organize} · ${name}`}>{copy.organize}</Button>
      </DialogTrigger>
      <DialogContent showCloseButton={!pending}>
        <DialogHeader><DialogTitle>{copy.organize}</DialogTitle><DialogDescription>{name}</DialogDescription></DialogHeader>
        <form onSubmit={save}>
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor={`${id}-group`}>{copy.group}</FieldLabel>
              <Input id={`${id}-group`} name="group" defaultValue={organization?.group ?? ""} disabled={pending} />
            </Field>
            <Field>
              <FieldLabel htmlFor={`${id}-labels`}>{copy.labels}</FieldLabel>
              <Textarea id={`${id}-labels`} name="labels" defaultValue={organization?.labels.join("\n")} disabled={pending} />
            </Field>
            {failed && <Alert variant="destructive"><AlertDescription>{copy.organizationError}</AlertDescription></Alert>}
            <DialogFooter><Button type="submit" disabled={pending}>{copy.saveOrganization}</Button></DialogFooter>
          </FieldGroup>
        </form>
      </DialogContent>
    </Dialog>
  );
}

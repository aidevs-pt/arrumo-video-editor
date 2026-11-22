"use client";

import { invoke } from "@tauri-apps/api/core";
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { ImagePlus } from "lucide-react";
import { useEffect, useState, type FormEvent } from "react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Field, FieldGroup, FieldLabel, FieldLegend, FieldSet } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";
import { ColorPicker, SWATCHES } from "../color-picker";
import type { Workspace } from "../home/workspace-links";
import { WORKSPACE_ICONS, WorkspaceMark, useWorkspaceImage } from "./workspace-mark";

type ImageAsset = { id: number; name: string; kind: string };

function ImageChoice({ workspaceId, asset, selected, onSelect }: {
  workspaceId: number; asset: ImageAsset; selected: boolean; onSelect: () => void;
}) {
  const url = useWorkspaceImage(workspaceId, asset.id);
  return <button type="button" role="radio" aria-checked={selected} aria-label={asset.name} onClick={onSelect}
    className={cn("aspect-square overflow-hidden rounded-[25%] bg-muted outline-offset-2 focus-visible:outline-2 focus-visible:outline-attention",
      selected && "outline-2 outline-foreground")}>
    {/* Managed local preview provided by the desktop host. */}
    {/* eslint-disable-next-line @next/next/no-img-element */}
    {url && <img src={url} alt="" className="size-full object-cover" />}
  </button>;
}

/** Name, color and icon (or image) of a workspace, so it is recognisable in the switcher. */
export function WorkspaceAppearanceDialog({ workspace, open, onOpenChange, onSaved }: {
  workspace: Workspace;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSaved: (workspace: Workspace) => void;
}) {
  const { messages: { workspaces: copy, profileHub } } = useLocale();
  const [name, setName] = useState(workspace.name);
  const [color, setColor] = useState(workspace.color ?? SWATCHES[0]);
  const [icon, setIcon] = useState(workspace.icon ?? "clapperboard");
  const [image, setImage] = useState<number | null>(workspace.imageAssetId ?? null);
  const [mode, setMode] = useState<"icon" | "image">(workspace.imageAssetId ? "image" : "icon");
  const [images, setImages] = useState<ImageAsset[]>([]);
  const [saving, setSaving] = useState(false);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    if (!open) return;
    setName(workspace.name);
    setColor(workspace.color ?? SWATCHES[0]);
    setIcon(workspace.icon ?? "clapperboard");
    setImage(workspace.imageAssetId ?? null);
    setMode(workspace.imageAssetId ? "image" : "icon");
    setFailed(false);
    invoke<ImageAsset[]>("list_assets", { workspaceId: workspace.id }).then(
      (assets) => setImages(assets.filter((asset) => asset.kind === "image")),
      () => setImages([]),
    );
  }, [open, workspace]);

  async function importImage() {
    const source = await pickFile({ multiple: false, title: copy.pickImage,
      filters: [{ name: copy.image, extensions: ["png", "jpg", "jpeg", "webp", "gif"] }] });
    if (typeof source !== "string") return;
    try {
      const asset = await invoke<ImageAsset>("import_asset", { workspaceId: workspace.id, source });
      setImages((current) => [...current, asset]);
      setImage(asset.id);
    } catch {
      setFailed(true);
    }
  }

  async function save(event: FormEvent) {
    event.preventDefault();
    if (saving || !name.trim()) return;
    setSaving(true);
    setFailed(false);
    try {
      if (name.trim() !== workspace.name) await invoke("rename_workspace", { workspaceId: workspace.id, name: name.trim() });
      onSaved(await invoke<Workspace>("set_workspace_appearance", {
        workspaceId: workspace.id, color, icon, imageAssetId: mode === "image" ? image : null,
      }));
      onOpenChange(false);
    } catch {
      setFailed(true);
    } finally {
      setSaving(false);
    }
  }

  const preview = { id: workspace.id, color, icon, imageAssetId: mode === "image" ? image : null };
  return <Dialog open={open} onOpenChange={(next) => { if (!saving) onOpenChange(next); }}>
    <DialogContent className="sm:max-w-md">
      <DialogHeader>
        <DialogTitle>{copy.customize}</DialogTitle>
        <DialogDescription>{copy.customizeHint}</DialogDescription>
      </DialogHeader>
      <form onSubmit={save}>
        <FieldGroup>
          <div className="flex items-end gap-3">
            <WorkspaceMark workspace={preview} className="size-10" />
            <Field className="flex-1">
              <FieldLabel htmlFor="workspace-rename">{copy.name}</FieldLabel>
              <Input id="workspace-rename" value={name} onChange={(event) => setName(event.target.value)} maxLength={120} required />
            </Field>
          </div>
          <ToggleGroup type="single" variant="outline" value={mode} className="w-full"
            onValueChange={(value) => { if (value) setMode(value as "icon" | "image"); }}>
            <ToggleGroupItem value="icon" className="flex-1">{copy.icon}</ToggleGroupItem>
            <ToggleGroupItem value="image" className="flex-1">{copy.image}</ToggleGroupItem>
          </ToggleGroup>
          {mode === "icon" ? <>
            <FieldSet>
              <FieldLegend variant="label">{copy.icon}</FieldLegend>
              <div role="radiogroup" aria-label={copy.icon} className="grid grid-cols-8 gap-1.5">
                {Object.entries(WORKSPACE_ICONS).map(([key, Icon]) => (
                  <button key={key} type="button" role="radio" aria-checked={icon === key} aria-label={key} onClick={() => setIcon(key)}
                    className={cn("grid aspect-square place-items-center rounded-md border text-foreground transition-colors hover:bg-muted focus-visible:outline-2 focus-visible:outline-attention motion-reduce:transition-none",
                      icon === key && "border-transparent text-white")}
                    style={icon === key ? { backgroundColor: color } : undefined}>
                    <Icon className="size-4" />
                  </button>
                ))}
              </div>
            </FieldSet>
            <FieldSet>
              <FieldLegend variant="label">{copy.color}</FieldLegend>
              <ColorPicker value={color} onChange={(value) => { if (value) setColor(value); }}
                label={copy.color} customLabel={profileHub.customColor} />
            </FieldSet>
          </> : (
            <FieldSet>
              <FieldLegend variant="label">{copy.image}</FieldLegend>
              <div role="radiogroup" aria-label={copy.image} className="grid grid-cols-5 gap-2">
                {images.map((asset) => <ImageChoice key={asset.id} workspaceId={workspace.id} asset={asset}
                  selected={image === asset.id} onSelect={() => setImage(asset.id)} />)}
                <button type="button" onClick={() => void importImage()} aria-label={copy.importImage}
                  className="grid aspect-square place-items-center rounded-[25%] border border-dashed text-muted-foreground hover:border-primary hover:text-primary focus-visible:outline-2 focus-visible:outline-attention">
                  <ImagePlus className="size-5" />
                </button>
              </div>
              {!images.length && <p className="text-sm text-muted-foreground">{copy.noImages}</p>}
            </FieldSet>
          )}
          {failed && <Alert variant="destructive" role="alert"><AlertDescription>{copy.customizeError}</AlertDescription></Alert>}
          <DialogFooter>
            <Button type="button" variant="outline" disabled={saving} onClick={() => onOpenChange(false)}>{copy.cancel}</Button>
            <Button type="submit" disabled={saving || !name.trim() || (mode === "image" && !image)}>{saving ? copy.saving : copy.save}</Button>
          </DialogFooter>
        </FieldGroup>
      </form>
    </DialogContent>
  </Dialog>;
}

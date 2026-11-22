"use client";

import { invoke } from "@tauri-apps/api/core";
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { ImagePlus } from "lucide-react";
import { useEffect, useState, type FormEvent } from "react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel, FieldLegend, FieldSet } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { useLocale } from "@/lib/locale-provider";
import { cn } from "@/lib/utils";
import { ColorPicker, SWATCHES } from "../color-picker";
import type { Workspace } from "../home/workspace-links";
import { useWorkspaceCatalog } from "../home/workspace-router";
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

/** Workspace settings › Brand: name, color and icon (or image), so the workspace is recognisable in the switcher. */
export function WorkspaceAppearance({ workspace }: { workspace: Workspace }) {
  const { messages: { workspaces: copy, profileHub } } = useLocale();
  const { update } = useWorkspaceCatalog();
  const [name, setName] = useState(workspace.name);
  const [color, setColor] = useState(workspace.color ?? SWATCHES[0]);
  const [icon, setIcon] = useState(workspace.icon ?? "clapperboard");
  const [image, setImage] = useState<number | null>(workspace.imageAssetId ?? null);
  const [mode, setMode] = useState<"icon" | "image">(workspace.imageAssetId ? "image" : "icon");
  const [images, setImages] = useState<ImageAsset[]>([]);
  const [saving, setSaving] = useState(false);
  const [result, setResult] = useState<"saved" | "failed" | null>(null);

  useEffect(() => {
    invoke<ImageAsset[]>("list_assets", { workspaceId: workspace.id }).then(
      (assets) => setImages(assets.filter((asset) => asset.kind === "image")),
      () => setImages([]),
    );
  }, [workspace.id]);

  async function importImage() {
    const source = await pickFile({ multiple: false, title: copy.pickImage,
      filters: [{ name: copy.image, extensions: ["png", "jpg", "jpeg", "webp", "gif"] }] });
    if (typeof source !== "string") return;
    try {
      const asset = await invoke<ImageAsset>("import_asset", { workspaceId: workspace.id, source });
      setImages((current) => [...current, asset]);
      setImage(asset.id);
      setResult(null);
    } catch {
      setResult("failed");
    }
  }

  const imageAssetId = mode === "image" ? image : null;
  const dirty = name.trim() !== workspace.name || color !== (workspace.color ?? SWATCHES[0])
    || icon !== (workspace.icon ?? "clapperboard") || imageAssetId !== (workspace.imageAssetId ?? null);

  async function save(event: FormEvent) {
    event.preventDefault();
    if (saving || !name.trim() || !dirty) return;
    setSaving(true);
    setResult(null);
    try {
      if (name.trim() !== workspace.name) await invoke("rename_workspace", { workspaceId: workspace.id, name: name.trim() });
      update(await invoke<Workspace>("set_workspace_appearance", { workspaceId: workspace.id, color, icon, imageAssetId }));
      setResult("saved");
    } catch {
      setResult("failed");
    } finally {
      setSaving(false);
    }
  }

  const preview = { id: workspace.id, color, icon, imageAssetId };
  return <section id="appearance" aria-labelledby="workspace-appearance" className="scroll-mt-6">
    <div className="mb-4">
      <h2 id="workspace-appearance" className="text-[1.75rem]">{copy.customize}</h2>
      <p className="mt-1 text-muted-foreground">{copy.customizeHint}</p>
    </div>
    <form onSubmit={save} className="rounded-xl border bg-card p-5">
      <div className="grid gap-6 md:grid-cols-[minmax(0,18rem)_minmax(0,1fr)]">
        <FieldGroup>
          <div className="flex items-end gap-3">
            <WorkspaceMark workspace={preview} className="size-12" />
            <Field className="flex-1">
              <FieldLabel htmlFor="workspace-rename">{copy.name}</FieldLabel>
              <Input id="workspace-rename" value={name} onChange={(event) => { setName(event.target.value); setResult(null); }} maxLength={120} required />
            </Field>
          </div>
          <ToggleGroup type="single" variant="outline" value={mode} className="w-full"
            onValueChange={(value) => { if (value) { setMode(value as "icon" | "image"); setResult(null); } }}>
            <ToggleGroupItem value="icon" className="flex-1">{copy.icon}</ToggleGroupItem>
            <ToggleGroupItem value="image" className="flex-1">{copy.image}</ToggleGroupItem>
          </ToggleGroup>
        </FieldGroup>
        <FieldGroup>
          {mode === "icon" ? <>
            <FieldSet>
              <FieldLegend variant="label">{copy.icon}</FieldLegend>
              <div role="radiogroup" aria-label={copy.icon} className="grid grid-cols-8 gap-1.5 sm:max-w-md">
                {Object.entries(WORKSPACE_ICONS).map(([key, Icon]) => (
                  <button key={key} type="button" role="radio" aria-checked={icon === key} aria-label={key} onClick={() => { setIcon(key); setResult(null); }}
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
              <ColorPicker value={color} onChange={(value) => { if (value) { setColor(value); setResult(null); } }}
                label={copy.color} customLabel={profileHub.customColor} />
            </FieldSet>
          </> : (
            <FieldSet>
              <FieldLegend variant="label">{copy.image}</FieldLegend>
              <div role="radiogroup" aria-label={copy.image} className="grid grid-cols-5 gap-2 sm:max-w-md">
                {images.map((asset) => <ImageChoice key={asset.id} workspaceId={workspace.id} asset={asset}
                  selected={image === asset.id} onSelect={() => { setImage(asset.id); setResult(null); }} />)}
                <button type="button" onClick={() => void importImage()} aria-label={copy.importImage}
                  className="grid aspect-square place-items-center rounded-[25%] border border-dashed text-muted-foreground hover:border-primary hover:text-primary focus-visible:outline-2 focus-visible:outline-attention">
                  <ImagePlus className="size-5" />
                </button>
              </div>
              {!images.length && <p className="text-sm text-muted-foreground">{copy.noImages}</p>}
            </FieldSet>
          )}
        </FieldGroup>
      </div>
      {result === "failed" && <Alert variant="destructive" role="alert" className="mt-4"><AlertDescription>{copy.customizeError}</AlertDescription></Alert>}
      <div className="mt-5 flex flex-wrap items-center justify-end gap-3 border-t pt-4">
        {result === "saved" && <p role="status" className="text-sm font-semibold text-approval">{copy.appearanceSaved}</p>}
        <Button type="submit" disabled={saving || !name.trim() || !dirty || (mode === "image" && !image)}>{saving ? copy.saving : copy.save}</Button>
      </div>
    </form>
  </section>;
}

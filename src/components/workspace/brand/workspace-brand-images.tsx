"use client";

import { GenerateAssetDialog } from "@/components/workspace/assets/generate-asset-dialog";
import { useLocale } from "@/lib/locale-provider";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Images, TriangleAlert, Upload, X } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import Link from "next/link";
import { Checkbox } from "@/components/ui/checkbox";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Field, FieldLabel } from "@/components/ui/field";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Skeleton } from "@/components/ui/skeleton";
import { AssetThumbnail } from "../assets/asset-thumbnail";
import { workspaceHref } from "../home/workspace-links";

const roles = ["identity", "style"] as const;
type Role = typeof roles[number];
type Image = { id: number; name: string; kind: string; notes: string };
type Association = { asset_id: number; role: Role };

export function WorkspaceBrandImages({ workspaceId }: {
  workspaceId: number;

}) {
  const [images, setImages] = useState<Image[]>();
  const [associations, setAssociations] = useState<Association[]>([]);
  const [attempt, setAttempt] = useState(0);
  const [loadError, setLoadError] = useState(false);
  const [saveError, setSaveError] = useState(false);
  const [pending, setPending] = useState(false);
  const [uploadError, setUploadError] = useState(false);
  const [editing, setEditing] = useState<Role | null>(null);
  const { messages } = useLocale();
  const copy = messages.brandImages;

  useEffect(() => {
    let active = true;
    Promise.all([
      invoke<Image[]>("list_assets", { workspaceId }),
      invoke<Association[]>("list_brand_assets", { workspaceId }),
    ]).then(
      ([assets, assignments]) => {
        if (!active) return;
        setAssociations(assignments);
        setImages(assets.filter((asset) => asset.kind === "image"));
      },
      () => { if (active) setLoadError(true); },
    );
    return () => { active = false; };
  }, [workspaceId, attempt]);

  async function assign(assetId: number, role: Role, checked: boolean) {
    if (pending) return;
    setPending(true);
    setSaveError(false);
    try {
      await invoke(checked ? "attach_brand_asset" : "detach_brand_asset", { workspaceId, assetId, role });
      setAssociations((current) => checked
        ? [...current, { asset_id: assetId, role }]
        : current.filter((entry) => entry.asset_id !== assetId || entry.role !== role));
    } catch {
      setSaveError(true);
    } finally {
      setPending(false);
    }
  }

  async function upload(role: Role) {
    if (pending) return;
    setSaveError(false);
    const files = await open({ multiple: true, title: copy.upload,
      filters: [{ name: copy[role], extensions: ["png", "jpg", "jpeg", "webp", "gif", "svg"] }] }).catch(() => null);
    if (!files?.length) return;
    setPending(true);
    try {
      for (const source of files) {
        const asset = await invoke<Image>("import_asset", { workspaceId, source });
        if (asset.kind !== "image") continue;
        await invoke("attach_brand_asset", { workspaceId, assetId: asset.id, role });
        setImages((current) => [...(current ?? []), asset]);
        setAssociations((current) => [...current, { asset_id: asset.id, role }]);
      }
    } catch {
      setUploadError(true);
    } finally {
      setPending(false);
    }
  }

  async function attachGenerated(ids: number[]) {
    setSaveError(false);
    try {
      for (const assetId of ids) await invoke("attach_brand_asset", { workspaceId, assetId, role: "style" });
    } catch {
      setSaveError(true);
    }
    setAttempt((value) => value + 1);
  }

  function header(role: Role) {
    return (
      <div className="mb-4 flex flex-wrap items-end justify-between gap-3">
        <div>
          <h2 id={`brand-${role}-title`} className="text-[1.75rem]">{copy[role]}</h2>
          <p className="text-muted-foreground">{copy[role === "style" ? "styleHint" : "identityHint"]}</p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button variant="outline" disabled={pending || !images?.length} onClick={() => { setSaveError(false); setEditing(role); }}>
            <Images data-icon="inline-start" />{copy.manage}
          </Button>
          <Button variant="outline" disabled={pending} onClick={() => void upload(role)}><Upload data-icon="inline-start" />{copy.upload}</Button>
          {role === "style" && <GenerateAssetDialog workspaceId={workspaceId} disabled={pending} imagesOnly onCreated={(ids) => void attachGenerated(ids)} />}
        </div>
      </div>
    );
  }

  const assigned = (role: Role) => (images ?? []).filter((image) => associations.some((item) => item.asset_id === image.id && item.role === role));
  const empty = <p className="rounded-xl border border-dashed p-6 text-center text-muted-foreground">{copy.noAssignments}</p>;

  return (
    <div className="mt-12 flex flex-col gap-12" aria-busy={pending}>
      {uploadError && <Alert variant="destructive"><AlertDescription>{copy.uploadError}</AlertDescription></Alert>}
      {loadError ? (
        <Alert variant="destructive">
          <AlertDescription>{copy.loadError}</AlertDescription>
          <Button type="button" variant="outline" onClick={() => {
            setLoadError(false);
            setAttempt((value) => value + 1);
          }}>{copy.retry}</Button>
        </Alert>
      ) : !images ? (
        <div role="status" aria-label={copy.loading} className="flex flex-col gap-5">
          <Skeleton className="h-40 w-full" /><Skeleton className="h-24 w-full" />
        </div>
      ) : <>
        <section aria-labelledby="brand-style-title">
          {header("style")}
          {assigned("style").length ? <ul className="grid grid-cols-[repeat(auto-fill,minmax(min(14rem,100%),1fr))] gap-4">
            {assigned("style").map((image) => (
              <li key={image.id} className="group relative flex min-w-0 flex-col gap-2">
                <AssetThumbnail workspaceId={workspaceId} assetId={image.id} kind="image" className="w-full" />
                <Button variant="secondary" size="icon-sm" aria-label={copy.unassign.replace("{name}", image.name)} disabled={pending}
                  className="absolute end-2 top-2 opacity-0 transition-opacity duration-150 group-hover:opacity-100 focus-visible:opacity-100"
                  onClick={() => void assign(image.id, "style", false)}><X /></Button>
                {image.notes.trim() ? <p className="line-clamp-2 text-sm">{image.notes}</p> : <>
                  <p className="flex items-center gap-1.5 text-sm text-[oklch(50%_.13_60)]"><TriangleAlert className="size-4" aria-hidden="true" />{copy.noContext}</p>
                  <Button asChild variant="outline" size="sm"><Link href={workspaceHref("assets", workspaceId)}>{copy.describe}</Link></Button>
                </>}
              </li>
            ))}
          </ul> : empty}
        </section>
        <section aria-labelledby="brand-identity-title">
          {header("identity")}
          {assigned("identity").length ? <ul className="divide-y rounded-xl border bg-card">
            {assigned("identity").map((image) => (
              <li key={image.id} className="flex items-center gap-4 px-4 py-3">
                <AssetThumbnail workspaceId={workspaceId} assetId={image.id} kind="image" fit="contain" className="aspect-square w-12 shrink-0" />
                <div className="min-w-0 flex-1">
                  <p className="truncate font-semibold">{image.name}</p>
                  {image.notes.trim() ? <p className="truncate text-sm text-muted-foreground">{image.notes}</p>
                    : <Link href={workspaceHref("assets", workspaceId)} className="text-sm text-[oklch(50%_.13_60)] hover:underline">{copy.missingNotes}</Link>}
                </div>
                {!image.notes.trim() && <TriangleAlert className="size-4 shrink-0 text-[oklch(55%_.14_70)]" aria-hidden="true" />}
                <Button variant="ghost" size="icon-sm" aria-label={copy.unassign.replace("{name}", image.name)} disabled={pending}
                  onClick={() => void assign(image.id, "identity", false)}><X /></Button>
              </li>
            ))}
          </ul> : empty}
        </section>
      </>}
      <Dialog open={editing !== null} onOpenChange={(open) => { if (!open && !pending) setEditing(null); }}>
        {editing && <DialogContent showCloseButton={!pending}>
          <DialogHeader><DialogTitle>{copy[editing]}</DialogTitle><DialogDescription>{copy.manageHint}</DialogDescription></DialogHeader>
          <ScrollArea className="max-h-64">
            <div className="flex flex-col gap-2 p-1">
              {images?.map((image) => {
                const id = `brand-${editing}-${image.id}`;
                return (
                  <Field key={image.id} orientation="horizontal" className="rounded-lg border p-2">
                    <Checkbox id={id} disabled={pending}
                      checked={associations.some((entry) => entry.asset_id === image.id && entry.role === editing)}
                      onCheckedChange={(checked) => void assign(image.id, editing, checked === true)} />
                    <FieldLabel htmlFor={id} className="wrap-anywhere">{image.name}</FieldLabel>
                  </Field>
                );
              })}
            </div>
          </ScrollArea>
          {saveError && <Alert variant="destructive"><AlertDescription>{copy.saveError}</AlertDescription></Alert>}
        </DialogContent>}
      </Dialog>
    </div>
  );
}

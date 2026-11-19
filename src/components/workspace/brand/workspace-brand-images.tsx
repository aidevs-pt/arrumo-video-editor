"use client";

import { useLocale } from "@/lib/locale-provider";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Images, Plus } from "lucide-react";
import Link from "next/link";
import { Card, CardContent } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia } from "@/components/ui/empty";
import { Field, FieldLabel } from "@/components/ui/field";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Skeleton } from "@/components/ui/skeleton";
import { AssetThumbnail } from "../assets/asset-thumbnail";
import { workspaceHref } from "../home/workspace-links";

const roles = ["identity", "style"] as const;
type Role = typeof roles[number];
type Image = { id: number; name: string; kind: string };
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

  return (
    <div className="mt-6 flex flex-col gap-7" aria-busy={pending}>
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
          <Skeleton className="h-24 w-full" /><Skeleton className="h-24 w-full" />
        </div>
      ) : roles.map((role) => {
        const assigned = images.filter((image) => associations.some((item) => item.asset_id === image.id && item.role === role));
        return (
          <section key={role} aria-labelledby={`brand-${role}-title`} className="flex flex-col gap-3">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <h2 id={`brand-${role}-title`} className="font-semibold">{copy[role]}</h2>
              {images.length > 0 ? (
                <Button variant="outline" size="sm" onClick={() => { setSaveError(false); setEditing(role); }}>
                  <Plus data-icon="inline-start" />{copy.manage}
                </Button>
              ) : (
                <Button asChild variant="outline" size="sm">
                  <Link href={workspaceHref("assets", workspaceId)}><Plus data-icon="inline-start" />{messages.assets.pick}</Link>
                </Button>
              )}
            </div>
            {assigned.length > 0 ? (
              <ul className="grid grid-cols-[repeat(auto-fill,minmax(min(10rem,100%),1fr))] gap-3">
                {assigned.map((image) => (
                  <li key={image.id}>
                    <Card size="sm" className="gap-2 pt-0">
                      <AssetThumbnail workspaceId={workspaceId} assetId={image.id} kind="image" />
                      <CardContent><p title={image.name} className="truncate font-medium">{image.name}</p></CardContent>
                    </Card>
                  </li>
                ))}
              </ul>
            ) : (
              <Empty className="min-h-24 rounded-xl border py-3">
                <EmptyHeader><EmptyMedia variant="icon"><Images /></EmptyMedia>
                  <EmptyDescription>{images.length ? copy.noAssignments : copy.empty}</EmptyDescription>
                </EmptyHeader>
              </Empty>
            )}
          </section>
        );
      })}
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

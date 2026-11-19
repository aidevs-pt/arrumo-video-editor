"use client";

import { useLocale } from "@/lib/locale-provider";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel, FieldLegend, FieldSet } from "@/components/ui/field";

const roles = ["identity", "style"] as const;
type Role = typeof roles[number];
type Image = { id: number; name: string; kind: string };
type Association = { asset_id: number; role: Role };

export function WorkspaceBrandImages({ workspaceId, onSaving }: {
  workspaceId: number;

  onSaving: (saving: boolean) => void;
}) {
  const [images, setImages] = useState<Image[]>();
  const [associations, setAssociations] = useState<Association[]>([]);
  const [attempt, setAttempt] = useState(0);
  const [loadError, setLoadError] = useState(false);
  const [saveError, setSaveError] = useState(false);
  const [pending, setPending] = useState(false);
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
    onSaving(true);
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
      onSaving(false);
    }
  }

  return (
    <section className="mt-8 [&>h2]:mb-3 [&_summary]:cursor-pointer [&_summary]:font-semibold" aria-labelledby="brand-images-title" aria-busy={pending}>
      <h2 id="brand-images-title" className="flex min-w-0 items-baseline gap-2.5 font-[680]">{copy.title}</h2>
      {loadError ? (
        <Alert variant="destructive">
          <AlertDescription>{copy.loadError}</AlertDescription>
          <Button type="button" variant="outline" onClick={() => {
            setLoadError(false);
            setAttempt((value) => value + 1);
          }}>{copy.retry}</Button>
        </Alert>
      ) : !images ? <p role="status">{copy.loading}</p> : images.length === 0 ? <p>{copy.empty}</p> : (
        <div className="grid grid-cols-[repeat(auto-fit,minmax(min(240px,100%),1fr))] gap-6 border-t pt-4 [&_label]:wrap-anywhere">
          {roles.map((role) => (
            <FieldSet key={role} disabled={pending}>
              <FieldLegend>{copy[role]}</FieldLegend>
              <FieldGroup>
                {images.map((image) => {
                  const id = `brand-${role}-${image.id}`;
                  return (
                    <Field key={image.id} orientation="horizontal">
                      <input id={id} type="checkbox"
                        aria-label={`${copy[role]} · ${image.name}`}
                        checked={associations.some((entry) => entry.asset_id === image.id && entry.role === role)}
                        onChange={(event) => void assign(image.id, role, event.target.checked)} />
                      <FieldLabel htmlFor={id}>{image.name}</FieldLabel>
                    </Field>
                  );
                })}
              </FieldGroup>
            </FieldSet>
          ))}
        </div>
      )}
      {saveError && <Alert variant="destructive"><AlertDescription>{copy.saveError}</AlertDescription></Alert>}
    </section>
  );
}

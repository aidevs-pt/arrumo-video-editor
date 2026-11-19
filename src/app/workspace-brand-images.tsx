"use client";

import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel, FieldLegend, FieldSet } from "@/components/ui/field";

const roles = ["identity", "style"] as const;
type Role = typeof roles[number];
type Image = { id: number; name: string; kind: string };
type Association = { asset_id: number; role: Role };

const en = {
  title: "Brand images", identity: "Brand assets", style: "Style references",
  empty: "Import an image in workspace Assets to assign it here.",
  loading: "Loading brand images…", loadError: "Brand images couldn’t be loaded.",
  retry: "Retry", saveError: "The image assignment couldn’t be saved.",
};
const translations: Record<"en" | "es" | "pt", typeof en> = {
  en,
  es: {
    title: "Imágenes de marca", identity: "Recursos de marca", style: "Referencias de estilo",
    empty: "Importa una imagen en Recursos para asignarla aquí.",
    loading: "Cargando imágenes de marca…", loadError: "No se pudieron cargar las imágenes de marca.",
    retry: "Reintentar", saveError: "No se pudo guardar la asignación.",
  },
  pt: {
    title: "Imagens da marca", identity: "Recursos da marca", style: "Referências de estilo",
    empty: "Importe uma imagem nos Recursos para a atribuir aqui.",
    loading: "A carregar as imagens da marca…", loadError: "Não foi possível carregar as imagens da marca.",
    retry: "Tentar novamente", saveError: "Não foi possível guardar a atribuição.",
  },
};

export function WorkspaceBrandImages({ workspaceId, locale, onSaving }: {
  workspaceId: number;
  locale: keyof typeof translations;
  onSaving: (saving: boolean) => void;
}) {
  const [images, setImages] = useState<Image[]>();
  const [associations, setAssociations] = useState<Association[]>([]);
  const [attempt, setAttempt] = useState(0);
  const [loadError, setLoadError] = useState(false);
  const [saveError, setSaveError] = useState(false);
  const [pending, setPending] = useState(false);
  const copy = translations[locale];

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
    <section className="workspace-feedback" aria-labelledby="brand-images-title" aria-busy={pending}>
      <h2 id="brand-images-title" className="profile-name">{copy.title}</h2>
      {loadError ? (
        <Alert variant="destructive">
          <AlertDescription>{copy.loadError}</AlertDescription>
          <Button type="button" variant="outline" onClick={() => {
            setLoadError(false);
            setAttempt((value) => value + 1);
          }}>{copy.retry}</Button>
        </Alert>
      ) : !images ? <p role="status">{copy.loading}</p> : images.length === 0 ? <p>{copy.empty}</p> : (
        <div className="brand-image-roles">
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

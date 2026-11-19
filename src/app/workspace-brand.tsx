"use client";

import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { WorkspaceBrandImages } from "./workspace-brand-images";

const en = {
  title: "Brand", back: "Back to profiles", colors: "Brand colors", picker: "Color",
  add: "Add color", remove: "Remove", empty: "No brand colors yet.",
  loading: "Loading brand colors…", loadError: "Brand colors couldn’t be loaded.", retry: "Retry",
  save: "Save colors", saved: "Brand colors saved.", saveError: "Brand colors couldn’t be saved.",
};
const translations: Record<"en" | "es" | "pt", typeof en> = {
  en,
  es: {
    title: "Marca", back: "Volver a los perfiles", colors: "Colores de marca", picker: "Color",
    add: "Añadir color", remove: "Eliminar", empty: "Aún no hay colores de marca.",
    loading: "Cargando colores…", loadError: "No se pudieron cargar los colores.", retry: "Reintentar",
    save: "Guardar colores", saved: "Colores guardados.", saveError: "No se pudieron guardar los colores.",
  },
  pt: {
    title: "Marca", back: "Voltar aos perfis", colors: "Cores da marca", picker: "Cor",
    add: "Adicionar cor", remove: "Remover", empty: "Ainda não há cores da marca.",
    loading: "A carregar as cores…", loadError: "Não foi possível carregar as cores.", retry: "Tentar novamente",
    save: "Guardar cores", saved: "Cores guardadas.", saveError: "Não foi possível guardar as cores.",
  },
};

export function WorkspaceBrand({ workspace, locale, onBack }: {
  workspace: { id: number; name: string };
  locale: keyof typeof translations;
  onBack: () => void;
}) {
  const heading = useRef<HTMLHeadingElement>(null);
  const [colors, setColors] = useState<string[]>();
  const [candidate, setCandidate] = useState("#000000");
  const [loadError, setLoadError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [saving, setSaving] = useState(false);
  const [savingImages, setSavingImages] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [result, setResult] = useState<"saved" | "saveError" | null>(null);
  const copy = translations[locale];

  useEffect(() => { heading.current?.focus(); }, []);
  useEffect(() => {
    let active = true;
    invoke<string[]>("get_brand_colors", { workspaceId: workspace.id }).then(
      (value) => { if (active) setColors(value); },
      () => { if (active) setLoadError(true); },
    );
    return () => { active = false; };
  }, [workspace.id, attempt]);

  function update(next: string[]) {
    setColors(next);
    setDirty(true);
    setResult(null);
  }

  async function save() {
    if (!dirty || saving || !colors) return;
    setSaving(true);
    setResult(null);
    try {
      const stored = await invoke<string[]>("set_brand_colors", { workspaceId: workspace.id, colors });
      setColors(stored);
      setDirty(false);
      setResult("saved");
    } catch {
      setResult("saveError");
    } finally {
      setSaving(false);
    }
  }

  return (
    <main className="workspace-shell profile-shell">
      <Button type="button" variant="ghost" onClick={onBack} disabled={saving || savingImages}>
        <ArrowLeft data-icon="inline-start" />
        {copy.back}
      </Button>
      <div className="workspace-heading">
        <h1 ref={heading} tabIndex={-1}>{copy.title}</h1>
        <p>{workspace.name}</p>
      </div>
      {loadError ? (
        <Alert variant="destructive">
          <AlertDescription>{copy.loadError}</AlertDescription>
          <Button type="button" variant="outline" onClick={() => {
            setLoadError(false);
            setAttempt((current) => current + 1);
          }}>{copy.retry}</Button>
        </Alert>
      ) : !colors ? <p role="status">{copy.loading}</p> : (
        <section aria-labelledby="brand-colors">
          <h2 id="brand-colors" className="profile-name">{copy.colors}</h2>
          {colors.length ? (
            <ul className="brand-palette">
              {colors.map((color) => (
                <li key={color}>
                  <span className="brand-swatch" style={{ backgroundColor: color }} aria-hidden="true" />
                  <span>{color}</span>
                  <Button type="button" variant="ghost" size="sm" disabled={saving}
                    aria-label={`${copy.remove} ${color}`} onClick={() => update(colors.filter((item) => item !== color))}>{copy.remove}</Button>
                </li>
              ))}
            </ul>
          ) : <p className="profile-guidance">{copy.empty}</p>}
          <div className="brand-palette-actions">
            <Field orientation="horizontal" className="brand-color-picker">
              <FieldLabel htmlFor="brand-candidate">{copy.picker}</FieldLabel>
              <Input id="brand-candidate" type="color" value={candidate} disabled={saving}
                onChange={(event) => setCandidate(event.target.value)} />
            </Field>
            <Button type="button" variant="outline" disabled={saving || colors.includes(candidate.toUpperCase())}
              onClick={() => update([...colors, candidate.toUpperCase()])}>{copy.add}</Button>
            <Button type="button" disabled={!dirty || saving} onClick={() => void save()}>{copy.save}</Button>
          </div>
          {result === "saved" && <p role="status">{copy.saved}</p>}
          {result === "saveError" && <p role="alert">{copy.saveError}</p>}
        </section>
      )}
      <WorkspaceBrandImages workspaceId={workspace.id} locale={locale} onSaving={setSavingImages} />
    </main>
  );
}

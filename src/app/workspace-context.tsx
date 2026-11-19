"use client";

import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Textarea } from "@/components/ui/textarea";
import { WorkspaceFeedback } from "./workspace-feedback";

const en = {
  title: "Context", back: "Back to profiles", label: "Production context (Markdown)",
  template: "## Company or channel\n\n## Author\n\n## Audience\n\n## Editorial guidance\n",
  loading: "Loading context…", loadError: "Context couldn’t be loaded.", retry: "Retry",
  save: "Save context", saved: "Context saved.", saveError: "Context couldn’t be saved. Try again.",
};

const copyByLocale: Record<"en" | "es" | "pt", typeof en> = {
  en,
  es: {
    title: "Contexto", back: "Volver a los perfiles", label: "Contexto de producción (Markdown)",
    template: "## Empresa o canal\n\n## Autoría\n\n## Audiencia\n\n## Pautas editoriales\n",
    loading: "Cargando contexto…", loadError: "No se pudo cargar el contexto.", retry: "Reintentar",
    save: "Guardar contexto", saved: "Contexto guardado.", saveError: "No se pudo guardar el contexto. Inténtalo de nuevo.",
  },
  pt: {
    title: "Contexto", back: "Voltar aos perfis", label: "Contexto de produção (Markdown)",
    template: "## Empresa ou canal\n\n## Autoria\n\n## Público\n\n## Orientações editoriais\n",
    loading: "A carregar o contexto…", loadError: "Não foi possível carregar o contexto.", retry: "Tentar novamente",
    save: "Guardar contexto", saved: "Contexto guardado.", saveError: "Não foi possível guardar o contexto. Tente novamente.",
  },
};

export function WorkspaceContext({ workspace, locale, onBack }: {
  workspace: { id: number; name: string };
  locale: keyof typeof copyByLocale;
  onBack: () => void;
}) {
  const heading = useRef<HTMLHeadingElement>(null);
  const [markdown, setMarkdown] = useState<string>();
  const [loadError, setLoadError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [saving, setSaving] = useState(false);
  const [result, setResult] = useState<"saved" | "saveError" | null>(null);
  const copy = copyByLocale[locale];

  useEffect(() => { heading.current?.focus(); }, []);
  useEffect(() => {
    let active = true;
    invoke<string | null>("get_workspace_context", { workspaceId: workspace.id }).then(
      (value) => { if (active) setMarkdown(value ?? copy.template); },
      () => { if (active) setLoadError(true); },
    );
    return () => { active = false; };
  }, [workspace.id, copy.template, attempt]);

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (markdown === undefined || saving) return;
    setSaving(true);
    setResult(null);
    try {
      await invoke("set_workspace_context", { workspaceId: workspace.id, markdown });
      setResult("saved");
    } catch {
      setResult("saveError");
    } finally {
      setSaving(false);
    }
  }

  return (
    <main className="workspace-shell profile-shell">
      <Button type="button" variant="ghost" onClick={onBack} disabled={saving}>
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
      ) : markdown === undefined ? <p role="status">{copy.loading}</p> : (
        <form className="workspace-form" onSubmit={save} aria-busy={saving}>
          <FieldGroup>
            <Field data-disabled={saving}>
              <FieldLabel htmlFor="workspace-context">{copy.label}</FieldLabel>
              <Textarea id="workspace-context" rows={10} value={markdown} disabled={saving} onChange={(event) => {
                setMarkdown(event.target.value);
                setResult(null);
              }} />
            </Field>
            {result === "saved" && <p role="status">{copy.saved}</p>}
            {result === "saveError" && <Alert variant="destructive"><AlertDescription>{copy.saveError}</AlertDescription></Alert>}
            <Button type="submit" className="self-start" disabled={saving}>{copy.save}</Button>
          </FieldGroup>
        </form>
      )}
      <WorkspaceFeedback workspaceId={workspace.id} locale={locale} />
    </main>
  );
}

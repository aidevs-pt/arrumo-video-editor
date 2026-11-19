"use client";

import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";

const en = {
  title: "Agent feedback", empty: "No feedback has been recorded.",
  loading: "Loading feedback…", loadError: "Feedback couldn’t be loaded.", retry: "Retry",
  edit: "Edit feedback", cardTitle: "Title", markdown: "Feedback (Markdown)",
  save: "Save feedback", cancel: "Cancel", saveError: "Feedback couldn’t be saved.",
};
const translations: Record<"en" | "es" | "pt", typeof en> = {
  en,
  es: {
    title: "Observaciones del agente", empty: "Aún no hay observaciones.",
    loading: "Cargando observaciones…", loadError: "No se pudieron cargar las observaciones.", retry: "Reintentar",
    edit: "Editar observación", cardTitle: "Título", markdown: "Observación (Markdown)",
    save: "Guardar observación", cancel: "Cancelar", saveError: "No se pudo guardar la observación.",
  },
  pt: {
    title: "Observações do agente", empty: "Ainda não há observações.",
    loading: "A carregar as observações…", loadError: "Não foi possível carregar as observações.", retry: "Tentar novamente",
    edit: "Editar observação", cardTitle: "Título", markdown: "Observação (Markdown)",
    save: "Guardar observação", cancel: "Cancelar", saveError: "Não foi possível guardar a observação.",
  },
};

type Feedback = { id: number; title: string; markdown: string };

export function WorkspaceFeedback({ workspaceId, locale }: {
  workspaceId: number;
  locale: keyof typeof translations;
}) {
  const [cards, setCards] = useState<Feedback[]>();
  const [loadError, setLoadError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [editing, setEditing] = useState<number | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState(false);
  const copy = translations[locale];

  useEffect(() => {
    let active = true;
    invoke<Feedback[]>("list_workspace_feedback", { workspaceId }).then(
      (value) => { if (active) setCards(value); },
      () => { if (active) setLoadError(true); },
    );
    return () => { active = false; };
  }, [workspaceId, attempt]);

  async function save(event: FormEvent<HTMLFormElement>, id: number) {
    event.preventDefault();
    if (saving) return;
    const fields = new FormData(event.currentTarget);
    const title = String(fields.get("title") ?? "").trim();
    const markdown = String(fields.get("markdown") ?? "");
    setSaveError(false);
    setSaving(true);
    try {
      const updated = await invoke<Feedback>("update_workspace_feedback", { workspaceId, feedbackId: id, title, markdown });
      setCards((current) => current?.map((card) => card.id === updated.id ? updated : card));
      setEditing(null);
    } catch {
      setSaveError(true);
    } finally {
      setSaving(false);
    }
  }

  return (
    <section className="workspace-feedback" aria-labelledby="feedback-title">
      <h2 id="feedback-title" className="profile-name">{copy.title}</h2>
      {loadError ? (
        <Alert variant="destructive">
          <AlertDescription>{copy.loadError}</AlertDescription>
          <Button type="button" variant="outline" onClick={() => {
            setLoadError(false);
            setAttempt((value) => value + 1);
          }}>{copy.retry}</Button>
        </Alert>
      ) : !cards ? <p role="status">{copy.loading}</p> : cards.length === 0 ? <p>{copy.empty}</p> : (
        <ul className="workspace-rows">
          {cards.map((card) => (
            <li key={card.id}>
              <details>
                <summary>{card.title}</summary>
                {editing === card.id ? (
                  <form className="asset-note-form" onSubmit={(event) => void save(event, card.id)}>
                    <fieldset disabled={saving}>
                      <FieldGroup>
                        <Field>
                          <FieldLabel htmlFor="feedback-edit-title">{copy.cardTitle}</FieldLabel>
                          <Input id="feedback-edit-title" name="title" defaultValue={card.title} required />
                        </Field>
                        <Field>
                          <FieldLabel htmlFor="feedback-edit-markdown">{copy.markdown}</FieldLabel>
                          <Textarea id="feedback-edit-markdown" name="markdown" defaultValue={card.markdown} required />
                        </Field>
                        {saveError && <p role="alert">{copy.saveError}</p>}
                        <div className="profile-edit-actions">
                          <Button type="submit">{copy.save}</Button>
                          <Button type="button" variant="outline" onClick={() => {
                            setSaveError(false);
                            setEditing(null);
                          }}>{copy.cancel}</Button>
                        </div>
                      </FieldGroup>
                    </fieldset>
                  </form>
                ) : (
                  <>
                    <p className="profile-guidance">{card.markdown}</p>
                    <Button type="button" className="asset-note-form" size="sm" variant="outline" disabled={editing !== null}
                      onClick={() => setEditing(card.id)}>{copy.edit}</Button>
                  </>
                )}
              </details>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

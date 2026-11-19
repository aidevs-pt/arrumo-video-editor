"use client";

import { useLocale } from "@/lib/locale-provider";
import { WorkspaceRows } from "@/components/workspace/layout";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";

type Feedback = { id: number; title: string; markdown: string };

export function WorkspaceFeedback({ workspaceId }: {
  workspaceId: number;

}) {
  const [cards, setCards] = useState<Feedback[]>();
  const [loadError, setLoadError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [editing, setEditing] = useState<number | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState(false);
  const { messages } = useLocale();
  const copy = messages.feedback;

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
    <section className="mt-8 [&>h2]:mb-3 [&_summary]:cursor-pointer [&_summary]:font-semibold" aria-labelledby="feedback-title">
      <h2 id="feedback-title" className="flex min-w-0 items-baseline gap-2.5 font-[680]">{copy.title}</h2>
      {loadError ? (
        <Alert variant="destructive">
          <AlertDescription>{copy.loadError}</AlertDescription>
          <Button type="button" variant="outline" onClick={() => {
            setLoadError(false);
            setAttempt((value) => value + 1);
          }}>{copy.retry}</Button>
        </Alert>
      ) : !cards ? <p role="status">{copy.loading}</p> : cards.length === 0 ? <p>{copy.empty}</p> : (
        <WorkspaceRows>
          {cards.map((card) => (
            <li key={card.id}>
              <details>
                <summary>{card.title}</summary>
                {editing === card.id ? (
                  <form className="mt-3.5" onSubmit={(event) => void save(event, card.id)}>
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
                        <div className="flex flex-wrap items-baseline gap-2.5">
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
                    <p className="mt-1.25 whitespace-pre-wrap text-muted-foreground">{card.markdown}</p>
                    <Button type="button" className="mt-3.5" size="sm" variant="outline" disabled={editing !== null}
                      onClick={() => setEditing(card.id)}>{copy.edit}</Button>
                  </>
                )}
              </details>
            </li>
          ))}
        </WorkspaceRows>
      )}
    </section>
  );
}

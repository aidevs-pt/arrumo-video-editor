"use client";

import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "@/components/ui/accordion";
import { Skeleton } from "@/components/ui/skeleton";
import { useLocale } from "@/lib/locale-provider";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Pencil, Trash2 } from "lucide-react";
import { useAgentChanges } from "@/lib/agent-changes";

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
  const [removeError, setRemoveError] = useState(false);
  const [agent, setAgent] = useState<string | null>(null);
  const { messages } = useLocale();
  const copy = messages.feedback;

  const agentChanges = useAgentChanges();
  useEffect(() => {
    let active = true;
    Promise.all([
      invoke<{ effective_provider: string | null }>("get_agent_settings", { workspaceId }),
      invoke<{ id: string; label: string }[]>("list_agent_plugins"),
    ]).then(([settings, plugins]) => {
      if (active) setAgent(plugins.find(({ id }) => id === settings.effective_provider)?.label ?? null);
    }, () => {});
    return () => { active = false; };
  }, [workspaceId, agentChanges]);

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

  async function remove(id: number) {
    setRemoveError(false);
    try {
      await invoke("remove_workspace_feedback", { workspaceId, feedbackId: id });
      setCards((current) => current?.filter((card) => card.id !== id));
    } catch {
      setRemoveError(true);
    }
  }

  return (
    <section aria-labelledby="feedback-title">
      <div className="mb-1 flex items-baseline justify-between gap-3">
        <h2 id="feedback-title" className="text-[1.75rem]">{agent ? copy.title.replace("{agent}", agent) : copy.titleFallback}</h2>
        {cards && cards.length > 0 && <span className="rounded-[5px] bg-secondary px-2 text-sm font-bold tabular-nums">{cards.length}</span>}
      </div>
      <p className="mb-4 text-muted-foreground">{copy.hint.replace("{agent}", agent ?? copy.titleFallback)}</p>
      {removeError && <Alert variant="destructive" className="mb-3"><AlertDescription>{copy.removeError}</AlertDescription></Alert>}
      {loadError ? (
        <Alert variant="destructive">
          <AlertDescription>{copy.loadError}</AlertDescription>
          <Button type="button" variant="outline" onClick={() => {
            setLoadError(false);
            setAttempt((value) => value + 1);
          }}>{copy.retry}</Button>
        </Alert>
      ) : !cards ? <Skeleton className="h-10 w-full" role="status" aria-label={copy.loading} />
        : cards.length === 0 ? <p className="rounded-xl border border-dashed p-5 text-muted-foreground">{copy.empty}</p> : (
        <Accordion type="multiple" className="rounded-xl border bg-card">
          {cards.map((card) => (
            <AccordionItem value={String(card.id)} key={card.id} className="px-5">
              <AccordionTrigger className="gap-3 text-base">
                <span className="flex min-w-0 items-center gap-3">
                  {/* eslint-disable-next-line @next/next/no-img-element */}
                  <img src="/arrumo-face.svg" alt="" aria-hidden="true" className="size-4 shrink-0 opacity-70 dark:invert" />
                  <span className="font-semibold">{card.title}</span>
                </span>
              </AccordionTrigger>
              <AccordionContent className="ps-7">
                {editing === card.id ? (
                  <form className="mt-1" onSubmit={(event) => void save(event, card.id)}>
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
                        {saveError && <p role="alert" className="text-destructive">{copy.saveError}</p>}
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
                  <div className="flex flex-wrap items-end justify-between gap-3">
                    <p className="max-w-[70ch] text-[0.9375rem] whitespace-pre-wrap text-foreground">{card.markdown}</p>
                    <div className="flex gap-2">
                      <Button type="button" size="sm" variant="outline" disabled={editing !== null}
                        onClick={() => setEditing(card.id)}><Pencil data-icon="inline-start" />{copy.edit}</Button>
                      <Button type="button" size="sm" variant="outline" disabled={editing !== null}
                        onClick={() => void remove(card.id)}><Trash2 data-icon="inline-start" />{copy.remove}</Button>
                    </div>
                  </div>
                )}
              </AccordionContent>
            </AccordionItem>
          ))}
        </Accordion>
      )}
    </section>
  );
}

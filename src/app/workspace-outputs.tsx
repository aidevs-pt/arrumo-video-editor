"use client";

import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { FormatDelivery } from "./format-delivery";
import { outputCopy } from "./output-copy";

type OutputFormat = { id: number; name: string; width: number; height: number; enabled: boolean };

export function WorkspaceOutputs({
  workspace,
  locale,
  onBack,
}: {
  workspace: { id: number; name: string };
  locale: keyof typeof outputCopy;
  onBack: () => void;
}) {
  const [deliveryFormat, setDeliveryFormat] = useState<OutputFormat | null>(null);
  const [formats, setFormats] = useState<OutputFormat[]>([]);
  const [status, setStatus] = useState<"loading" | "ready" | "error">("loading");
  const [attempt, setAttempt] = useState(0);
  const [editing, setEditing] = useState<OutputFormat | null>(null);
  const [removing, setRemoving] = useState<number | null>(null);
  const [pending, setPending] = useState(false);
  const [failure, setFailure] = useState<"save" | "remove" | null>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const formName = useRef<HTMLInputElement>(null);
  const copy = outputCopy[locale];

  useEffect(() => { heading.current?.focus(); }, []);
  useEffect(() => { if (editing) formName.current?.focus(); }, [editing]);
  useEffect(() => {
    let active = true;
    invoke<OutputFormat[]>("list_output_formats", { workspaceId: workspace.id }).then(
      (rows) => {
        if (!active) return;
        setFormats(rows);
        setStatus("ready");
      },
      () => { if (active) setStatus("error"); },
    );
    return () => { active = false; };
  }, [workspace.id, attempt]);

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending || status !== "ready" || removing !== null) return;
    const form = event.currentTarget;
    const fields = new FormData(form);
    const name = String(fields.get("name") ?? "").trim();
    if (!name) {
      formName.current?.focus();
      return;
    }
    const settings = {
      name,
      width: Number(fields.get("width")),
      height: Number(fields.get("height")),
      enabled: fields.has("enabled"),
    };
    setPending(true);
    setFailure(null);
    try {
      const format = await invoke<OutputFormat>(editing ? "update_output_format" : "create_output_format", {
        workspaceId: workspace.id,
        ...(editing && { formatId: editing.id }),
        settings,
      });
      setFormats((current) => editing ? current.map((row) => row.id === format.id ? format : row) : [...current, format]);
      if (editing) setEditing(null);
      else form.reset();
    } catch {
      setFailure("save");
    } finally {
      setPending(false);
    }
  }

  async function remove(formatId: number) {
    if (pending || removing !== formatId) return;
    setPending(true);
    setFailure(null);
    try {
      await invoke<boolean>("remove_output_format", { workspaceId: workspace.id, formatId });
      setFormats((current) => current.filter((row) => row.id !== formatId));
      setRemoving(null);
    } catch {
      setFailure("remove");
    } finally {
      setPending(false);
    }
  }

  if (deliveryFormat) {
    return <FormatDelivery
      workspaceId={workspace.id}
      format={deliveryFormat}
      locale={locale}
      onBack={() => setDeliveryFormat(null)}
    />;
  }

  return (
    <main className="workspace-shell profile-shell">
      <Button type="button" variant="ghost" onClick={onBack} disabled={pending}>
        <ArrowLeft data-icon="inline-start" />
        {copy.back}
      </Button>
      <div className="workspace-heading">
        <h1 ref={heading} tabIndex={-1}>{copy.title}</h1>
        <p>{workspace.name}</p>
      </div>
      <div className="workspace-layout">
        <section className="workspace-list" aria-labelledby="format-list-title">
          <h2 id="format-list-title">{copy.list}</h2>
          {status === "loading" && <p role="status">{copy.loading}</p>}
          {status === "error" && (
            <Alert variant="destructive">
              <AlertDescription>{copy.loadError}</AlertDescription>
              <Button type="button" variant="outline" onClick={() => {
                setStatus("loading");
                setAttempt((value) => value + 1);
              }}>{copy.retry}</Button>
            </Alert>
          )}
          {status === "ready" && (formats.length ? (
            <ul className="workspace-rows" aria-live="polite">
              {formats.map((format) => (
                <li key={format.id}>
                  <div className="profile-row-heading">
                    <div>
                      <h3 className="profile-name">{format.name}</h3>
                      <p className="profile-guidance">{format.width} × {format.height} · {format.enabled ? copy.enabled : copy.disabled}</p>
                    </div>
                    {removing === format.id ? (
                      <div>
                        <p>{copy.confirm}</p>
                        <div className="profile-edit-actions">
                          <Button type="button" size="sm" variant="destructive" disabled={pending} onClick={() => void remove(format.id)}>{copy.remove}</Button>
                          <Button type="button" size="sm" variant="outline" disabled={pending} onClick={() => {
                            setRemoving(null);
                            setFailure(null);
                          }}>{copy.cancel}</Button>
                        </div>
                        {failure === "remove" && <p role="alert">{copy.removeError}</p>}
                      </div>
                    ) : (
                      <div className="profile-edit-actions">
                        <Button type="button" size="sm" variant="outline" disabled={pending || editing !== null || removing !== null}
                          aria-label={`${copy.delivery} · ${format.name}`} onClick={() => setDeliveryFormat(format)}>{copy.delivery}</Button>
                        <Button type="button" size="sm" variant="outline" disabled={pending || editing !== null || removing !== null}
                          aria-label={`${copy.edit} ${format.name}`} onClick={() => {
                            setFailure(null);
                            setEditing(format);
                          }}>{copy.edit}</Button>
                        <Button type="button" size="sm" variant="ghost" disabled={pending || editing !== null || removing !== null}
                          aria-label={`${copy.remove} ${format.name}`} onClick={() => {
                            setFailure(null);
                            setRemoving(format.id);
                          }}>{copy.remove}</Button>
                      </div>
                    )}
                  </div>
                </li>
              ))}
            </ul>
          ) : <p>{copy.empty}</p>)}
        </section>
        <form key={editing?.id ?? "new"} className="workspace-form" onSubmit={save} aria-labelledby="format-form-title" aria-busy={pending}>
          <h2 id="format-form-title">{editing ? copy.editTitle : copy.create}</h2>
          <fieldset disabled={pending || status !== "ready" || removing !== null}>
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="format-name">{copy.name}</FieldLabel>
                <Input id="format-name" name="name" defaultValue={editing?.name} ref={formName} required />
              </Field>
              <Field>
                <FieldLabel htmlFor="format-width">{copy.width}</FieldLabel>
                <Input id="format-width" name="width" type="number" min={1} max={4294967295} defaultValue={editing?.width} required />
              </Field>
              <Field>
                <FieldLabel htmlFor="format-height">{copy.height}</FieldLabel>
                <Input id="format-height" name="height" type="number" min={1} max={4294967295} defaultValue={editing?.height} required />
              </Field>
              <Field orientation="horizontal">
                <input id="format-enabled" name="enabled" type="checkbox" defaultChecked={editing?.enabled ?? true} />
                <FieldLabel htmlFor="format-enabled">{copy.enabled}</FieldLabel>
              </Field>
              {failure === "save" && <Alert variant="destructive"><AlertDescription>{copy.saveError}</AlertDescription></Alert>}
              <div className="profile-edit-actions">
                <Button type="submit">{editing ? copy.save : copy.create}</Button>
                {editing && <Button type="button" variant="outline" onClick={() => {
                  setEditing(null);
                  setFailure(null);
                }}>{copy.cancel}</Button>}
              </div>
            </FieldGroup>
          </fieldset>
        </form>
      </div>
    </main>
  );
}

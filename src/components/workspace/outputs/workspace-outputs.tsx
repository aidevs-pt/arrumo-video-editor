"use client";

import { Monitor } from "lucide-react";
import { WorkspaceSettingsHeading } from "@/components/workspace/settings/workspace-settings-heading";
import { useLocale } from "@/lib/locale-provider";
import { WorkspaceColumns, WorkspaceForm, WorkspaceRows, WorkspaceShell } from "@/components/workspace/layout";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle, AlertDialogTrigger } from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { FormatDelivery } from "./format-delivery";

type OutputFormat = { id: number; name: string; width: number; height: number; enabled: boolean };

export function WorkspaceOutputs({
  workspace,
}: {
  workspace: { id: number; name: string };

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
  const { messages } = useLocale();
  const copy = messages.outputs;

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
      onBack={() => setDeliveryFormat(null)}
    />;
  }

  return (
    <WorkspaceShell>
      <WorkspaceSettingsHeading workspace={workspace} current="outputs" headingRef={heading} />
      <WorkspaceColumns>
        <section className="min-w-0 [&>p]:mt-3 [&>p]:max-w-[60ch] [&>p]:text-muted-foreground" aria-labelledby="format-list-title">
          <h2 id="format-list-title">{copy.list}</h2>
          {status === "loading" && <Skeleton className="h-32 w-full" role="status" aria-label={copy.loading} />}
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
            <WorkspaceRows  aria-live="polite">
              {formats.map((format) => (
                <li key={format.id}>
                  <div className="flex flex-wrap items-baseline justify-between gap-2.5">
                    <div className="grid min-w-0 gap-1.5">
                      <h3 className="wrap-anywhere font-semibold">{format.name}</h3>
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="text-xs text-muted-foreground tabular-nums">{format.width} × {format.height}</span>
                        <Badge variant={format.enabled ? "secondary" : "outline"}>{format.enabled ? copy.enabled : copy.disabled}</Badge>
                      </div>
                    </div>
                    <div className="flex flex-wrap items-center gap-2">
                      <Button type="button" size="sm" variant="outline" disabled={pending || editing !== null || removing !== null}
                        aria-label={`${copy.delivery} · ${format.name}`} onClick={() => setDeliveryFormat(format)}>{copy.delivery}</Button>
                      <Button type="button" size="sm" variant="outline" disabled={pending || editing !== null || removing !== null}
                        aria-label={`${copy.edit} ${format.name}`} onClick={() => {
                          setFailure(null);
                          setEditing(format);
                        }}>{copy.edit}</Button>
                      <AlertDialog open={removing === format.id} onOpenChange={(open) => {
                        if (pending) return;
                        setFailure(null);
                        setRemoving(open ? format.id : null);
                      }}>
                        <AlertDialogTrigger asChild>
                          <Button type="button" size="sm" variant="ghost" disabled={pending || editing !== null || removing !== null}
                            aria-label={`${copy.remove} ${format.name}`}>{copy.remove}</Button>
                        </AlertDialogTrigger>
                        <AlertDialogContent>
                          <AlertDialogHeader>
                            <AlertDialogTitle>{copy.remove} · {format.name}</AlertDialogTitle>
                            <AlertDialogDescription>{copy.confirm}</AlertDialogDescription>
                          </AlertDialogHeader>
                          {failure === "remove" && <Alert variant="destructive"><AlertDescription>{copy.removeError}</AlertDescription></Alert>}
                          <AlertDialogFooter>
                            <AlertDialogCancel disabled={pending}>{copy.cancel}</AlertDialogCancel>
                            <AlertDialogAction variant="destructive" disabled={pending} onClick={(event) => {
                              event.preventDefault();
                              void remove(format.id);
                            }}>{copy.remove}</AlertDialogAction>
                          </AlertDialogFooter>
                        </AlertDialogContent>
                      </AlertDialog>
                    </div>
                  </div>
                </li>
              ))}
            </WorkspaceRows>
          ) : (
            <Empty>
              <EmptyHeader>
                <EmptyMedia variant="icon"><Monitor /></EmptyMedia>
                <EmptyTitle>{copy.empty}</EmptyTitle>
              </EmptyHeader>
            </Empty>
          ))}
        </section>
        <WorkspaceForm key={editing?.id ?? "new"}  onSubmit={save} aria-labelledby="format-form-title" aria-busy={pending}>
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
                <Switch id="format-enabled" name="enabled" defaultChecked={editing?.enabled ?? true} />
                <FieldLabel htmlFor="format-enabled">{copy.enabled}</FieldLabel>
              </Field>
              {failure === "save" && <Alert variant="destructive"><AlertDescription>{copy.saveError}</AlertDescription></Alert>}
              <div className="flex flex-wrap items-baseline gap-2.5">
                <Button type="submit">{editing ? copy.save : copy.create}</Button>
                {editing && <Button type="button" variant="outline" onClick={() => {
                  setEditing(null);
                  setFailure(null);
                }}>{copy.cancel}</Button>}
              </div>
            </FieldGroup>
          </fieldset>
        </WorkspaceForm>
      </WorkspaceColumns>
    </WorkspaceShell>
  );
}

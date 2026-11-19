"use client";

import { invoke } from "@tauri-apps/api/core";
import { useEffect, useId, useRef, useState, type FormEvent } from "react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription,
  AlertDialogFooter, AlertDialogHeader, AlertDialogTitle, AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { useLocale } from "@/lib/locale-provider";
import type { PluginSettingDeclaration } from "./plugin-preferences";

type Status = {
  appConfigured: boolean;
  workspaceConfigured: boolean;
  source: "app" | "workspace" | null;
};

export function PluginCredentialField({ field, pluginId, workspaceId }: {
  field: PluginSettingDeclaration;
  pluginId: string;
  workspaceId: number | null;
}) {
  const { messages: { pluginCredentials: copy, pluginCatalog, workspaces } } = useLocale();
  const id = useId();
  const input = useRef<HTMLInputElement>(null);
  const [status, setStatus] = useState<Status>();
  const [pending, setPending] = useState(false);
  const [hasInput, setHasInput] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [feedback, setFeedback] = useState<"unavailable" | "saveError" | "saved">();

  useEffect(() => {
    let active = true;
    invoke<Status>("read_plugin_credential_status", { pluginId, settingId: field.id, workspaceId }).then(
      (result) => { if (active) setStatus(result); },
      () => { if (active) setFeedback("unavailable"); },
    );
    return () => { active = false; };
  }, [attempt, field.id, pluginId, workspaceId]);

  async function write(secret: string | null) {
    setPending(true);
    setFeedback(undefined);
    try {
      const result = await invoke<Status>("write_plugin_credential", { pluginId, settingId: field.id, workspaceId, secret });
      setStatus(result);
      setFeedback("saved");
    } catch {
      setFeedback("saveError");
    } finally {
      setPending(false);
    }
  }

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const control = input.current;
    if (!control || control.value === "" || pending) return;
    const secret = control.value;
    control.value = "";
    setHasInput(false);
    void write(secret);
  }

  if (feedback === "unavailable") return (
    <Alert variant="destructive">
      <AlertDescription>{field.label}. {copy.unavailable}</AlertDescription>
      <Button variant="outline" onClick={() => { setFeedback(undefined); setAttempt((value) => value + 1); }}>{workspaces.retry}</Button>
    </Alert>
  );
  if (!status) return <Skeleton className="h-16" aria-label={field.label} />;
  const configured = workspaceId === null ? status.appConfigured : status.workspaceConfigured;

  return <form className="flex flex-col gap-2" onSubmit={submit}>
    <Field>
      <FieldLabel htmlFor={id}>{field.label}</FieldLabel>
      <FieldDescription id={id + "-description"}>{field.description}</FieldDescription>
      <Input ref={input} id={id} type="password" autoComplete="new-password" spellCheck={false}
        aria-describedby={id + "-description"} disabled={pending} maxLength={2048}
        onChange={(event) => setHasInput(event.target.value !== "")} />
      <FieldDescription>{copy[status.source ?? "unset"]}</FieldDescription>
    </Field>
    <div className="flex flex-wrap gap-2">
      <Button size="sm" disabled={pending || !hasInput}>{copy.save}</Button>
      {configured && <AlertDialog>
        <AlertDialogTrigger asChild>
          <Button type="button" variant="outline" size="sm" disabled={pending}>{copy.remove}</Button>
        </AlertDialogTrigger>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{copy.remove}</AlertDialogTitle>
            <AlertDialogDescription>{workspaceId === null ? copy.removeApp : copy.removeWorkspace}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{pluginCatalog.cancel}</AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={() => void write(null)}>{copy.remove}</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>}
    </div>
    {feedback === "saved" && <p role="status" className="text-sm text-muted-foreground">{copy.saved}</p>}
    {feedback === "saveError" && <Alert variant="destructive"><AlertDescription>{copy.saveError}</AlertDescription></Alert>}
  </form>;
}

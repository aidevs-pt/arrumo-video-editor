"use client";

import { invoke } from "@tauri-apps/api/core";
import { useEffect, useId, useState, type FormEvent } from "react";

import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "@/components/ui/accordion";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import { Textarea } from "@/components/ui/textarea";
import { useLocale } from "@/lib/locale-provider";

type InstalledAction = {
  pluginId: string;
  pluginName: string;
  packageSha256: string;
  action: { id: string; label: string; description: string };
};

export function PluginActions({ workspaceId, revision = "" }: {
  workspaceId: number | null;
  revision?: string;
}) {
  const { messages: { pluginActions: copy, workspaces } } = useLocale();
  const [actions, setActions] = useState<InstalledAction[]>([]);
  const [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let active = true;
    invoke<InstalledAction[]>("list_plugin_actions", { workspaceId }).then(
      (items) => { if (active) { setActions(items); setFailed(false); } },
      () => { if (active) { setActions([]); setFailed(true); } },
    );
    return () => { active = false; };
  }, [workspaceId, revision, attempt]);

  if (failed) return <Alert variant="destructive">
    <AlertDescription>{copy.loadError}</AlertDescription>
    <Button type="button" variant="outline" onClick={() => setAttempt((value) => value + 1)}>{workspaces.retry}</Button>
  </Alert>;
  if (!actions.length) return null;

  return <Card>
    <CardHeader><CardTitle>{copy.title}</CardTitle></CardHeader>
    <CardContent>
      <Accordion type="single" collapsible>
        {actions.map((entry) => {
          const key = `${entry.pluginId}:${entry.action.id}:${entry.packageSha256}`;
          return <AccordionItem key={key} value={key}>
            <AccordionTrigger className="wrap-anywhere">
              <span>{entry.action.label}<span className="block text-xs text-muted-foreground">{entry.pluginName}</span></span>
            </AccordionTrigger>
            <AccordionContent><ActionForm entry={entry} workspaceId={workspaceId} /></AccordionContent>
          </AccordionItem>;
        })}
      </Accordion>
    </CardContent>
  </Card>;
}

function ActionForm({ entry, workspaceId }: { entry: InstalledAction; workspaceId: number | null }) {
  const { messages: { pluginActions: copy } } = useLocale();
  const field = useId();
  const result = useId();
  const [input, setInput] = useState("{}");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<"invalidInput" | "runError">();
  const [output, setOutput] = useState<string>();

  async function run(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending) return;
    setError(undefined);
    setOutput(undefined);
    try {
      JSON.parse(input);
    } catch {
      setError("invalidInput");
      return;
    }
    setPending(true);
    try {
      const value = await invoke<string>("invoke_reviewed_plugin_action", {
        workspaceId, inputJson: input, actionId: entry.action.id,
        pluginId: entry.pluginId, packageSha256: entry.packageSha256,
      });
      setOutput(value);
    } catch (reason) {
      setError(reason === "plugin_action_input_invalid" ? "invalidInput" : "runError");
    } finally {
      setPending(false);
    }
  }

  return <form className="flex flex-col gap-3" onSubmit={run} aria-busy={pending}>
    <Field data-invalid={error === "invalidInput"}>
      <FieldDescription id={field + "-description"}>{entry.action.description}</FieldDescription>
      <FieldLabel htmlFor={field}>{copy.input}</FieldLabel>
      <Textarea id={field} aria-describedby={field + "-description"} aria-invalid={error === "invalidInput"}
        className="max-h-60 font-mono" spellCheck={false} maxLength={65_536} disabled={pending}
        value={input} onChange={(event) => { setInput(event.target.value); setError(undefined); setOutput(undefined); }} />
    </Field>
    <Button type="submit" size="sm" className="self-start" disabled={pending}>
      {pending ? copy.running : copy.run}
    </Button>
    {error && <Alert variant="destructive"><AlertDescription>{copy[error]}</AlertDescription></Alert>}
    {output !== undefined && <section className="flex min-w-0 flex-col gap-2" aria-labelledby={result}>
      <h3 id={result} className="text-sm font-medium">{copy.result}</h3>
      <pre tabIndex={0} className="max-h-72 overflow-auto rounded-md border bg-muted p-3 text-xs">{output}</pre>
    </section>}
    <span role="status" className="sr-only">{output !== undefined ? copy.finished : ""}</span>
  </form>;
}
